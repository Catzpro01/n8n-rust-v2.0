// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::config::DATABASE_QUEUE_CAPACITY;
use crate::error::AppError;
use rusqlite::limits::Limit;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, SyncSender};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const BUSY_TIMEOUT_MILLIS: u64 = 5_000;
const MAX_VALUE_BYTES: i32 = 16 * 1024 * 1024;
const MAX_SQL_BYTES: i32 = 1024 * 1024;
const MAX_BOUND_PARAMETERS: i32 = 999;

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseIdentity {
    pub runtime_version: String,
    pub runtime_version_number: i32,
    pub approved_runtime: bool,
    pub journal_mode: String,
    pub synchronous: String,
    pub foreign_keys: bool,
    pub busy_timeout_millis: u64,
    pub max_value_bytes: i32,
    pub max_sql_bytes: i32,
    pub max_bound_parameters: i32,
}

pub struct DatabaseWorker {
    stop: SyncSender<DatabaseCommand>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

enum DatabaseCommand {
    Stop,
}

impl DatabaseWorker {
    pub fn start(
        state_dir: &Path,
        minimum_version: i32,
    ) -> Result<(Self, DatabaseIdentity), AppError> {
        prepare_state_directory(state_dir)?;
        let database_path = state_dir.join("workflow.sqlite3");
        let (command_sender, command_receiver) = mpsc::sync_channel(DATABASE_QUEUE_CAPACITY);
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("workflowd-sqlite".into())
            .spawn(move || {
                let opened = open_database(&database_path, minimum_version);
                match opened {
                    Ok((connection, identity)) => {
                        if startup_sender.send(Ok(identity)).is_ok() {
                            let _connection_owner = connection;
                            let _ = command_receiver.recv();
                        }
                    }
                    Err(error) => {
                        let _ = startup_sender.send(Err(error.to_string()));
                    }
                }
            })
            .map_err(|error| {
                AppError::Database(format!("cannot start SQLite owner thread: {error}"))
            })?;

        match startup_receiver.recv() {
            Ok(Ok(identity)) => Ok((
                Self {
                    stop: command_sender,
                    thread: Mutex::new(Some(thread)),
                },
                identity,
            )),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(AppError::Database(error))
            }
            Err(error) => {
                let _ = thread.join();
                Err(AppError::Database(format!(
                    "SQLite owner thread ended during startup: {error}"
                )))
            }
        }
    }
}

impl Drop for DatabaseWorker {
    fn drop(&mut self) {
        let _ = self.stop.try_send(DatabaseCommand::Stop);
        if let Ok(thread) = self.thread.get_mut() {
            if let Some(thread) = thread.take() {
                let _ = thread.join();
            }
        }
    }
}

fn prepare_state_directory(state_dir: &Path) -> Result<(), AppError> {
    fs::create_dir_all(state_dir).map_err(|error| {
        AppError::Database(format!(
            "cannot create state directory {}: {error}",
            state_dir.display()
        ))
    })?;
    fs::set_permissions(state_dir, fs::Permissions::from_mode(0o700)).map_err(|error| {
        AppError::Database(format!(
            "cannot protect state directory {}: {error}",
            state_dir.display()
        ))
    })?;
    Ok(())
}

fn open_database(
    path: &PathBuf,
    minimum_version: i32,
) -> Result<(Connection, DatabaseIdentity), AppError> {
    let runtime_version_number = rusqlite::version_number();
    if runtime_version_number < minimum_version || runtime_version_number >= 4_000_000 {
        return Err(AppError::Database(format!(
            "SQLite runtime {} ({runtime_version_number}) is outside approved range {minimum_version}..4000000",
            rusqlite::version()
        )));
    }

    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(database_error("open SQLite"))?;
    validate_compile_options(&connection)?;
    connection
        .busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MILLIS))
        .map_err(database_error("set busy timeout"))?;

    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
        .map_err(database_error("enable WAL"))?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(database_error("set FULL synchronous"))?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(database_error("enable foreign keys"))?;

    connection
        .set_limit(Limit::SQLITE_LIMIT_LENGTH, MAX_VALUE_BYTES)
        .map_err(database_error("set maximum value bytes"))?;
    connection
        .set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, MAX_SQL_BYTES)
        .map_err(database_error("set maximum SQL bytes"))?;
    connection
        .set_limit(Limit::SQLITE_LIMIT_VARIABLE_NUMBER, MAX_BOUND_PARAMETERS)
        .map_err(database_error("set maximum bound parameters"))?;

    connection
        .execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE IF NOT EXISTS platform_meta (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             ) STRICT;
             INSERT INTO platform_meta(key, value) VALUES ('schema', '1')
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value;
             COMMIT;",
        )
        .map_err(database_error("initialize schema"))?;

    let synchronous: i64 = connection
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .map_err(database_error("read synchronous"))?;
    let foreign_keys: i64 = connection
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .map_err(database_error("read foreign_keys"))?;
    let busy_timeout_millis: i64 = connection
        .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
        .map_err(database_error("read busy_timeout"))?;
    let busy_timeout_millis = u64::try_from(busy_timeout_millis)
        .map_err(|_| AppError::Database("busy timeout read-back was negative".into()))?;

    let identity = DatabaseIdentity {
        runtime_version: rusqlite::version().into(),
        runtime_version_number,
        approved_runtime: true,
        journal_mode: journal_mode.to_ascii_lowercase(),
        synchronous: match synchronous {
            2 => "full".into(),
            other => format!("unexpected-{other}"),
        },
        foreign_keys: foreign_keys == 1,
        busy_timeout_millis,
        max_value_bytes: connection
            .limit(Limit::SQLITE_LIMIT_LENGTH)
            .map_err(database_error("read maximum value bytes"))?,
        max_sql_bytes: connection
            .limit(Limit::SQLITE_LIMIT_SQL_LENGTH)
            .map_err(database_error("read maximum SQL bytes"))?,
        max_bound_parameters: connection
            .limit(Limit::SQLITE_LIMIT_VARIABLE_NUMBER)
            .map_err(database_error("read maximum bound parameters"))?,
    };
    validate_identity(&identity)?;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
        AppError::Database(format!(
            "cannot protect database {}: {error}",
            path.display()
        ))
    })?;
    Ok((connection, identity))
}

fn validate_compile_options(connection: &Connection) -> Result<(), AppError> {
    let mut statement = connection
        .prepare("PRAGMA compile_options")
        .map_err(database_error("read SQLite compile options"))?;
    let options = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(database_error("read SQLite compile options"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error("read SQLite compile option row"))?;
    let has_thread_safety = options.iter().any(|option| option == "THREADSAFE=1");
    let forbidden = [
        "THREADSAFE=0",
        "OMIT_FOREIGN_KEY",
        "OMIT_TRIGGER",
        "OMIT_WAL",
    ];
    let rejected: Vec<_> = options
        .iter()
        .filter(|option| forbidden.contains(&option.as_str()))
        .cloned()
        .collect();
    if has_thread_safety && rejected.is_empty() {
        Ok(())
    } else {
        Err(AppError::Database(format!(
            "SQLite compile options are outside the approved runtime: threadsafe={has_thread_safety}, rejected={rejected:?}"
        )))
    }
}

fn validate_identity(identity: &DatabaseIdentity) -> Result<(), AppError> {
    let valid = identity.journal_mode == "wal"
        && identity.synchronous == "full"
        && identity.foreign_keys
        && identity.busy_timeout_millis == BUSY_TIMEOUT_MILLIS
        && identity.max_value_bytes == MAX_VALUE_BYTES
        && identity.max_sql_bytes == MAX_SQL_BYTES
        && identity.max_bound_parameters == MAX_BOUND_PARAMETERS;
    if valid {
        Ok(())
    } else {
        Err(AppError::Database(format!(
            "durability settings failed read-back: {identity:?}"
        )))
    }
}

fn database_error(context: &'static str) -> impl FnOnce(rusqlite::Error) -> AppError {
    move |error| AppError::Database(format!("{context}: {error}"))
}

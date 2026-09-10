// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::error::AppError;
use std::{env, net::SocketAddr, path::PathBuf};
pub const TOKIO_CORE_WORKERS: usize = 1;
pub const BLOCKING_THREADS_MAX: usize = 2;
pub const DATABASE_QUEUE_CAPACITY: usize = 32;
#[derive(Debug, Clone)]
pub struct ServeConfig {
    pub bind: SocketAddr,
    pub state_dir: PathBuf,
    pub cgroup_dir: Option<PathBuf>,
    pub sqlite_min_version: i32,
    pub tls: Option<TlsConfig>,
    pub master_key_file: Option<PathBuf>,
    pub control_origin: String,
    pub session_ttl_seconds: i64,
    pub login_max_failures: i64,
    pub argon_memory_kib: u32,
    pub argon_iterations: u32,
    pub draft_lease_ttl_seconds: i64,
    pub draft_takeover_grace_seconds: i64,
    pub draft_snapshot_interval: u64,
    pub draft_history_limit: usize,
    pub draft_undo_limit: usize,
}
#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub certificate: PathBuf,
    pub private_key: PathBuf,
}
impl ServeConfig {
    pub fn from_environment() -> Result<Self, AppError> {
        let bind = env::var("WORKFLOWD_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8787".into())
            .parse()
            .map_err(|e| AppError::Configuration(format!("invalid WORKFLOWD_BIND: {e}")))?;
        let state_dir = env::var_os("WORKFLOWD_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/var/lib/workflow-rust"));
        let cgroup_dir = env::var_os("WORKFLOWD_CGROUP_DIR").map(PathBuf::from);
        let sqlite_min_version = number("WORKFLOWD_SQLITE_MIN_VERSION", 3047002)?;
        let certificate = env::var_os("WORKFLOWD_TLS_CERT").map(PathBuf::from);
        let private_key = env::var_os("WORKFLOWD_TLS_KEY").map(PathBuf::from);
        let tls = match (certificate, private_key) {
            (Some(certificate), Some(private_key)) => Some(TlsConfig {
                certificate,
                private_key,
            }),
            (None, None) => None,
            _ => {
                return Err(AppError::Configuration(
                    "WORKFLOWD_TLS_CERT and WORKFLOWD_TLS_KEY must be set together".into(),
                ))
            }
        };
        let scheme = if tls.is_some() { "https" } else { "http" };
        let control_origin =
            env::var("WORKFLOWD_CONTROL_ORIGIN").unwrap_or_else(|_| format!("{scheme}://{bind}"));
        let session_ttl_seconds = number("WORKFLOWD_SESSION_TTL_SECONDS", 3600)?;
        let login_max_failures = number("WORKFLOWD_LOGIN_MAX_FAILURES", 3)?;
        let argon_memory_kib = number("WORKFLOWD_ARGON_MEMORY_KIB", 19456)?;
        let argon_iterations = number("WORKFLOWD_ARGON_ITERATIONS", 2)?;
        let draft_lease_ttl_seconds = number("WORKFLOWD_DRAFT_LEASE_TTL_SECONDS", 30)?;
        let draft_takeover_grace_seconds = number("WORKFLOWD_DRAFT_TAKEOVER_GRACE_SECONDS", 10)?;
        let draft_snapshot_interval = number("WORKFLOWD_DRAFT_SNAPSHOT_INTERVAL", 16)?;
        let draft_history_limit = number("WORKFLOWD_DRAFT_HISTORY_LIMIT", 64)?;
        let draft_undo_limit = number("WORKFLOWD_DRAFT_UNDO_LIMIT", 32)?;
        if session_ttl_seconds < 1
            || login_max_failures < 1
            || argon_memory_kib < 8192
            || argon_iterations < 1
            || draft_lease_ttl_seconds < 2
            || draft_takeover_grace_seconds < 1
            || draft_snapshot_interval < 1
            || draft_undo_limit < 1
            || draft_history_limit < draft_undo_limit
            || draft_lease_ttl_seconds > 3600
            || draft_takeover_grace_seconds > 300
            || draft_snapshot_interval > 1024
            || draft_history_limit > 4096
            || draft_undo_limit > 512
        {
            return Err(AppError::Configuration(
                "security or Draft limits are outside accepted bounds".into(),
            ));
        }
        Ok(Self {
            bind,
            state_dir,
            cgroup_dir,
            sqlite_min_version,
            tls,
            master_key_file: env::var_os("WORKFLOWD_MASTER_KEY_FILE").map(PathBuf::from),
            control_origin,
            session_ttl_seconds,
            login_max_failures,
            argon_memory_kib,
            argon_iterations,
            draft_lease_ttl_seconds,
            draft_takeover_grace_seconds,
            draft_snapshot_interval,
            draft_history_limit,
            draft_undo_limit,
        })
    }
}
fn number<T>(name: &str, default: T) -> Result<T, AppError>
where
    T: std::str::FromStr + ToString,
    <T as std::str::FromStr>::Err: std::fmt::Display,
{
    env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .parse()
        .map_err(|e| AppError::Configuration(format!("invalid {name}: {e}")))
}

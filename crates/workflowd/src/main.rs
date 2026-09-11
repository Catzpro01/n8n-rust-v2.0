// SPDX-License-Identifier: AGPL-3.0-or-later

mod app;
mod artifact;
mod artifact_http;
mod assets;
mod canonical;
mod cgroup;
mod compiler;
mod config;
mod database;
mod draft;
mod draft_http;
mod error;
mod generate_engine;
mod identity;
mod owner_http;
mod publication;
mod publication_http;
mod run;
mod run_engine;
mod run_http;
mod security;

use crate::app::AppState;
use crate::config::{ServeConfig, BLOCKING_THREADS_MAX, TOKIO_CORE_WORKERS};
use crate::database::DatabaseWorker;
use crate::error::AppError;
use crate::identity::{ReleaseIdentity, PRODUCT_NAME};
use axum_server::Handle;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Builder;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

fn main() {
    if let Err(error) = dispatch() {
        eprintln!(
            "{}",
            serde_json::json!({"event":"startup_failed","error":error.to_string()})
        );
        std::process::exit(1);
    }
}

fn dispatch() -> Result<(), AppError> {
    match env::args().nth(1).as_deref() {
        Some("serve") | None => serve(),
        Some("version") | Some("--version") | Some("-V") => {
            println!("{PRODUCT_NAME} workflowd {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(command) => Err(AppError::Configuration(format!(
            "unknown command {command:?}; expected serve or version"
        ))),
    }
}

fn serve() -> Result<(), AppError> {
    let subscriber = FmtSubscriber::builder()
        .json()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .map_err(|error| AppError::Runtime(format!("cannot initialize tracing: {error}")))?;

    let config = ServeConfig::from_environment()?;
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| AppError::Runtime("cannot install rustls ring provider".into()))?;

    let (database_worker, database_identity) =
        DatabaseWorker::start(&config.state_dir, config.sqlite_min_version)?;
    let security = Arc::new(security::SecurityService::initialize(
        &config.state_dir,
        &config,
    )?);
    let artifacts = Arc::new(
        artifact::ArtifactService::initialize(&config.state_dir, security.clone())
            .map_err(|error| AppError::Database(format!("Artifact schema: {error}")))?,
    );
    let drafts = Arc::new(
        draft::DraftService::initialize(&config)
            .map_err(|error| AppError::Database(format!("draft schema: {error}")))?,
    );
    let publications = Arc::new(
        publication::PublicationService::initialize(&config, security.clone())
            .map_err(|error| AppError::Database(format!("publication schema: {error}")))?,
    );
    let runs = Arc::new(
        run::RunService::initialize(&config, artifacts.clone())
            .map_err(|error| AppError::Database(format!("Run schema: {error}")))?,
    );
    let state = AppState {
        _database_worker: Arc::new(database_worker),
        release: ReleaseIdentity::new(database_identity.runtime_version.clone()),
        database: database_identity,
        resources: cgroup::discover(config.cgroup_dir.as_deref()),
        security,
        artifacts,
        drafts,
        publications,
        runs,
    };

    let runtime = Builder::new_multi_thread()
        .worker_threads(TOKIO_CORE_WORKERS)
        .max_blocking_threads(BLOCKING_THREADS_MAX)
        .thread_name("workflowd-core")
        .enable_all()
        .build()
        .map_err(|error| AppError::Runtime(format!("cannot build Tokio runtime: {error}")))?;
    runtime.block_on(run_server(config, state))
}

async fn run_server(config: ServeConfig, state: AppState) -> Result<(), AppError> {
    let router = app::router(state);
    let handle = Handle::new();
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        info!(event = "shutdown_requested");
        shutdown_handle.graceful_shutdown(Some(Duration::from_secs(5)));
    });

    info!(
        event = "server_listening",
        address = %config.bind,
        transport = if config.tls.is_some() { "https" } else { "http" },
        tokio_core_workers = TOKIO_CORE_WORKERS,
        blocking_threads_max = BLOCKING_THREADS_MAX
    );

    let result = if let Some(tls) = config.tls {
        let rustls =
            axum_server::tls_rustls::RustlsConfig::from_pem_file(tls.certificate, tls.private_key)
                .await
                .map_err(|error| AppError::Server(format!("cannot load TLS identity: {error}")))?;
        axum_server::bind_rustls(config.bind, rustls)
            .handle(handle)
            .serve(router.into_make_service())
            .await
    } else {
        axum_server::bind(config.bind)
            .handle(handle)
            .serve(router.into_make_service())
            .await
    };

    result.map_err(|error| {
        error!(event = "server_failed", error = %error);
        AppError::Server(error.to_string())
    })
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

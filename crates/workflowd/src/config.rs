// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::AppError;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

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
            .map_err(|error| AppError::Configuration(format!("invalid WORKFLOWD_BIND: {error}")))?;
        let state_dir = env::var_os("WORKFLOWD_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/var/lib/workflow-rust"));
        let cgroup_dir = env::var_os("WORKFLOWD_CGROUP_DIR").map(PathBuf::from);
        let sqlite_min_version = env::var("WORKFLOWD_SQLITE_MIN_VERSION")
            .unwrap_or_else(|_| "3047002".into())
            .parse()
            .map_err(|error| {
                AppError::Configuration(format!("invalid WORKFLOWD_SQLITE_MIN_VERSION: {error}"))
            })?;

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
                ));
            }
        };

        Ok(Self {
            bind,
            state_dir,
            cgroup_dir,
            sqlite_min_version,
            tls,
        })
    }
}

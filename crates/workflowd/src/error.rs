// SPDX-License-Identifier: AGPL-3.0-or-later
use thiserror::Error;
#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration rejected: {0}")]
    Configuration(String),
    #[error("database startup rejected: {0}")]
    Database(String),
    #[error("security startup rejected: {0}")]
    Security(String),
    #[error("server failed: {0}")]
    Server(String),
    #[error("runtime failed: {0}")]
    Runtime(String),
}

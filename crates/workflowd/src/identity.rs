// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::config::{BLOCKING_THREADS_MAX, TOKIO_CORE_WORKERS};
use serde::Serialize;

pub const PRODUCT_NAME: &str = "Canopy Workbench";
pub const API_VERSION: &str = "v1";

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseIdentity {
    pub product: &'static str,
    pub version: &'static str,
    pub api_version: &'static str,
    pub build_commit: &'static str,
    pub source_date_epoch: &'static str,
    pub editor_manifest_sha256: &'static str,
    pub sqlite_version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityIdentity {
    pub api_version: &'static str,
    pub capabilities: [&'static str; 24],
    pub runtime: RuntimeIdentity,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeIdentity {
    pub tokio_core_workers: usize,
    pub blocking_threads_max: usize,
    pub database_queue_capacity: usize,
}

impl ReleaseIdentity {
    pub fn new(sqlite_version: String) -> Self {
        Self {
            product: PRODUCT_NAME,
            version: env!("CARGO_PKG_VERSION"),
            api_version: API_VERSION,
            build_commit: env!("WORKFLOWD_BUILD_COMMIT"),
            source_date_epoch: env!("WORKFLOWD_SOURCE_DATE_EPOCH"),
            editor_manifest_sha256: env!("WORKFLOWD_EDITOR_MANIFEST_SHA256"),
            sqlite_version,
        }
    }
}

impl CapabilityIdentity {
    pub fn current() -> Self {
        Self {
            api_version: API_VERSION,
            capabilities: [
                "embedded-editor-shell",
                "durable-sqlite",
                "resource-identity",
                "direct-tls",
                "single-owner",
                "encrypted-recovery-root",
                "node-contract-v1alpha1",
                "semantic-draft-commands",
                "single-writer-draft-lease",
                "durable-undo-redo",
                "encrypted-browser-recovery-copy",
                "deterministic-pure-compiler",
                "signed-immutable-revisions",
                "pinned-execution-plans",
                "non-destructive-signed-rollback",
                "durable-run-admission",
                "deterministic-manual-trigger",
                "checkpointed-causal-trace",
                "reconnectable-run-sse",
                "cooperative-run-cancellation",
                "bounded-generate-items",
                "progressive-generate-checkpoints",
                "encrypted-owner-artifacts",
                "authorized-artifact-content",
            ],
            runtime: RuntimeIdentity {
                tokio_core_workers: TOKIO_CORE_WORKERS,
                blocking_threads_max: BLOCKING_THREADS_MAX,
                database_queue_capacity: crate::config::DATABASE_QUEUE_CAPACITY,
            },
        }
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::assets;
use crate::cgroup::ResourceIdentity;
use crate::database::{DatabaseIdentity, DatabaseWorker};
use crate::identity::{CapabilityIdentity, ReleaseIdentity, API_VERSION};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub _database_worker: Arc<DatabaseWorker>,
    pub database: DatabaseIdentity,
    pub resources: ResourceIdentity,
    pub release: ReleaseIdentity,
}

#[derive(Serialize)]
struct LiveResponse {
    status: &'static str,
    api_version: &'static str,
}

#[derive(Serialize)]
struct ReadyResponse {
    status: &'static str,
    api_version: &'static str,
    checks: ReadinessChecks,
}

#[derive(Serialize)]
struct ReadinessChecks {
    sqlite: DatabaseIdentity,
    editor_assets: &'static str,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(liveness))
        .route("/health/ready", get(readiness))
        .route("/api/v1/release", get(release))
        .route("/api/v1/capabilities", get(capabilities))
        .route("/api/v1/resources", get(resources))
        .fallback(assets::serve)
        .with_state(state)
}

async fn liveness() -> Json<LiveResponse> {
    Json(LiveResponse {
        status: "alive",
        api_version: API_VERSION,
    })
}

async fn readiness(State(state): State<AppState>) -> Json<ReadyResponse> {
    Json(ReadyResponse {
        status: "ready",
        api_version: API_VERSION,
        checks: ReadinessChecks {
            sqlite: state.database,
            editor_assets: "embedded",
        },
    })
}

async fn release(State(state): State<AppState>) -> Json<ReleaseIdentity> {
    Json(state.release)
}

async fn capabilities() -> Json<CapabilityIdentity> {
    Json(CapabilityIdentity::current())
}

async fn resources(State(state): State<AppState>) -> Json<ResourceIdentity> {
    Json(state.resources)
}

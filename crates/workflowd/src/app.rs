// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{
    assets,
    cgroup::ResourceIdentity,
    database::{DatabaseIdentity, DatabaseWorker},
    identity::{CapabilityIdentity, ReleaseIdentity, API_VERSION},
    owner_http,
    security::{RecoveryHealth, SecurityService},
};
use axum::{
    extract::{DefaultBodyLimit, State},
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;
#[derive(Clone)]
pub struct AppState {
    pub _database_worker: Arc<DatabaseWorker>,
    pub database: DatabaseIdentity,
    pub resources: ResourceIdentity,
    pub release: ReleaseIdentity,
    pub security: Arc<SecurityService>,
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
    recovery: RecoveryHealth,
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
        .route("/api/v1/setup", post(owner_http::setup))
        .route("/api/v1/session/login", post(owner_http::login))
        .route("/api/v1/session/renew", post(owner_http::renew))
        .route("/api/v1/session/logout", post(owner_http::logout))
        .route(
            "/api/v1/recovery/acknowledge",
            post(owner_http::acknowledge),
        )
        .route("/api/v1/audit", get(owner_http::audit))
        .route("/public/v1/health/live", get(liveness))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .fallback(assets::serve)
        .with_state(state)
}
async fn liveness() -> Json<LiveResponse> {
    Json(LiveResponse {
        status: "alive",
        api_version: API_VERSION,
    })
}
async fn readiness(State(s): State<AppState>) -> Json<ReadyResponse> {
    Json(ReadyResponse {
        status: "ready",
        api_version: API_VERSION,
        checks: ReadinessChecks {
            sqlite: s.database,
            editor_assets: "embedded",
        },
        recovery: s.security.recovery_health(),
    })
}
async fn release(State(s): State<AppState>) -> Json<ReleaseIdentity> {
    Json(s.release)
}
async fn capabilities() -> Json<CapabilityIdentity> {
    Json(CapabilityIdentity::current())
}
async fn resources(State(s): State<AppState>) -> Json<ResourceIdentity> {
    Json(s.resources)
}

// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{
    artifact::ArtifactService,
    artifact_http, assets,
    cgroup::ResourceIdentity,
    database::{DatabaseIdentity, DatabaseWorker},
    draft::DraftService,
    draft_http,
    identity::{CapabilityIdentity, ReleaseIdentity, API_VERSION},
    owner_http,
    publication::PublicationService,
    publication_http,
    run::RunService,
    run_http,
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
    pub artifacts: Arc<ArtifactService>,
    pub drafts: Arc<DraftService>,
    pub publications: Arc<PublicationService>,
    pub runs: Arc<RunService>,
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
        .route("/api/v1/catalog", get(draft_http::catalog))
        .route("/api/v1/artifacts", post(artifact_http::upload))
        .route(
            "/api/v1/artifacts/{artifact_id}",
            get(artifact_http::metadata),
        )
        .route(
            "/api/v1/artifacts/{artifact_id}/preview",
            get(artifact_http::preview),
        )
        .route(
            "/api/v1/artifacts/{artifact_id}/content",
            get(artifact_http::content),
        )
        .route(
            "/api/v1/node-contracts/{namespace}/{name}/{version}",
            get(draft_http::contract),
        )
        .route("/api/v1/workflows", post(draft_http::create))
        .route("/api/v1/workflows/{id}", get(draft_http::load))
        .route(
            "/api/v1/workflows/{id}/draft-commands",
            post(draft_http::command),
        )
        .route(
            "/api/v1/workflows/{id}/compile-preview",
            post(publication_http::preview),
        )
        .route(
            "/api/v1/workflows/{id}/publish",
            post(publication_http::publish),
        )
        .route(
            "/api/v1/workflows/{id}/rollback",
            post(publication_http::rollback),
        )
        .route(
            "/api/v1/workflows/{id}/publication",
            get(publication_http::status),
        )
        .route(
            "/api/v1/workflows/{id}/revisions/{revision_id}",
            get(publication_http::revision),
        )
        .route("/api/v1/workflows/{id}/runs", post(run_http::admit))
        .route("/api/v1/runs/{run_id}", get(run_http::status))
        .route("/api/v1/runs/{run_id}/cancel", post(run_http::cancel))
        .route("/api/v1/runs/{run_id}/trace", get(run_http::trace))
        .route("/api/v1/runs/{run_id}/events", get(run_http::events))
        .route(
            "/api/v1/workflows/{id}/editor-session",
            post(draft_http::editor_session),
        )
        .route(
            "/api/v1/workflows/{id}/editing/open",
            post(draft_http::open_editing),
        )
        .route(
            "/api/v1/workflows/{id}/editing/acquire",
            post(draft_http::acquire),
        )
        .route(
            "/api/v1/workflows/{id}/editing/status",
            get(draft_http::editing_status),
        )
        .route(
            "/api/v1/workflows/{id}/editing/heartbeat",
            post(draft_http::heartbeat),
        )
        .route(
            "/api/v1/workflows/{id}/editing/release",
            post(draft_http::release),
        )
        .route(
            "/api/v1/workflows/{id}/editing/takeover/request",
            post(draft_http::request_takeover),
        )
        .route(
            "/api/v1/workflows/{id}/editing/takeover/respond",
            post(draft_http::respond_takeover),
        )
        .route(
            "/api/v1/workflows/{id}/editing/takeover/claim",
            post(draft_http::claim_takeover),
        )
        .route("/api/v1/workflows/{id}/history", get(draft_http::history))
        .route(
            "/api/v1/workflows/{id}/recovery/reconcile",
            post(draft_http::reconcile),
        )
        .route(
            "/api/v1/workflows/{id}/recovery-forks",
            get(draft_http::list_forks),
        )
        .route(
            "/api/v1/workflows/{id}/recovery-forks/{fork_id}/apply",
            post(draft_http::apply_fork),
        )
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

// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{
    app::AppState,
    draft::{CreateWorkflow, DraftCommand, DraftError},
    owner_http,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Serialize)]
struct Problem {
    r#type: &'static str,
    title: &'static str,
    status: u16,
    code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_draft_version: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorSessionState {
    viewport: Value,
    selection: Vec<String>,
    open_panels: Vec<String>,
    search_query: String,
}
pub async fn catalog(State(s): State<AppState>, h: HeaderMap) -> Response {
    if let Err(e) = read_auth(&s, &h).await {
        return e;
    }
    Json(s.drafts.catalog()).into_response()
}
pub async fn contract(
    State(s): State<AppState>,
    h: HeaderMap,
    Path((namespace, name, version)): Path<(String, String, String)>,
) -> Response {
    if let Err(e) = read_auth(&s, &h).await {
        return e;
    }
    match s.drafts.contract(&namespace, &name, &version) {
        Ok(v) => Json(v).into_response(),
        Err(e) => problem(e),
    }
}
pub async fn create(
    State(s): State<AppState>,
    h: HeaderMap,
    Json(r): Json<CreateWorkflow>,
) -> Response {
    if let Err(e) = write_auth(&s, &h).await {
        return e;
    }
    let drafts = s.drafts.clone();
    match tokio::task::spawn_blocking(move || drafts.create(r)).await {
        Ok(Ok(v)) => (StatusCode::CREATED, Json(v)).into_response(),
        Ok(Err(e)) => problem(e),
        Err(_) => problem(DraftError::Storage("worker".into())),
    }
}
pub async fn load(State(s): State<AppState>, h: HeaderMap, Path(id): Path<String>) -> Response {
    if let Err(e) = read_auth(&s, &h).await {
        return e;
    }
    let drafts = s.drafts.clone();
    match tokio::task::spawn_blocking(move || drafts.load(&id)).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => problem(e),
        Err(_) => problem(DraftError::Storage("worker".into())),
    }
}
pub async fn command(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<DraftCommand>,
) -> Response {
    if let Err(e) = write_auth(&s, &h).await {
        return e;
    }
    let drafts = s.drafts.clone();
    match tokio::task::spawn_blocking(move || drafts.command(&id, r)).await {
        Ok(Ok(v)) => Json(v).into_response(),
        Ok(Err(e)) => problem(e),
        Err(_) => problem(DraftError::Storage("worker".into())),
    }
}
pub async fn editor_session(
    State(s): State<AppState>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<EditorSessionState>,
) -> Response {
    if let Err(e) = write_auth(&s, &h).await {
        return e;
    }
    let _transient = (r.viewport, r.selection, r.open_panels, r.search_query);
    let drafts = s.drafts.clone();
    match tokio::task::spawn_blocking(move || drafts.load(&id)).await {
        Ok(Ok(v)) => Json(
            json!({"workflow_id":v.workflow_id,"draft_version":v.draft_version,"stored":false}),
        )
        .into_response(),
        Ok(Err(e)) => problem(e),
        Err(_) => problem(DraftError::Storage("worker".into())),
    }
}
async fn read_auth(s: &AppState, h: &HeaderMap) -> Result<(), Response> {
    let token = owner_http::cookie(h).ok_or_else(unauthorized)?;
    let security = s.security.clone();
    match tokio::task::spawn_blocking(move || security.authenticate(&token)).await {
        Ok(Ok(_)) => Ok(()),
        _ => Err(unauthorized()),
    }
}
async fn write_auth(s: &AppState, h: &HeaderMap) -> Result<(), Response> {
    let (token, csrf) =
        owner_http::mutation_credentials(s, h).map_err(IntoResponse::into_response)?;
    let security = s.security.clone();
    match tokio::task::spawn_blocking(move || security.require_csrf(&token, &csrf)).await {
        Ok(Ok(_)) => Ok(()),
        _ => Err(forbidden("csrf_rejected")),
    }
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"type":"urn:canopy:problem","status":401,"code":"unauthorized"})),
    )
        .into_response()
}
fn forbidden(code: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({"type":"urn:canopy:problem","status":403,"code":code})),
    )
        .into_response()
}
fn problem(e: DraftError) -> Response {
    let (status, code, current, title) = match e {
        DraftError::NotFound => (
            StatusCode::NOT_FOUND,
            "not_found",
            None,
            "Draft resource was not found",
        ),
        DraftError::AlreadyExists => (
            StatusCode::CONFLICT,
            "workflow_exists",
            None,
            "Workflow already exists",
        ),
        DraftError::Stale { current } => (
            StatusCode::CONFLICT,
            "stale_draft_version",
            Some(current),
            "Draft Version is stale",
        ),
        DraftError::DuplicateIdentity => (
            StatusCode::CONFLICT,
            "duplicate_identity",
            None,
            "Identity already exists",
        ),
        DraftError::InvalidContractLock => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_contract_lock",
            None,
            "Node Contract Lock was rejected",
        ),
        DraftError::Invalid(field) => {
            tracing::info!(event = "draft_input_rejected", field);
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_draft_command",
                None,
                "Draft Command was rejected",
            )
        }
        DraftError::Storage(reason) => {
            tracing::error!(event = "draft_storage_failed", reason);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                None,
                "Draft request failed",
            )
        }
    };
    (
        status,
        Json(Problem {
            r#type: "urn:canopy:draft-problem",
            title,
            status: status.as_u16(),
            code,
            current_draft_version: current,
        }),
    )
        .into_response()
}

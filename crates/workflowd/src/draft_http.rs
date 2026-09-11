// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{
    app::AppState,
    draft::{
        ApplyForkRequest, CreateWorkflow, DraftCommand, DraftError, EditingQuery, OpenEditing,
        ReconcileRequest, SessionOnly, TakeoverRequest, TakeoverResponse,
    },
    owner_http,
};
use axum::{
    extract::{Path, Query, State},
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

pub async fn catalog(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(error) = read_auth(&state, &headers).await {
        return error;
    }
    Json(state.drafts.catalog()).into_response()
}
pub async fn contract(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((namespace, name, version)): Path<(String, String, String)>,
) -> Response {
    if let Err(error) = read_auth(&state, &headers).await {
        return error;
    }
    match state.drafts.contract(&namespace, &name, &version) {
        Ok(value) => Json(value).into_response(),
        Err(error) => problem(error),
    }
}
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateWorkflow>,
) -> Response {
    if let Err(error) = write_auth(&state, &headers).await {
        return error;
    }
    run(move || state.drafts.create(request), StatusCode::CREATED).await
}
pub async fn load(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
) -> Response {
    if let Err(error) = read_auth(&state, &headers).await {
        return error;
    }
    run(move || state.drafts.load(&workflow_id), StatusCode::OK).await
}
pub async fn command(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<DraftCommand>,
) -> Response {
    if let Err(error) = write_auth(&state, &headers).await {
        return error;
    }
    run(
        move || state.drafts.command(&workflow_id, request),
        StatusCode::OK,
    )
    .await
}
pub async fn open_editing(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<OpenEditing>,
) -> Response {
    if let Err(error) = write_auth(&state, &headers).await {
        return error;
    }
    run(
        move || state.drafts.open_editing(&workflow_id, request),
        StatusCode::OK,
    )
    .await
}
pub async fn acquire(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<OpenEditing>,
) -> Response {
    if let Err(error) = write_auth(&state, &headers).await {
        return error;
    }
    run(
        move || {
            state
                .drafts
                .acquire(&workflow_id, &request.editor_session_id, &request.label)
        },
        StatusCode::OK,
    )
    .await
}
pub async fn editing_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Query(query): Query<EditingQuery>,
) -> Response {
    if let Err(error) = read_auth(&state, &headers).await {
        return error;
    }
    run(
        move || {
            state
                .drafts
                .editing_status(&workflow_id, &query.editor_session_id)
        },
        StatusCode::OK,
    )
    .await
}
pub async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<SessionOnly>,
) -> Response {
    if let Err(error) = write_auth(&state, &headers).await {
        return error;
    }
    run(
        move || state.drafts.heartbeat(&workflow_id, request),
        StatusCode::OK,
    )
    .await
}
pub async fn release(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<SessionOnly>,
) -> Response {
    if let Err(error) = write_auth(&state, &headers).await {
        return error;
    }
    run(
        move || state.drafts.release(&workflow_id, request),
        StatusCode::OK,
    )
    .await
}
pub async fn request_takeover(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<TakeoverRequest>,
) -> Response {
    if let Err(error) = write_auth(&state, &headers).await {
        return error;
    }
    run(
        move || state.drafts.request_takeover(&workflow_id, request),
        StatusCode::OK,
    )
    .await
}
pub async fn respond_takeover(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<TakeoverResponse>,
) -> Response {
    if let Err(error) = write_auth(&state, &headers).await {
        return error;
    }
    run(
        move || state.drafts.respond_takeover(&workflow_id, request),
        StatusCode::OK,
    )
    .await
}
pub async fn claim_takeover(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<TakeoverRequest>,
) -> Response {
    if let Err(error) = write_auth(&state, &headers).await {
        return error;
    }
    run(
        move || state.drafts.claim_takeover(&workflow_id, request),
        StatusCode::OK,
    )
    .await
}
pub async fn history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
) -> Response {
    if let Err(error) = read_auth(&state, &headers).await {
        return error;
    }
    run(move || state.drafts.history(&workflow_id), StatusCode::OK).await
}
pub async fn reconcile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<ReconcileRequest>,
) -> Response {
    if let Err(error) = write_auth(&state, &headers).await {
        return error;
    }
    let drafts = state.drafts.clone();
    match tokio::task::spawn_blocking(move || drafts.reconcile(&workflow_id, request)).await {
        Ok(Ok(result)) => {
            let status = if result.status == "conflict_fork" {
                StatusCode::CONFLICT
            } else {
                StatusCode::OK
            };
            (status, Json(result)).into_response()
        }
        Ok(Err(error)) => problem(error),
        Err(_) => problem(DraftError::Storage("worker".into())),
    }
}
pub async fn list_forks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
) -> Response {
    if let Err(error) = read_auth(&state, &headers).await {
        return error;
    }
    run(
        move || state.drafts.list_forks(&workflow_id),
        StatusCode::OK,
    )
    .await
}
pub async fn apply_fork(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((workflow_id, fork_id)): Path<(String, String)>,
    Json(request): Json<ApplyForkRequest>,
) -> Response {
    if let Err(error) = write_auth(&state, &headers).await {
        return error;
    }
    run(
        move || state.drafts.apply_fork(&workflow_id, &fork_id, request),
        StatusCode::OK,
    )
    .await
}
pub async fn editor_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<EditorSessionState>,
) -> Response {
    if let Err(error) = write_auth(&state, &headers).await {
        return error;
    }
    let _transient = (
        request.viewport,
        request.selection,
        request.open_panels,
        request.search_query,
    );
    let drafts = state.drafts.clone();
    match tokio::task::spawn_blocking(move||drafts.load(&workflow_id)).await{
        Ok(Ok(value))=>Json(json!({"workflow_id":value.workflow_id,"draft_version":value.draft_version,"stored":false})).into_response(),
        Ok(Err(error))=>problem(error),Err(_)=>problem(DraftError::Storage("worker".into())),
    }
}

async fn run<T, F>(operation: F, status: StatusCode) -> Response
where
    T: Serialize + Send + 'static,
    F: FnOnce() -> Result<T, DraftError> + Send + 'static,
{
    match tokio::task::spawn_blocking(operation).await {
        Ok(Ok(value)) => (status, Json(value)).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(DraftError::Storage("worker".into())),
    }
}
pub(crate) async fn read_auth(state: &AppState, headers: &HeaderMap) -> Result<(), Response> {
    let token = owner_http::cookie(headers).ok_or_else(unauthorized)?;
    let security = state.security.clone();
    match tokio::task::spawn_blocking(move || security.authenticate(&token)).await {
        Ok(Ok(_)) => Ok(()),
        _ => Err(unauthorized()),
    }
}
pub(crate) async fn write_auth(state: &AppState, headers: &HeaderMap) -> Result<(), Response> {
    let (token, csrf) =
        owner_http::mutation_credentials(state, headers).map_err(IntoResponse::into_response)?;
    let security = state.security.clone();
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
fn problem(error: DraftError) -> Response {
    let (status, code, current, title) = match error {
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
        DraftError::LeaseRequired => (
            StatusCode::LOCKED,
            "draft_lease_required",
            None,
            "This Editor Session is read-only",
        ),
        DraftError::TakeoverPending => (
            StatusCode::CONFLICT,
            "takeover_pending",
            None,
            "Another takeover request is pending",
        ),
        DraftError::TakeoverTooEarly => (
            StatusCode::CONFLICT,
            "takeover_grace_active",
            None,
            "Takeover grace has not elapsed",
        ),
        DraftError::NothingToUndo => (
            StatusCode::CONFLICT,
            "nothing_to_undo",
            None,
            "No retained command can be undone",
        ),
        DraftError::NothingToRedo => (
            StatusCode::CONFLICT,
            "nothing_to_redo",
            None,
            "No retained command can be redone",
        ),
        DraftError::ForkResolved => (
            StatusCode::CONFLICT,
            "recovery_fork_resolved",
            None,
            "Recovery fork is already resolved",
        ),
        DraftError::Invalid(field) => {
            tracing::info!(event = "draft_input_rejected", field);
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_draft_command",
                None,
                "Draft request was rejected",
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

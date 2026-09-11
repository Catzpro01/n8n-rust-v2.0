// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{
    app::AppState,
    draft_http,
    revision::{CompileDiagnostic, PublishRequest, RevisionError},
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyRequest {}

#[derive(Serialize)]
struct Problem {
    r#type: &'static str,
    title: &'static str,
    status: u16,
    code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_draft_version: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostics: Option<Vec<CompileDiagnostic>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required_acknowledgements: Option<Vec<String>>,
}

pub async fn compile_draft(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
) -> Response {
    if let Err(error) = draft_http::read_auth(&state, &headers).await {
        return error;
    }
    let revisions = state.revisions.clone();
    match tokio::task::spawn_blocking(move || revisions.compile_preview(&workflow_id)).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(RevisionError::Storage("worker".into())),
    }
}

pub async fn publish(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<PublishRequest>,
) -> Response {
    if let Err(error) = draft_http::write_auth(&state, &headers).await {
        return error;
    }
    let revisions = state.revisions.clone();
    match tokio::task::spawn_blocking(move || revisions.publish(&workflow_id, request)).await {
        Ok(Ok(value)) => (StatusCode::CREATED, Json(value)).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(RevisionError::Storage("worker".into())),
    }
}

pub async fn publication(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
) -> Response {
    if let Err(error) = draft_http::read_auth(&state, &headers).await {
        return error;
    }
    let revisions = state.revisions.clone();
    match tokio::task::spawn_blocking(move || revisions.publication(&workflow_id)).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(RevisionError::Storage("worker".into())),
    }
}

pub async fn revision(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((workflow_id, revision_number)): Path<(String, u64)>,
) -> Response {
    if let Err(error) = draft_http::read_auth(&state, &headers).await {
        return error;
    }
    let revisions = state.revisions.clone();
    match tokio::task::spawn_blocking(move || revisions.revision(&workflow_id, revision_number)).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(RevisionError::Storage("worker".into())),
    }
}

pub async fn diff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
) -> Response {
    if let Err(error) = draft_http::read_auth(&state, &headers).await {
        return error;
    }
    let revisions = state.revisions.clone();
    match tokio::task::spawn_blocking(move || revisions.diff(&workflow_id)).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(RevisionError::Storage("worker".into())),
    }
}

pub async fn rollback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(_request): Json<EmptyRequest>,
) -> Response {
    if let Err(error) = draft_http::write_auth(&state, &headers).await {
        return error;
    }
    let revisions = state.revisions.clone();
    match tokio::task::spawn_blocking(move || revisions.rollback(&workflow_id)).await {
        Ok(Ok(value)) => Json(value).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(RevisionError::Storage("worker".into())),
    }
}

fn problem(error: RevisionError) -> Response {
    let (status, code, title, current, diagnostics, required) = match error {
        RevisionError::NotFound => (
            StatusCode::NOT_FOUND,
            "not_found",
            "Revision resource was not found",
            None,
            None,
            None,
        ),
        RevisionError::Stale { current } => (
            StatusCode::CONFLICT,
            "stale_draft_version",
            "Draft Version is stale",
            Some(current),
            None,
            None,
        ),
        RevisionError::LeaseRequired => (
            StatusCode::LOCKED,
            "draft_lease_required",
            "Publishing requires the current Draft Lease",
            None,
            None,
            None,
        ),
        RevisionError::Compilation { diagnostics } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "compilation_failed",
            "Compilation blocked publication",
            None,
            Some(diagnostics),
            None,
        ),
        RevisionError::WarningsUnacknowledged { required } => (
            StatusCode::CONFLICT,
            "publication_warnings_unacknowledged",
            "Designated warnings require explicit acknowledgement",
            None,
            None,
            Some(required),
        ),
        RevisionError::NoPrecedingRevision => (
            StatusCode::CONFLICT,
            "no_preceding_revision",
            "There is no preceding Published Revision to roll back to",
            None,
            None,
            None,
        ),
        RevisionError::Storage(reason) => {
            tracing::error!(event = "revision_request_failed", reason);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Revision request failed",
                None,
                None,
                None,
            )
        }
    };
    (
        status,
        Json(Problem {
            r#type: "urn:canopy:revision-problem",
            title,
            status: status.as_u16(),
            code,
            current_draft_version: current,
            diagnostics,
            required_acknowledgements: required,
        }),
    )
        .into_response()
}

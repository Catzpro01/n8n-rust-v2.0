// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{
    app::AppState,
    draft_http,
    publication::{CompilePreviewRequest, PublicationError, PublishRequest, RollbackRequest},
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
struct Problem {
    r#type: &'static str,
    title: &'static str,
    status: u16,
    code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_draft_version: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required_acknowledgements: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagnostics: Option<Vec<crate::compiler::Diagnostic>>,
}

pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<CompilePreviewRequest>,
) -> Response {
    if let Err(error) = draft_http::write_auth(&state, &headers).await {
        return error;
    }
    let publications = state.publications.clone();
    run(
        move || publications.preview(&workflow_id, request),
        StatusCode::OK,
    )
    .await
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
    let publications = state.publications.clone();
    match tokio::task::spawn_blocking(move || publications.publish(&workflow_id, request)).await {
        Ok(Ok((record, created))) => (
            if created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            Json(record),
        )
            .into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(PublicationError::Storage(
            "publication worker failed".into(),
        )),
    }
}

pub async fn rollback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<RollbackRequest>,
) -> Response {
    if let Err(error) = draft_http::write_auth(&state, &headers).await {
        return error;
    }
    let publications = state.publications.clone();
    match tokio::task::spawn_blocking(move || publications.rollback(&workflow_id, request)).await {
        Ok(Ok((record, _created))) => (StatusCode::OK, Json(record)).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(PublicationError::Storage("rollback worker failed".into())),
    }
}

pub async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
) -> Response {
    if let Err(error) = draft_http::read_auth(&state, &headers).await {
        return error;
    }
    let publications = state.publications.clone();
    run(move || publications.status(&workflow_id), StatusCode::OK).await
}

pub async fn revision(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((workflow_id, revision_id)): Path<(String, String)>,
) -> Response {
    if let Err(error) = draft_http::read_auth(&state, &headers).await {
        return error;
    }
    let publications = state.publications.clone();
    run(
        move || publications.load_revision(&workflow_id, &revision_id),
        StatusCode::OK,
    )
    .await
}

async fn run<T, F>(operation: F, status: StatusCode) -> Response
where
    T: Serialize + Send + 'static,
    F: FnOnce() -> Result<T, PublicationError> + Send + 'static,
{
    match tokio::task::spawn_blocking(operation).await {
        Ok(Ok(value)) => (status, Json(value)).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(PublicationError::Storage(
            "publication worker failed".into(),
        )),
    }
}

fn problem(error: PublicationError) -> Response {
    let (status, code, title, current, required, diagnostics) = match error {
        PublicationError::NotFound => (
            StatusCode::NOT_FOUND,
            "not_found",
            "Publication resource was not found",
            None,
            None,
            None,
        ),
        PublicationError::LeaseRequired => (
            StatusCode::LOCKED,
            "draft_lease_required",
            "This Editor Session is read-only",
            None,
            None,
            None,
        ),
        PublicationError::CompileInputChanged {
            current_draft_version,
        } => (
            StatusCode::CONFLICT,
            "compile_input_changed",
            "The compile input changed",
            Some(current_draft_version),
            None,
            None,
        ),
        PublicationError::CompileFailed { diagnostics } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "compile_failed",
            "Compilation contains blocking errors",
            None,
            None,
            Some(diagnostics),
        ),
        PublicationError::WarningAcknowledgementRequired { required } => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "warning_ack_required",
            "Designated warnings require acknowledgement",
            None,
            Some(required),
            None,
        ),
        PublicationError::RollbackTargetNotPreceding => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "rollback_target_not_preceding",
            "Rollback requires a preceding immutable Revision",
            None,
            None,
            None,
        ),
        PublicationError::NoCurrentPublication => (
            StatusCode::CONFLICT,
            "no_current_publication",
            "The Workflow has no current Published Revision",
            None,
            None,
            None,
        ),
        PublicationError::RequestIdentityConflict => (
            StatusCode::CONFLICT,
            "request_identity_conflict",
            "The request identity was already used for different content",
            None,
            None,
            None,
        ),
        PublicationError::Invalid(field) => {
            tracing::info!(event = "publication_input_rejected", field);
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_publication_request",
                "The publication request was rejected",
                None,
                None,
                None,
            )
        }
        PublicationError::Integrity(reason) => {
            tracing::error!(event = "publication_integrity_failed", reason);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "publication_integrity_failed",
                "Publication integrity verification failed",
                None,
                None,
                None,
            )
        }
        PublicationError::Storage(reason) => {
            tracing::error!(event = "publication_storage_failed", reason);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Publication request failed",
                None,
                None,
                None,
            )
        }
    };
    (
        status,
        Json(Problem {
            r#type: "urn:canopy:publication-problem",
            title,
            status: status.as_u16(),
            code,
            current_draft_version: current,
            required_acknowledgements: required,
            diagnostics,
        }),
    )
        .into_response()
}

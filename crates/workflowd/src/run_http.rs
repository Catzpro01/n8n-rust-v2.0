// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    app::AppState,
    draft_http,
    run::{AdmitRunRequest, CancelRunRequest, RunError, RunSubscription, StreamFrame},
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio_stream::{wrappers::ReceiverStream, Stream};

#[derive(Serialize)]
struct Problem {
    r#type: &'static str,
    title: &'static str,
    status: u16,
    code: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<&'static str>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventQuery {
    cursor: Option<String>,
}

pub async fn admit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(workflow_id): Path<String>,
    Json(request): Json<AdmitRunRequest>,
) -> Response {
    if let Err(error) = draft_http::write_auth(&state, &headers).await {
        return error;
    }
    let publications = state.publications.clone();
    let runs = state.runs.clone();
    let verification = request.clone();
    match tokio::task::spawn_blocking(move || {
        if let Some(existing) = runs.existing_admission(&workflow_id, &request)? {
            return Ok(existing);
        }
        let status = publications.status(&workflow_id).map_err(|error| {
            RunError::Integrity(format!("publication verification failed: {error:?}"))
        })?;
        let current = status
            .current_published
            .ok_or(RunError::NoCurrentPublication)?;
        let event = status
            .current_event
            .ok_or_else(|| RunError::Integrity("current publication event is missing".into()))?;
        if current.revision_id != verification.revision_id
            || current.plan_digest != verification.plan_digest
            || event.envelope.event_id != verification.publication_event_id
        {
            return Err(RunError::StalePublication);
        }
        runs.admit(&workflow_id, request)
    })
    .await
    {
        Ok(Ok(result)) => (
            if result.created {
                StatusCode::CREATED
            } else {
                StatusCode::OK
            },
            Json(result),
        )
            .into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(RunError::Storage("Run admission worker failed".into())),
    }
}

pub async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Response {
    if let Err(error) = draft_http::read_auth(&state, &headers).await {
        return error;
    }
    let runs = state.runs.clone();
    match tokio::task::spawn_blocking(move || runs.status(&run_id)).await {
        Ok(Ok(run)) => Json(run).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(RunError::Storage("Run status worker failed".into())),
    }
}

pub async fn cancel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Json(request): Json<CancelRunRequest>,
) -> Response {
    if let Err(error) = draft_http::write_auth(&state, &headers).await {
        return error;
    }
    let runs = state.runs.clone();
    match tokio::task::spawn_blocking(move || runs.cancel(&run_id, request)).await {
        Ok(Ok(result)) => (StatusCode::OK, Json(result)).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(RunError::Storage("Run cancellation worker failed".into())),
    }
}

pub async fn trace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Response {
    if let Err(error) = draft_http::read_auth(&state, &headers).await {
        return error;
    }
    let runs = state.runs.clone();
    match tokio::task::spawn_blocking(move || runs.trace(&run_id)).await {
        Ok(Ok(trace)) => Json(trace).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(RunError::Storage("Causal Trace worker failed".into())),
    }
}

pub async fn events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Query(query): Query<EventQuery>,
) -> Response {
    if let Err(error) = draft_http::read_auth(&state, &headers).await {
        return error;
    }
    let last_event_id = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or(query.cursor);
    let runs = state.runs.clone();
    match tokio::task::spawn_blocking(move || runs.subscribe(&run_id, last_event_id.as_deref()))
        .await
    {
        Ok(Ok(subscription)) => Sse::new(SseEventStream::new(subscription))
            .keep_alive(
                KeepAlive::new()
                    .interval(Duration::from_secs(15))
                    .text("canopy-keep-alive"),
            )
            .into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(RunError::Storage("Run SSE worker failed".into())),
    }
}

struct SseEventStream {
    receiver: ReceiverStream<StreamFrame>,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl SseEventStream {
    fn new(subscription: RunSubscription) -> Self {
        Self {
            receiver: ReceiverStream::new(subscription.receiver),
            _permit: subscription.permit,
        }
    }
}

impl Stream for SseEventStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.receiver).poll_next(context) {
            Poll::Ready(Some(frame)) => Poll::Ready(Some(Ok(Event::default()
                .event(frame.event)
                .id(frame.id)
                .data(frame.data)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn problem(error: RunError) -> Response {
    let (status, code, title, field) = match error {
        RunError::NotFound => (
            StatusCode::NOT_FOUND,
            "run_not_found",
            "The Run was not found",
            None,
        ),
        RunError::NoCurrentPublication => (
            StatusCode::CONFLICT,
            "no_current_publication",
            "The Workflow has no current Published Revision",
            None,
        ),
        RunError::StalePublication => (
            StatusCode::CONFLICT,
            "stale_publication_target",
            "The current publication no longer matches the requested revision and plan",
            None,
        ),
        RunError::RequestIdentityConflict => (
            StatusCode::CONFLICT,
            "request_identity_conflict",
            "The request identity was already used for different content",
            None,
        ),
        RunError::AdmissionFull => (
            StatusCode::TOO_MANY_REQUESTS,
            "run_admission_full",
            "The bounded nonterminal Run allowance is full",
            None,
        ),
        RunError::SubscriberFull => (
            StatusCode::TOO_MANY_REQUESTS,
            "sse_subscriber_limit",
            "The bounded SSE subscriber allowance is full",
            None,
        ),
        RunError::Invalid(field) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_run_request",
            "The Run request was rejected",
            Some(field),
        ),
        RunError::TooLarge(field) => (
            StatusCode::PAYLOAD_TOO_LARGE,
            "run_value_too_large",
            "A bounded Run value exceeded its byte limit",
            Some(field),
        ),
        RunError::Integrity(reason) => {
            tracing::error!(event = "run_integrity_failed", reason);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "run_integrity_failed",
                "Run integrity verification failed",
                None,
            )
        }
        RunError::Storage(reason) => {
            tracing::error!(event = "run_storage_failed", reason);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "The Run request failed",
                None,
            )
        }
    };
    (
        status,
        Json(Problem {
            r#type: "urn:canopy:run-problem",
            title,
            status: status.as_u16(),
            code,
            field,
        }),
    )
        .into_response()
}

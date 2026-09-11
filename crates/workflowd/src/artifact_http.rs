// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    app::AppState,
    artifact::{ArtifactError, ArtifactView, MAX_ARTIFACT_BYTES, MAX_PREVIEW_BYTES},
    draft_http,
};
use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewQuery {
    bytes: Option<usize>,
}

#[derive(Serialize)]
struct Problem {
    r#type: &'static str,
    title: &'static str,
    status: u16,
    code: &'static str,
}

pub async fn upload(State(state): State<AppState>, headers: HeaderMap, body: Body) -> Response {
    if let Err(error) = draft_http::write_auth(&state, &headers).await {
        return error;
    }
    if headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > MAX_ARTIFACT_BYTES as u64)
    {
        return problem(ArtifactError::TooLarge);
    }
    let media_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .split(';')
        .next()
        .unwrap_or("application/octet-stream")
        .to_owned();
    let reference_id = format!("owner-upload-{}", random_suffix());
    let artifacts = state.artifacts.clone();
    let mut lease = match tokio::task::spawn_blocking({
        let artifacts = artifacts.clone();
        move || artifacts.begin_upload()
    })
    .await
    {
        Ok(Ok(lease)) => lease,
        Ok(Err(error)) => return problem(error),
        Err(_) => return problem(ArtifactError::Storage("Artifact worker failed".into())),
    };
    let mut stream = body.into_data_stream();
    while let Some(next) = stream.next().await {
        let chunk = match next {
            Ok(chunk) => chunk,
            Err(_) => {
                artifacts.abandon_upload(lease);
                return problem(ArtifactError::Invalid("body"));
            }
        };
        let service = artifacts.clone();
        let result = tokio::task::spawn_blocking(move || {
            let result = service.append_upload(&mut lease, &chunk);
            (result, lease)
        })
        .await;
        match result {
            Ok((Ok(()), returned)) => lease = returned,
            Ok((Err(error), returned)) => {
                artifacts.abandon_upload(returned);
                return problem(error);
            }
            Err(_) => return problem(ArtifactError::Storage("Artifact worker failed".into())),
        }
    }
    match tokio::task::spawn_blocking(move || {
        artifacts.finalize_upload(lease, &media_type, &reference_id, "owner_upload", 1)
    })
    .await
    {
        Ok(Ok(view)) => (StatusCode::CREATED, Json(view)).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(ArtifactError::Storage("Artifact worker failed".into())),
    }
}

pub async fn metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(artifact_id): Path<String>,
) -> Response {
    if draft_http::read_auth(&state, &headers).await.is_err() {
        return problem(ArtifactError::NotAuthorized);
    }
    let artifacts = state.artifacts.clone();
    match tokio::task::spawn_blocking(move || artifacts.metadata(&artifact_id)).await {
        Ok(Ok(view)) => Json(view).into_response(),
        Ok(Err(error)) => problem(error),
        Err(_) => problem(ArtifactError::Storage("Artifact worker failed".into())),
    }
}

pub async fn content(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(artifact_id): Path<String>,
) -> Response {
    if draft_http::read_auth(&state, &headers).await.is_err() {
        return problem(ArtifactError::NotAuthorized);
    }
    let range = match headers.get(header::RANGE) {
        Some(value) => match value.to_str().ok().and_then(parse_single_range) {
            Some(range) => Some(range),
            None => return problem(ArtifactError::Invalid("range")),
        },
        None => None,
    };
    let artifacts = state.artifacts.clone();
    if let Some(range) = range {
        return match tokio::task::spawn_blocking(move || {
            artifacts.content(&artifact_id, Some(range))
        })
        .await
        {
            Ok(Ok((view, bytes, start, end, total))) => {
                let mut response = (StatusCode::PARTIAL_CONTENT, bytes).into_response();
                set_verified_content_headers(&mut response, &view);
                if let Ok(value) = HeaderValue::from_str(&format!("bytes {start}-{end}/{total}")) {
                    response.headers_mut().insert(header::CONTENT_RANGE, value);
                }
                response
            }
            Ok(Err(error)) => problem(error),
            Err(_) => problem(ArtifactError::Storage("Artifact worker failed".into())),
        };
    }

    match tokio::task::spawn_blocking(move || artifacts.stream_content(&artifact_id)).await {
        Ok(Ok((view, mut artifact_stream))) => {
            let (sender, receiver) = tokio::sync::mpsc::channel(2);
            tokio::task::spawn_blocking(move || loop {
                match artifact_stream.next_chunk() {
                    Ok(Some(chunk)) => {
                        if sender
                            .blocking_send(Ok::<Bytes, std::io::Error>(Bytes::from(chunk)))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        let _ = sender.blocking_send(Err(std::io::Error::other(error.to_string())));
                        break;
                    }
                }
            });
            let stream = futures_util::stream::unfold(receiver, |mut receiver| async move {
                receiver.recv().await.map(|item| (item, receiver))
            });
            let mut response = Response::new(Body::from_stream(stream));
            set_verified_content_headers(&mut response, &view);
            if let Ok(value) = HeaderValue::from_str(&view.reference.logical_bytes.to_string()) {
                response.headers_mut().insert(header::CONTENT_LENGTH, value);
            }
            response
        }
        Ok(Err(error)) => problem(error),
        Err(_) => problem(ArtifactError::Storage("Artifact worker failed".into())),
    }
}

pub async fn preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(artifact_id): Path<String>,
    Query(query): Query<PreviewQuery>,
) -> Response {
    if draft_http::read_auth(&state, &headers).await.is_err() {
        return problem(ArtifactError::NotAuthorized);
    }
    let maximum = query.bytes.unwrap_or(MAX_PREVIEW_BYTES);
    let artifacts = state.artifacts.clone();
    match tokio::task::spawn_blocking(move || artifacts.preview(&artifact_id, maximum)).await {
        Ok(Ok((view, bytes))) => {
            let mut response = bytes.into_response();
            if let Ok(value) = HeaderValue::from_str(&view.reference.media_type) {
                response.headers_mut().insert(header::CONTENT_TYPE, value);
            }
            response.headers_mut().insert(
                "x-canopy-artifact-integrity",
                HeaderValue::from_static("verified"),
            );
            response
        }
        Ok(Err(error)) => problem(error),
        Err(_) => problem(ArtifactError::Storage("Artifact worker failed".into())),
    }
}

fn set_verified_content_headers(response: &mut Response, view: &ArtifactView) {
    if let Ok(value) = HeaderValue::from_str(&view.reference.media_type) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    response
        .headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response.headers_mut().insert(
        "x-canopy-artifact-integrity",
        HeaderValue::from_static("verified"),
    );
}

fn parse_single_range(value: &str) -> Option<(u64, u64)> {
    let value = value.strip_prefix("bytes=")?;
    if value.contains(',') {
        return None;
    }
    let (start, end) = value.split_once('-')?;
    let start = start.parse::<u64>().ok()?;
    let end = end.parse::<u64>().ok()?;
    (start <= end).then_some((start, end))
}

fn problem(error: ArtifactError) -> Response {
    let (status, title, code) = match error {
        ArtifactError::NotAuthorized => (
            StatusCode::FORBIDDEN,
            "Artifact reference denied",
            "artifact_reference_denied",
        ),
        ArtifactError::Invalid(_) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            "Invalid Artifact request",
            "invalid_artifact_request",
        ),
        ArtifactError::TooLarge => (
            StatusCode::PAYLOAD_TOO_LARGE,
            "Artifact limit exceeded",
            "artifact_too_large",
        ),
        ArtifactError::Integrity(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Artifact integrity failed",
            "artifact_integrity_failed",
        ),
        ArtifactError::Storage(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Artifact storage unavailable",
            "artifact_storage_unavailable",
        ),
    };
    (
        status,
        Json(Problem {
            r#type: "about:blank",
            title,
            status: status.as_u16(),
            code,
        }),
    )
        .into_response()
}

fn random_suffix() -> String {
    use rand_core::{OsRng, RngCore};
    let mut bytes = [0_u8; 12];
    OsRng.fill_bytes(&mut bytes);
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

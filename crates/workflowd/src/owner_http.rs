// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{
    app::AppState,
    security::{AuditView, SecurityError, SessionGrant},
};
use axum::{
    extract::State,
    http::{
        header::{COOKIE, ORIGIN, SET_COOKIE},
        HeaderMap, HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
pub struct SetupRequest {
    email: String,
    password: String,
    recovery_passphrase: String,
}
#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}
#[derive(Deserialize)]
pub struct RecoveryAck {
    checksum: String,
}
#[derive(Serialize)]
struct Problem {
    r#type: &'static str,
    title: &'static str,
    status: u16,
    code: &'static str,
}
#[derive(Serialize)]
struct AuditResponse {
    events: Vec<AuditView>,
}
pub async fn setup(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(r): Json<SetupRequest>,
) -> Response {
    if let Err(e) = origin(&s, &headers) {
        return e.into_response();
    }
    let service = s.security.clone();
    result(
        tokio::task::spawn_blocking(move || {
            service.setup(&r.email, &r.password, &r.recovery_passphrase)
        })
        .await,
    )
    .map(|v| (StatusCode::CREATED, Json(v)).into_response())
    .unwrap_or_else(IntoResponse::into_response)
}
pub async fn login(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(r): Json<LoginRequest>,
) -> Response {
    if let Err(e) = origin(&s, &headers) {
        return e.into_response();
    }
    let service = s.security.clone();
    match result(tokio::task::spawn_blocking(move || service.login(&r.email, &r.password)).await) {
        Ok(g) => session_response(StatusCode::OK, g),
        Err(e) => e.into_response(),
    }
}
pub async fn renew(State(s): State<AppState>, headers: HeaderMap) -> Response {
    let (token, csrf) = match mutation_credentials(&s, &headers) {
        Ok(credentials) => credentials,
        Err(error) => return error.into_response(),
    };
    let service = s.security.clone();
    match result(tokio::task::spawn_blocking(move || service.renew(&token, &csrf)).await) {
        Ok(g) => session_response(StatusCode::OK, g),
        Err(e) => e.into_response(),
    }
}
pub async fn logout(State(s): State<AppState>, headers: HeaderMap) -> Response {
    let (token, csrf) = match mutation_credentials(&s, &headers) {
        Ok(credentials) => credentials,
        Err(error) => return error.into_response(),
    };
    let service = s.security.clone();
    match result(tokio::task::spawn_blocking(move || service.logout(&token, &csrf)).await) {
        Ok(()) => {
            let mut r = StatusCode::NO_CONTENT.into_response();
            r.headers_mut().insert(
                SET_COOKIE,
                HeaderValue::from_static(
                    "canopy_session=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0",
                ),
            );
            r
        }
        Err(e) => e.into_response(),
    }
}
pub async fn acknowledge(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(r): Json<RecoveryAck>,
) -> Response {
    let (token, csrf) = match mutation_credentials(&s, &headers) {
        Ok(credentials) => credentials,
        Err(error) => return error.into_response(),
    };
    let service = s.security.clone();
    match result(
        tokio::task::spawn_blocking(move || service.acknowledge(&token, &csrf, &r.checksum)).await,
    ) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => e.into_response(),
    }
}
pub async fn audit(State(s): State<AppState>, headers: HeaderMap) -> Response {
    let Some(token) = cookie(&headers) else {
        return ApiError::Unauthorized.into_response();
    };
    let service = s.security.clone();
    match result(tokio::task::spawn_blocking(move || service.audit(&token)).await) {
        Ok(events) => Json(AuditResponse { events }).into_response(),
        Err(e) => e.into_response(),
    }
}
fn session_response(status: StatusCode, g: SessionGrant) -> Response {
    let cookie = format!(
        "canopy_session={}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age={}",
        g.token,
        (g.expires_at - now()).max(0)
    );
    let mut r = (status, Json(g)).into_response();
    r.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("safe cookie"),
    );
    r
}
pub(crate) fn mutation_credentials(
    s: &AppState,
    headers: &HeaderMap,
) -> Result<(String, String), ApiError> {
    origin(s, headers)?;
    let token = cookie(headers).ok_or(ApiError::Unauthorized)?;
    let csrf = headers
        .get("x-canopy-csrf")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .ok_or(ApiError::Csrf)?;
    Ok((token, csrf))
}
fn origin(s: &AppState, h: &HeaderMap) -> Result<(), ApiError> {
    if h.get(ORIGIN).and_then(|v| v.to_str().ok()) == Some(s.security.origin()) {
        Ok(())
    } else {
        Err(ApiError::Origin)
    }
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub(crate) fn cookie(h: &HeaderMap) -> Option<String> {
    h.get(COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|v| v.strip_prefix("canopy_session=").map(str::to_owned))
        .filter(|v| !v.is_empty())
}
fn result<T>(r: Result<Result<T, SecurityError>, tokio::task::JoinError>) -> Result<T, ApiError> {
    r.map_err(|_| ApiError::Internal)?.map_err(Into::into)
}
pub(crate) enum ApiError {
    SetupClosed,
    Input,
    Credentials,
    Limited,
    Unauthorized,
    Csrf,
    Origin,
    RecoveryChecksum,
    Internal,
}
impl From<SecurityError> for ApiError {
    fn from(e: SecurityError) -> Self {
        match e {
            SecurityError::SetupClosed => Self::SetupClosed,
            SecurityError::InvalidInput(field) => {
                tracing::info!(event = "security_input_rejected", field);
                Self::Input
            }
            SecurityError::InvalidCredentials => Self::Credentials,
            SecurityError::LoginLimited => Self::Limited,
            SecurityError::Unauthorized => Self::Unauthorized,
            SecurityError::Csrf => Self::Csrf,
            SecurityError::RecoveryChecksum => Self::RecoveryChecksum,
            SecurityError::Internal(reason) => {
                tracing::error!(event = "security_request_failed", reason);
                Self::Internal
            }
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, title) = match self {
            Self::SetupClosed => (
                StatusCode::CONFLICT,
                "setup_closed",
                "Setup is already complete",
            ),
            Self::Input => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_input",
                "Input was rejected",
            ),
            Self::Credentials => (
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Authentication failed",
            ),
            Self::Limited => (
                StatusCode::TOO_MANY_REQUESTS,
                "login_limited",
                "Login is temporarily limited",
            ),
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Authentication required",
            ),
            Self::Csrf => (
                StatusCode::FORBIDDEN,
                "csrf_rejected",
                "Request proof was rejected",
            ),
            Self::Origin => (
                StatusCode::FORBIDDEN,
                "origin_rejected",
                "Request origin was rejected",
            ),
            Self::RecoveryChecksum => (
                StatusCode::CONFLICT,
                "recovery_checksum_mismatch",
                "Recovery checksum did not match",
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Request could not be completed",
            ),
        };
        (
            status,
            Json(Problem {
                r#type: "urn:canopy:problem",
                title,
                status: status.as_u16(),
                code,
            }),
        )
            .into_response()
    }
}

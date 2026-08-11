// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use super::{app_error::AppError, AppState};

const SESSION_USERNAME_KEY: &str = "username";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginInput {
    username: String,
    password: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthSessionResponse {
    authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/auth/session", get(session_status))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
}

fn no_store_header() -> [(header::HeaderName, HeaderValue); 1] {
    [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))]
}

async fn session_username(session: &Session) -> Result<Option<String>, AppError> {
    session
        .get::<String>(SESSION_USERNAME_KEY)
        .await
        .map_err(|error| AppError::internal(format!("failed to read management session: {error}")))
}

async fn session_status(
    State(state): State<Arc<AppState>>,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let username = session_username(&session)
        .await?
        .filter(|username| state.management_auth.matches_username(username));
    Ok((
        no_store_header(),
        Json(AuthSessionResponse {
            authenticated: username.is_some(),
            username,
        }),
    ))
}

async fn login(
    State(state): State<Arc<AppState>>,
    session: Session,
    Json(input): Json<LoginInput>,
) -> Result<impl IntoResponse, AppError> {
    if !state
        .management_auth
        .matches(&input.username, &input.password)
    {
        return Err(AppError::invalid_credentials());
    }

    session
        .cycle_id()
        .await
        .map_err(|error| AppError::internal(format!("failed to rotate session id: {error}")))?;
    session
        .insert(SESSION_USERNAME_KEY, &input.username)
        .await
        .map_err(|error| {
            AppError::internal(format!("failed to create management session: {error}"))
        })?;

    Ok((
        no_store_header(),
        Json(AuthSessionResponse {
            authenticated: true,
            username: Some(input.username),
        }),
    ))
}

async fn logout(session: Session) -> Result<impl IntoResponse, AppError> {
    session.flush().await.map_err(|error| {
        AppError::internal(format!("failed to destroy management session: {error}"))
    })?;
    Ok((no_store_header(), StatusCode::NO_CONTENT))
}

fn same_origin(origin: &str, host: &str) -> bool {
    origin
        .parse::<Uri>()
        .ok()
        .and_then(|uri| uri.authority().map(|authority| authority.as_str() == host))
        .unwrap_or(false)
}

pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

fn has_valid_basic_auth(state: &AppState, request: &Request) -> bool {
    let Some(encoded) = authorization_parameter(request, "Basic") else {
        return false;
    };
    let Ok(decoded) = STANDARD.decode(encoded) else {
        return false;
    };
    let Some(separator) = decoded.iter().position(|byte| *byte == b':') else {
        return false;
    };
    state
        .management_auth
        .matches_bytes(&decoded[..separator], &decoded[separator + 1..])
}

fn authorization_parameter<'a>(request: &'a Request, expected_scheme: &str) -> Option<&'a str> {
    let value = request
        .headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let (scheme, parameter) = value.split_once(' ')?;
    (scheme.eq_ignore_ascii_case(expected_scheme) && !parameter.is_empty()).then_some(parameter)
}

fn management_unauthorized() -> Response {
    super::json_error_response(
        StatusCode::UNAUTHORIZED,
        "UNAUTHORIZED",
        "Management authentication required",
    )
}

fn bearer_unauthorized() -> Response {
    let mut response = super::json_error_response(
        StatusCode::UNAUTHORIZED,
        "UNAUTHORIZED",
        "MCP bearer token required",
    );
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Bearer realm=\"Moor MCP\""),
    );
    response
}

pub async fn origin_middleware(req: Request, next: Next) -> Response {
    let headers = req.headers();
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if !origin.is_empty() && !same_origin(origin, host) {
        return super::json_error_response(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Cross-origin requests are not allowed",
        );
    }
    next.run(req).await
}

pub async fn management_auth_middleware(
    State(state): State<Arc<AppState>>,
    session: Session,
    req: Request,
    next: Next,
) -> Response {
    let valid_session = match session_username(&session).await {
        Ok(username) => {
            username.is_some_and(|username| state.management_auth.matches_username(&username))
        }
        Err(error) => return error.into_response(),
    };
    if valid_session || has_valid_basic_auth(&state, &req) {
        return next.run(req).await;
    }
    management_unauthorized()
}

pub async fn mcp_auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let token = authorization_parameter(&req, "Bearer").map(str::to_owned);
    if let Some(token) = token {
        if state.mcp_auth.matches(&token).await {
            return next.run(req).await;
        }
    }
    bearer_unauthorized()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_same_origin_authority() {
        assert!(same_origin("https://moor.example", "moor.example"));
        assert!(same_origin("http://localhost:9223", "localhost:9223"));
    }

    #[test]
    fn rejects_cross_origin_authority() {
        assert!(!same_origin("https://evil.example", "moor.example"));
        assert!(!same_origin("null", "moor.example"));
    }
}

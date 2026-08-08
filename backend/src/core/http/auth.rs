// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::AppState;
use axum::{
    extract::Request,
    http::{header, HeaderValue, StatusCode, Uri},
    middleware::Next,
    response::Response,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::sync::Arc;

fn same_origin(origin: &str, host: &str) -> bool {
    origin
        .parse::<Uri>()
        .ok()
        .and_then(|uri| uri.authority().map(|authority| authority.as_str() == host))
        .unwrap_or(false)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

fn has_valid_basic_auth(state: &AppState, request: &Request) -> bool {
    let Some(encoded) = authorization_parameter(request, "Basic")
    else {
        return false;
    };
    let Ok(decoded) = STANDARD.decode(encoded) else {
        return false;
    };
    let Some(separator) = decoded.iter().position(|byte| *byte == b':') else {
        return false;
    };
    constant_time_eq(
        &decoded[..separator],
        state.management_auth.username.as_bytes(),
    ) && constant_time_eq(
        &decoded[separator + 1..],
        state.management_auth.password.as_bytes(),
    )
}

fn has_valid_bearer_auth(state: &AppState, request: &Request) -> bool {
    authorization_parameter(request, "Bearer")
        .map(|token| constant_time_eq(token.as_bytes(), state.mcp_auth.token.as_bytes()))
        .unwrap_or(false)
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

fn basic_unauthorized() -> Response {
    let mut response = super::json_error_response(
        StatusCode::UNAUTHORIZED,
        "UNAUTHORIZED",
        "Management authentication required",
    );
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Basic realm=\"Moor\", charset=\"UTF-8\""),
    );
    response
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

pub async fn auth_middleware(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let headers = req.headers();
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !origin.is_empty() && !same_origin(origin, host) {
        return super::json_error_response(
            StatusCode::FORBIDDEN,
            "FORBIDDEN",
            "Cross-origin requests are not allowed",
        );
    }

    let path = req.uri().path();
    if path == "/api/health" {
        return next.run(req).await;
    }
    if path == "/mcp" {
        if !has_valid_bearer_auth(&state, &req) {
            return bearer_unauthorized();
        }
    } else if !has_valid_basic_auth(&state, &req) {
        return basic_unauthorized();
    }

    next.run(req).await
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

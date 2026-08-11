// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, HeaderValue},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::Serialize;

use crate::core::http::{app_error::AppError, AppState};
use crate::core::services::mcp_token;

#[derive(Serialize)]
struct McpTokenResponse {
    token: String,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/security/mcp-token", get(get_mcp_token))
        .route("/api/security/mcp-token/rotate", post(rotate_mcp_token))
}

fn no_store_header() -> [(header::HeaderName, HeaderValue); 1] {
    [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))]
}

async fn get_mcp_token(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    (
        no_store_header(),
        Json(McpTokenResponse {
            token: state.mcp_auth.reveal().await,
        }),
    )
}

async fn rotate_mcp_token(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, AppError> {
    let token = mcp_token::rotate(&state.db).map_err(AppError::internal)?;
    state.mcp_auth.replace(token.clone()).await;
    Ok((no_store_header(), Json(McpTokenResponse { token })))
}

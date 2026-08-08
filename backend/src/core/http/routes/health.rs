// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use crate::core::http::AppState;
use axum::{extract::State, response::Json, routing::get, Router};
use serde_json::{json, Value};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/runtime", get(runtime))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn runtime(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "port": state.port,
        "baseUrl": state.public_url,
        "mcpUrl": format!("{}/mcp", state.public_url),
        "version": state.version,
        "pid": std::process::id(),
        "managementAuth": "basic",
        "mcpAuth": "bearer",
    }))
}

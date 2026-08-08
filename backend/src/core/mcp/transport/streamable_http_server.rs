// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use crate::core::http::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::sync::Arc;

/// Handle incoming MCP Streamable HTTP requests at `/mcp`.
/// Supports both JSON responses and SSE streaming.
pub async fn handle_mcp_request(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
) -> Response {
    let headers = req.headers().clone();
    let accept = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let agent_info = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Read request body
    let body_bytes = match axum::body::to_bytes(req.into_body(), 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST, "Request body too large").into_response(),
    };

    // Accept header determines response format.
    let accepts_sse = accept.contains("text/event-stream");

    // Parse JSON-RPC request
    let raw_message: serde_json::Value = match serde_json::from_slice(&body_bytes) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json")],
                crate::core::mcp::jsonrpc::make_error(
                    crate::core::mcp::jsonrpc::Id::Number(0),
                    crate::core::mcp::jsonrpc::PARSE_ERROR,
                    "Invalid JSON-RPC",
                )
                .to_string(),
            )
                .into_response()
        }
    };
    if raw_message.get("id").is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    let parsed = match crate::core::mcp::jsonrpc::parse_request_value(&raw_message) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                [(header::CONTENT_TYPE, "application/json")],
                crate::core::mcp::jsonrpc::make_error(
                    crate::core::mcp::jsonrpc::Id::Number(0),
                    crate::core::mcp::jsonrpc::INVALID_REQUEST,
                    "Invalid JSON-RPC request",
                )
                .to_string(),
            )
                .into_response()
        }
    };

    let (id, method, params) = parsed;

    // Route to MCP server handler
    let response = crate::core::mcp::server::handle_request(
        id,
        &method,
        params,
        state.clone(),
        agent_info.as_deref(),
    )
    .await;

    if accepts_sse {
        // Return as SSE event stream
        let sse_data = format!("event: message\ndata: {}\n\n", response);
        (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/event-stream"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            sse_data,
        )
            .into_response()
    } else {
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            response.to_string(),
        )
            .into_response()
    }
}

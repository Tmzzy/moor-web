// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

pub mod http_client;
pub mod mcp_client;
pub mod stdio_client;
pub mod streamable_http_server;

use std::time::Duration;

/// Keeps recoverable HTTP session expiry distinct from ordinary request failures.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("{0}")]
    SessionInvalid(String),
    #[error("{0}")]
    Other(String),
}

impl From<String> for McpError {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}

pub(crate) fn format_timeout_duration(timeout: Duration) -> String {
    if timeout.as_millis().is_multiple_of(1000) {
        format!("{}s", timeout.as_secs())
    } else {
        format!("{}ms", timeout.as_millis())
    }
}

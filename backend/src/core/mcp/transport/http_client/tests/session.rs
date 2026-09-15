// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::*;

type RequestTrace = Arc<Mutex<Vec<(HeaderMap, Value)>>>;

#[derive(Clone)]
struct ProbeState {
    trace: RequestTrace,
    protocol_version: Value,
    with_session: bool,
    initialize_as_sse: bool,
    call_response: Option<(StatusCode, String)>,
}

impl Default for ProbeState {
    fn default() -> Self {
        Self {
            trace: Arc::default(),
            protocol_version: serde_json::json!("2025-03-26"),
            with_session: true,
            initialize_as_sse: false,
            call_response: None,
        }
    }
}

async fn probe_endpoint(
    State(state): State<ProbeState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> axum::response::Response {
    state.trace.lock().await.push((headers, body.clone()));
    let method = body["method"].as_str().unwrap_or_default();
    let id = body["id"].clone();
    match method {
        "initialize" => {
            let mut result = serde_json::json!({"capabilities": {}, "serverInfo": {}});
            if !state.protocol_version.is_null() {
                result["protocolVersion"] = state.protocol_version;
            }
            let response = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
            let mut response = if state.initialize_as_sse {
                (
                    [("content-type", "text/event-stream")],
                    format!("event: message\ndata: {response}\n\n"),
                )
                    .into_response()
            } else {
                Json(response).into_response()
            };
            if state.with_session {
                response.headers_mut().insert(
                    MCP_SESSION_ID_HEADER,
                    "probe-session".parse().expect("valid session header"),
                );
            }
            response
        }
        "notifications/initialized" => StatusCode::ACCEPTED.into_response(),
        "tools/list" => Json(serde_json::json!({
            "jsonrpc": "2.0", "id": id, "result": {"tools": []}
        }))
        .into_response(),
        _ => match state.call_response {
            Some(response) => response.into_response(),
            None => Json(serde_json::json!({
                "jsonrpc": "2.0", "id": id, "result": {"ok": true}
            }))
            .into_response(),
        },
    }
}

struct TestServer {
    url: String,
    task: tokio::task::JoinHandle<()>,
}

impl TestServer {
    async fn spawn(app: Router) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("test server address");
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("serve test requests");
        });
        Self {
            url: format!("http://{addr}"),
            task,
        }
    }

    async fn connect(&self, path: &str) -> McpClient {
        McpClient::connect_http(HttpConnectConfig {
            server_name: "probe".into(),
            url: format!("{}{path}", self.url),
            headers: HashMap::from([("MCP-Protocol-Version".into(), "2024-11-05".into())]),
            request_timeout_ms: 1_000,
        })
        .await
        .expect("connect test client")
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn session_expiry_reaches_client_and_notifications_as_a_typed_error() {
    for status in [StatusCode::BAD_REQUEST, StatusCode::NOT_FOUND] {
        for message in [
            "Invalid SESSION ID",
            "session expired",
            "Session not found",
            "Unknown session",
            r#"{"error":{"message":"No valid session ID provided"}}"#,
        ] {
            let state = ProbeState {
                call_response: Some((status, message.into())),
                ..ProbeState::default()
            };
            let app = Router::new()
                .route("/mcp", post(probe_endpoint))
                .with_state(state);
            let server = TestServer::spawn(app).await;
            let client = server.connect("/mcp").await;
            let err = client
                .call_tool("echo", serde_json::json!({}))
                .await
                .expect_err("expired session");
            assert!(matches!(err, McpError::SessionInvalid(body) if body.contains(message)));

            let transport = HttpClientTransport::new(
                &format!("{}/mcp", server.url),
                HashMap::new(),
                Duration::from_secs(1),
            );
            // Unknown mode with a recorded session must still report expiry before SSE fallback.
            *transport.session_id.lock().await = Some("probe-session".into());
            let err = transport
                .send_notification("notifications/cancelled", None)
                .await
                .expect_err("expired notification session");
            assert!(matches!(err, McpError::SessionInvalid(_)));
            let err = transport
                .send_request(1, "tools/call", None)
                .await
                .expect_err("expired request session must not fall back to SSE");
            assert!(matches!(err, McpError::SessionInvalid(_)));
        }
    }
}

#[tokio::test]
async fn unrelated_errors_and_responses_without_a_session_are_not_session_expiry() {
    for (with_session, status, body) in [
        (
            true,
            StatusCode::BAD_REQUEST,
            r#"{"error":{"message":"Invalid arguments"}}"#,
        ),
        (true, StatusCode::NOT_FOUND, "Unknown tool"),
        (true, StatusCode::NOT_FOUND, ""),
        (true, StatusCode::UNAUTHORIZED, "Session expired"),
        (true, StatusCode::FORBIDDEN, "Session expired"),
        (true, StatusCode::INTERNAL_SERVER_ERROR, "Session expired"),
        (
            true,
            StatusCode::OK,
            r#"{"error":{"message":"Session expired"}}"#,
        ),
        (false, StatusCode::BAD_REQUEST, "Session expired"),
        (false, StatusCode::NOT_FOUND, "Session expired"),
    ] {
        let state = ProbeState {
            with_session,
            call_response: Some((status, body.into())),
            ..ProbeState::default()
        };
        let app = Router::new()
            .route("/mcp", post(probe_endpoint))
            .with_state(state);
        let server = TestServer::spawn(app).await;
        let client = server.connect("/mcp").await;
        let err = client
            .call_tool("echo", serde_json::json!({}))
            .await
            .expect_err("call should fail");
        assert!(
            matches!(err, McpError::Other(_)),
            "status={status}, body={body}, session={with_session}"
        );
    }
}

#[tokio::test]
async fn handshake_negotiates_version_before_all_following_requests() {
    for (version, initialize_as_sse, expected) in [
        (serde_json::json!("2025-03-26"), false, "2025-03-26"),
        (serde_json::json!("2024-11-05"), true, "2024-11-05"),
        (Value::Null, false, DEFAULT_HTTP_PROTOCOL_VERSION),
        (
            serde_json::json!("  "),
            false,
            DEFAULT_HTTP_PROTOCOL_VERSION,
        ),
        (serde_json::json!(42), false, DEFAULT_HTTP_PROTOCOL_VERSION),
    ] {
        let state = ProbeState {
            protocol_version: version,
            initialize_as_sse,
            ..ProbeState::default()
        };
        let trace = state.trace.clone();
        let app = Router::new()
            .route("/mcp", post(probe_endpoint))
            .with_state(state);
        let server = TestServer::spawn(app).await;
        let client = server.connect("/mcp").await;
        client.list_tools().await.expect("list tools");
        client
            .call_tool("echo", serde_json::json!({}))
            .await
            .expect("call tool");
        let trace = trace.lock().await;
        let observed: Vec<_> = trace
            .iter()
            .map(|(headers, body)| {
                let versions: Vec<_> = headers
                    .get_all(MCP_PROTOCOL_VERSION_HEADER)
                    .iter()
                    .map(|v| v.to_str().expect("version header"))
                    .collect();
                (
                    body["method"].as_str().expect("method"),
                    versions,
                    headers
                        .get(MCP_SESSION_ID_HEADER)
                        .map(|v| v.to_str().expect("session header")),
                )
            })
            .collect();
        assert_eq!(
            trace[0].1["params"]["protocolVersion"],
            DEFAULT_HTTP_PROTOCOL_VERSION
        );
        assert_eq!(
            observed,
            vec![
                ("initialize", vec![DEFAULT_HTTP_PROTOCOL_VERSION], None),
                (
                    "notifications/initialized",
                    vec![expected],
                    Some("probe-session")
                ),
                ("tools/list", vec![expected], Some("probe-session")),
                ("tools/call", vec![expected], Some("probe-session")),
            ]
        );
    }
}

#[derive(Clone)]
struct SseVersionState {
    messages: SseTestState,
    trace: RequestTrace,
}

async fn versioned_sse_endpoint(
    State(state): State<SseVersionState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    state
        .trace
        .lock()
        .await
        .push((headers, serde_json::json!({"method": "GET"})));
    sse_endpoint(State(state.messages)).await
}

async fn versioned_message_endpoint(
    State(state): State<SseVersionState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    state.trace.lock().await.push((headers, body.clone()));
    if let Some(id) = body.get("id") {
        let result = match body["method"].as_str() {
            Some("initialize") => serde_json::json!({"protocolVersion": "2024-11-05"}),
            Some("tools/list") => serde_json::json!({"tools": []}),
            _ => serde_json::json!({"ok": true}),
        };
        state
            .messages
            .sender
            .send(serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}))
            .expect("queue SSE result");
    }
    StatusCode::ACCEPTED
}

#[tokio::test]
async fn legacy_sse_uses_the_negotiated_version_for_message_posts() {
    let (sender, receiver) = mpsc::unbounded_channel();
    let trace = Arc::default();
    let state = SseVersionState {
        messages: SseTestState {
            sender,
            receiver: Arc::new(Mutex::new(Some(receiver))),
        },
        trace: Arc::clone(&trace),
    };
    let app = Router::new()
        .route(
            "/sse",
            get(versioned_sse_endpoint).post(streamable_unsupported),
        )
        .route("/messages", post(versioned_message_endpoint))
        .with_state(state);
    let server = TestServer::spawn(app).await;
    let client = server.connect("/sse").await;
    client.list_tools().await.expect("list SSE tools");
    client
        .call_tool("echo", serde_json::json!({}))
        .await
        .expect("call SSE tool");
    let trace = trace.lock().await;
    let observed: Vec<_> = trace
        .iter()
        .map(|(headers, body)| {
            (
                body["method"].as_str().expect("method"),
                headers
                    .get_all(MCP_PROTOCOL_VERSION_HEADER)
                    .iter()
                    .map(|v| v.to_str().expect("version"))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        observed,
        vec![
            ("GET", vec![DEFAULT_HTTP_PROTOCOL_VERSION]),
            ("initialize", vec![DEFAULT_HTTP_PROTOCOL_VERSION]),
            ("notifications/initialized", vec!["2024-11-05"]),
            ("tools/list", vec!["2024-11-05"]),
            ("tools/call", vec!["2024-11-05"]),
        ]
    );
}

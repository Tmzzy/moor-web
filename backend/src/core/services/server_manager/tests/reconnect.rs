// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::*;
use crate::core::db::server_repo::ServerInsertInput;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex as StdMutex;
use tokio::sync::Semaphore;

struct Fixture {
    data_dir: std::path::PathBuf,
    db: Arc<Database>,
    manager: Arc<ServerManager>,
    id: String,
    profile_id: String,
}

impl Fixture {
    fn new(connector: Arc<dyn McpConnector>) -> Self {
        let data_dir = temp_data_dir("http-reconnect");
        std::fs::create_dir_all(&data_dir).expect("create test data directory");
        let (db, manager) = build_manager_with_fake_connector(&data_dir, connector);
        let profile_id = ProfileRepository::new(&db)
            .create("Recovery tests")
            .expect("create recovery test profile")
            .id;
        Self {
            data_dir,
            db,
            manager,
            id: uuid::Uuid::new_v4().to_string(),
            profile_id,
        }
    }

    async fn start(&self, url: &str) {
        ServerRepository::new(&self.db)
            .insert_one_with_id(
                &self.id,
                0,
                &ServerInsertInput {
                    name: "recovery".into(),
                    connection_type: "http".into(),
                    command: None,
                    args: None,
                    url: Some(url.into()),
                    env: None,
                    headers: None,
                    working_dir: None,
                    auto_start: false,
                    profile_ids: vec![self.profile_id.clone()],
                },
            )
            .expect("insert HTTP server");
        ProfileRepository::new(&self.db)
            .assign_to_profile(&self.profile_id, std::slice::from_ref(&self.id))
            .expect("assign HTTP server to profile");
        self.manager.load_from_db().await;
        self.manager
            .start_server(&self.id)
            .await
            .expect("start HTTP server");
    }

    fn cached_tools(&self) -> Value {
        let mut tools = ToolDiscoveryRepository::new(&self.db)
            .find_by_server_id(&self.id)
            .expect("read cached tools");
        tools.sort_by(|a, b| a.tool_name.cmp(&b.tool_name));
        serde_json::to_value(tools).expect("serialize cached tools")
    }

    async fn slot(&self) -> ServerSlot {
        self.manager
            .slots
            .lock()
            .await
            .get(&self.id)
            .expect("server slot")
            .clone()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

fn discovered_tools(generation: &str) -> Vec<ToolInsert> {
    vec![
        ToolInsert {
            name: "echo".into(),
            description: Some(format!("Echo {generation}")),
            input_schema: Some(json!({"type": "object", "title": generation})),
        },
        ToolInsert {
            name: format!("only_{generation}"),
            description: None,
            input_schema: Some(json!({"type": "object"})),
        },
    ]
}

#[derive(Clone, Debug, PartialEq)]
struct CallRecord {
    name: String,
    args: Value,
    timeout_ms: u32,
}

struct SessionProbe {
    calls: StdMutex<Vec<CallRecord>>,
    called: Semaphore,
    dropped: AtomicBool,
}

impl Default for SessionProbe {
    fn default() -> Self {
        Self {
            calls: StdMutex::default(),
            called: Semaphore::new(0),
            dropped: AtomicBool::new(false),
        }
    }
}

enum CallOutcome {
    Success,
    Expired,
    Other,
}

struct RecoverySession {
    outcome: CallOutcome,
    probe: Arc<SessionProbe>,
    timeout_ms: u32,
}

impl McpSession for RecoverySession {
    fn list_tools(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ToolInsert>, String>> + Send + '_>> {
        Box::pin(async { Ok(vec![]) })
    }

    fn call_tool<'a>(
        &'a self,
        tool_name: &'a str,
        args: Value,
    ) -> Pin<Box<dyn Future<Output = Result<Value, McpError>> + Send + 'a>> {
        Box::pin(async move {
            self.probe
                .calls
                .lock()
                .expect("record tool call")
                .push(CallRecord {
                    name: tool_name.into(),
                    args,
                    timeout_ms: self.timeout_ms,
                });
            self.probe.called.add_permits(1);
            match self.outcome {
                CallOutcome::Success => Ok(json!({"ok": true})),
                CallOutcome::Expired => Err(McpError::SessionInvalid("Session expired".into())),
                CallOutcome::Other => Err(McpError::Other(
                    "400 Session expired is ordinary error text".into(),
                )),
            }
        })
    }

    fn disconnect(&mut self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn set_request_timeout_ms(&mut self, request_timeout_ms: u32) {
        self.timeout_ms = request_timeout_ms;
    }

    fn alive_receiver(&self) -> Option<tokio::sync::watch::Receiver<bool>> {
        None
    }
}

impl Drop for RecoverySession {
    fn drop(&mut self) {
        self.probe.dropped.store(true, Ordering::SeqCst);
    }
}

struct ConnectStep {
    result: Result<(Vec<ToolInsert>, Box<dyn McpSession>), String>,
    release: Option<Arc<Semaphore>>,
}

impl ConnectStep {
    fn session(generation: &str, outcome: CallOutcome) -> (Self, Arc<SessionProbe>) {
        let probe = Arc::new(SessionProbe::default());
        (
            Self {
                result: Ok((
                    discovered_tools(generation),
                    Box::new(RecoverySession {
                        outcome,
                        probe: probe.clone(),
                        timeout_ms: 0,
                    }),
                )),
                release: None,
            },
            probe,
        )
    }
}

struct RecoveryConnector {
    steps: StdMutex<VecDeque<ConnectStep>>,
    calls: AtomicUsize,
    started: Semaphore,
}

impl RecoveryConnector {
    fn new(steps: Vec<ConnectStep>) -> Arc<Self> {
        Arc::new(Self {
            steps: StdMutex::new(steps.into()),
            calls: AtomicUsize::new(0),
            started: Semaphore::new(0),
        })
    }
}

impl McpConnector for RecoveryConnector {
    fn connect<'a>(
        &'a self,
        _config: &'a StoredServerConfig,
        _timeouts: ServerTimeouts,
    ) -> BoxedConnectFuture<'a> {
        Box::pin(async move {
            let step = self
                .steps
                .lock()
                .expect("connect steps")
                .pop_front()
                .expect("unexpected additional connection");
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.started.add_permits(1);
            if let Some(release) = step.release {
                release
                    .acquire()
                    .await
                    .expect("release test connection")
                    .forget();
            }
            step.result
        })
    }
}

async fn acquire(permits: &Semaphore, count: u32) {
    tokio::time::timeout(Duration::from_secs(2), permits.acquire_many(count))
        .await
        .expect("test operation did not reach its checkpoint")
        .expect("checkpoint semaphore")
        .forget();
}

#[tokio::test]
async fn concurrent_expired_calls_share_a_new_session_and_preserve_arguments_and_timeout() {
    let (old, old_probe) = ConnectStep::session("A", CallOutcome::Expired);
    let (mut new, new_probe) = ConnectStep::session("B", CallOutcome::Success);
    let release = Arc::new(Semaphore::new(0));
    new.release = Some(release.clone());
    let connector = RecoveryConnector::new(vec![old, new]);
    let fixture = Fixture::new(connector.clone());
    fixture.start("http://unused.invalid/mcp").await;
    settings::update_settings(
        &fixture.db,
        json!({"advanced": {"mcpRequestTimeoutMs": 6_123}}),
    )
    .expect("update runtime timeout");
    let before = fixture.slot().await;
    let mut events = fixture.manager.event_bus.subscribe();

    let first_manager = fixture.manager.clone();
    let first_profile_id = fixture.profile_id.clone();
    let first = tokio::spawn(async move {
        first_manager
            .call_tool(&first_profile_id, "recovery__echo", json!({"caller": 1}))
            .await
    });
    acquire(&connector.started, 2).await;
    acquire(&old_probe.called, 1).await;
    let second_manager = fixture.manager.clone();
    let second_profile_id = fixture.profile_id.clone();
    let second = tokio::spawn(async move {
        second_manager
            .call_tool(&second_profile_id, "recovery__echo", json!({"caller": 2}))
            .await
    });
    acquire(&old_probe.called, 1).await;
    release.add_permits(1);
    first
        .await
        .expect("first caller task")
        .expect("first caller recovered");
    second
        .await
        .expect("second caller task")
        .expect("second caller recovered");

    assert_eq!(connector.calls.load(Ordering::SeqCst), 2);
    let mut old_calls = old_probe.calls.lock().expect("old calls").clone();
    let mut new_calls = new_probe.calls.lock().expect("new calls").clone();
    old_calls.sort_by_key(|call| call.args["caller"].as_u64());
    new_calls.sort_by_key(|call| call.args["caller"].as_u64());
    assert_eq!(old_calls, new_calls);
    assert!(new_calls
        .iter()
        .all(|call| call.name == "echo" && call.timeout_ms == 6_123));
    let after = fixture.slot().await;
    assert_eq!(after.start_token, before.start_token);
    assert_eq!(after.status.as_str(), "running");
    assert!(!Arc::ptr_eq(
        after.session.as_ref().expect("new session"),
        before.session.as_ref().expect("old session")
    ));
    assert!(matches!(events.try_recv(), Ok(Evt::ServerTools { .. })));
    assert!(matches!(
        events.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn recovered_tools_remain_scoped_to_the_assigned_profile() {
    let (old, _) = ConnectStep::session("A", CallOutcome::Expired);
    let (new, new_probe) = ConnectStep::session("B", CallOutcome::Success);
    let connector = RecoveryConnector::new(vec![old, new]);
    let fixture = Fixture::new(connector.clone());
    fixture.start("http://unused.invalid/mcp").await;
    let other_profile = ProfileRepository::new(&fixture.db)
        .create("Unassigned profile")
        .expect("create unassigned profile");

    fixture
        .manager
        .call_tool(&fixture.profile_id, "recovery__echo", json!({}))
        .await
        .expect("assigned profile recovers the expired session");
    let err = fixture
        .manager
        .call_tool(&other_profile.id, "recovery__echo", json!({}))
        .await
        .expect_err("unassigned profile must not call the recovered tool");

    assert_eq!(err, "Tool \"recovery__echo\" not found or disabled");
    assert_eq!(connector.calls.load(Ordering::SeqCst), 2);
    assert_eq!(new_probe.calls.lock().expect("new calls").len(), 1);
}

#[tokio::test]
async fn ordinary_errors_never_reconnect_even_if_the_message_mentions_session_expiry() {
    let (old, probe) = ConnectStep::session("A", CallOutcome::Other);
    let connector = RecoveryConnector::new(vec![old]);
    let fixture = Fixture::new(connector.clone());
    fixture.start("http://unused.invalid/mcp").await;
    let err = fixture
        .manager
        .call_tool(&fixture.profile_id, "recovery__echo", json!({}))
        .await
        .expect_err("ordinary error");
    assert_eq!(err, "400 Session expired is ordinary error text");
    assert_eq!(connector.calls.load(Ordering::SeqCst), 1);
    assert_eq!(probe.calls.lock().expect("calls").len(), 1);
}

#[tokio::test]
async fn expiry_on_the_retry_is_returned_without_a_third_call_or_connection() {
    let (old, old_probe) = ConnectStep::session("A", CallOutcome::Expired);
    let (new, new_probe) = ConnectStep::session("B", CallOutcome::Expired);
    let connector = RecoveryConnector::new(vec![old, new]);
    let fixture = Fixture::new(connector.clone());
    fixture.start("http://unused.invalid/mcp").await;
    let err = fixture
        .manager
        .call_tool(&fixture.profile_id, "recovery__echo", json!({}))
        .await
        .expect_err("retry also expires");
    assert_eq!(err, "Session expired");
    assert_eq!(connector.calls.load(Ordering::SeqCst), 2);
    assert_eq!(old_probe.calls.lock().expect("old calls").len(), 1);
    assert_eq!(new_probe.calls.lock().expect("new calls").len(), 1);
}

#[tokio::test]
async fn reconnect_failure_preserves_the_session_cache_and_lifecycle() {
    let (old, old_probe) = ConnectStep::session("A", CallOutcome::Expired);
    let failure = ConnectStep {
        result: Err("initialize failed".into()),
        release: None,
    };
    let connector = RecoveryConnector::new(vec![old, failure]);
    let fixture = Fixture::new(connector);
    fixture.start("http://unused.invalid/mcp").await;
    let before = fixture.slot().await;
    let cache = fixture.cached_tools();
    let mut events = fixture.manager.event_bus.subscribe();
    let err = fixture
        .manager
        .call_tool(&fixture.profile_id, "recovery__echo", json!({}))
        .await
        .expect_err("reconnect fails");
    assert!(err.contains("initialize failed"));
    assert_eq!(old_probe.calls.lock().expect("old calls").len(), 1);
    assert_eq!(fixture.cached_tools(), cache);
    let after = fixture.slot().await;
    assert!(Arc::ptr_eq(
        after.session.as_ref().expect("session"),
        before.session.as_ref().expect("original session")
    ));
    assert_eq!(
        (after.start_token, after.status.as_str()),
        (before.start_token, "running")
    );
    assert!(matches!(
        events.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn cache_failure_rolls_back_and_discards_the_new_client_without_retrying() {
    let (old, _) = ConnectStep::session("A", CallOutcome::Expired);
    let (mut new, new_probe) = ConnectStep::session("B", CallOutcome::Success);
    // A duplicate returned tool fails INSERT after DELETE and earlier inserts have run.
    new.result
        .as_mut()
        .expect("successful connect")
        .0
        .push(ToolInsert {
            name: "echo".into(),
            description: None,
            input_schema: None,
        });
    let connector = RecoveryConnector::new(vec![old, new]);
    let fixture = Fixture::new(connector);
    fixture.start("http://unused.invalid/mcp").await;
    let cache = fixture.cached_tools();
    let before = fixture.slot().await;
    let mut events = fixture.manager.event_bus.subscribe();
    let err = fixture
        .manager
        .call_tool(&fixture.profile_id, "recovery__echo", json!({}))
        .await
        .expect_err("cache insert fails");
    assert!(err.contains("Failed to cache tools") && err.contains("UNIQUE constraint failed"));
    assert_eq!(fixture.cached_tools(), cache);
    assert!(new_probe.calls.lock().expect("candidate calls").is_empty());
    assert!(new_probe.dropped.load(Ordering::SeqCst));
    let after = fixture.slot().await;
    assert!(Arc::ptr_eq(
        after.session.as_ref().expect("session"),
        before.session.as_ref().expect("original session")
    ));
    assert_eq!(
        (after.start_token, after.status.as_str()),
        (before.start_token, "running")
    );
    assert!(matches!(
        events.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn reconnect_timeout_discards_the_candidate_and_does_not_retry() {
    let (old, _) = ConnectStep::session("A", CallOutcome::Expired);
    let (mut new, new_probe) = ConnectStep::session("B", CallOutcome::Success);
    new.release = Some(Arc::new(Semaphore::new(0)));
    let connector = RecoveryConnector::new(vec![old, new]);
    let fixture = Fixture::new(connector);
    fixture.start("http://unused.invalid/mcp").await;
    settings::update_settings(
        &fixture.db,
        json!({"advanced": {"mcpServerStartTimeoutMs": 5_000}}),
    )
    .expect("set reconnect timeout");
    let cache = fixture.cached_tools();
    let mut events = fixture.manager.event_bus.subscribe();
    let err = tokio::time::timeout(
        Duration::from_secs(8),
        fixture
            .manager
            .call_tool(&fixture.profile_id, "recovery__echo", json!({})),
    )
    .await
    .expect("reconnect must respect configured deadline")
    .expect_err("reconnect times out");
    assert!(err.contains("MCP reconnect") && err.contains("timed out after 5s"));
    assert!(new_probe.dropped.load(Ordering::SeqCst));
    assert!(new_probe.calls.lock().expect("candidate calls").is_empty());
    assert_eq!(fixture.cached_tools(), cache);
    assert!(matches!(
        events.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn stop_remove_and_restart_reject_a_late_reconnect_result() {
    for action in ["stop", "remove", "restart"] {
        let (old, _) = ConnectStep::session("A", CallOutcome::Expired);
        let (mut candidate, candidate_probe) = ConnectStep::session("B", CallOutcome::Success);
        let release = Arc::new(Semaphore::new(0));
        candidate.release = Some(release.clone());
        let (restart, _) = ConnectStep::session("C", CallOutcome::Success);
        let connector = RecoveryConnector::new(vec![old, candidate, restart]);
        let fixture = Fixture::new(connector.clone());
        fixture.start("http://unused.invalid/mcp").await;
        let manager = fixture.manager.clone();
        let profile_id = fixture.profile_id.clone();
        let call = tokio::spawn(async move {
            manager
                .call_tool(&profile_id, "recovery__echo", json!({}))
                .await
        });
        acquire(&connector.started, 2).await;
        match action {
            "stop" => fixture
                .manager
                .stop_server(&fixture.id)
                .await
                .expect("stop during reconnect"),
            "remove" => assert!(fixture.manager.remove_server(&fixture.id).await),
            "restart" => {
                fixture
                    .manager
                    .stop_server(&fixture.id)
                    .await
                    .expect("stop before restart");
                fixture
                    .manager
                    .start_server(&fixture.id)
                    .await
                    .expect("restart during reconnect");
            }
            _ => unreachable!(),
        }
        let cache_after_action = fixture.cached_tools();
        let mut events = fixture.manager.event_bus.subscribe();
        release.add_permits(1);
        let err = call
            .await
            .expect("caller task")
            .expect_err("late reconnect must be rejected");
        assert!(err.contains("superseded or stopped"), "{action}: {err}");
        assert!(candidate_probe.dropped.load(Ordering::SeqCst));
        assert!(candidate_probe
            .calls
            .lock()
            .expect("candidate calls")
            .is_empty());
        assert_eq!(fixture.cached_tools(), cache_after_action, "{action}");
        let managed = fixture.manager.get_server(&fixture.id).await;
        assert_eq!(
            managed.map(|server| server.status),
            match action {
                "stop" => Some("stopped".into()),
                "restart" => Some("running".into()),
                _ => None,
            }
        );
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }
}

#[derive(Debug)]
struct WireRequest {
    method: String,
    session: Option<String>,
    version: String,
    body: Value,
    cached_title: Option<String>,
}

struct WireState {
    db: Arc<Database>,
    server_id: String,
    initializes: AtomicUsize,
    requests: StdMutex<Vec<WireRequest>>,
    expiry_status: StatusCode,
    fail_second_list: bool,
}

async fn wire_endpoint(
    State(state): State<Arc<WireState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let method = body["method"].as_str().expect("JSON-RPC method");
    let session = headers
        .get("mcp-session-id")
        .map(|value| value.to_str().expect("session id").to_string());
    let version = headers
        .get("mcp-protocol-version")
        .expect("protocol header")
        .to_str()
        .expect("protocol version")
        .to_string();
    let cached_title = ToolDiscoveryRepository::new(&state.db)
        .find_by_server_id(&state.server_id)
        .expect("read tool cache")
        .into_iter()
        .find(|tool| tool.tool_name == "echo")
        .and_then(|tool| tool.input_schema)
        .and_then(|schema| schema["title"].as_str().map(String::from));
    state
        .requests
        .lock()
        .expect("record wire request")
        .push(WireRequest {
            method: method.into(),
            session: session.clone(),
            version: version.clone(),
            body: body.clone(),
            cached_title,
        });
    let id = &body["id"];
    if method == "initialize" {
        assert!(
            session.is_none(),
            "new initialize must not carry the expired session"
        );
        assert_eq!(version, "2025-11-25");
        assert_eq!(body["params"]["protocolVersion"], "2025-11-25");
        let generation = state.initializes.fetch_add(1, Ordering::SeqCst);
        let (session, protocol) = if generation == 0 {
            ("session-A", "2025-03-26")
        } else {
            ("session-B", "2025-06-18")
        };
        return ([("mcp-session-id", session)], Json(json!({"jsonrpc": "2.0", "id": id, "result": {"protocolVersion": protocol, "capabilities": {"tools": {}}, "serverInfo": {"name": "wire", "version": "1"}}}))).into_response();
    }
    let (generation, expected_version) = match session.as_deref() {
        Some("session-A") => ("A", "2025-03-26"),
        Some("session-B") => ("B", "2025-06-18"),
        _ => return (StatusCode::BAD_REQUEST, "No valid session ID provided").into_response(),
    };
    assert_eq!(version, expected_version);
    match method {
        "notifications/initialized" => StatusCode::ACCEPTED.into_response(),
        "tools/list" if state.fail_second_list && generation == "B" => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": {"message": "tool list unavailable"}})),
        )
            .into_response(),
        "tools/list" => {
            let tools: Vec<_> = discovered_tools(generation).into_iter().map(|tool| json!({"name": tool.name, "description": tool.description, "inputSchema": tool.input_schema})).collect();
            Json(json!({"jsonrpc": "2.0", "id": id, "result": {"tools": tools}})).into_response()
        }
        "tools/call" if generation == "A" => (
            state.expiry_status,
            Json(json!({"error": {"message": "Session expired"}})),
        )
            .into_response(),
        "tools/call" => {
            Json(json!({"jsonrpc": "2.0", "id": id, "result": body["params"]["arguments"]}))
                .into_response()
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

struct WireServer {
    task: tokio::task::JoinHandle<()>,
    url: String,
}

impl WireServer {
    async fn start(state: Arc<WireState>) -> Self {
        let app = Router::new()
            .route("/mcp", post(wire_endpoint))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock HTTP server");
        let addr = listener.local_addr().expect("mock HTTP address");
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve mock HTTP");
        });
        Self {
            task,
            url: format!("http://{addr}/mcp"),
        }
    }
}

impl Drop for WireServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn http_recovery_negotiates_new_session_replaces_schema_then_retries_original_call() {
    for expiry_status in [StatusCode::BAD_REQUEST, StatusCode::NOT_FOUND] {
        let fixture = Fixture::new(Arc::new(StdioHttpConnector));
        let state = Arc::new(WireState {
            db: fixture.db.clone(),
            server_id: fixture.id.clone(),
            initializes: AtomicUsize::new(0),
            requests: StdMutex::default(),
            expiry_status,
            fail_second_list: false,
        });
        let server = WireServer::start(state.clone()).await;
        fixture.start(&server.url).await;
        let original = fixture.slot().await;
        let mut events = fixture.manager.event_bus.subscribe();
        let args = json!({"message": "original arguments", "nested": {"count": 3}});
        let result = fixture
            .manager
            .call_tool(&fixture.profile_id, "recovery__echo", args.clone())
            .await
            .expect("recover HTTP session");
        assert_eq!(result, args);
        assert_eq!(state.initializes.load(Ordering::SeqCst), 2);
        {
            let requests = state.requests.lock().expect("wire trace");
            let observed: Vec<_> = requests
                .iter()
                .map(|request| {
                    (
                        request.method.as_str(),
                        request.session.as_deref(),
                        request.version.as_str(),
                    )
                })
                .collect();
            assert_eq!(
                observed,
                vec![
                    ("initialize", None, "2025-11-25"),
                    ("notifications/initialized", Some("session-A"), "2025-03-26"),
                    ("tools/list", Some("session-A"), "2025-03-26"),
                    ("tools/call", Some("session-A"), "2025-03-26"),
                    ("initialize", None, "2025-11-25"),
                    ("notifications/initialized", Some("session-B"), "2025-06-18"),
                    ("tools/list", Some("session-B"), "2025-06-18"),
                    ("tools/call", Some("session-B"), "2025-06-18"),
                ]
            );
            assert_eq!(requests[3].body["params"], requests[7].body["params"]);
            assert_eq!(requests[3].cached_title.as_deref(), Some("A"));
            assert_eq!(requests[7].cached_title.as_deref(), Some("B"));
        }
        let cache = fixture.cached_tools();
        assert_eq!(cache[0]["inputSchema"]["title"], "B");
        assert_eq!(cache[0]["description"], "Echo B");
        assert_eq!(cache[1]["toolName"], "only_B");
        assert_eq!(cache.as_array().expect("cached tool array").len(), 2);
        let after = fixture.slot().await;
        assert_eq!(
            (after.start_token, after.status.as_str()),
            (original.start_token, "running")
        );
        assert!(matches!(events.try_recv(), Ok(Evt::ServerTools { .. })));
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }
}

#[tokio::test]
async fn failed_tools_list_after_http_reconnect_preserves_cache_and_does_not_retry() {
    let fixture = Fixture::new(Arc::new(StdioHttpConnector));
    let state = Arc::new(WireState {
        db: fixture.db.clone(),
        server_id: fixture.id.clone(),
        initializes: AtomicUsize::new(0),
        requests: StdMutex::default(),
        expiry_status: StatusCode::BAD_REQUEST,
        fail_second_list: true,
    });
    let server = WireServer::start(state.clone()).await;
    fixture.start(&server.url).await;
    let cache = fixture.cached_tools();
    let before = fixture.slot().await;
    let mut events = fixture.manager.event_bus.subscribe();
    let err = fixture
        .manager
        .call_tool(&fixture.profile_id, "recovery__echo", json!({}))
        .await
        .expect_err("new tools/list fails");
    assert!(err.contains("tool list unavailable"));
    assert_eq!(fixture.cached_tools(), cache);
    assert_eq!(state.initializes.load(Ordering::SeqCst), 2);
    assert_eq!(
        state
            .requests
            .lock()
            .expect("wire trace")
            .iter()
            .filter(|request| request.method == "tools/call")
            .count(),
        1
    );
    let after = fixture.slot().await;
    assert!(Arc::ptr_eq(
        after.session.as_ref().expect("session"),
        before.session.as_ref().expect("original session")
    ));
    assert!(matches!(
        events.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}

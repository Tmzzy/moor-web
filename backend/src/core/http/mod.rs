// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

pub mod app_error;
mod auth;
pub mod routes;

use crate::core::db::Database;
use crate::core::services::event_bus::EventBus;
use crate::core::services::server_manager::ServerManager;
use axum::{http::StatusCode, middleware, response::IntoResponse, Json, Router};
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

pub struct ManagementAuth {
    username: String,
    password: String,
}

impl ManagementAuth {
    pub fn basic(username: String, password: String) -> Self {
        Self {
            username,
            password,
        }
    }
}

pub struct McpAuth {
    token: String,
}

impl McpAuth {
    pub fn bearer(token: String) -> Self {
        Self { token }
    }
}

pub struct AppState {
    pub db: Arc<Database>,
    pub management_auth: ManagementAuth,
    pub mcp_auth: McpAuth,
    pub version: String,
    pub port: u16,
    pub public_url: String,
    pub static_dir: PathBuf,
    pub event_bus: Arc<EventBus>,
    pub server_manager: Arc<ServerManager>,
}

impl AppState {
    /// 生产构造函数:组装 axum 共享状态的全部协作者。
    pub fn new(
        db: Arc<Database>,
        management_auth: ManagementAuth,
        mcp_auth: McpAuth,
        version: String,
        port: u16,
        public_url: String,
        static_dir: PathBuf,
        event_bus: Arc<EventBus>,
        server_manager: Arc<ServerManager>,
    ) -> Self {
        Self {
            db,
            management_auth,
            mcp_auth,
            version,
            port,
            public_url,
            static_dir,
            event_bus,
            server_manager,
        }
    }

    /// 测试构造函数:在一个临时目录里 open+migrate 一个全新 Database,
    /// 初始化默认 settings,装上默认 EventBus + 真实 ServerManager,
    /// 返回可直接喂给路由的 AppState。
    #[cfg(test)]
    pub fn for_test(data_dir: &std::path::Path) -> Arc<Self> {
        use crate::core::db::Database;
        use crate::core::services::settings;
        std::fs::create_dir_all(data_dir).expect("failed to create temp data dir");
        let db = Arc::new(Database::open(&data_dir.join("moor.db")).expect("failed to open db"));
        db.run_migrations().expect("failed to run migrations");
        settings::init_settings(db.as_ref(), data_dir).expect("failed to init settings");
        let event_bus = Arc::new(EventBus::new(16));
        Arc::new(Self::new(
            db.clone(),
            ManagementAuth::basic("test".to_string(), "test-password".to_string()),
            McpAuth::bearer("test-mcp-token".to_string()),
            "test".to_string(),
            19323,
            "http://localhost:19323".to_string(),
            data_dir.join("dist"),
            event_bus.clone(),
            Arc::new(ServerManager::new(db, event_bus)),
        ))
    }
}

pub fn create_app(state: Arc<AppState>) -> Router {
    let static_files = ServeDir::new(state.static_dir.clone())
        .fallback(ServeFile::new(state.static_dir.join("index.html")));
    let mcp_routes = Router::new().route(
        "/mcp",
        axum::routing::any(crate::core::mcp::transport::streamable_http_server::handle_mcp_request),
    );

    Router::new()
        .merge(routes::health::router())
        .merge(routes::servers::router())
        .merge(routes::profiles::router())
        .merge(routes::logs::router())
        .merge(routes::settings::router())
        .merge(routes::events::router())
        .merge(routes::import_routes::router())
        .merge(mcp_routes)
        .fallback_service(static_files)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ))
        .with_state(state)
}

pub fn json_error_response(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> axum::response::Response {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message.into() } })),
    )
        .into_response()
}

pub async fn start_server(state: Arc<AppState>, host: &str, port: u16) -> Result<(), String> {
    let addr = format!("{host}:{port}");
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind {addr}: {e}"))?;
    let app = create_app(state);
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{header, Request},
    };
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use tower::ServiceExt;

    fn basic_authorization() -> String {
        format!("Basic {}", STANDARD.encode("test:test-password"))
    }

    #[tokio::test]
    async fn spa_deep_route_returns_index_with_ok_status() {
        let data_dir = std::env::temp_dir().join(format!("moor-spa-{}", uuid::Uuid::new_v4()));
        let static_dir = data_dir.join("dist");
        std::fs::create_dir_all(&static_dir).expect("failed to create static directory");
        std::fs::write(static_dir.join("index.html"), "<html>Moor SPA</html>")
            .expect("failed to write test index");

        let app = create_app(AppState::for_test(&data_dir));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/settings")
                    .header(header::AUTHORIZATION, basic_authorization())
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("SPA request failed");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("failed to read SPA response");
        assert_eq!(body.as_ref(), b"<html>Moor SPA</html>");

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn enforces_route_specific_authentication() {
        let data_dir = std::env::temp_dir().join(format!("moor-auth-{}", uuid::Uuid::new_v4()));
        let static_dir = data_dir.join("dist");
        std::fs::create_dir_all(&static_dir).expect("failed to create static directory");
        std::fs::write(static_dir.join("index.html"), "<html>Moor SPA</html>")
            .expect("failed to write test index");
        let app = create_app(AppState::for_test(&data_dir));

        let health = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .expect("failed to build health request"),
            )
            .await
            .expect("health request failed");
        assert_eq!(health.status(), StatusCode::OK);

        let management_without_auth = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("failed to build management request"),
            )
            .await
            .expect("management request failed");
        assert_eq!(management_without_auth.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            management_without_auth
                .headers()
                .get(header::WWW_AUTHENTICATE)
                .and_then(|value| value.to_str().ok()),
            Some("Basic realm=\"Moor\", charset=\"UTF-8\"")
        );

        let management_with_auth = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/runtime")
                    .header(header::AUTHORIZATION, basic_authorization())
                    .body(Body::empty())
                    .expect("failed to build authenticated management request"),
            )
            .await
            .expect("authenticated management request failed");
        assert_eq!(management_with_auth.status(), StatusCode::OK);
        let management_body = axum::body::to_bytes(management_with_auth.into_body(), usize::MAX)
            .await
            .expect("failed to read runtime response");
        assert!(!management_body
            .windows(b"test-password".len())
            .any(|window| window == b"test-password"));
        assert!(!management_body
            .windows(b"test-mcp-token".len())
            .any(|window| window == b"test-mcp-token"));

        let mcp_without_auth = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .body(Body::from("{}"))
                    .expect("failed to build MCP request"),
            )
            .await
            .expect("MCP request failed");
        assert_eq!(mcp_without_auth.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            mcp_without_auth
                .headers()
                .get(header::WWW_AUTHENTICATE)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer realm=\"Moor MCP\"")
        );

        let mcp_with_basic_auth = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header(header::AUTHORIZATION, basic_authorization())
                    .body(Body::from("{}"))
                    .expect("failed to build MCP request with Basic auth"),
            )
            .await
            .expect("MCP request with Basic auth failed");
        assert_eq!(mcp_with_basic_auth.status(), StatusCode::UNAUTHORIZED);

        let mcp_with_bearer_auth = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header(header::AUTHORIZATION, "Bearer test-mcp-token")
                    .body(Body::from("{}"))
                    .expect("failed to build authenticated MCP request"),
            )
            .await
            .expect("authenticated MCP request failed");
        assert_eq!(mcp_with_bearer_auth.status(), StatusCode::ACCEPTED);

        let _ = std::fs::remove_dir_all(data_dir);
    }
}

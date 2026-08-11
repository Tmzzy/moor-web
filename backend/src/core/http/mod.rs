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
use time::Duration;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::services::{ServeDir, ServeFile};
use tower_sessions::{cookie::SameSite, Expiry, MemoryStore, SessionManagerLayer};

pub struct ManagementAuth {
    username: String,
    password: String,
}

impl ManagementAuth {
    pub fn basic(username: String, password: String) -> Self {
        Self { username, password }
    }

    pub fn matches(&self, username: &str, password: &str) -> bool {
        self.matches_bytes(username.as_bytes(), password.as_bytes())
    }

    pub fn matches_bytes(&self, username: &[u8], password: &[u8]) -> bool {
        let username_matches = auth::constant_time_eq(username, self.username.as_bytes());
        let password_matches = auth::constant_time_eq(password, self.password.as_bytes());
        username_matches & password_matches
    }

    pub fn matches_username(&self, username: &str) -> bool {
        auth::constant_time_eq(username.as_bytes(), self.username.as_bytes())
    }
}

pub struct McpAuth {
    token: RwLock<String>,
}

impl McpAuth {
    pub fn bearer(token: String) -> Self {
        Self {
            token: RwLock::new(token),
        }
    }

    pub async fn matches(&self, token: &str) -> bool {
        auth::constant_time_eq(token.as_bytes(), self.token.read().await.as_bytes())
    }

    pub async fn reveal(&self) -> String {
        self.token.read().await.clone()
    }

    pub async fn replace(&self, token: String) {
        *self.token.write().await = token;
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
    #[expect(
        clippy::too_many_arguments,
        reason = "the application composition root passes each dependency explicitly"
    )]
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
    let session_layer = SessionManagerLayer::new(MemoryStore::default())
        .with_name("moor_session")
        .with_http_only(true)
        .with_same_site(SameSite::Strict)
        .with_secure(state.public_url.starts_with("https://"))
        .with_expiry(Expiry::OnInactivity(Duration::days(7)));

    let public_routes = Router::new()
        .merge(routes::health::public_router())
        .merge(auth::router());
    let management_routes = Router::new()
        .merge(routes::health::router())
        .merge(routes::servers::router())
        .merge(routes::profiles::router())
        .merge(routes::logs::router())
        .merge(routes::settings::router())
        .merge(routes::events::router())
        .merge(routes::import_routes::router())
        .merge(routes::security::router())
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::management_auth_middleware,
        ));
    let mcp_routes = Router::new()
        .route(
            "/mcp",
            axum::routing::any(
                crate::core::mcp::transport::streamable_http_server::handle_mcp_request,
            ),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::mcp_auth_middleware,
        ));

    Router::new()
        .merge(public_routes)
        .merge(management_routes)
        .merge(mcp_routes)
        .fallback_service(static_files)
        .layer(middleware::from_fn(auth::origin_middleware))
        .layer(session_layer)
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
                    .body(Body::empty())
                    .expect("failed to build request"),
            )
            .await
            .expect("SPA request failed");

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get(header::WWW_AUTHENTICATE).is_none());
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
                    .uri("/api/runtime")
                    .body(Body::empty())
                    .expect("failed to build management request"),
            )
            .await
            .expect("management request failed");
        assert_eq!(management_without_auth.status(), StatusCode::UNAUTHORIZED);
        assert!(management_without_auth
            .headers()
            .get(header::WWW_AUTHENTICATE)
            .is_none());

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

    #[tokio::test]
    async fn login_session_grants_access_and_logout_revokes_it() {
        let data_dir = std::env::temp_dir().join(format!("moor-login-{}", uuid::Uuid::new_v4()));
        let static_dir = data_dir.join("dist");
        std::fs::create_dir_all(&static_dir).expect("failed to create static directory");
        std::fs::write(static_dir.join("index.html"), "<html>Moor SPA</html>")
            .expect("failed to write test index");
        let app = create_app(AppState::for_test(&data_dir));

        let login = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"test","password":"test-password"}"#,
                    ))
                    .expect("failed to build login request"),
            )
            .await
            .expect("login request failed");
        assert_eq!(login.status(), StatusCode::OK);
        assert_eq!(
            login
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store"),
        );
        let set_cookie = login
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .expect("login should set session cookie")
            .to_string();
        assert!(set_cookie.contains("HttpOnly"));
        assert!(set_cookie.contains("SameSite=Strict"));
        assert!(set_cookie.contains("Max-Age=604800"));
        let cookie = set_cookie
            .split(';')
            .next()
            .expect("session cookie should contain a value")
            .to_string();

        let authenticated = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/runtime")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .expect("failed to build authenticated request"),
            )
            .await
            .expect("authenticated request failed");
        assert_eq!(authenticated.status(), StatusCode::OK);

        let logout = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/logout")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .expect("failed to build logout request"),
            )
            .await
            .expect("logout request failed");
        assert_eq!(logout.status(), StatusCode::NO_CONTENT);

        let after_logout = app
            .oneshot(
                Request::builder()
                    .uri("/api/runtime")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .expect("failed to build post-logout request"),
            )
            .await
            .expect("post-logout request failed");
        assert_eq!(after_logout.status(), StatusCode::UNAUTHORIZED);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn invalid_login_returns_json_without_browser_challenge() {
        let data_dir = std::env::temp_dir().join(format!("moor-login-{}", uuid::Uuid::new_v4()));
        let app = create_app(AppState::for_test(&data_dir));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"username":"test","password":"wrong"}"#))
                    .expect("failed to build invalid login request"),
            )
            .await
            .expect("invalid login request failed");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().get(header::WWW_AUTHENTICATE).is_none());
        assert!(response.headers().get(header::SET_COOKIE).is_none());

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn cross_origin_login_is_rejected() {
        let data_dir = std::env::temp_dir().join(format!("moor-origin-{}", uuid::Uuid::new_v4()));
        let app = create_app(AppState::for_test(&data_dir));

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header(header::HOST, "moor.example")
                    .header(header::ORIGIN, "https://evil.example")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"test","password":"test-password"}"#,
                    ))
                    .expect("failed to build cross-origin login request"),
            )
            .await
            .expect("cross-origin login request failed");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn token_endpoint_rotates_the_mcp_bearer_credential() {
        let data_dir = std::env::temp_dir().join(format!("moor-token-{}", uuid::Uuid::new_v4()));
        let app = create_app(AppState::for_test(&data_dir));

        let current = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/security/mcp-token")
                    .header(header::AUTHORIZATION, basic_authorization())
                    .body(Body::empty())
                    .expect("failed to build token request"),
            )
            .await
            .expect("token request failed");
        assert_eq!(current.status(), StatusCode::OK);
        assert_eq!(
            current
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store"),
        );
        let current_body = axum::body::to_bytes(current.into_body(), usize::MAX)
            .await
            .expect("failed to read token response");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&current_body)
                .expect("token response should be JSON")["token"],
            "test-mcp-token",
        );

        let rotated = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/security/mcp-token/rotate")
                    .header(header::AUTHORIZATION, basic_authorization())
                    .body(Body::empty())
                    .expect("failed to build token rotation request"),
            )
            .await
            .expect("token rotation request failed");
        assert_eq!(rotated.status(), StatusCode::OK);
        let rotated_body = axum::body::to_bytes(rotated.into_body(), usize::MAX)
            .await
            .expect("failed to read token rotation response");
        let rotated_token = serde_json::from_slice::<serde_json::Value>(&rotated_body)
            .expect("rotation response should be JSON")["token"]
            .as_str()
            .expect("rotation response should include token")
            .to_string();
        assert_ne!(rotated_token, "test-mcp-token");

        let old_token = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header(header::AUTHORIZATION, "Bearer test-mcp-token")
                    .body(Body::from("{}"))
                    .expect("failed to build old token request"),
            )
            .await
            .expect("old token request failed");
        assert_eq!(old_token.status(), StatusCode::UNAUTHORIZED);

        let new_token = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header(header::AUTHORIZATION, format!("Bearer {rotated_token}"))
                    .body(Body::from("{}"))
                    .expect("failed to build new token request"),
            )
            .await
            .expect("new token request failed");
        assert_eq!(new_token.status(), StatusCode::ACCEPTED);

        let _ = std::fs::remove_dir_all(data_dir);
    }
}

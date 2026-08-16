// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use crate::core::http::app_error::AppError;
use crate::core::http::AppState;
use crate::core::services::profile_service::ProfileService;
use axum::{
    extract::{Path, State},
    http::{header, HeaderValue},
    response::IntoResponse,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/profiles", get(list).post(create))
        .route(
            "/api/profiles/{id}",
            get(get_one).put(update).delete(remove),
        )
        .route("/api/profiles/{id}/mcp-token", get(get_mcp_token))
        .route(
            "/api/profiles/{id}/mcp-token/rotate",
            post(rotate_mcp_token),
        )
        .route(
            "/api/profiles/{profileId}/servers/{serverId}",
            get(get_profile_server).put(upsert_profile_server),
        )
}

#[derive(Serialize)]
struct ProfileMcpTokenResponse {
    token: String,
}

fn no_store_header() -> [(header::HeaderName, HeaderValue); 1] {
    [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))]
}

async fn get_mcp_token(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = ProfileService::get_mcp_token(&state.db, &id).map_err(AppError::from)?;
    Ok((no_store_header(), Json(ProfileMcpTokenResponse { token })))
}

async fn rotate_mcp_token(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let token = ProfileService::rotate_mcp_token(&state.db, &id).map_err(AppError::from)?;
    Ok((no_store_header(), Json(ProfileMcpTokenResponse { token })))
}

async fn list(State(state): State<Arc<AppState>>) -> Result<Json<Value>, AppError> {
    let profiles = ProfileService::list(&state.db).map_err(AppError::from)?;
    Ok(Json(
        serde_json::to_value(profiles).map_err(|e| AppError::internal(e.to_string()))?,
    ))
}

#[derive(Deserialize)]
struct CreateBody {
    name: String,
}

async fn create(
    State(state): State<Arc<AppState>>,
    axum::Json(body): axum::Json<CreateBody>,
) -> Result<(axum::http::StatusCode, Json<Value>), AppError> {
    if body.name.is_empty() {
        return Err(AppError::validation("name is required"));
    }
    let profile = ProfileService::create(&state.db, &body.name).map_err(AppError::from)?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::to_value(profile).map_err(|e| AppError::internal(e.to_string()))?),
    ))
}

async fn get_one(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let (profile, servers) = ProfileService::get_detail(&state.db, &id).map_err(AppError::from)?;
    let mut profile_value =
        serde_json::to_value(profile).map_err(|e| AppError::internal(e.to_string()))?;
    if let Some(obj) = profile_value.as_object_mut() {
        obj.insert(
            "servers".to_string(),
            serde_json::to_value(servers).map_err(|e| AppError::internal(e.to_string()))?,
        );
    }
    Ok(Json(profile_value))
}

#[derive(Deserialize)]
struct UpdateBody {
    name: Option<String>,
}

async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<UpdateBody>,
) -> Result<Json<Value>, AppError> {
    let profile =
        ProfileService::update(&state.db, &id, body.name.as_deref()).map_err(AppError::from)?;
    Ok(Json(
        serde_json::to_value(profile).map_err(|e| AppError::internal(e.to_string()))?,
    ))
}

async fn remove(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    ProfileService::remove(&state.db, &id).map_err(AppError::from)?;
    Ok(Json(json!({ "success": true })))
}

async fn get_profile_server(
    State(state): State<Arc<AppState>>,
    Path((profile_id, server_id)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    let server = ProfileService::get_profile_server(&state.db, &profile_id, &server_id)
        .map_err(AppError::from)?;
    Ok(Json(
        serde_json::to_value(server).map_err(|e| AppError::internal(e.to_string()))?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpsertProfileServerBody {
    enabled: Option<bool>,
    disabled_tools: Option<Vec<String>>,
}

async fn upsert_profile_server(
    State(state): State<Arc<AppState>>,
    Path((profile_id, server_id)): Path<(String, String)>,
    axum::Json(body): axum::Json<UpsertProfileServerBody>,
) -> Result<Json<Value>, AppError> {
    let result = ProfileService::upsert_profile_server(
        &state.db,
        &profile_id,
        &server_id,
        body.enabled,
        body.disabled_tools.as_ref(),
    )
    .map_err(AppError::from)?;
    Ok(Json(
        serde_json::to_value(result).map_err(|e| AppError::internal(e.to_string()))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::profile_repo::ProfileRepository;
    use axum::body::Body;
    use std::sync::Arc;
    use std::time::SystemTime;
    use tower::ServiceExt;

    fn temp_data_dir(test_name: &str) -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time is before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("moor-profiles-route-{test_name}-{timestamp}"))
    }

    fn test_state(data_dir: std::path::PathBuf) -> Arc<AppState> {
        AppState::for_test(&data_dir)
    }

    #[tokio::test]
    async fn profile_token_endpoint_reveals_and_rotates_only_that_profile_token() {
        let data_dir = temp_data_dir("profile-token");
        let state = test_state(data_dir.clone());
        let profile = ProfileRepository::new(&state.db)
            .create("Work")
            .expect("failed to create profile");
        let app = router().with_state(state.clone());

        let current = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri(format!("/api/profiles/{}/mcp-token", profile.id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("token route should respond");
        assert_eq!(current.status(), axum::http::StatusCode::OK);
        assert_eq!(
            current.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("no-store"))
        );
        let current_body = axum::body::to_bytes(current.into_body(), usize::MAX)
            .await
            .expect("read token response");
        let current_token = serde_json::from_slice::<serde_json::Value>(&current_body)
            .expect("token response should be JSON")["token"]
            .as_str()
            .expect("token should be present")
            .to_string();

        let rotated = app
            .oneshot(
                axum::http::Request::builder()
                    .method(axum::http::Method::POST)
                    .uri(format!("/api/profiles/{}/mcp-token/rotate", profile.id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("rotation route should respond");
        let rotated_body = axum::body::to_bytes(rotated.into_body(), usize::MAX)
            .await
            .expect("read rotated token response");
        let rotated_token = serde_json::from_slice::<serde_json::Value>(&rotated_body)
            .expect("rotation response should be JSON")["token"]
            .as_str()
            .expect("rotated token should be present")
            .to_string();

        assert_ne!(current_token, rotated_token);
        assert_eq!(
            ProfileRepository::new(&state.db)
                .find_mcp_token(&profile.id)
                .expect("query stored token")
                .as_deref(),
            Some(rotated_token.as_str())
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

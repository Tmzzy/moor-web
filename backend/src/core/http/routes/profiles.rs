// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use crate::core::http::app_error::AppError;
use crate::core::http::AppState;
use crate::core::services::profile_service::ProfileService;
use axum::{
    extract::{Path, State},
    response::Json,
    routing::{get, put},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/profiles", get(list).post(create))
        .route(
            "/api/profiles/{id}",
            get(get_one).put(update).delete(remove),
        )
        .route("/api/profiles/{id}/activate", put(activate))
        .route(
            "/api/profiles/{profileId}/servers/{serverId}",
            get(get_profile_server).put(upsert_profile_server),
        )
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

async fn activate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let profile =
        ProfileService::activate(&state.db, &state.event_bus, &id).map_err(AppError::from)?;
    Ok(Json(
        serde_json::to_value(profile).map_err(|e| AppError::internal(e.to_string()))?,
    ))
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
    async fn activate_profile_accepts_frontend_put_route() {
        let data_dir = temp_data_dir("activate-put");
        let state = test_state(data_dir.clone());
        let repo = ProfileRepository::new(&state.db);
        repo.seed_default().expect("failed to seed profile");
        let profile = repo.create("Work").expect("failed to create profile");

        let response = router()
            .with_state(state)
            .oneshot(
                axum::http::Request::builder()
                    .method(axum::http::Method::PUT)
                    .uri(format!("/api/profiles/{}/activate", profile.id))
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("route should respond");

        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

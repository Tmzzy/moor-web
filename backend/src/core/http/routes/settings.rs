// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use crate::core::http::app_error::AppError;
use crate::core::http::AppState;
use crate::core::services::event_bus::Evt;
use crate::core::services::settings as settings_store;
use axum::{
    extract::State,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::Value;
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/settings", get(get_settings).patch(update_settings))
        .route("/api/settings/reset", post(reset_settings))
}

async fn get_settings(State(state): State<Arc<AppState>>) -> Result<Json<Value>, AppError> {
    let settings = settings_store::get_settings(&state.db).map_err(AppError::internal)?;
    Ok(Json(
        serde_json::to_value(settings).map_err(|e| AppError::internal(e.to_string()))?,
    ))
}

async fn update_settings(
    State(state): State<Arc<AppState>>,
    axum::Json(body): axum::Json<Value>,
) -> Result<Json<Value>, AppError> {
    let settings = settings_store::update_settings(&state.db, body).map_err(|e| match e {
        settings_store::SettingsError::Validation(m) => AppError::validation(m),
        settings_store::SettingsError::Internal(m) => AppError::internal(m),
    })?;
    let settings_value =
        serde_json::to_value(&settings).map_err(|e| AppError::internal(e.to_string()))?;

    state.event_bus.emit(Evt::SettingsChanged {
        settings: settings_value.clone(),
    });
    Ok(Json(settings_value))
}

async fn reset_settings(State(state): State<Arc<AppState>>) -> Result<Json<Value>, AppError> {
    let defaults = settings_store::reset_settings(&state.db).map_err(AppError::internal)?;
    let defaults_value =
        serde_json::to_value(&defaults).map_err(|e| AppError::internal(e.to_string()))?;
    state.event_bus.emit(Evt::SettingsChanged {
        settings: defaults_value.clone(),
    });
    Ok(Json(defaults_value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::SystemTime;

    fn temp_data_dir(test_name: &str) -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time is before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("moor-settings-route-{test_name}-{timestamp}"))
    }

    fn test_state(data_dir: std::path::PathBuf) -> Arc<AppState> {
        AppState::for_test(&data_dir)
    }

    #[tokio::test]
    async fn get_settings_returns_defaults_for_fresh_store() {
        let data_dir = temp_data_dir("fresh");
        let Json(value) = get_settings(State(test_state(data_dir.clone())))
            .await
            .expect("settings should load");
        assert_eq!(value["advanced"]["mcpRequestTimeoutMs"], 30_000);
        assert_eq!(value["general"]["autoStartServersOnLaunch"], false);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn reset_settings_returns_defaults_without_writing_json() {
        let data_dir = temp_data_dir("reset");
        let state = test_state(data_dir.clone());
        std::fs::write(
            settings_store::settings_path(&data_dir),
            r#"{"advanced":{"mcpRequestTimeoutMs":45000}}"#,
        )
        .expect("failed to write stale json");
        let _ = update_settings(
            State(state.clone()),
            axum::Json(serde_json::json!({ "advanced": { "mcpRequestTimeoutMs": 60000 } })),
        )
        .await
        .expect("settings update should succeed");

        let Json(value) = reset_settings(State(state.clone()))
            .await
            .expect("settings reset should succeed");
        assert_eq!(value["advanced"]["mcpRequestTimeoutMs"], 30_000);
        assert_eq!(
            settings_store::get_settings(&state.db)
                .expect("settings should load")
                .advanced
                .mcp_request_timeout_ms,
            30_000
        );
        assert_eq!(
            settings_store::read_settings_file(&data_dir)
                .advanced
                .mcp_request_timeout_ms,
            45_000
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn update_settings_rejects_invalid_timeout_with_validation_error() {
        let data_dir = temp_data_dir("invalid-timeout");
        let err = update_settings(
            State(test_state(data_dir.clone())),
            axum::Json(serde_json::json!({ "advanced": { "mcpRequestTimeoutMs": 1000 } })),
        )
        .await
        .expect_err("invalid port should be rejected");
        assert_eq!(err.status_code(), axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(err.code(), "VALIDATION_ERROR");
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn settings_changed_event_payload_is_settings_object() {
        let data_dir = temp_data_dir("event-payload");
        let state = test_state(data_dir.clone());
        let mut events = state.event_bus.subscribe();

        let Json(value) = update_settings(
            State(state),
            axum::Json(serde_json::json!({
                "general": { "autoStartServersOnLaunch": true }
            })),
        )
        .await
        .expect("settings update should succeed");

        let evt = events
            .recv()
            .await
            .expect("settings event should be emitted");
        assert_eq!(evt.name(), "settings:changed");
        let payload = evt.payload();
        assert_eq!(payload, value);
        assert_eq!(payload["general"]["autoStartServersOnLaunch"], true);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

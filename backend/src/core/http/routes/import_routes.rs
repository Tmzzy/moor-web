// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use crate::core::config::clients;
use crate::core::config::converter::ConvertInput;
use crate::core::config::import_parser::ScannedServer;
use crate::core::config::scanner::{self, ImportPreview};
use crate::core::config::{converter, import_parser, snippets};
use crate::core::http::app_error::AppError;
use crate::core::http::AppState;
use crate::core::services::import_service;
use crate::core::services::server_service::ServerService;
use axum::{
    extract::{DefaultBodyLimit, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

const MAX_IMPORT_BODY_BYTES: usize = 512 * 1024;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/import/scan", post(scan))
        .route("/api/import/parse", post(parse))
        .route(
            "/api/import/execute",
            post(execute).layer(DefaultBodyLimit::max(MAX_IMPORT_BODY_BYTES)),
        )
        .route("/api/import/snippets", get(snippets_handler))
        .route("/api/import/convert", post(convert))
}

async fn scan(State(state): State<Arc<AppState>>) -> Result<Json<Value>, AppError> {
    let parsed = scanner::scan_all_configs();
    let existing_names = ServerService::find_all_names(&state.db);
    let preview = scanner::build_import_preview(&parsed, &existing_names);
    Ok(Json(
        serde_json::to_value(preview).map_err(|e| AppError::internal(e.to_string()))?,
    ))
}

#[derive(Deserialize)]
struct ParseBody {
    content: Option<String>,
}

async fn parse(axum::Json(body): axum::Json<ParseBody>) -> Result<Json<Value>, AppError> {
    let content = body.content.unwrap_or_default();
    if content.trim().is_empty() {
        return Err(AppError::validation("content is required"));
    }
    if content.len() > MAX_IMPORT_BODY_BYTES {
        return Err(AppError::payload_too_large(
            "content exceeds maximum allowed size",
        ));
    }

    let parsed = import_parser::parse_json_mcp_config(&content, "json-import");
    let preview = ImportPreview {
        scanned: parsed.servers.len() + parsed.unsupported.len(),
        new_servers: parsed.servers.len(),
        servers: parsed.servers,
        duplicates: vec![],
        unsupported: parsed.unsupported,
        errors: parsed.errors,
        diagnostics: parsed.diagnostics,
    };
    Ok(Json(
        serde_json::to_value(preview).map_err(|e| AppError::internal(e.to_string()))?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteBody {
    servers: Option<Vec<ScannedServer>>,
    profile_ids: Vec<String>,
}

async fn execute(
    State(state): State<Arc<AppState>>,
    axum::Json(body): axum::Json<ExecuteBody>,
) -> Result<Json<Value>, AppError> {
    if body.profile_ids.is_empty() {
        return Err(AppError::validation("at least one profileId is required"));
    }
    let result = import_service::execute_import(
        &state.db,
        &state.server_manager,
        body.servers,
        &body.profile_ids,
    )
    .await
    .map_err(AppError::from)?;

    Ok(Json(
        json!({ "imported": result.imported, "skipped": result.skipped }),
    ))
}

async fn snippets_handler(State(state): State<Arc<AppState>>) -> Result<Json<Value>, AppError> {
    let base_url = format!("{}/mcp", state.public_url);
    let result = snippets::generate_snippets(&base_url);
    Ok(Json(
        serde_json::to_value(result).map_err(|e| AppError::internal(e.to_string()))?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvertBody {
    source: Option<String>,
    target_client: Option<String>,
    content: Option<String>,
    source_client: Option<String>,
    server_ids: Option<Vec<String>>,
}

async fn convert(
    State(state): State<Arc<AppState>>,
    axum::Json(body): axum::Json<ConvertBody>,
) -> Result<Json<Value>, AppError> {
    let target_client = body.target_client.unwrap_or_default();
    let source = body.source.unwrap_or_else(|| "scan".to_string());

    if let Some(ref content) = body.content {
        if content.len() > MAX_IMPORT_BODY_BYTES {
            return Err(AppError::payload_too_large(
                "content exceeds maximum allowed size",
            ));
        }
    }

    if clients::get_client_by_id(&target_client).is_none() {
        return Err(AppError::validation(format!(
            "unknown client id: {target_client}"
        )));
    }

    let input = ConvertInput {
        source,
        source_client: body.source_client,
        content: body.content,
        server_ids: body.server_ids,
        target_client,
    };

    match converter::convert_config(&input, &state.db) {
        Ok(result) => Ok(Json(
            serde_json::to_value(result).map_err(|e| AppError::internal(e.to_string()))?,
        )),
        Err(e) => Err(AppError::internal(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::profile_repo::ProfileRepository;
    use crate::core::db::Database;
    use std::sync::Arc;
    use std::time::SystemTime;

    fn temp_data_dir(test_name: &str) -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time is before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("moor-import-{test_name}-{timestamp}"))
    }

    fn test_state(data_dir: std::path::PathBuf) -> Arc<AppState> {
        AppState::for_test(&data_dir)
    }

    fn fail_profile_server_inserts(db: &Database) {
        db.exec(
            "CREATE TRIGGER fail_profile_server_insert
             BEFORE INSERT ON profile_servers
             BEGIN
               SELECT RAISE(ABORT, 'profile insert failed');
             END;",
        )
        .expect("failed to create failing profile trigger");
    }

    #[tokio::test]
    async fn execute_rolls_back_import_when_profile_assignment_fails() {
        let data_dir = temp_data_dir("execute-profile-failure");
        let state = test_state(data_dir.clone());
        let profile_id = ProfileRepository::new(&state.db)
            .create("Test")
            .expect("failed to create profile")
            .id;
        fail_profile_server_inserts(&state.db);

        let result = execute(
            State(state.clone()),
            axum::Json(ExecuteBody {
                servers: Some(vec![ScannedServer {
                    name: "Imported".to_string(),
                    connection_type: "stdio".to_string(),
                    command: Some("node".to_string()),
                    args: None,
                    url: None,
                    env: None,
                    headers: None,
                    working_dir: None,
                    source: "test".to_string(),
                }]),
                profile_ids: vec![profile_id],
            }),
        )
        .await;

        assert!(result.is_err());
        let ids = state
            .db
            .query_all("SELECT id FROM mcp_servers", &[], |row| {
                row.get::<_, String>(0)
            })
            .expect("failed to query servers");
        for id in &ids {
            assert!(state.server_manager.get_server(id).await.is_none());
        }
        assert!(ids.is_empty());

        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn execute_rejects_an_empty_profile_scope() {
        let data_dir = temp_data_dir("execute-profile-required");
        let state = test_state(data_dir.clone());

        let error = execute(
            State(state),
            axum::Json(ExecuteBody {
                servers: Some(vec![]),
                profile_ids: vec![],
            }),
        )
        .await
        .expect_err("empty profile scope should fail");

        assert_eq!(error.status_code(), axum::http::StatusCode::BAD_REQUEST);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn snippets_use_configured_public_url() {
        let data_dir = temp_data_dir("snippets-port");
        let state = test_state(data_dir.clone());
        let expected_url = format!("{}/mcp", state.public_url);

        let Json(value) = snippets_handler(State(state))
            .await
            .expect("snippets should succeed");
        let snippets = value.as_array().expect("snippets should be an array");
        assert!(
            snippets.iter().any(|snippet| snippet["snippet"]
                .as_str()
                .unwrap_or("")
                .contains(&expected_url)),
            "snippets should contain the configured public URL {expected_url}"
        );
        assert!(snippets.iter().all(|snippet| snippet["snippet"]
            .as_str()
            .unwrap_or("")
            .contains("MOOR_PROFILE_TOKEN")));
        assert!(!value.to_string().contains("test-mcp-token"));
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[test]
    fn convert_body_accepts_frontend_camel_case_fields() {
        let body: ConvertBody = serde_json::from_value(serde_json::json!({
            "source": "paste",
            "targetClient": "claude-code",
            "sourceClient": "cursor",
            "serverIds": ["server-a"],
            "content": "{}"
        }))
        .expect("frontend convert payload should deserialize");

        assert_eq!(body.target_client.as_deref(), Some("claude-code"));
        assert_eq!(body.source_client.as_deref(), Some("cursor"));
        assert_eq!(
            body.server_ids.as_deref(),
            Some(&["server-a".to_string()][..])
        );
    }
}

// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::clients;
use super::formatters;
use super::import_parser::{self, ScannedServer};
use super::scanner;
use crate::core::db::server_repo::ServerRepository;
use crate::core::db::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertResult {
    pub content: String,
    pub warnings: Vec<String>,
    pub target_path: String,
    pub target_client: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertInput {
    pub source: String, // "moor" | "scan" | "paste"
    pub source_client: Option<String>,
    pub content: Option<String>,
    pub server_ids: Option<Vec<String>>,
    pub target_client: String,
}

pub fn convert_config(input: &ConvertInput, db: &Database) -> Result<ConvertResult, String> {
    let target = clients::get_client_by_id(&input.target_client)
        .ok_or_else(|| format!("Unknown target client: {}", input.target_client))?;

    let (servers, source_warnings) = resolve_source_servers(input, db)?;
    if servers.is_empty() {
        if !source_warnings.is_empty() {
            return Err(source_warnings.join("; "));
        }
        return Err("No servers found to convert".to_string());
    }

    let formatter = formatters::format_for_client(target.id)
        .ok_or_else(|| format!("No formatter for client: {}", target.id))?;

    let result = formatter(&servers, target);
    let target_path = clients::resolve_config_paths(target)[0]
        .display()
        .to_string();

    Ok(ConvertResult {
        content: result.content,
        warnings: [&source_warnings[..], &result.warnings[..]].concat(),
        target_path,
        target_client: target.id.to_string(),
    })
}

fn resolve_source_servers(
    input: &ConvertInput,
    db: &Database,
) -> Result<(Vec<ScannedServer>, Vec<String>), String> {
    match input.source.as_str() {
        "moor" => resolve_from_moor(input.server_ids.as_deref().unwrap_or_default(), db),
        "scan" => resolve_from_scan(input.source_client.as_deref()),
        "paste" => resolve_from_paste(
            input.content.as_deref().unwrap_or_default(),
            input.source_client.as_deref(),
        ),
        _ => Ok((vec![], vec![])),
    }
}

fn resolve_from_moor(
    server_ids: &[String],
    db: &Database,
) -> Result<(Vec<ScannedServer>, Vec<String>), String> {
    if server_ids.is_empty() {
        return Ok((vec![], vec![]));
    }
    let repo = ServerRepository::new(db);
    let rows = repo.find_by_ids(server_ids)?;
    let servers: Vec<ScannedServer> = rows
        .iter()
        .map(|row| {
            let mut base = ScannedServer {
                name: row.name.clone(),
                connection_type: row.connection_type.clone(),
                source: "moor".to_string(),
                command: None,
                args: None,
                url: None,
                env: None,
                headers: None,
                working_dir: None,
            };
            if row.connection_type == "stdio" {
                base.command = row.command.clone();
                base.args = row.args.as_ref().and_then(|v| v.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                });
            } else {
                base.url = row.url.clone();
                base.headers = row
                    .headers
                    .as_ref()
                    .and_then(|v| serde_json::from_value(v.clone()).ok());
            }
            base.env = row
                .env
                .as_ref()
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            base.working_dir = row.working_dir.clone();
            base
        })
        .collect();
    Ok((servers, vec![]))
}

fn resolve_from_scan(
    source_client: Option<&str>,
) -> Result<(Vec<ScannedServer>, Vec<String>), String> {
    let client_id = match source_client {
        Some(id) => id,
        None => return Ok((vec![], vec![])),
    };
    let parsed = scanner::scan_client_config(client_id);
    let warnings = parsed_import_warnings(&parsed);
    Ok((parsed.servers, warnings))
}

fn resolve_from_paste(
    content: &str,
    source_client: Option<&str>,
) -> Result<(Vec<ScannedServer>, Vec<String>), String> {
    if content.trim().is_empty() {
        return Ok((vec![], vec![]));
    }
    let client = source_client.and_then(clients::get_client_by_id);
    let source = source_client.unwrap_or("paste");
    let parsed = match client {
        Some(c) if c.format == "toml" => import_parser::parse_codex_toml_config(content, source),
        _ => import_parser::parse_json_mcp_config(content, source),
    };
    let warnings = parsed_import_warnings(&parsed);
    Ok((parsed.servers, warnings))
}

fn parsed_import_warnings(parsed: &import_parser::ParsedImport) -> Vec<String> {
    let mut warnings: Vec<String> = parsed
        .errors
        .iter()
        .map(|e| format!("Parse error: {e}"))
        .collect();
    warnings.extend(
        parsed
            .unsupported
            .iter()
            .map(|s| format!("Skipped unsupported server \"{}\": {}", s.name, s.reason)),
    );
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::server_repo::ServerRepository;
    use std::time::SystemTime;

    fn temp_db() -> Database {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("moor-converter-{ts}.db"));
        let db = Database::open(&path).expect("open db");
        db.run_migrations().expect("migrate");
        db
    }

    #[test]
    fn converts_selected_moor_servers_to_cursor() {
        let db = temp_db();
        let repo = ServerRepository::new(&db);
        repo.insert_one_with_id(
            "first",
            0,
            &crate::core::db::server_repo::ServerInsertInput {
                name: "first".into(),
                connection_type: "stdio".into(),
                command: Some("node".into()),
                args: Some(vec!["first.js".into()]),
                url: None,
                env: Some(
                    [("FIRST".to_string(), "1".to_string())]
                        .into_iter()
                        .collect(),
                ),
                headers: None,
                working_dir: Some("/tmp/first".into()),
                auto_start: false,
                profile_ids: vec![],
            },
        )
        .expect("insert first");
        repo.insert_one_with_id(
            "second",
            1,
            &crate::core::db::server_repo::ServerInsertInput {
                name: "second".into(),
                connection_type: "http".into(),
                command: None,
                args: None,
                url: Some("https://mcp.example.com/mcp".into()),
                env: None,
                headers: Some(
                    [("Authorization".to_string(), "Bearer ${TOKEN}".to_string())]
                        .into_iter()
                        .collect(),
                ),
                working_dir: None,
                auto_start: false,
                profile_ids: vec![],
            },
        )
        .expect("insert second");

        let input = ConvertInput {
            source: "moor".to_string(),
            source_client: None,
            content: None,
            server_ids: Some(vec!["second".to_string(), "first".to_string()]),
            target_client: "cursor".to_string(),
        };
        let result = convert_config(&input, &db).expect("convert");

        assert_eq!(result.target_client, "cursor");
        assert!(result.content.contains("first"));
        assert!(result.content.contains("second"));
        assert!(result.content.contains("https://mcp.example.com/mcp"));
    }
}

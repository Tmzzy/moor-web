// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::clients::{self, ClientMeta};
use super::import_parser::{self, ParsedImport};
use std::fs;

use super::import_parser::ScannedServer;

fn read_file_if_exists(path: &std::path::Path) -> Option<String> {
    if !path.exists() {
        return None;
    }
    fs::read_to_string(path).ok()
}

fn parse_config_file(config_path: &std::path::Path, source: &str, format: &str) -> ParsedImport {
    let content = match read_file_if_exists(config_path) {
        Some(c) => c,
        None => return ParsedImport::default(),
    };
    let parsed = match format {
        "toml" => import_parser::parse_codex_toml_config(&content, source),
        _ => import_parser::parse_json_mcp_config(&content, source),
    };
    ignore_missing_mcp_sections(parsed, source)
}

fn ignore_missing_mcp_sections(result: ParsedImport, source: &str) -> ParsedImport {
    let missing_errors: std::collections::HashSet<String> = [
        format!("{source}: no mcpServers or mcp key found"),
        format!("{source}: no mcp_servers key found"),
    ]
    .into_iter()
    .collect();

    let only_missing = result.servers.is_empty()
        && result.unsupported.is_empty()
        && !result.errors.is_empty()
        && result.errors.iter().all(|e| missing_errors.contains(e));

    if only_missing {
        ParsedImport::default()
    } else {
        result
    }
}

fn scan_client(client: &ClientMeta) -> ParsedImport {
    let paths = clients::resolve_config_paths(client);
    let results: Vec<ParsedImport> = paths
        .iter()
        .map(|p| parse_config_file(p, client.id, client.format))
        .collect();

    let mut merged = merge_results(&results);

    if paths.len() > 1 {
        let mut seen = std::collections::HashSet::new();
        merged.servers.retain(|s| seen.insert(s.name.clone()));
    }

    merged
}

pub fn scan_all_configs() -> ParsedImport {
    let results: Vec<ParsedImport> = clients::ALL_CLIENTS.iter().map(scan_client).collect();
    merge_results(&results)
}

pub fn scan_client_config(client_id: &str) -> ParsedImport {
    match clients::get_client_by_id(client_id) {
        Some(client) => scan_client(client),
        None => ParsedImport::default(),
    }
}

fn merge_results(results: &[ParsedImport]) -> ParsedImport {
    let mut merged = ParsedImport::default();
    for r in results {
        merged.servers.extend(r.servers.clone());
        merged.unsupported.extend(r.unsupported.clone());
        merged.errors.extend(r.errors.clone());
        merged.diagnostics.extend(r.diagnostics.clone());
    }
    merged
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub scanned: usize,
    pub new_servers: usize,
    pub servers: Vec<ScannedServer>,
    pub duplicates: Vec<ScannedServer>,
    pub unsupported: Vec<super::import_parser::UnsupportedServer>,
    pub errors: Vec<String>,
    pub diagnostics: Vec<super::import_parser::ImportDiagnostic>,
}

pub fn build_import_preview(
    parsed: &ParsedImport,
    existing_names: &std::collections::HashSet<String>,
) -> ImportPreview {
    let mut seen = existing_names.clone();
    let mut servers = vec![];
    let mut duplicates = vec![];

    for server in &parsed.servers {
        if seen.contains(&server.name) {
            duplicates.push(server.clone());
        } else {
            seen.insert(server.name.clone());
            servers.push(server.clone());
        }
    }

    ImportPreview {
        scanned: parsed.servers.len() + parsed.unsupported.len(),
        new_servers: servers.len(),
        servers,
        duplicates,
        unsupported: parsed.unsupported.clone(),
        errors: parsed.errors.clone(),
        diagnostics: parsed.diagnostics.clone(),
    }
}

pub fn partition_import_candidates(
    servers: &[ScannedServer],
    existing_names: &std::collections::HashSet<String>,
) -> (Vec<ScannedServer>, Vec<String>) {
    let mut seen = existing_names.clone();
    let mut candidates = vec![];
    let mut skipped = vec![];

    for server in servers {
        if seen.contains(&server.name) {
            skipped.push(server.name.clone());
        } else {
            seen.insert(server.name.clone());
            candidates.push(server.clone());
        }
    }

    (candidates, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scanned_server(name: &str) -> ScannedServer {
        ScannedServer {
            name: name.to_string(),
            connection_type: "stdio".to_string(),
            source: "test".to_string(),
            command: Some("node".to_string()),
            args: Some(vec!["server.js".to_string()]),
            url: None,
            env: None,
            headers: None,
            working_dir: None,
        }
    }

    #[test]
    fn partitions_duplicate_import_names() {
        let existing_names = std::collections::HashSet::from(["existing".to_string()]);
        let (candidates, skipped) = partition_import_candidates(
            &[scanned_server("existing"), scanned_server("new")],
            &existing_names,
        );
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "new");
        assert_eq!(skipped, vec!["existing"]);
    }
}

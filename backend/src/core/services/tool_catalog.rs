// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use crate::core::db::tool_discovery_repo::{ProfileTool, ToolDiscoveryRepository};
use crate::core::db::Database;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ToolCatalogEntry {
    pub server_id: String,
    pub server_name: String,
    pub tool_name: String,
    pub exposed_name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDetail {
    pub tool_name: String,
    pub exposed_name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
    pub disabled: bool,
}

pub struct ToolCatalogService;

impl ToolCatalogService {
    pub fn get_tool_catalog(
        db: &Database,
        profile_id: &str,
        visible_server_ids: Option<&std::collections::HashSet<String>>,
    ) -> Vec<ToolCatalogEntry> {
        let tool_repo = ToolDiscoveryRepository::new(db);
        let profile_tools = match tool_repo.find_by_profile_id(profile_id) {
            Ok(t) => t,
            Err(_) => return vec![],
        };

        let visible = match visible_server_ids {
            Some(ids) => profile_tools
                .into_iter()
                .filter(|t| ids.contains(&t.server_id))
                .collect(),
            None => profile_tools,
        };

        build_tool_catalog_entries(visible)
    }

    pub fn get_tool_details(
        db: &Database,
        server_id: &str,
        profile_id: &str,
        visible_server_ids: Option<&std::collections::HashSet<String>>,
    ) -> Vec<ToolDetail> {
        let tool_repo = ToolDiscoveryRepository::new(db);
        let profile_tools = match tool_repo.find_by_profile_id(profile_id) {
            Ok(t) => t,
            Err(_) => return vec![],
        };

        let server_name = crate::core::db::server_repo::ServerRepository::new(db)
            .find_by_id(server_id)
            .ok()
            .flatten()
            .map(|server| server.name)
            .unwrap_or_else(|| "server".to_string());

        let visible_tools: Vec<_> = match visible_server_ids {
            Some(ids) => profile_tools
                .into_iter()
                .filter(|t| ids.contains(&t.server_id))
                .collect(),
            None => profile_tools,
        };
        let catalog = build_tool_catalog_entries(visible_tools);

        let disabled = tool_repo
            .find_disabled_tools_for_server(profile_id, server_id)
            .unwrap_or_default();
        let discovered = match tool_repo.find_by_server_id(server_id) {
            Ok(tools) => tools,
            Err(_) => return vec![],
        };

        discovered
            .into_iter()
            .map(|tool| {
                let server_slug = normalize_server_name(&server_name);
                let is_disabled = disabled.contains(&tool.tool_name);
                let exposed_name = catalog
                    .iter()
                    .find(|entry| entry.server_id == server_id && entry.tool_name == tool.tool_name)
                    .map(|entry| entry.exposed_name.clone())
                    .unwrap_or_else(|| format!("{server_slug}__{}", tool.tool_name));
                ToolDetail {
                    exposed_name,
                    tool_name: tool.tool_name,
                    description: tool.description,
                    input_schema: tool.input_schema,
                    disabled: is_disabled,
                }
            })
            .collect()
    }
}

fn build_tool_catalog_entries(profile_tools: Vec<ProfileTool>) -> Vec<ToolCatalogEntry> {
    let enabled: Vec<_> = profile_tools
        .into_iter()
        .filter(|tool| !tool.disabled_tools.contains(&tool.tool_name))
        .collect();
    let mut server_ids_by_base_name = std::collections::HashMap::<String, Vec<String>>::new();
    for tool in &enabled {
        let server_slug = normalize_server_name(&tool.server_name);
        let base_name = format!("{server_slug}__{}", tool.tool_name);
        server_ids_by_base_name
            .entry(base_name)
            .or_default()
            .push(tool.server_id.clone());
    }

    enabled
        .into_iter()
        .map(|tool| {
            let server_slug = normalize_server_name(&tool.server_name);
            let base_name = format!("{server_slug}__{}", tool.tool_name);
            let server_ids = server_ids_by_base_name
                .get(&base_name)
                .expect("base name should exist");
            let exposed_name = if server_ids.len() > 1 {
                format!(
                    "{server_slug}_{}__{}",
                    shortest_unique_server_id_prefix(&tool.server_id, server_ids),
                    tool.tool_name
                )
            } else {
                base_name
            };
            ToolCatalogEntry {
                server_id: tool.server_id,
                server_name: tool.server_name,
                tool_name: tool.tool_name,
                exposed_name,
                description: tool.description,
                input_schema: tool.input_schema,
            }
        })
        .collect()
}

fn shortest_unique_server_id_prefix(server_id: &str, server_ids: &[String]) -> String {
    for length in std::cmp::min(8, server_id.len())..=server_id.len() {
        let prefix = &server_id[..length];
        if server_ids
            .iter()
            .all(|candidate| candidate == server_id || !candidate.starts_with(prefix))
        {
            return prefix.to_string();
        }
    }
    server_id.to_string()
}

fn normalize_server_name(name: &str) -> String {
    let slug: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let slug = slug.trim_matches('_');
    if slug.is_empty() {
        "server".to_string()
    } else {
        slug.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::profile_repo::ProfileRepository;
    use crate::core::db::server_repo::ServerRepository;
    use crate::core::db::tool_discovery_repo::{ToolDiscoveryRepository, ToolInsert};
    use std::time::SystemTime;

    fn temp_db_path(test_name: &str) -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time is before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("moor-tool-catalog-{test_name}-{timestamp}.db"))
    }

    fn insert_server(db: &Database, id: &str, name: &str) {
        use crate::core::db::server_repo::ServerInsertInput;
        ServerRepository::new(db)
            .insert_one_with_id(
                id,
                0,
                &ServerInsertInput {
                    name: name.into(),
                    connection_type: "stdio".into(),
                    command: Some("node".into()),
                    args: None,
                    url: None,
                    env: None,
                    headers: None,
                    working_dir: None,
                    auto_start: false,
                    profile_ids: vec![],
                },
            )
            .expect("failed to insert server");
    }

    #[test]
    fn adds_short_unique_server_id_prefix_when_server_slugs_collide() {
        let db_path = temp_db_path("slug-collision");
        let db = Database::open(&db_path).expect("failed to open db");
        db.run_migrations().expect("failed to migrate");
        let profile_repo = ProfileRepository::new(&db);
        let profile_id = profile_repo
            .create("Test")
            .expect("failed to create profile")
            .id;

        insert_server(&db, "aaaaaaaa1111", "GitHub MCP");
        insert_server(&db, "aaaaaaaa2222", "github-mcp");
        insert_server(&db, "aaaaaaaa3333", "github mcp");
        profile_repo
            .assign_to_profile(
                &profile_id,
                &[
                    "aaaaaaaa1111".to_string(),
                    "aaaaaaaa2222".to_string(),
                    "aaaaaaaa3333".to_string(),
                ],
            )
            .expect("failed to assign profile servers");

        let tool_repo = ToolDiscoveryRepository::new(&db);
        let search_tool = [ToolInsert {
            name: "search".to_string(),
            description: None,
            input_schema: None,
        }];
        tool_repo
            .replace_tools_for_server("aaaaaaaa1111", &search_tool)
            .expect("failed to insert tools for first server");
        tool_repo
            .replace_tools_for_server("aaaaaaaa2222", &search_tool)
            .expect("failed to insert tools for second server");
        tool_repo
            .replace_tools_for_server("aaaaaaaa3333", &search_tool)
            .expect("failed to insert tools for disabled server");
        profile_repo
            .upsert_profile_server(
                &profile_id,
                "aaaaaaaa3333",
                Some(true),
                Some(&vec!["search".to_string()]),
            )
            .expect("failed to disable tool");

        let exposed_names: Vec<_> = ToolCatalogService::get_tool_catalog(&db, &profile_id, None)
            .into_iter()
            .map(|tool| tool.exposed_name)
            .collect();

        assert_eq!(
            exposed_names,
            vec![
                "github_mcp_aaaaaaaa1__search".to_string(),
                "github_mcp_aaaaaaaa2__search".to_string(),
            ]
        );

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn keeps_server_slug_for_disabled_server_tool_details() {
        let db_path = temp_db_path("disabled-server-details");
        let db = Database::open(&db_path).expect("failed to open db");
        db.run_migrations().expect("failed to migrate");
        let profile_repo = ProfileRepository::new(&db);
        let profile_id = profile_repo
            .create("Test")
            .expect("failed to create profile")
            .id;

        insert_server(&db, "server-a", "Alpha");
        profile_repo
            .assign_to_profile(&profile_id, &["server-a".to_string()])
            .expect("failed to assign profile server");
        profile_repo
            .upsert_profile_server(&profile_id, "server-a", Some(false), None)
            .expect("failed to disable server");
        ToolDiscoveryRepository::new(&db)
            .replace_tools_for_server(
                "server-a",
                &[ToolInsert {
                    name: "search".to_string(),
                    description: None,
                    input_schema: None,
                }],
            )
            .expect("failed to insert tool");

        let exposed_names: Vec<_> =
            ToolCatalogService::get_tool_details(&db, "server-a", &profile_id, None)
                .into_iter()
                .map(|tool| tool.exposed_name)
                .collect();

        assert_eq!(exposed_names, vec!["alpha__search".to_string()]);

        let _ = std::fs::remove_file(db_path);
    }
}

// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::clients;
use super::formatters;
use super::import_parser::ScannedServer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientSnippet {
    pub client: String,
    pub description: String,
    pub snippet: String,
    pub cli_command: String,
}

pub fn generate_snippets(mcp_url: &str) -> Vec<ClientSnippet> {
    let moor_server = ScannedServer {
        name: "moor".to_string(),
        connection_type: "http".to_string(),
        url: Some(mcp_url.to_string()),
        source: "moor".to_string(),
        command: None,
        args: None,
        env: None,
        headers: Some(HashMap::from([(
            "Authorization".to_string(),
            "Bearer {env:MOOR_PROFILE_TOKEN}".to_string(),
        )])),
        working_dir: None,
    };

    clients::ALL_CLIENTS
        .iter()
        .map(|client| {
            let formatter = formatters::format_for_client(client.id)
                .unwrap_or(formatters::format_for_claude_code);
            let result = formatter(std::slice::from_ref(&moor_server), client);
            let config_path = client
                .config_path_segments
                .first()
                .map(|segments| format!("~/{}", segments.join("/")))
                .unwrap_or_else(|| "~".to_string());
            ClientSnippet {
                client: client.name.to_string(),
                description: client.description.to_string(),
                snippet: result.content,
                cli_command: format!(
                    "# Edit {} and add the {}.moor entry above.",
                    config_path, client.top_level_key
                ),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_snippets_reference_the_profile_token_environment_variable() {
        let snippets = generate_snippets("https://moor.example/mcp");

        let expected_token_references = [
            ("Claude Code", "Bearer ${MOOR_PROFILE_TOKEN}"),
            ("Codex", "bearer_token_env_var = \"MOOR_PROFILE_TOKEN\""),
            ("OpenCode", "Bearer {env:MOOR_PROFILE_TOKEN}"),
            ("Cursor", "Bearer ${env:MOOR_PROFILE_TOKEN}"),
        ];
        assert_eq!(snippets.len(), expected_token_references.len());
        for (client, token_reference) in expected_token_references {
            let snippet = snippets
                .iter()
                .find(|snippet| snippet.client == client)
                .unwrap_or_else(|| panic!("missing snippet for {client}"));
            assert!(
                snippet.snippet.contains(token_reference),
                "{client} snippet did not contain {token_reference}: {}",
                snippet.snippet
            );
            assert!(snippet.snippet.contains("https://moor.example/mcp"));
            assert!(snippet.cli_command.starts_with("# Edit ~/"));
            assert!(!snippet.cli_command.contains("/nonexistent/"));
        }
    }
}

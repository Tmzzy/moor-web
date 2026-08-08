// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::clients::ClientMeta;
use super::import_parser::ScannedServer;
use serde_json::Value;

pub struct FormatResult {
    pub content: String,
    pub warnings: Vec<String>,
}

fn non_empty(obj: &Option<std::collections::HashMap<String, String>>) -> bool {
    obj.as_ref().map(|m| !m.is_empty()).unwrap_or(false)
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn toml_key(key: &str) -> String {
    if key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        key.to_string()
    } else {
        toml_string(key)
    }
}

fn toml_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|v| toml_string(v))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn toml_inline_table(values: &std::collections::HashMap<String, String>) -> String {
    format!(
        "{{ {} }}",
        values
            .iter()
            .map(|(k, v)| format!("{} = {}", toml_string(k), toml_string(v)))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn rewrite_headers(
    headers: &Option<std::collections::HashMap<String, String>>,
    client: &ClientMeta,
) -> Option<std::collections::HashMap<String, String>> {
    headers.as_ref().map(|h| {
        h.iter()
            .map(|(k, v)| (k.clone(), rewrite_header_value(v, client)))
            .collect()
    })
}

fn rewrite_header_value(value: &str, client: &ClientMeta) -> String {
    let patterns = [
        regex_lite::Regex::new(r"\$\{env:([A-Za-z_][A-Za-z0-9_]*)\}").ok(),
        regex_lite::Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-[^}]*)?\}").ok(),
        regex_lite::Regex::new(r"\{env:([A-Za-z_][A-Za-z0-9_]*)\}").ok(),
    ];

    for pattern in patterns.iter().flatten() {
        if let Some(caps) = pattern.captures(value) {
            let name = &caps[1];
            let env_ref = env_ref_for_client(name, client);
            return value.replacen(&caps[0], &env_ref, 1);
        }
    }
    value.to_string()
}

fn env_ref_for_client(name: &str, client: &ClientMeta) -> String {
    match client.id {
        "claude-code" => format!("${{{name}}}"),
        "cursor" => format!("${{env:{name}}}"),
        _ => format!("{{env:{name}}}"),
    }
}

fn stdio_entry(server: &ScannedServer, client: &ClientMeta) -> Value {
    let mut entry = serde_json::Map::new();

    if client.id == "opencode" {
        entry.insert("type".to_string(), Value::String("local".to_string()));
        let cmd = server.command.as_deref().unwrap_or("");
        let mut cmd_arr = vec![Value::String(cmd.to_string())];
        if let Some(args) = &server.args {
            cmd_arr.extend(args.iter().map(|a| Value::String(a.clone())));
        }
        entry.insert("command".to_string(), Value::Array(cmd_arr));
        if non_empty(&server.env) {
            entry.insert(
                "environment".to_string(),
                serde_json::to_value(&server.env).unwrap_or(Value::Null),
            );
        }
        return Value::Object(entry);
    }

    if client.id == "cursor" {
        entry.insert("type".to_string(), Value::String("stdio".to_string()));
    }
    entry.insert(
        "command".to_string(),
        Value::String(server.command.clone().unwrap_or_default()),
    );
    if let Some(args) = &server.args {
        if !args.is_empty() {
            entry.insert(
                "args".to_string(),
                serde_json::to_value(args).unwrap_or(Value::Null),
            );
        }
    }
    if non_empty(&server.env) {
        entry.insert(
            "env".to_string(),
            serde_json::to_value(&server.env).unwrap_or(Value::Null),
        );
    }
    Value::Object(entry)
}

fn http_entry(server: &ScannedServer, client: &ClientMeta) -> Value {
    let mut entry = serde_json::Map::new();

    if client.id == "opencode" {
        entry.insert("type".to_string(), Value::String("remote".to_string()));
        entry.insert(
            "url".to_string(),
            Value::String(server.url.clone().unwrap_or_default()),
        );
        if let Some(headers) = rewrite_headers(&server.headers, client) {
            entry.insert(
                "headers".to_string(),
                serde_json::to_value(&headers).unwrap_or(Value::Null),
            );
        }
        return Value::Object(entry);
    }

    entry.insert(
        "url".to_string(),
        Value::String(server.url.clone().unwrap_or_default()),
    );
    if let Some(headers) = rewrite_headers(&server.headers, client) {
        let key = if client.id == "codex" {
            "http_headers"
        } else {
            "headers"
        };
        entry.insert(
            key.to_string(),
            serde_json::to_value(&headers).unwrap_or(Value::Null),
        );
    }
    Value::Object(entry)
}

fn format_json_mcp_servers(
    servers: &[ScannedServer],
    client: &ClientMeta,
    top_level_key: &str,
    extra_warnings: Vec<String>,
) -> FormatResult {
    let mut mcp_servers = serde_json::Map::new();
    for s in servers {
        let entry = if s.connection_type == "stdio" {
            stdio_entry(s, client)
        } else {
            http_entry(s, client)
        };
        mcp_servers.insert(s.name.clone(), entry);
    }

    let mut warnings = build_warnings(servers, client);
    warnings.extend(extra_warnings);

    FormatResult {
        content: serde_json::to_string_pretty(&serde_json::json!({ top_level_key: mcp_servers }))
            .unwrap_or_default(),
        warnings,
    }
}

fn build_warnings(servers: &[ScannedServer], client: &ClientMeta) -> Vec<String> {
    let mut warnings = vec![];
    if servers.iter().any(|s| non_empty(&s.headers)) && client.id == "codex" {
        warnings.push("Headers have been mapped to Codex's http_headers/env_http_headers. Please verify manually.".to_string());
    }
    if servers.iter().any(|s| s.working_dir.is_some())
        && (client.id == "opencode" || client.id == "cursor")
    {
        warnings.push(format!(
            "{} does not natively support the workingDir field. It has been ignored.",
            client.name
        ));
    }
    if servers.iter().any(|s| non_empty(&s.env)) && client.id == "opencode" {
        warnings
            .push("Environment variables have been mapped to the environment field.".to_string());
    }
    warnings
}

pub fn format_for_claude_code(servers: &[ScannedServer], client: &ClientMeta) -> FormatResult {
    format_json_mcp_servers(servers, client, "mcpServers", vec![])
}

pub fn format_for_codex(servers: &[ScannedServer], client: &ClientMeta) -> FormatResult {
    let mut lines = vec![];
    for s in servers {
        lines.push(format!("[mcp_servers.{}]", toml_key(&s.name)));
        if s.connection_type == "stdio" {
            lines.push(format!(
                "command = {}",
                toml_string(s.command.as_deref().unwrap_or(""))
            ));
            if let Some(args) = &s.args {
                if !args.is_empty() {
                    lines.push(format!("args = {}", toml_array(args)));
                }
            }
            if non_empty(&s.env) {
                lines.push(format!(
                    "env = {}",
                    toml_inline_table(s.env.as_ref().unwrap())
                ));
            }
            if let Some(wd) = &s.working_dir {
                lines.push(format!("cwd = {}", toml_string(wd)));
            }
        } else {
            lines.push(format!(
                "url = {}",
                toml_string(s.url.as_deref().unwrap_or(""))
            ));
            if let Some(headers) = &s.headers {
                for (k, v) in headers {
                    if k.to_lowercase() == "authorization"
                        && v.starts_with("Bearer {env:")
                        && v.ends_with("}")
                    {
                        if let Some(var) = v
                            .strip_prefix("Bearer {env:")
                            .and_then(|s| s.strip_suffix('}'))
                        {
                            lines.push(format!("bearer_token_env_var = {}", toml_string(var)));
                            continue;
                        }
                    }
                    lines.push(format!("http_headers = {}", toml_inline_table(headers)));
                    break;
                }
            }
        }
        lines.push("enabled = true".to_string());
        lines.push(String::new());
    }
    FormatResult {
        content: lines.join("\n").trim_end().to_string(),
        warnings: build_warnings(servers, client),
    }
}

pub fn format_for_opencode(servers: &[ScannedServer], client: &ClientMeta) -> FormatResult {
    let mut result = format_json_mcp_servers(servers, client, "mcp", vec![]);
    let mut parsed: Value = serde_json::from_str(&result.content).unwrap_or_default();
    if let Some(mcp) = parsed.get_mut("mcp").and_then(|v| v.as_object_mut()) {
        for (_key, entry) in mcp.iter_mut() {
            if let Some(obj) = entry.as_object_mut() {
                obj.insert("enabled".to_string(), Value::Bool(true));
            }
        }
    }
    if let Some(obj) = parsed.as_object_mut() {
        obj.insert(
            "$schema".to_string(),
            Value::String("https://opencode.ai/config.json".to_string()),
        );
    }
    result.content = serde_json::to_string_pretty(&parsed).unwrap_or_default();
    result
}

pub fn format_for_cursor(servers: &[ScannedServer], client: &ClientMeta) -> FormatResult {
    let has_http = servers.iter().any(|s| s.connection_type == "http");
    let extra = if has_http {
        vec!["HTTP servers use streamable-http transport by default. For SSE, change the type field to \"sse\".".to_string()]
    } else {
        vec![]
    };
    format_json_mcp_servers(servers, client, "mcpServers", extra)
}

pub fn format_for_client(
    client_id: &str,
) -> Option<fn(&[ScannedServer], &ClientMeta) -> FormatResult> {
    match client_id {
        "claude-code" => Some(format_for_claude_code),
        "codex" => Some(format_for_codex),
        "opencode" => Some(format_for_opencode),
        "cursor" => Some(format_for_cursor),
        _ => None,
    }
}

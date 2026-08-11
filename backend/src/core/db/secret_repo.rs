// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::Database;

pub const MCP_TOKEN_KEY: &str = "mcp_token";

pub fn load(db: &Database, key: &str) -> Result<Option<String>, String> {
    db.query_one("SELECT value FROM secrets WHERE key = ?1", &[&key], |row| {
        row.get(0)
    })
}

pub fn save(db: &Database, key: &str, value: &str) -> Result<(), String> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    db.run(
        "INSERT INTO secrets (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        &[&key, &value, &updated_at],
    )
}

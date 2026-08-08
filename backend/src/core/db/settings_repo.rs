// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::Database;
use std::collections::HashMap;

pub fn load_entries(db: &Database) -> Result<HashMap<String, String>, String> {
    let rows = db.query_all("SELECT key, value FROM settings", &[], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    Ok(rows.into_iter().collect())
}

pub fn save_entries(db: &Database, entries: &[(&str, String)]) -> Result<(), String> {
    let now = chrono::Utc::now().to_rfc3339();
    db.transaction(|conn| {
        for (key, value) in entries {
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![key, value, &now],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
}

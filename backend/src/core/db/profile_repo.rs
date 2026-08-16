// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub server_count: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileServerState {
    pub enabled: bool,
    pub disabled_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDetailServer {
    #[serde(flatten)]
    pub server: super::server_repo::Server,
    pub profile_server: ProfileServerState,
}

fn map_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<Profile> {
    let server_count: Option<i64> = row.get("server_count")?;
    Ok(Profile {
        id: row.get("id")?,
        name: row.get("name")?,
        server_count,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub struct ProfileRepository<'a> {
    pub db: &'a Database,
}

impl<'a> ProfileRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn find_all(&self) -> Result<Vec<Profile>, String> {
        self.db.query_all(
            "SELECT p.*, COUNT(CASE WHEN ps.enabled = 1 THEN ps.server_id END) as server_count
             FROM profiles p
             LEFT JOIN profile_servers ps ON p.id = ps.profile_id
             GROUP BY p.id
             ORDER BY p.created_at DESC",
            &[],
            map_profile,
        )
    }

    pub fn find_by_id(&self, id: &str) -> Result<Option<Profile>, String> {
        self.db.query_one(
            "SELECT p.*, COUNT(CASE WHEN ps.enabled = 1 THEN ps.server_id END) as server_count
             FROM profiles p
             LEFT JOIN profile_servers ps ON p.id = ps.profile_id
             WHERE p.id = ?1
             GROUP BY p.id",
            &[&id],
            map_profile,
        )
    }

    pub fn find_id_by_mcp_token(&self, token: &str) -> Result<Option<String>, String> {
        self.db.query_one(
            "SELECT id FROM profiles WHERE mcp_token = ?1",
            &[&token],
            |row| row.get(0),
        )
    }

    pub fn find_mcp_token(&self, id: &str) -> Result<Option<String>, String> {
        self.db.query_one(
            "SELECT mcp_token FROM profiles WHERE id = ?1",
            &[&id],
            |row| row.get(0),
        )
    }

    pub fn rotate_mcp_token(&self, id: &str) -> Result<Option<String>, String> {
        let exists = self
            .db
            .query_one("SELECT id FROM profiles WHERE id = ?1", &[&id], |row| {
                row.get::<_, String>(0)
            })?;
        if exists.is_none() {
            return Ok(None);
        }

        let token = crate::core::services::mcp_token::generate()?;
        let now = chrono::Utc::now().to_rfc3339();
        self.db.run(
            "UPDATE profiles SET mcp_token = ?1, updated_at = ?2 WHERE id = ?3",
            &[&token, &now, &id],
        )?;
        Ok(Some(token))
    }

    pub fn create(&self, name: &str) -> Result<Profile, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let token = crate::core::services::mcp_token::generate()?;
        let now = chrono::Utc::now().to_rfc3339();
        self.db.run(
            "INSERT INTO profiles (id, name, mcp_token, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            &[&id, &name, &token, &now, &now],
        )?;
        self.find_by_id(&id)
            .and_then(|p| p.ok_or_else(|| "Created profile could not be reloaded".into()))
    }

    pub fn update(&self, id: &str, name: Option<&str>) -> Result<Option<Profile>, String> {
        let exists = self
            .db
            .query_one("SELECT id FROM profiles WHERE id = ?1", &[&id], |row| {
                row.get::<_, String>(0)
            })?;
        if exists.is_none() {
            return Ok(None);
        }
        if let Some(name) = name {
            let now = chrono::Utc::now().to_rfc3339();
            self.db.run(
                "UPDATE profiles SET name = ?1, updated_at = ?2 WHERE id = ?3",
                &[&name, &now, &id],
            )?;
        }
        self.find_by_id(id)
    }

    pub fn remove(&self, id: &str) -> Result<RemoveResult, String> {
        let existing =
            self.db
                .query_one("SELECT id FROM profiles WHERE id = ?1", &[&id], |row| {
                    row.get::<_, String>(0)
                })?;
        if existing.is_none() {
            return Ok(RemoveResult::NotFound);
        }

        self.db.transaction(|conn| {
            conn.execute(
                "UPDATE audit_logs SET profile_id = NULL WHERE profile_id = ?1",
                [id],
            )
            .map_err(|error| error.to_string())?;
            conn.execute("DELETE FROM profiles WHERE id = ?1", [id])
                .map_err(|error| error.to_string())?;
            Ok(())
        })?;
        Ok(RemoveResult::Success)
    }

    pub fn find_profile_servers(
        &self,
        profile_id: &str,
    ) -> Result<Vec<ProfileDetailServer>, String> {
        self.db.query_all(
            "SELECT ms.*, ps.enabled AS profile_enabled, ps.disabled_tools AS profile_disabled_tools
             FROM mcp_servers ms
             LEFT JOIN profile_servers ps ON ps.server_id = ms.id AND ps.profile_id = ?1
             ORDER BY ms.name ASC",
            &[&profile_id],
            |row| {
                let server = super::server_repo::map_server(row)?;
                let profile_enabled: Option<i64> = row.get("profile_enabled")?;
                let profile_disabled_tools: Option<String> = row.get("profile_disabled_tools")?;
                Ok(ProfileDetailServer {
                    server,
                    profile_server: ProfileServerState {
                        enabled: profile_enabled.is_some_and(|v| v != 0),
                        disabled_tools: profile_disabled_tools
                            .and_then(|s| serde_json::from_str(&s).ok())
                            .unwrap_or_default(),
                    },
                })
            },
        )
    }

    pub fn upsert_profile_server(
        &self,
        profile_id: &str,
        server_id: &str,
        enabled: Option<bool>,
        disabled_tools: Option<&Vec<String>>,
    ) -> Result<ProfileServerState, String> {
        let existing = self.db.query_one(
            "SELECT enabled, disabled_tools FROM profile_servers WHERE profile_id = ?1 AND server_id = ?2",
            &[&profile_id, &server_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )?;

        let (final_enabled, final_tools) = match (&existing, enabled, disabled_tools) {
            (Some(_), Some(en), Some(tools)) => (
                en as i64,
                serde_json::to_string(tools).unwrap_or_else(|_| "[]".into()),
            ),
            (Some((_, t)), Some(en), None) => (en as i64, t.clone()),
            (Some((e, _)), None, Some(tools)) => (
                *e,
                serde_json::to_string(tools).unwrap_or_else(|_| "[]".into()),
            ),
            (Some((e, t)), None, None) => (*e, t.clone()),
            (None, _, _) => (
                enabled.unwrap_or(true) as i64,
                disabled_tools.map_or_else(
                    || "[]".to_string(),
                    |t| serde_json::to_string(t).unwrap_or_else(|_| "[]".into()),
                ),
            ),
        };

        if existing.is_some() {
            self.db.run(
                "UPDATE profile_servers SET enabled = ?1, disabled_tools = ?2 WHERE profile_id = ?3 AND server_id = ?4",
                &[&final_enabled, &final_tools, &profile_id, &server_id],
            )?;
        } else {
            self.db.run(
                "INSERT INTO profile_servers (profile_id, server_id, enabled, disabled_tools) VALUES (?1, ?2, ?3, ?4)",
                &[&profile_id, &server_id, &final_enabled, &final_tools],
            )?;
        }

        Ok(ProfileServerState {
            enabled: final_enabled != 0,
            disabled_tools: serde_json::from_str(&final_tools).unwrap_or_default(),
        })
    }

    pub fn find_enabled_server_ids(&self) -> Result<Vec<String>, String> {
        self.db.query_all(
            "SELECT DISTINCT server_id FROM profile_servers WHERE enabled = 1",
            &[],
            |row| row.get(0),
        )
    }

    #[cfg(test)]
    pub fn assign_to_profile(&self, profile_id: &str, server_ids: &[String]) -> Result<(), String> {
        for server_id in server_ids {
            self.db.run(
                "INSERT OR IGNORE INTO profile_servers (profile_id, server_id, enabled, disabled_tools) VALUES (?1, ?2, 1, '[]')",
                &[&profile_id, &server_id],
            )?;
        }
        Ok(())
    }

    pub fn seed_initial(&self) -> Result<(), String> {
        let profile_count = self
            .db
            .query_one("SELECT COUNT(*) FROM profiles", &[], |row| {
                row.get::<_, i64>(0)
            })?
            .unwrap_or_default();
        if profile_count > 0 {
            return Ok(());
        }

        let id = uuid::Uuid::new_v4().to_string();
        let token = crate::core::services::mcp_token::generate()?;
        let now = chrono::Utc::now().to_rfc3339();
        self.db.run(
            "INSERT INTO profiles (id, name, mcp_token, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            &[&id, &"Main", &token, &now, &now],
        )
    }
}

pub enum RemoveResult {
    Success,
    NotFound,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::Database;
    use std::time::SystemTime;

    fn temp_db_path(test_name: &str) -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time is before unix epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("moor-profile-{test_name}-{timestamp}"))
            .join("moor.db")
    }

    #[test]
    fn seed_initial_preserves_existing_profiles() {
        let db_path = temp_db_path("preserve-existing");
        std::fs::create_dir_all(db_path.parent().unwrap()).expect("failed to create temp db dir");
        let db = Database::open(&db_path).expect("failed to open temp db");
        db.run_migrations().expect("failed to migrate temp db");
        let repo = ProfileRepository::new(&db);
        let first = repo.create("Work").expect("failed to create profile");

        repo.seed_initial().expect("failed to seed initial profile");

        let profiles = repo.find_all().expect("failed to list profiles");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, first.id);
        let _ = std::fs::remove_dir_all(db_path.parent().unwrap());
    }

    #[test]
    fn seed_initial_creates_main_profile_for_an_empty_database() {
        let db_path = temp_db_path("create-main");
        std::fs::create_dir_all(db_path.parent().unwrap()).expect("failed to create temp db dir");
        let db = Database::open(&db_path).expect("failed to open temp db");
        db.run_migrations().expect("failed to migrate temp db");
        let repo = ProfileRepository::new(&db);

        repo.seed_initial().expect("failed to seed initial profile");

        let profiles = repo.find_all().expect("failed to list profiles");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "Main");
        let _ = std::fs::remove_dir_all(db_path.parent().unwrap());
    }
}

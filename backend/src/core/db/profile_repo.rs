// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub is_active: bool,
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
    let is_active: i64 = row.get("is_active")?;
    let server_count: Option<i64> = row.get("server_count")?;
    Ok(Profile {
        id: row.get("id")?,
        name: row.get("name")?,
        is_active: is_active != 0,
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
            "SELECT p.*, COUNT(ps.server_id) as server_count
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
            "SELECT p.*, COUNT(ps.server_id) as server_count
             FROM profiles p
             LEFT JOIN profile_servers ps ON p.id = ps.profile_id
             WHERE p.id = ?1
             GROUP BY p.id",
            &[&id],
            map_profile,
        )
    }

    pub fn find_active_id(&self) -> Result<Option<String>, String> {
        self.db
            .query_one("SELECT id FROM profiles WHERE is_active = 1", &[], |row| {
                row.get(0)
            })
    }

    pub fn create(&self, name: &str) -> Result<Profile, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.db.run(
            "INSERT INTO profiles (id, name, is_active, created_at, updated_at) VALUES (?1, ?2, 0, ?3, ?4)",
            &[&id, &name, &now, &now],
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

    pub fn activate(&self, id: &str) -> Result<Option<Profile>, String> {
        let exists = self
            .db
            .query_one("SELECT id FROM profiles WHERE id = ?1", &[&id], |row| {
                row.get::<_, String>(0)
            })?;
        if exists.is_none() {
            return Ok(None);
        }
        self.db.transaction(|conn| {
            conn.execute("UPDATE profiles SET is_active = 0 WHERE id != ?1", [id])
                .map_err(|e| e.to_string())?;
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE profiles SET is_active = 1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![&now, &id],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })?;
        self.find_by_id(id)
    }

    pub fn remove(&self, id: &str) -> Result<RemoveResult, String> {
        let existing = self.db.query_one(
            "SELECT id, is_active FROM profiles WHERE id = ?1",
            &[&id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )?;
        match existing {
            None => Ok(RemoveResult::NotFound),
            Some((_, is_active)) if is_active != 0 => Ok(RemoveResult::Active),
            Some(_) => {
                self.db.run("DELETE FROM profiles WHERE id = ?1", &[&id])?;
                Ok(RemoveResult::Success)
            }
        }
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

    pub fn find_active_profile_server_ids(&self) -> Result<Vec<String>, String> {
        let active_id = self.find_active_id()?;
        let Some(active_id) = active_id else {
            return Ok(vec![]);
        };
        self.db.query_all(
            "SELECT server_id FROM profile_servers WHERE profile_id = ?1 AND enabled = 1",
            &[&active_id],
            |row| row.get(0),
        )
    }

    #[cfg(test)]
    pub fn assign_to_active_profile(&self, server_ids: &[String]) -> Result<(), String> {
        let active =
            self.db
                .query_one("SELECT id FROM profiles WHERE is_active = 1", &[], |row| {
                    row.get::<_, String>(0)
                })?;
        let Some(active_id) = active else {
            return Ok(());
        };
        for server_id in server_ids {
            self.db.run(
                "INSERT OR IGNORE INTO profile_servers (profile_id, server_id, enabled, disabled_tools) VALUES (?1, ?2, 1, '[]')",
                &[&active_id, &server_id],
            )?;
        }
        Ok(())
    }

    pub fn seed_default(&self) -> Result<(), String> {
        let existing_profiles =
            self.db
                .query_all("SELECT id, is_active FROM profiles", &[], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })?;
        if existing_profiles
            .iter()
            .any(|(_, is_active)| *is_active != 0)
        {
            return Ok(());
        }

        let rows = self.db.query_all(
            "SELECT id FROM profiles WHERE name = 'Default'",
            &[],
            |row| row.get::<_, String>(0),
        )?;
        let now = chrono::Utc::now().to_rfc3339();
        if rows.is_empty() {
            let id = uuid::Uuid::new_v4().to_string();
            self.db.run(
                "INSERT INTO profiles (id, name, is_active, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?4)",
                &[&id, &"Default", &now, &now],
            )?;
        } else {
            self.db.run(
                "UPDATE profiles SET is_active = 1, updated_at = ?1 WHERE id = ?2",
                &[&now, &rows[0]],
            )?;
        }
        Ok(())
    }
}

pub enum RemoveResult {
    Success,
    NotFound,
    Active,
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
    fn seed_default_preserves_existing_active_profile() {
        let db_path = temp_db_path("preserve-active");
        std::fs::create_dir_all(db_path.parent().unwrap()).expect("failed to create temp db dir");
        let db = Database::open(&db_path).expect("failed to open temp db");
        db.run_migrations().expect("failed to migrate temp db");
        let repo = ProfileRepository::new(&db);
        let first = repo.create("Work").expect("failed to create profile");
        repo.activate(&first.id)
            .expect("failed to activate profile");

        repo.seed_default().expect("failed to seed default");

        assert_eq!(repo.find_active_id().unwrap(), Some(first.id));
        let _ = std::fs::remove_dir_all(db_path.parent().unwrap());
    }
}

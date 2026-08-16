// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use crate::core::db::profile_repo::ProfileRepository;
use crate::core::db::server_repo::{Server, ServerInsertInput, ServerRepository};
use crate::core::db::Database;
use crate::core::services::server_manager::ServerManager;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct CreateServerInput {
    pub name: String,
    pub connection_type: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub headers: Option<HashMap<String, String>>,
    pub working_dir: Option<String>,
    pub auto_start: bool,
    pub profile_ids: Vec<String>,
}

impl CreateServerInput {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("name is required".into());
        }
        match self.connection_type.as_str() {
            "stdio" if self.command.as_ref().is_none_or(|c| c.is_empty()) => {
                return Err("command is required for stdio".into());
            }
            "http" if self.url.as_ref().is_none_or(|u| u.is_empty()) => {
                return Err("url is required for http".into());
            }
            "stdio" | "http" => {}
            _ => return Err("connectionType must be 'stdio' or 'http'".into()),
        }
        if self.profile_ids.is_empty() {
            return Err("at least one profileId is required".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum UpdateField<T> {
    #[default]
    Unset,
    Set(Option<T>),
}

impl<T> UpdateField<T> {
    fn is_set(&self) -> bool {
        matches!(self, Self::Set(_))
    }
}

impl<'de, T> Deserialize<'de> for UpdateField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Self::Set)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateServerInput {
    pub name: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: UpdateField<Vec<String>>,
    pub url: Option<String>,
    #[serde(default)]
    pub env: UpdateField<HashMap<String, String>>,
    #[serde(default)]
    pub headers: UpdateField<HashMap<String, String>>,
    #[serde(default)]
    pub working_dir: UpdateField<String>,
    pub auto_start: Option<bool>,
}

impl UpdateServerInput {
    fn validate_for(&self, connection_type: &str) -> Result<(), String> {
        if self.name.as_ref().is_some_and(|name| name.is_empty()) {
            return Err("name is required".into());
        }
        match connection_type {
            "stdio" => {
                if self
                    .command
                    .as_ref()
                    .is_some_and(|command| command.is_empty())
                {
                    return Err("command is required for stdio".into());
                }
                if self.url.is_some() || self.headers.is_set() {
                    return Err("http-only fields cannot update a stdio server".into());
                }
            }
            "http" => {
                if self.url.as_ref().is_some_and(|url| url.is_empty()) {
                    return Err("url is required for http".into());
                }
                if self.command.is_some() || self.args.is_set() || self.working_dir.is_set() {
                    return Err("stdio-only fields cannot update an http server".into());
                }
            }
            _ => return Err("connectionType must be 'stdio' or 'http'".into()),
        }
        Ok(())
    }
}

pub struct ServerService;

pub enum ServerServiceError {
    NotFound(String),
    Validation(String),
    InvalidOrder(String),
    Internal(String),
}

/// 单一映射点:ServerServiceError → AppError,避免路由层重复分类领域错误。
impl From<ServerServiceError> for crate::core::http::app_error::AppError {
    fn from(e: ServerServiceError) -> Self {
        match e {
            ServerServiceError::NotFound(m) => Self::not_found(m),
            ServerServiceError::Validation(m) => Self::validation(m),
            ServerServiceError::InvalidOrder(m) => Self::order_invalid(m),
            ServerServiceError::Internal(m) => Self::internal(m),
        }
    }
}

fn serialize_json<T: Serialize>(field: &str, value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| format!("serialize {field}: {e}"))
}

fn serialize_nullable_json<T: Serialize>(
    field: &str,
    value: &Option<T>,
) -> Result<Option<String>, String> {
    value.as_ref().map(|v| serialize_json(field, v)).transpose()
}

impl ServerService {
    pub async fn insert_server(
        db: &Database,
        server_manager: &Arc<ServerManager>,
        input: &CreateServerInput,
    ) -> Result<Server, ServerServiceError> {
        let servers = Self::insert_servers(db, server_manager, std::slice::from_ref(input)).await?;
        let server = servers.into_iter().next().ok_or_else(|| {
            ServerServiceError::Internal("Created server could not be reloaded".to_string())
        })?;

        Ok(server)
    }

    /// 写入数据库后会把每个成功创建的 server 注册到内存态 server_manager。
    pub async fn insert_servers(
        db: &Database,
        server_manager: &Arc<ServerManager>,
        inputs: &[CreateServerInput],
    ) -> Result<Vec<Server>, ServerServiceError> {
        let servers = Self::insert_servers_transaction(db, inputs)?;

        for server in &servers {
            server_manager.add_server(server).await;
        }

        Ok(servers)
    }

    fn insert_servers_transaction(
        db: &Database,
        inputs: &[CreateServerInput],
    ) -> Result<Vec<Server>, ServerServiceError> {
        let known_profile_ids: HashSet<String> = ProfileRepository::new(db)
            .find_all()
            .map_err(ServerServiceError::Internal)?
            .into_iter()
            .map(|profile| profile.id)
            .collect();
        // 校验集中在 service 层(repo 只管持久化);通过后把领域字段映射成
        // repo 的输入结构体,事务 SQL 全部由 ServerRepository 内部处理。
        let repo_inputs: Vec<ServerInsertInput> = inputs
            .iter()
            .map(|i| -> Result<ServerInsertInput, ServerServiceError> {
                i.validate().map_err(ServerServiceError::Validation)?;
                if let Some(missing) = i
                    .profile_ids
                    .iter()
                    .find(|profile_id| !known_profile_ids.contains(*profile_id))
                {
                    return Err(ServerServiceError::Validation(format!(
                        "Profile {missing} not found"
                    )));
                }
                Ok(ServerInsertInput {
                    name: i.name.clone(),
                    connection_type: i.connection_type.clone(),
                    command: i.command.clone(),
                    args: i.args.clone(),
                    url: i.url.clone(),
                    env: i.env.clone(),
                    headers: i.headers.clone(),
                    working_dir: i.working_dir.clone(),
                    auto_start: i.auto_start,
                    profile_ids: i.profile_ids.clone(),
                })
            })
            .collect::<Result<_, _>>()?;
        ServerRepository::new(db)
            .insert_batch_with_profiles(&repo_inputs)
            .map_err(ServerServiceError::Internal)
    }

    pub fn list_servers(db: &Database) -> Result<Vec<Server>, String> {
        ServerRepository::new(db).find_all()
    }

    pub fn get_server(db: &Database, id: &str) -> Result<Option<Server>, String> {
        ServerRepository::new(db).find_by_id(id)
    }

    pub fn get_server_profile_ids(
        db: &Database,
        id: &str,
    ) -> Result<Vec<String>, ServerServiceError> {
        ServerRepository::new(db)
            .find_by_id(id)
            .map_err(ServerServiceError::Internal)?
            .ok_or_else(|| ServerServiceError::NotFound("Server not found".into()))?;
        ServerRepository::new(db)
            .find_profile_ids(id)
            .map_err(ServerServiceError::Internal)
    }

    pub fn update_server_profile_ids(
        db: &Database,
        id: &str,
        profile_ids: &[String],
    ) -> Result<Vec<String>, ServerServiceError> {
        let known_profile_ids: HashSet<String> = ProfileRepository::new(db)
            .find_all()
            .map_err(ServerServiceError::Internal)?
            .into_iter()
            .map(|profile| profile.id)
            .collect();
        if let Some(missing) = profile_ids
            .iter()
            .find(|profile_id| !known_profile_ids.contains(*profile_id))
        {
            return Err(ServerServiceError::Validation(format!(
                "Profile {missing} not found"
            )));
        }

        ServerRepository::new(db)
            .find_by_id(id)
            .map_err(ServerServiceError::Internal)?
            .ok_or_else(|| ServerServiceError::NotFound("Server not found".into()))?;
        ServerRepository::new(db)
            .replace_profile_ids(id, profile_ids)
            .map_err(ServerServiceError::Internal)
    }

    pub async fn update_server(
        db: &Database,
        server_manager: &Arc<ServerManager>,
        id: &str,
        body: &UpdateServerInput,
    ) -> Result<Server, ServerServiceError> {
        let (new_name, new_auto_start) = {
            let repo = ServerRepository::new(db);
            let existing = repo
                .find_by_id(id)
                .map_err(ServerServiceError::Internal)?
                .ok_or_else(|| ServerServiceError::NotFound("Server not found".into()))?;
            body.validate_for(&existing.connection_type)
                .map_err(ServerServiceError::Validation)?;

            let mut set_clauses = Vec::new();
            let mut param_idx = 1u32;
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            let mut new_name: Option<String> = None;
            let mut new_auto_start: Option<bool> = None;

            if let Some(value) = &body.name {
                set_clauses.push(format!("name = ?{param_idx}"));
                params.push(Box::new(value.clone()));
                new_name = Some(value.clone());
                param_idx += 1;
            }
            if let Some(value) = &body.command {
                set_clauses.push(format!("command = ?{param_idx}"));
                params.push(Box::new(value.clone()));
                param_idx += 1;
            }
            if let UpdateField::Set(value) = &body.args {
                set_clauses.push(format!("args = ?{param_idx}"));
                let serialized =
                    serialize_nullable_json("args", value).map_err(ServerServiceError::Internal)?;
                params.push(Box::new(serialized));
                param_idx += 1;
            }
            if let Some(value) = &body.url {
                set_clauses.push(format!("url = ?{param_idx}"));
                params.push(Box::new(value.clone()));
                param_idx += 1;
            }
            if let UpdateField::Set(value) = &body.env {
                set_clauses.push(format!("env = ?{param_idx}"));
                let serialized =
                    serialize_nullable_json("env", value).map_err(ServerServiceError::Internal)?;
                params.push(Box::new(serialized));
                param_idx += 1;
            }
            if let UpdateField::Set(value) = &body.headers {
                set_clauses.push(format!("headers = ?{param_idx}"));
                let serialized = serialize_nullable_json("headers", value)
                    .map_err(ServerServiceError::Internal)?;
                params.push(Box::new(serialized));
                param_idx += 1;
            }
            if let UpdateField::Set(value) = &body.working_dir {
                set_clauses.push(format!("working_dir = ?{param_idx}"));
                params.push(Box::new(value.clone()));
                param_idx += 1;
            }
            if let Some(value) = body.auto_start {
                set_clauses.push(format!("auto_start = ?{param_idx}"));
                params.push(Box::new(value as i64));
                new_auto_start = Some(value);
                param_idx += 1;
            }

            let now = chrono::Utc::now().to_rfc3339();
            set_clauses.push(format!("updated_at = ?{param_idx}"));
            params.push(Box::new(now));

            if !set_clauses.is_empty() {
                let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                    params.iter().map(|p| p.as_ref()).collect();
                repo.update(id, &set_clauses.join(", "), &param_refs)
                    .map_err(ServerServiceError::Internal)?;
            }

            (new_name, new_auto_start)
        };

        server_manager
            .update_server_memory(id, new_name.as_deref(), new_auto_start)
            .await;

        ServerRepository::new(db)
            .find_by_id(id)
            .map_err(ServerServiceError::Internal)?
            .ok_or_else(|| {
                ServerServiceError::Internal("Updated server could not be reloaded".into())
            })
    }

    pub async fn delete_server(
        db: &Database,
        server_manager: &Arc<ServerManager>,
        id: &str,
    ) -> Result<(), ServerServiceError> {
        ServerRepository::new(db)
            .find_by_id(id)
            .map_err(ServerServiceError::Internal)?
            .ok_or_else(|| ServerServiceError::NotFound("Server not found".into()))?;
        if !server_manager.remove_server(id).await {
            return Err(ServerServiceError::Internal(
                "Failed to delete server".into(),
            ));
        }
        Ok(())
    }

    pub fn find_all_names(db: &Database) -> HashSet<String> {
        ServerRepository::new(db)
            .find_all_names()
            .map(|rows| rows.into_iter().map(|(_, name)| name).collect())
            .unwrap_or_default()
    }

    pub fn reorder(
        db: &Database,
        server_ids: &[String],
    ) -> Result<Vec<Server>, ServerServiceError> {
        let repo = ServerRepository::new(db);
        let existing_ids = repo.find_ids().map_err(ServerServiceError::Internal)?;
        let existing_set: HashSet<_> = existing_ids.iter().collect();
        let new_set: HashSet<_> = server_ids.iter().collect();
        if existing_set != new_set {
            return Err(ServerServiceError::InvalidOrder(
                "Server order must include every existing server exactly once.".into(),
            ));
        }
        repo.reorder(server_ids)
            .map_err(ServerServiceError::Internal)?;
        repo.find_all().map_err(ServerServiceError::Internal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_create_input(profile_ids: Vec<String>) -> CreateServerInput {
        CreateServerInput {
            name: "Test".to_string(),
            connection_type: "stdio".to_string(),
            command: Some("node".to_string()),
            args: None,
            url: None,
            env: None,
            headers: None,
            working_dir: None,
            auto_start: false,
            profile_ids,
        }
    }

    #[test]
    fn create_server_input_rejects_an_empty_profile_scope() {
        let error = valid_create_input(vec![])
            .validate()
            .expect_err("empty profile scope should fail");

        assert_eq!(error, "at least one profileId is required");
    }

    #[test]
    fn update_server_input_rejects_invalid_json_shapes() {
        assert!(
            serde_json::from_value::<UpdateServerInput>(serde_json::json!({
                "args": "--flag"
            }))
            .is_err()
        );

        assert!(
            serde_json::from_value::<UpdateServerInput>(serde_json::json!({
                "name": "Valid",
                "extra": true
            }))
            .is_err()
        );

        let parsed = serde_json::from_value::<UpdateServerInput>(serde_json::json!({
            "args": null,
            "env": null,
            "headers": null,
            "workingDir": null,
            "autoStart": true
        }))
        .expect("nullable update payload should deserialize");

        assert_eq!(parsed.args, UpdateField::Set(None));
        assert_eq!(parsed.env, UpdateField::Set(None));
        assert_eq!(parsed.headers, UpdateField::Set(None));
        assert_eq!(parsed.working_dir, UpdateField::Set(None));
        assert_eq!(parsed.auto_start, Some(true));

        let empty = serde_json::from_value::<UpdateServerInput>(serde_json::json!({}))
            .expect("empty update payload should deserialize");
        assert_eq!(empty.args, UpdateField::Unset);
        assert_eq!(empty.env, UpdateField::Unset);
        assert_eq!(empty.headers, UpdateField::Unset);
        assert_eq!(empty.working_dir, UpdateField::Unset);
    }
}

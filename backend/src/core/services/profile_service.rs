// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

//! Profile 领域服务。
//!
//! 封装 Profile 持久化与领域校验,让路由层只负责 HTTP 形状。

use crate::core::db::profile_repo::{
    Profile, ProfileDetailServer, ProfileRepository, ProfileServerState, RemoveResult,
};
use crate::core::db::Database;

pub struct ProfileService;

/// profile 操作可能产生的领域错误。与 ServerServiceError 对齐,
/// 让路由层能把 NotFound 等映射到正确的 HTTP 状态码。
pub enum ProfileServiceError {
    NotFound(String),
    Validation(String),
    Internal(String),
}

/// 单一映射点:ProfileServiceError → AppError。
impl From<ProfileServiceError> for crate::core::http::app_error::AppError {
    fn from(e: ProfileServiceError) -> Self {
        match e {
            ProfileServiceError::NotFound(m) => Self::not_found(m),
            ProfileServiceError::Validation(m) => Self::validation(m),
            ProfileServiceError::Internal(m) => Self::internal(m),
        }
    }
}

impl ProfileService {
    pub fn list(db: &Database) -> Result<Vec<Profile>, ProfileServiceError> {
        ProfileRepository::new(db)
            .find_all()
            .map_err(ProfileServiceError::Internal)
    }

    pub fn create(db: &Database, name: &str) -> Result<Profile, ProfileServiceError> {
        if name.is_empty() {
            return Err(ProfileServiceError::Validation("name is required".into()));
        }
        ProfileRepository::new(db)
            .create(name)
            .map_err(ProfileServiceError::Internal)
    }

    pub fn get_detail(
        db: &Database,
        id: &str,
    ) -> Result<(Profile, Vec<ProfileDetailServer>), ProfileServiceError> {
        let repo = ProfileRepository::new(db);
        let profile = repo
            .find_by_id(id)
            .map_err(ProfileServiceError::Internal)?
            .ok_or_else(|| ProfileServiceError::NotFound("Profile not found".into()))?;
        let servers = repo
            .find_profile_servers(id)
            .map_err(ProfileServiceError::Internal)?;
        Ok((profile, servers))
    }

    pub fn update(
        db: &Database,
        id: &str,
        name: Option<&str>,
    ) -> Result<Profile, ProfileServiceError> {
        ProfileRepository::new(db)
            .update(id, name)
            .map_err(ProfileServiceError::Internal)?
            .ok_or_else(|| ProfileServiceError::NotFound("Profile not found".into()))
    }

    pub fn remove(db: &Database, id: &str) -> Result<(), ProfileServiceError> {
        match ProfileRepository::new(db)
            .remove(id)
            .map_err(ProfileServiceError::Internal)?
        {
            RemoveResult::Success => Ok(()),
            RemoveResult::NotFound => {
                Err(ProfileServiceError::NotFound("Profile not found".into()))
            }
        }
    }

    pub fn get_mcp_token(db: &Database, id: &str) -> Result<String, ProfileServiceError> {
        ProfileRepository::new(db)
            .find_mcp_token(id)
            .map_err(ProfileServiceError::Internal)?
            .ok_or_else(|| ProfileServiceError::NotFound("Profile not found".into()))
    }

    pub fn rotate_mcp_token(db: &Database, id: &str) -> Result<String, ProfileServiceError> {
        ProfileRepository::new(db)
            .rotate_mcp_token(id)
            .map_err(ProfileServiceError::Internal)?
            .ok_or_else(|| ProfileServiceError::NotFound("Profile not found".into()))
    }

    pub fn get_profile_server(
        db: &Database,
        profile_id: &str,
        server_id: &str,
    ) -> Result<ProfileDetailServer, ProfileServiceError> {
        let servers = ProfileRepository::new(db)
            .find_profile_servers(profile_id)
            .map_err(ProfileServiceError::Internal)?;
        servers
            .into_iter()
            .find(|s| s.server.id == server_id)
            .ok_or_else(|| ProfileServiceError::NotFound("Server not found in profile".into()))
    }

    pub fn upsert_profile_server(
        db: &Database,
        profile_id: &str,
        server_id: &str,
        enabled: Option<bool>,
        disabled_tools: Option<&Vec<String>>,
    ) -> Result<ProfileServerState, ProfileServiceError> {
        ProfileRepository::new(db)
            .upsert_profile_server(profile_id, server_id, enabled, disabled_tools)
            .map_err(ProfileServiceError::Internal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn validation_error_maps_to_bad_request() {
        let err: crate::core::http::app_error::AppError =
            ProfileServiceError::Validation("name is required".into()).into();

        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
        assert_eq!(err.code(), "VALIDATION_ERROR");
    }
}

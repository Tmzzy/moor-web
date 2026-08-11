// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

use crate::core::db::{secret_repo, Database};

const TOKEN_PREFIX: &str = "moor_";
const TOKEN_RANDOM_BYTES: usize = 32;

pub fn validate(token: &str) -> Result<(), String> {
    if token.is_empty() {
        return Err("MCP token must not be empty".to_string());
    }
    if token.chars().any(char::is_whitespace) {
        return Err("MCP token must not contain whitespace".to_string());
    }
    Ok(())
}

pub fn generate() -> Result<String, String> {
    let mut bytes = [0_u8; TOKEN_RANDOM_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("failed to generate MCP token: {error}"))?;
    Ok(format!("{TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes)))
}

pub fn initialize(db: &Database, bootstrap_token: Option<&str>) -> Result<String, String> {
    if let Some(token) = secret_repo::load(db, secret_repo::MCP_TOKEN_KEY)? {
        validate(&token).map_err(|error| format!("persisted {error}"))?;
        return Ok(token);
    }

    let token = match bootstrap_token {
        Some(token) => {
            validate(token)?;
            token.to_string()
        }
        None => generate()?,
    };
    secret_repo::save(db, secret_repo::MCP_TOKEN_KEY, &token)?;
    Ok(token)
}

pub fn rotate(db: &Database) -> Result<String, String> {
    let token = generate()?;
    secret_repo::save(db, secret_repo::MCP_TOKEN_KEY, &token)?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Database {
        let path = std::env::temp_dir().join(format!("moor-token-{}.db", uuid::Uuid::new_v4()));
        let db = Database::open(&path).expect("failed to open token test database");
        db.run_migrations()
            .expect("failed to migrate token test database");
        db
    }

    #[test]
    fn initialize_generates_and_reuses_a_persisted_token() {
        let db = database();
        let first = initialize(&db, None).expect("token should initialize");
        let second =
            initialize(&db, Some("ignored-bootstrap")).expect("persisted token should load");

        assert_eq!(first, second);
    }

    #[test]
    fn initialize_uses_bootstrap_only_when_store_is_empty() {
        let db = database();
        let token = initialize(&db, Some("legacy-token")).expect("bootstrap should initialize");

        assert_eq!(token, "legacy-token");
    }

    #[test]
    fn generated_token_has_prefix_and_no_whitespace() {
        let token = generate().expect("token should generate");

        assert!(token.starts_with(TOKEN_PREFIX));
        assert!(!token.chars().any(char::is_whitespace));
    }

    #[test]
    fn rotate_replaces_the_persisted_token() {
        let db = database();
        let first = initialize(&db, None).expect("token should initialize");
        let second = rotate(&db).expect("token should rotate");

        assert_ne!(first, second);
        assert_eq!(
            secret_repo::load(&db, secret_repo::MCP_TOKEN_KEY)
                .expect("token should load")
                .as_deref(),
            Some(second.as_str()),
        );
    }
}

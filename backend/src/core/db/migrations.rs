// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::Database;

pub fn run_migrations(db: &Database) -> Result<(), String> {
    db.exec(
        "CREATE TABLE IF NOT EXISTS profiles (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            mcp_token TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS mcp_servers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            connection_type TEXT NOT NULL CHECK(connection_type IN ('stdio', 'http')),
            command TEXT,
            args TEXT,
            url TEXT,
            env TEXT,
            headers TEXT,
            working_dir TEXT,
            auto_start INTEGER NOT NULL DEFAULT 0,
            sort_order INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'stopped' CHECK(status IN ('stopped', 'starting', 'running', 'error')),
            error_message TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS profile_servers (
            profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
            server_id TEXT NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
            enabled INTEGER NOT NULL DEFAULT 1,
            disabled_tools TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (profile_id, server_id)
        );

        CREATE TABLE IF NOT EXISTS tool_discoveries (
            server_id TEXT NOT NULL REFERENCES mcp_servers(id) ON DELETE CASCADE,
            tool_name TEXT NOT NULL,
            exposed_name TEXT NOT NULL,
            description TEXT,
            input_schema TEXT,
            discovered_at TEXT NOT NULL,
            PRIMARY KEY (server_id, tool_name)
        );

        CREATE TABLE IF NOT EXISTS audit_logs (
            id TEXT PRIMARY KEY,
            timestamp TEXT NOT NULL,
            profile_id TEXT REFERENCES profiles(id),
            server_id TEXT REFERENCES mcp_servers(id),
            tool_name TEXT NOT NULL,
            arguments TEXT,
            result TEXT,
            error TEXT,
            duration_ms INTEGER,
            agent_info TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_audit_logs_timestamp ON audit_logs(timestamp);
        CREATE INDEX IF NOT EXISTS idx_audit_logs_tool_name ON audit_logs(tool_name);
        CREATE INDEX IF NOT EXISTS idx_audit_logs_server_id ON audit_logs(server_id);
        CREATE INDEX IF NOT EXISTS idx_tool_discoveries_server_id ON tool_discoveries(server_id);
        CREATE INDEX IF NOT EXISTS idx_tool_discoveries_exposed_name ON tool_discoveries(exposed_name);

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );",
    )?;

    ensure_column(db, "tool_discoveries", "exposed_name", "TEXT")?;
    ensure_column(db, "mcp_servers", "headers", "TEXT")?;
    ensure_column(db, "profiles", "mcp_token", "TEXT")?;
    backfill_profile_mcp_tokens(db)?;
    normalize_profiles_schema(db)?;
    db.exec("CREATE UNIQUE INDEX IF NOT EXISTS idx_profiles_mcp_token ON profiles(mcp_token);")?;
    db.exec("DROP TABLE IF EXISTS secrets;")?;
    ensure_column(
        db,
        "mcp_servers",
        "auto_start",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        db,
        "mcp_servers",
        "sort_order",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    backfill_server_sort_order(db)?;

    Ok(())
}

fn backfill_profile_mcp_tokens(db: &Database) -> Result<(), String> {
    let profile_ids = db.query_all(
        "SELECT id FROM profiles WHERE mcp_token IS NULL OR mcp_token = ''",
        &[],
        |row| row.get::<_, String>(0),
    )?;
    if profile_ids.is_empty() {
        return Ok(());
    }

    let tokens = profile_ids
        .iter()
        .map(|_| crate::core::services::mcp_token::generate())
        .collect::<Result<Vec<_>, _>>()?;
    db.transaction(|conn| {
        for (profile_id, token) in profile_ids.iter().zip(tokens.iter()) {
            conn.execute(
                "UPDATE profiles SET mcp_token = ?1 WHERE id = ?2",
                rusqlite::params![token, profile_id],
            )
            .map_err(|error| error.to_string())?;
        }
        Ok(())
    })
}

fn ensure_column(db: &Database, table: &str, column: &str, definition: &str) -> Result<(), String> {
    let all_cols = db.query_all(&format!("PRAGMA table_info({table})"), &[], |row| {
        row.get::<_, String>(1)
    })?;
    if all_cols.iter().any(|c| c == column) {
        return Ok(());
    }
    db.run(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        &[],
    )
}

fn normalize_profiles_schema(db: &Database) -> Result<(), String> {
    let columns = db.query_all("PRAGMA table_info(profiles)", &[], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(3)?))
    })?;
    let has_is_active = columns.iter().any(|(name, _)| name == "is_active");
    let token_is_not_null = columns
        .iter()
        .find(|(name, _)| name == "mcp_token")
        .is_some_and(|(_, not_null)| *not_null != 0);
    if !has_is_active && token_is_not_null {
        return Ok(());
    }

    let result = db.exec(
        "PRAGMA foreign_keys = OFF;
         BEGIN IMMEDIATE;
         CREATE TABLE profiles_next (
             id TEXT PRIMARY KEY,
             name TEXT NOT NULL,
             mcp_token TEXT NOT NULL,
             created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL
         );
         INSERT INTO profiles_next (id, name, mcp_token, created_at, updated_at)
         SELECT id, name, mcp_token, created_at, updated_at FROM profiles;
         DROP TABLE profiles;
         ALTER TABLE profiles_next RENAME TO profiles;
         COMMIT;",
    );
    if let Err(error) = result {
        let _ = db.exec("ROLLBACK;");
        let _ = db.exec("PRAGMA foreign_keys = ON;");
        return Err(error);
    }
    db.exec("PRAGMA foreign_keys = ON;")
}

fn backfill_server_sort_order(db: &Database) -> Result<(), String> {
    let rows = db.query_all(
        "SELECT id, sort_order FROM mcp_servers ORDER BY created_at DESC, id ASC",
        &[],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
    )?;
    if rows.len() <= 1 {
        return Ok(());
    }
    let needs_backfill = rows
        .iter()
        .map(|(_, o)| *o)
        .collect::<std::collections::HashSet<_>>()
        .len()
        == 1;
    if !needs_backfill {
        return Ok(());
    }
    db.transaction(|conn| {
        for (index, (id, _)) in rows.iter().enumerate() {
            conn.execute(
                "UPDATE mcp_servers SET sort_order = ?1 WHERE id = ?2",
                rusqlite::params![&(index as i64), id],
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn temp_db() -> Database {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("moor-migrations-{ts}.db"));
        Database::open(&path).expect("open db")
    }

    #[test]
    fn backfills_sort_order_newest_created_first() {
        let db = temp_db();
        db.exec(
            "CREATE TABLE mcp_servers (
                id TEXT PRIMARY KEY, name TEXT NOT NULL,
                connection_type TEXT NOT NULL CHECK(connection_type IN ('stdio','http')),
                command TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
            );",
        )
        .expect("create table");
        for (id, created) in [
            ("old", "2026-01-01T00:00:00.000Z"),
            ("new", "2026-01-03T00:00:00.000Z"),
            ("middle", "2026-01-02T00:00:00.000Z"),
        ] {
            db.run(
                "INSERT INTO mcp_servers (id, name, connection_type, command, created_at, updated_at) VALUES (?1, ?1, 'stdio', 'node', ?2, ?2)",
                &[&id, &created],
            )
            .expect("insert row");
        }

        run_migrations(&db).expect("migrate");

        let rows = db
            .query_all(
                "SELECT id, sort_order FROM mcp_servers ORDER BY sort_order ASC",
                &[],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .expect("query");
        assert_eq!(
            rows,
            vec![
                ("new".to_string(), 0),
                ("middle".to_string(), 1),
                ("old".to_string(), 2),
            ]
        );
    }

    #[test]
    fn backfills_distinct_tokens_for_existing_profiles() {
        let db = temp_db();
        db.exec(
            "CREATE TABLE profiles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO profiles (id, name, is_active, created_at, updated_at)
            VALUES
                ('personal', 'Personal', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                ('work', 'Work', 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
            CREATE TABLE secrets (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO secrets (key, value, updated_at)
            VALUES ('mcp_token', 'legacy-token', '2026-01-01T00:00:00Z');",
        )
        .expect("create legacy profiles table");

        run_migrations(&db).expect("migrate");

        let tokens = db
            .query_all("SELECT mcp_token FROM profiles ORDER BY id", &[], |row| {
                row.get::<_, String>(0)
            })
            .expect("query profile tokens");
        assert_eq!(tokens.len(), 2);
        assert_ne!(tokens[0], tokens[1]);
        assert!(tokens.iter().all(|token| token.starts_with("moor_")));

        let profile_columns = db
            .query_all("PRAGMA table_info(profiles)", &[], |row| {
                Ok((row.get::<_, String>(1)?, row.get::<_, i64>(3)?))
            })
            .expect("query profile columns");
        assert!(!profile_columns.iter().any(|(name, _)| name == "is_active"));
        assert!(profile_columns
            .iter()
            .find(|(name, _)| name == "mcp_token")
            .is_some_and(|(_, not_null)| *not_null != 0));
        let secrets_table = db
            .query_one(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'secrets'",
                &[],
                |row| row.get::<_, String>(0),
            )
            .expect("query secrets table");
        assert!(secrets_table.is_none());
    }
}

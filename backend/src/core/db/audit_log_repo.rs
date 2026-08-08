// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use super::Database;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogEntry {
    pub id: String,
    pub timestamp: String,
    pub profile_id: Option<String>,
    pub server_id: Option<String>,
    pub tool_name: String,
    pub arguments: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: Option<i64>,
    pub agent_info: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogStats {
    pub total_calls: i64,
    pub error_calls: i64,
    pub error_rate: f64,
    pub avg_duration_ms: Option<f64>,
    pub top_tools: Vec<TopTool>,
    pub top_servers: Vec<TopServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopTool {
    pub tool_name: String,
    pub count: i64,
    pub avg_duration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopServer {
    pub server_id: String,
    pub count: i64,
}

fn map_audit_log(row: &rusqlite::Row<'_>) -> rusqlite::Result<AuditLogEntry> {
    let args_str: Option<String> = row.get("arguments")?;
    let result_str: Option<String> = row.get("result")?;
    Ok(AuditLogEntry {
        id: row.get("id")?,
        timestamp: row.get("timestamp")?,
        profile_id: row.get("profile_id")?,
        server_id: row.get("server_id")?,
        tool_name: row.get("tool_name")?,
        arguments: args_str.and_then(|s| serde_json::from_str(&s).ok()),
        result: result_str.and_then(|s| serde_json::from_str(&s).ok()),
        error: row.get("error")?,
        duration_ms: row.get("duration_ms")?,
        agent_info: row.get("agent_info")?,
    })
}

pub struct AuditLogRepository<'a> {
    pub db: &'a Database,
}

impl<'a> AuditLogRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn query_logs(
        &self,
        server_id: Option<&str>,
        tool_name: Option<&str>,
        from: Option<&str>,
        to: Option<&str>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<AuditLogEntry>, String> {
        let mut sql = "SELECT * FROM audit_logs WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(sid) = server_id {
            sql += " AND server_id = ?";
            params.push(Box::new(sid.to_string()));
        }
        if let Some(tn) = tool_name {
            sql += " AND tool_name = ?";
            params.push(Box::new(tn.to_string()));
        }
        if let Some(f) = from {
            sql += " AND timestamp >= ?";
            params.push(Box::new(f.to_string()));
        }
        if let Some(t) = to {
            sql += " AND timestamp <= ?";
            params.push(Box::new(t.to_string()));
        }

        let safe_limit = limit.map_or(50, |l| l.clamp(1, 200));
        let safe_offset = offset.unwrap_or(0);
        sql += " ORDER BY timestamp DESC LIMIT ? OFFSET ?";
        params.push(Box::new(safe_limit));
        params.push(Box::new(safe_offset));

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        self.db.query_all(&sql, &param_refs, map_audit_log)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert(
        &self,
        id: &str,
        timestamp: &str,
        profile_id: Option<&str>,
        server_id: Option<&str>,
        tool_name: &str,
        arguments: Option<&serde_json::Value>,
        result: Option<&serde_json::Value>,
        error: Option<&str>,
        duration_ms: i64,
        agent_info: Option<&str>,
    ) -> Result<(), String> {
        let args_json = arguments.map(|v| serde_json::to_string(v).unwrap_or_default());
        let result_json = result.map(|v| serde_json::to_string(v).unwrap_or_default());
        self.db.run(
            "INSERT INTO audit_logs (id, timestamp, profile_id, server_id, tool_name, arguments, result, error, duration_ms, agent_info)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            &[&id, &timestamp, &profile_id, &server_id, &tool_name, &args_json, &result_json, &error, &duration_ms, &agent_info],
        )
    }

    pub fn get_stats(&self) -> Result<LogStats, String> {
        let summary = self.db.query_one(
            "SELECT COUNT(*) as total_calls, COALESCE(SUM(CASE WHEN error IS NOT NULL THEN 1 ELSE 0 END), 0) as error_calls, AVG(duration_ms) as avg_duration_ms FROM audit_logs",
            &[],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<f64>>(2)?,
                ))
            },
        )?;

        let (total_calls, error_calls, avg_duration_ms) = summary.unwrap_or((0, 0, None));
        let error_rate = if total_calls > 0 {
            error_calls as f64 / total_calls as f64
        } else {
            0.0
        };

        let top_tools = self.db.query_all(
            "SELECT tool_name, COUNT(*) as count, AVG(duration_ms) as avg_duration FROM audit_logs GROUP BY tool_name ORDER BY count DESC LIMIT 10",
            &[],
            |row| {
                Ok(TopTool {
                    tool_name: row.get(0)?,
                    count: row.get(1)?,
                    avg_duration: row.get(2)?,
                })
            },
        )?;

        let top_servers = self.db.query_all(
            "SELECT server_id, COUNT(*) as count FROM audit_logs WHERE server_id IS NOT NULL GROUP BY server_id ORDER BY count DESC LIMIT 10",
            &[],
            |row| {
                Ok(TopServer {
                    server_id: row.get(0)?,
                    count: row.get(1)?,
                })
            },
        )?;

        Ok(LogStats {
            total_calls,
            error_calls,
            error_rate,
            avg_duration_ms,
            top_tools,
            top_servers,
        })
    }
}

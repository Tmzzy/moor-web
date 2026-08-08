// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

//! 工具调用审计记录器。
//!
//! 封装审计启用检查、活动 Profile 解析、脱敏和写库。审计失败不影响工具调用主路径。

use crate::core::db::audit_log_repo::AuditLogRepository;
use crate::core::db::profile_repo::ProfileRepository;
use crate::core::db::Database;
use crate::core::services::audit_redaction::redact_for_audit;
use crate::core::services::settings::audit_logging_enabled;

/// 一次工具调用的完整记录。调用方填好这些字段,其余(profile 解析、
/// 脱敏、enabled 检查、写库)由 [AuditRecorder::record] 处理。
pub struct ToolCallRecord<'a> {
    pub server_id: Option<&'a str>,
    pub tool_name: &'a str,
    pub arguments: &'a serde_json::Value,
    pub result: Option<&'a serde_json::Value>,
    pub error: Option<&'a str>,
    pub duration_ms: i64,
    pub agent_info: Option<&'a str>,
}

/// 审计记录器。无状态——每次调用从 Database 读取 enabled 标志和活动 profile。
pub struct AuditRecorder;

impl AuditRecorder {
    /// 若审计已开启,则把这条记录(脱敏后)写入 audit_logs;否则直接返回。
    /// 失败静默吞掉(审计是副作用,不应影响工具调用的主路径)。
    pub fn record(db: &Database, entry: ToolCallRecord<'_>) {
        if !audit_logging_enabled(db) {
            return;
        }

        let profile_id = ProfileRepository::new(db).find_active_id().ok().flatten();
        let redacted_args = redact_for_audit(entry.arguments);
        let redacted_result = entry.result.map(redact_for_audit);

        let _ = AuditLogRepository::new(db).insert(
            &uuid::Uuid::new_v4().to_string(),
            &chrono::Utc::now().to_rfc3339(),
            profile_id.as_deref(),
            entry.server_id,
            entry.tool_name,
            Some(&redacted_args),
            redacted_result.as_ref(),
            entry.error,
            entry.duration_ms,
            entry.agent_info,
        );
    }
}

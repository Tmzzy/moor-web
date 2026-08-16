// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use crate::core::config::import_parser::ScannedServer;
use crate::core::config::scanner;
use crate::core::db::Database;
use crate::core::services::server_manager::ServerManager;
use crate::core::services::server_service::{CreateServerInput, ServerService, ServerServiceError};
use std::sync::Arc;

pub struct ImportResult {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
}

pub async fn execute_import(
    db: &Arc<Database>,
    server_manager: &Arc<ServerManager>,
    servers: Option<Vec<ScannedServer>>,
    profile_ids: &[String],
) -> Result<ImportResult, ServerServiceError> {
    let existing_names = ServerService::find_all_names(db);
    let all_servers = match servers {
        Some(s) if !s.is_empty() => s,
        _ => scanner::scan_all_configs().servers,
    };
    let (candidates, skipped) = scanner::partition_import_candidates(&all_servers, &existing_names);

    let inputs: Vec<CreateServerInput> = candidates
        .iter()
        .map(|sc| CreateServerInput {
            name: sc.name.clone(),
            connection_type: sc.connection_type.clone(),
            command: sc.command.clone(),
            args: sc.args.clone(),
            url: sc.url.clone(),
            env: sc.env.clone(),
            headers: sc.headers.clone(),
            working_dir: sc.working_dir.clone(),
            auto_start: false,
            profile_ids: profile_ids.to_vec(),
        })
        .collect();

    let imported_servers = ServerService::insert_servers(db, server_manager, &inputs).await?;
    let imported = imported_servers.into_iter().map(|s| s.name).collect();

    Ok(ImportResult { imported, skipped })
}

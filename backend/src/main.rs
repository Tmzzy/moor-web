// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use std::{
    env, fs,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

mod core;

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 9223;

fn env_port() -> Result<u16, String> {
    match env::var("MOOR_PORT") {
        Ok(value) => value
            .parse::<u16>()
            .map_err(|_| "MOOR_PORT must be a valid TCP port".to_string()),
        Err(_) => Ok(DEFAULT_PORT),
    }
}

fn data_dir() -> PathBuf {
    env::var_os("MOOR_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"))
}

fn static_dir() -> PathBuf {
    env::var_os("MOOR_STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist"))
}

fn management_auth() -> Result<core::http::ManagementAuth, String> {
    let username = env::var("MOOR_USERNAME").unwrap_or_else(|_| "moor".to_string());
    let password =
        env::var("MOOR_PASSWORD").map_err(|_| "MOOR_PASSWORD is required".to_string())?;
    if username.is_empty() || password.is_empty() {
        return Err("MOOR_USERNAME and MOOR_PASSWORD must not be empty".to_string());
    }
    if username.contains(':') {
        return Err("MOOR_USERNAME must not contain ':'".to_string());
    }
    Ok(core::http::ManagementAuth::basic(username, password))
}

fn mcp_auth() -> Result<core::http::McpAuth, String> {
    let token =
        env::var("MOOR_MCP_TOKEN").map_err(|_| "MOOR_MCP_TOKEN is required".to_string())?;
    if token.is_empty() {
        return Err("MOOR_MCP_TOKEN must not be empty".to_string());
    }
    if token.chars().any(char::is_whitespace) {
        return Err("MOOR_MCP_TOKEN must not contain whitespace".to_string());
    }
    Ok(core::http::McpAuth::bearer(token))
}

fn healthcheck(port: u16) -> Result<(), String> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let timeout = Duration::from_secs(3);
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| format!("healthcheck could not connect to {address}: {error}"))?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| format!("healthcheck could not set read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| format!("healthcheck could not set write timeout: {error}"))?;
    write!(
        stream,
        "GET /api/health HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| format!("healthcheck request failed: {error}"))?;

    let mut status_line = String::new();
    BufReader::new(stream)
        .read_line(&mut status_line)
        .map_err(|error| format!("healthcheck response failed: {error}"))?;
    if status_line.split_whitespace().nth(1) == Some("200") {
        Ok(())
    } else {
        Err(format!(
            "healthcheck returned an unexpected status: {}",
            status_line.trim_end()
        ))
    }
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut terminate = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let port = env_port()?;
    if env::args().nth(1).as_deref() == Some("--healthcheck") {
        return healthcheck(port);
    }

    let management_auth = management_auth()?;
    let mcp_auth = mcp_auth()?;
    let host = env::var("MOOR_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let data_dir = data_dir();
    let static_dir = static_dir();
    let public_url = env::var("MOOR_PUBLIC_URL")
        .unwrap_or_else(|_| format!("http://localhost:{port}"))
        .trim_end_matches('/')
        .to_string();

    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("failed to create {}: {error}", data_dir.display()))?;
    let db = Arc::new(core::db::Database::open(&data_dir.join("moor.db"))?);
    db.run_migrations()?;
    let settings = core::services::settings::init_settings(&db, &data_dir)?;

    core::db::profile_repo::ProfileRepository::new(&db).seed_default()?;

    let event_bus = Arc::new(core::services::event_bus::EventBus::new(256));
    let server_manager = Arc::new(core::services::server_manager::ServerManager::new(
        db.clone(),
        event_bus.clone(),
    ));
    server_manager.load_from_db().await;

    if settings.general.auto_start_servers_on_launch {
        let manager = server_manager.clone();
        tokio::spawn(async move {
            manager.start_auto_start_servers().await;
        });
    }

    let state = Arc::new(core::http::AppState::new(
        db,
        management_auth,
        mcp_auth,
        env!("CARGO_PKG_VERSION").to_string(),
        port,
        public_url.clone(),
        static_dir,
        event_bus,
        server_manager.clone(),
    ));

    eprintln!(
        "Moor {version} listening on {host}:{port}",
        version = env!("CARGO_PKG_VERSION")
    );
    eprintln!("Management UI: {public_url}");
    eprintln!("MCP endpoint: {public_url}/mcp");

    let server = core::http::start_server(state, &host, port);
    tokio::pin!(server);
    let result = tokio::select! {
        result = &mut server => result,
        _ = shutdown_signal() => Ok(()),
    };

    server_manager.stop_all().await;
    result
}

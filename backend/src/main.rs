// SPDX-License-Identifier: Apache-2.0
// Modified from the original Moor project for this Web/Docker distribution; see NOTICE.

use std::{
    env,
    ffi::OsStr,
    fs,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpStream},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Duration,
};

mod core;

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: u16 = 9223;
const RUNTIME_DIRECTORY_ENV_VARS: [&str; 10] = [
    "HOME",
    "XDG_CACHE_HOME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "NPM_CONFIG_CACHE",
    "UV_CACHE_DIR",
    "UV_PYTHON_INSTALL_DIR",
    "UV_PYTHON_BIN_DIR",
    "UV_TOOL_DIR",
    "UV_TOOL_BIN_DIR",
];

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

fn normalize_absolute_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) {
                    normalized.pop();
                }
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn ensure_runtime_directories<K, V>(
    data_dir: &Path,
    variables: impl IntoIterator<Item = (K, V)>,
) -> Result<(), String>
where
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    let canonical_data_dir = fs::canonicalize(data_dir)
        .map_err(|error| format!("failed to resolve {}: {error}", data_dir.display()))?;

    for (name, value) in variables {
        let Some(name) = name.as_ref().to_str() else {
            continue;
        };
        if !RUNTIME_DIRECTORY_ENV_VARS.contains(&name) {
            continue;
        }

        let path = PathBuf::from(value.as_ref());
        let absolute_path = if path.is_absolute() {
            path.clone()
        } else {
            env::current_dir()
                .map_err(|error| {
                    format!(
                        "failed to resolve runtime directory {name}={}: {error}",
                        path.display()
                    )
                })?
                .join(&path)
        };
        let absolute_path = normalize_absolute_path(&absolute_path);
        let Some(existing_ancestor) = absolute_path.ancestors().find(|path| path.exists()) else {
            continue;
        };
        let canonical_ancestor = fs::canonicalize(existing_ancestor).map_err(|error| {
            format!(
                "failed to resolve runtime directory {name}={}: {error}",
                path.display()
            )
        })?;
        if !canonical_ancestor.starts_with(&canonical_data_dir) {
            continue;
        }

        fs::create_dir_all(&absolute_path).map_err(|error| {
            format!(
                "failed to create runtime directory {name}={}: {error}",
                path.display()
            )
        })?;
    }

    Ok(())
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
    let host = env::var("MOOR_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let data_dir = data_dir();
    let static_dir = static_dir();
    let public_url = env::var("MOOR_PUBLIC_URL")
        .unwrap_or_else(|_| format!("http://localhost:{port}"))
        .trim_end_matches('/')
        .to_string();

    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("failed to create {}: {error}", data_dir.display()))?;
    ensure_runtime_directories(&data_dir, env::vars_os())?;
    let db = Arc::new(core::db::Database::open(&data_dir.join("moor.db"))?);
    db.run_migrations()?;
    let settings = core::services::settings::init_settings(&db, &data_dir)?;

    core::db::profile_repo::ProfileRepository::new(&db).seed_initial()?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    struct TestDataDir(PathBuf);

    impl TestDataDir {
        fn new(name: &str) -> Self {
            let path = env::temp_dir().join(format!("moor-{name}-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).expect("failed to create test data directory");
            Self(path)
        }
    }

    impl Drop for TestDataDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn ensure_runtime_directories_creates_configured_paths_inside_data_dir() {
        let data_dir = TestDataDir::new("runtime-directories-create");
        let home = data_dir.0.join("runtime/home");
        let uv_cache = data_dir.0.join("runtime/uv/cache");
        let variables = [
            (OsString::from("HOME"), home.as_os_str().to_owned()),
            (
                OsString::from("UV_CACHE_DIR"),
                uv_cache.as_os_str().to_owned(),
            ),
        ];

        ensure_runtime_directories(&data_dir.0, variables)
            .expect("runtime directories should be created");

        assert!(home.is_dir() && uv_cache.is_dir());
    }

    #[test]
    fn ensure_runtime_directories_ignores_unrelated_and_outside_paths() {
        let data_dir = TestDataDir::new("runtime-directories-ignore");
        let unrelated = data_dir.0.join("unrelated");
        let outside = env::temp_dir().join(format!("moor-outside-{}", uuid::Uuid::new_v4()));
        let variables = [
            (
                OsString::from("UNRELATED_DIRECTORY"),
                unrelated.as_os_str().to_owned(),
            ),
            (OsString::from("HOME"), outside.as_os_str().to_owned()),
        ];

        ensure_runtime_directories(&data_dir.0, variables)
            .expect("ignored runtime paths should not fail");

        assert!(!unrelated.exists() && !outside.exists());
    }

    #[test]
    fn ensure_runtime_directories_ignores_parent_traversal_outside_data_dir() {
        let data_dir = TestDataDir::new("runtime-directories-traversal");
        let outside_name = format!("moor-outside-{}", uuid::Uuid::new_v4());
        let outside = data_dir
            .0
            .join("runtime")
            .join("..")
            .join("..")
            .join(&outside_name);
        let resolved_outside = data_dir
            .0
            .parent()
            .expect("test data directory should have a parent")
            .join(outside_name);
        let variables = [(OsString::from("HOME"), outside.as_os_str().to_owned())];

        ensure_runtime_directories(&data_dir.0, variables)
            .expect("parent traversal outside the data directory should be ignored");

        assert!(!resolved_outside.exists());
    }

    #[test]
    fn normalize_absolute_path_does_not_traverse_above_root() {
        let normalized = normalize_absolute_path(Path::new("/../../data/runtime/home"));

        assert_eq!(normalized, Path::new("/data/runtime/home"));
    }

    #[test]
    fn ensure_runtime_directories_error_includes_variable_and_path() {
        let data_dir = TestDataDir::new("runtime-directories-error");
        let blocker = data_dir.0.join("runtime-blocker");
        fs::write(&blocker, b"not a directory").expect("failed to create blocker file");
        let path = blocker.join("home");
        let variables = [(OsString::from("HOME"), path.as_os_str().to_owned())];

        let error = ensure_runtime_directories(&data_dir.0, variables)
            .expect_err("creating a directory below a file should fail");

        assert!(error.contains("HOME") && error.contains(&path.display().to_string()));
    }
}

//! CrateBay Engine compatibility client management.
//!
//! Compatibility operations use `bollard` with `Arc<Docker>` for shared access.
//! The default connection path is the CrateBay built-in runtime. External
//! Docker-compatible endpoints are only used when a caller provides a host
//! string explicitly.

use bollard::Docker;
use std::time::Duration;

use crate::error::AppError;
use crate::runtime;

#[derive(Debug, Clone)]
enum DockerHostTarget {
    UnixSocket(String),
    NamedPipe(String),
    Http(String),
}

const DOCKER_PING_TIMEOUT_SECS: u64 = 5;
const DOCKER_DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Returns `true` when the source string represents the built-in runtime.
pub fn is_builtin_source(source: Option<&str>) -> bool {
    matches!(source, Some("builtin") | Some("built-in") | Some("runtime"))
}

fn parse_docker_host_target(raw: &str) -> Option<DockerHostTarget> {
    let host = raw.trim();
    if host.is_empty() {
        return None;
    }

    if let Some(path) = host.strip_prefix("unix://") {
        return Some(DockerHostTarget::UnixSocket(path.to_string()));
    }

    if let Some(path) = host.strip_prefix("tcp://") {
        return Some(DockerHostTarget::Http(format!("http://{}", path)));
    }

    if host.starts_with("http://") || host.starts_with("https://") {
        return Some(DockerHostTarget::Http(host.to_string()));
    }

    // Named pipe formats commonly used by Docker on Windows:
    // - npipe:////./pipe/docker_engine
    // - \\.\pipe\docker_engine
    if let Some(rest) = host.strip_prefix("npipe:") {
        let rest = rest.trim_start_matches('/');
        let rest = rest
            .strip_prefix("./pipe/")
            .or_else(|| rest.strip_prefix(".\\pipe\\"))
            .unwrap_or(rest);
        if !rest.is_empty() {
            return Some(DockerHostTarget::NamedPipe(format!(r"\\.\pipe\{}", rest)));
        }
    }

    if host.starts_with(r"\\.\pipe\") {
        return Some(DockerHostTarget::NamedPipe(host.to_string()));
    }

    // Support bare Unix socket paths when users pass `--docker-host /path/to/docker.sock`.
    if host.starts_with('/') {
        return Some(DockerHostTarget::UnixSocket(host.to_string()));
    }

    None
}

async fn try_connect_target(target: DockerHostTarget) -> Option<Docker> {
    match target {
        DockerHostTarget::UnixSocket(path) => {
            #[cfg(unix)]
            {
                let docker = Docker::connect_with_unix(
                    &path,
                    DOCKER_PING_TIMEOUT_SECS,
                    bollard::API_DEFAULT_VERSION,
                )
                .ok()?;
                if !crate::docker::is_available(&docker).await {
                    return None;
                }
                Docker::connect_with_unix(
                    &path,
                    DOCKER_DEFAULT_TIMEOUT_SECS,
                    bollard::API_DEFAULT_VERSION,
                )
                .ok()
            }
            #[cfg(not(unix))]
            {
                let _ = path;
                None
            }
        }
        DockerHostTarget::NamedPipe(pipe) => {
            #[cfg(windows)]
            {
                let docker = Docker::connect_with_named_pipe(
                    &pipe,
                    DOCKER_PING_TIMEOUT_SECS,
                    bollard::API_DEFAULT_VERSION,
                )
                .ok()?;
                if !crate::docker::is_available(&docker).await {
                    return None;
                }
                Docker::connect_with_named_pipe(
                    &pipe,
                    DOCKER_DEFAULT_TIMEOUT_SECS,
                    bollard::API_DEFAULT_VERSION,
                )
                .ok()
            }
            #[cfg(not(windows))]
            {
                let _ = pipe;
                None
            }
        }
        DockerHostTarget::Http(url) => {
            let docker = Docker::connect_with_http(
                &url,
                DOCKER_PING_TIMEOUT_SECS,
                bollard::API_DEFAULT_VERSION,
            )
            .ok()?;
            if !crate::docker::is_available(&docker).await {
                return None;
            }
            Docker::connect_with_http(
                &url,
                DOCKER_DEFAULT_TIMEOUT_SECS,
                bollard::API_DEFAULT_VERSION,
            )
            .ok()
        }
    }
}

/// Create a client connection to an explicit Engine-compatible host.
pub async fn connect_host(host: &str) -> Result<Docker, AppError> {
    let target = parse_docker_host_target(host).ok_or_else(|| {
        AppError::Runtime(format!("Unsupported Engine host format: {}", host.trim()))
    })?;

    try_connect_target(target)
        .await
        .ok_or_else(|| AppError::Runtime(format!("Engine host is not reachable: {}", host.trim())))
}

/// Create a CrateBay Engine compatibility client without starting the runtime.
///
/// Attempts connections in priority order:
/// 1. Already-running built-in runtime socket
/// 2. Already-running built-in runtime TCP (Linux/Windows)
///
/// This intentionally does not probe Bollard local defaults; CrateBay's
/// product path is the built-in runtime.
pub async fn connect() -> Result<Docker, AppError> {
    // 1. Try built-in runtime socket.
    let runtime_mgr = runtime::create_runtime_manager();
    let runtime_socket = runtime_mgr.engine_socket_path();
    if runtime_socket.exists() {
        tracing::debug!(
            "Trying built-in runtime socket: {}",
            runtime_socket.display()
        );
        #[cfg(unix)]
        {
            let socket_str = runtime_socket.to_str().unwrap_or_default();
            if let Some(docker) =
                try_connect_target(DockerHostTarget::UnixSocket(socket_str.to_string())).await
            {
                tracing::info!(
                    "Connected via built-in runtime: {}",
                    runtime_socket.display()
                );
                return Ok(docker);
            }
            tracing::debug!(
                "Built-in runtime socket not responsive: {}",
                runtime_socket.display()
            );
        }
    }

    // 2. Try built-in runtime TCP endpoint (Linux/Windows).
    //
    // On Linux and Windows the built-in runtime exposes the compatibility API
    // via a TCP endpoint (hostfwd / WSL localhost forwarding).
    // `engine_socket_path()` may not exist, so we attempt these endpoints
    // opportunistically.
    #[cfg(target_os = "linux")]
    {
        let host = runtime::linux::linux_engine_host();
        if let Some(target) = parse_docker_host_target(&host) {
            if let Some(docker) = try_connect_target(target).await {
                tracing::info!("Connected via built-in Linux runtime TCP endpoint");
                return Ok(docker);
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let host = runtime::windows::windows_engine_host();
        if let Some(target) = parse_docker_host_target(&host) {
            if let Some(docker) = try_connect_target(target).await {
                tracing::info!("Connected via built-in Windows runtime TCP endpoint");
                return Ok(docker);
            }
        }

        // Optional future: named pipe proxy for the built-in runtime.
        for pipe in [r"\\.\pipe\cratebay-engine", r"\\.\pipe\cratebay-docker"] {
            let Some(target) = parse_docker_host_target(pipe) else {
                continue;
            };
            if let Some(docker) = try_connect_target(target).await {
                tracing::info!(
                    "Connected via built-in Windows runtime named pipe: {}",
                    pipe
                );
                return Ok(docker);
            }
        }
    }

    Err(AppError::Runtime(
        "Built-in CrateBay Engine endpoint is not reachable".to_string(),
    ))
}

/// Attempt to connect, returning None if CrateBay Engine is not available.
pub async fn try_connect() -> Option<Docker> {
    connect().await.ok()
}

/// Check if the CrateBay Engine compatibility API is accessible.
pub async fn is_available(docker: &Docker) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_secs(DOCKER_PING_TIMEOUT_SECS), docker.ping()).await,
        Ok(Ok(_))
    )
}

/// Get CrateBay Engine compatibility version information.
pub async fn version(docker: &Docker) -> Result<bollard::system::Version, AppError> {
    docker.version().await.map_err(AppError::Docker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_docker_host_target_supports_unix_socket() {
        let target = parse_docker_host_target("unix:///var/run/docker.sock");
        assert!(matches!(target, Some(DockerHostTarget::UnixSocket(_))));
    }

    #[test]
    fn parse_docker_host_target_supports_bare_unix_path() {
        let target = parse_docker_host_target("/var/run/docker.sock");
        assert!(matches!(target, Some(DockerHostTarget::UnixSocket(_))));
    }

    #[test]
    fn parse_docker_host_target_supports_tcp() {
        let target = parse_docker_host_target("tcp://127.0.0.1:2375");
        match target {
            Some(DockerHostTarget::Http(url)) => assert_eq!(url, "http://127.0.0.1:2375"),
            other => panic!("unexpected target: {:?}", other),
        }
    }

    #[test]
    fn parse_docker_host_target_supports_http() {
        let target = parse_docker_host_target("http://localhost:2375");
        match target {
            Some(DockerHostTarget::Http(url)) => assert_eq!(url, "http://localhost:2375"),
            other => panic!("unexpected target: {:?}", other),
        }
    }

    #[test]
    fn parse_docker_host_target_supports_npipe() {
        let target = parse_docker_host_target("npipe:////./pipe/docker_engine");
        match target {
            Some(DockerHostTarget::NamedPipe(pipe)) => {
                assert!(pipe.contains("docker_engine"), "pipe: {}", pipe);
            }
            other => panic!("unexpected target: {:?}", other),
        }
    }

    #[test]
    fn parse_docker_host_target_rejects_context_names() {
        assert!(parse_docker_host_target("").is_none());
        assert!(parse_docker_host_target("desktop-linux").is_none());
        assert!(parse_docker_host_target("orbstack").is_none());
    }

    #[test]
    fn builtin_source_detection_accepts_legacy_labels() {
        assert!(is_builtin_source(Some("builtin")));
        assert!(is_builtin_source(Some("built-in")));
        assert!(is_builtin_source(Some("runtime")));
        assert!(!is_builtin_source(Some("tcp://127.0.0.1:2375")));
        assert!(!is_builtin_source(None));
    }

    #[test]
    fn default_connection_does_not_probe_bollard_local_defaults() {
        let source = include_str!("docker.rs");
        let forbidden = "connect_with_".to_string() + "local_defaults";

        assert!(
            !source.contains(&forbidden),
            "default connection must not auto-detect external Docker engines"
        );
    }
}

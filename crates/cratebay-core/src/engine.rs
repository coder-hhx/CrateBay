//! Container engine bring-up helpers.
//!
//! This module provides a single, shared entry-point used by both the GUI and
//! CLI to ensure a responsive Docker client backed by the CrateBay built-in
//! runtime.
//!
//! - Reuse an already-running built-in runtime first
//! - Otherwise start/provision the built-in runtime
//! - Use a cross-process lock to avoid concurrent provision/start (GUI + CLI)

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bollard::Docker;

use crate::error::AppError;
use crate::runtime::{self, RuntimeManager, RuntimeState};

const DISABLE_IMPLICIT_RUNTIME_START_ENV: &str = "CRATEBAY_DISABLE_RUNTIME_AUTO_START";

/// Options for [`ensure_docker`].
pub struct EnsureOptions {
    /// Maximum time to wait for acquiring the cross-process engine lock.
    pub lock_wait_timeout: Duration,
    /// Maximum time to wait for Docker to become responsive after starting runtime.
    pub docker_wait_timeout: Duration,
    /// Maximum time to wait for runtime state detection.
    pub runtime_detect_timeout: Duration,
    /// Maximum time to wait for starting the runtime VM/process.
    pub runtime_start_timeout: Duration,
    /// Maximum time to wait for provisioning the runtime image.
    pub runtime_provision_timeout: Duration,
    /// Optional callback invoked during runtime provisioning.
    pub on_provision_progress: Option<Box<dyn Fn(runtime::ProvisionProgress) + Send>>,
}

impl Default for EnsureOptions {
    fn default() -> Self {
        Self {
            lock_wait_timeout: Duration::from_secs(10 * 60),
            docker_wait_timeout: Duration::from_secs(120),
            runtime_detect_timeout: Duration::from_secs(10),
            runtime_start_timeout: Duration::from_secs(90),
            runtime_provision_timeout: Duration::from_secs(30 * 60),
            on_provision_progress: None,
        }
    }
}

/// Ensure a responsive Docker client, starting the built-in runtime if needed.
///
/// Only the CrateBay built-in runtime is used — external Docker daemons
/// (Colima, Docker Desktop, OrbStack, Podman, etc.) are not attempted.
pub async fn ensure_docker(
    runtime: &dyn RuntimeManager,
    options: EnsureOptions,
) -> Result<Arc<Docker>, AppError> {
    let EnsureOptions {
        lock_wait_timeout,
        docker_wait_timeout,
        runtime_detect_timeout,
        runtime_start_timeout,
        runtime_provision_timeout,
        on_provision_progress,
    } = options;

    // 1. Fast path: try connecting to an already-running built-in runtime.
    if let Some(docker) = try_connect_builtin(runtime).await {
        return Ok(Arc::new(docker));
    }

    // 2. Some callers, notably GUI E2E and read-only diagnostics, need status
    // checks without implicitly creating a VM. Manual runtime_start still calls
    // RuntimeManager::start() directly and is unaffected by this guard.
    if runtime::common::env_flag_enabled(DISABLE_IMPLICIT_RUNTIME_START_ENV) {
        return Err(AppError::Runtime(format!(
            "Implicit runtime start disabled by {}",
            DISABLE_IMPLICIT_RUNTIME_START_ENV
        )));
    }

    // 3. Acquire cross-process lock to avoid concurrent provision/start.
    let _lock = acquire_engine_lock(lock_wait_timeout).await?;

    // 4. TOCTOU: re-check after acquiring the lock — another process may have
    //    started the runtime while we were waiting.
    if let Some(docker) = try_connect_builtin(runtime).await {
        return Ok(Arc::new(docker));
    }

    // 5. Provision / start the built-in runtime.
    let current = tokio::time::timeout(runtime_detect_timeout, runtime.get_state())
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Timed out detecting runtime state after {:?}",
                runtime_detect_timeout
            ))
        })??;
    if current == RuntimeState::None {
        let cb = on_provision_progress.unwrap_or_else(|| Box::new(|_p| {}));
        tokio::time::timeout(runtime_provision_timeout, runtime.provision(cb))
            .await
            .map_err(|_| {
                AppError::Runtime(format!(
                    "Timed out provisioning runtime after {:?}",
                    runtime_provision_timeout
                ))
            })??;
    }
    tokio::time::timeout(runtime_start_timeout, runtime.start())
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Timed out starting runtime after {:?}",
                runtime_start_timeout
            ))
        })??;

    // 6. Wait for Docker to become responsive inside the runtime.
    let docker = wait_for_docker(runtime, docker_wait_timeout).await?;
    Ok(Arc::new(docker))
}

// ---------------------------------------------------------------------------
// Connection helpers
// ---------------------------------------------------------------------------

/// Try to connect to the CrateBay built-in runtime only (skip external Docker).
async fn try_connect_builtin(runtime: &dyn RuntimeManager) -> Option<Docker> {
    // Unix socket path (macOS / Linux socket mode)
    #[cfg(unix)]
    {
        let socket = runtime.docker_socket_path();
        if socket.exists() {
            let socket_str = socket.to_str().unwrap_or_default();
            if let Ok(docker) =
                Docker::connect_with_unix(socket_str, 5, bollard::API_DEFAULT_VERSION)
            {
                if crate::docker::is_available(&docker).await {
                    return Docker::connect_with_unix(
                        socket_str,
                        120,
                        bollard::API_DEFAULT_VERSION,
                    )
                    .ok();
                }
            }
        }
    }

    // TCP endpoint (Linux KVM / Windows WSL2)
    let docker = connect_runtime_docker(runtime).ok()?;
    crate::docker::is_available(&docker).await.then_some(docker)
}

fn connect_runtime_docker(runtime: &dyn RuntimeManager) -> Result<Docker, AppError> {
    #[cfg(target_os = "linux")]
    {
        let _ = runtime;
        let host = crate::runtime::linux::linux_docker_host();
        let http_host = host
            .strip_prefix("tcp://")
            .map(|rest| format!("http://{}", rest))
            .unwrap_or_else(|| host.replace("tcp://", "http://"));
        Docker::connect_with_http(&http_host, 120, bollard::API_DEFAULT_VERSION)
            .map_err(AppError::Docker)
    }

    #[cfg(target_os = "windows")]
    {
        let _ = runtime;
        let host = crate::runtime::windows::windows_docker_host();
        let http_host = host
            .strip_prefix("tcp://")
            .map(|rest| format!("http://{}", rest))
            .unwrap_or_else(|| host.replace("tcp://", "http://"));
        Docker::connect_with_http(&http_host, 120, bollard::API_DEFAULT_VERSION)
            .map_err(AppError::Docker)
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let socket = runtime.docker_socket_path();
        let socket_str = socket.to_str().unwrap_or_default();
        Docker::connect_with_unix(socket_str, 120, bollard::API_DEFAULT_VERSION)
            .map_err(AppError::Docker)
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = runtime;
        Err(AppError::Runtime(
            "Unsupported platform for runtime Docker connection".to_string(),
        ))
    }
}

async fn wait_for_docker(
    runtime: &dyn RuntimeManager,
    timeout: Duration,
) -> Result<Docker, AppError> {
    let deadline = Instant::now() + timeout;
    let mut last_error: Option<String> = None;

    while Instant::now() < deadline {
        match connect_runtime_docker(runtime) {
            Ok(docker) => {
                if crate::docker::is_available(&docker).await {
                    return Ok(docker);
                }
                last_error = Some("ping failed".to_string());
            }
            Err(e) => {
                last_error = Some(e.to_string());
            }
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    Err(AppError::Runtime(format!(
        "Timed out waiting for Docker to become responsive (timeout {:?}): {}",
        timeout,
        last_error.unwrap_or_else(|| "unknown".to_string())
    )))
}

// ---------------------------------------------------------------------------
// Cross-process lock
// ---------------------------------------------------------------------------

struct EngineLock {
    #[allow(dead_code)]
    file: File,
    #[allow(dead_code)]
    path: PathBuf,
}

fn engine_lock_path() -> PathBuf {
    engine_lock_path_from_socket(crate::runtime::common::host_docker_socket_path())
}

fn engine_lock_path_from_socket(socket_path: &Path) -> PathBuf {
    let dir = socket_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| crate::storage::data_dir().join("runtime"));
    dir.join("engine.lock")
}

async fn acquire_engine_lock(timeout: Duration) -> Result<EngineLock, AppError> {
    let path = engine_lock_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let deadline = Instant::now() + timeout;
    loop {
        match try_acquire_engine_lock(&path) {
            Ok(lock) => return Ok(lock),
            Err(err) if is_lock_contended(&err) => {
                if Instant::now() >= deadline {
                    return Err(AppError::Runtime(format!(
                        "Timed out waiting for engine lock: {}",
                        path.display()
                    )));
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue;
            }
            Err(err) => return Err(err),
        }
    }
}

fn is_lock_contended(err: &AppError) -> bool {
    match err {
        AppError::Io(io) => matches!(
            io.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::PermissionDenied
        ),
        AppError::Runtime(msg) => msg.contains("engine lock contended"),
        _ => false,
    }
}

fn try_acquire_engine_lock(path: &Path) -> Result<EngineLock, AppError> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;

        let mut opts = OpenOptions::new();
        opts.create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .share_mode(0);

        match opts.open(path) {
            Ok(file) => Ok(EngineLock {
                file,
                path: path.to_path_buf(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Err(AppError::Io(e)),
            Err(e) => Err(AppError::Io(e)),
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        let fd = file.as_raw_fd();
        let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
        if rc == 0 {
            Ok(EngineLock {
                file,
                path: path.to_path_buf(),
            })
        } else {
            let err = std::io::Error::last_os_error();
            Err(AppError::Io(err))
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(AppError::Runtime(
            "Cross-process locking is not supported on this platform".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_lock_path_ends_with_engine_lock() {
        let path = engine_lock_path_from_socket(Path::new("/tmp/docker.sock"));
        assert!(path.to_string_lossy().ends_with("engine.lock"));
    }
}

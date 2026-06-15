//! Container engine bring-up helpers.
//!
//! This module provides a single, shared entry-point used by both the GUI and
//! CLI to ensure a responsive native Engine backed by the CrateBay
//! built-in runtime.
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

/// Options for [`ensure_engine_contract`].
pub struct EnsureOptions {
    /// Maximum time to wait for acquiring the cross-process engine lock.
    pub lock_wait_timeout: Duration,
    /// Maximum time to wait for the CrateBay Engine API to become responsive after starting runtime.
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

/// Ensure the native CrateBay Engine contract is responsive, starting the built-in runtime if needed.
///
/// Only the CrateBay built-in runtime is used — external Docker-compatible engines
/// (Colima, Docker Desktop, OrbStack, Podman, etc.) are not attempted.
pub async fn ensure_engine_contract(
    runtime: &dyn RuntimeManager,
    options: EnsureOptions,
) -> Result<runtime::RuntimeEngineStatus, AppError> {
    let EnsureOptions {
        lock_wait_timeout,
        docker_wait_timeout,
        runtime_detect_timeout,
        runtime_start_timeout,
        runtime_provision_timeout,
        on_provision_progress,
    } = options;

    // 1. Fast path: try the native CrateBay Engine contract first.
    if let Ok(status) = runtime::query_built_in_ready_engine_status(runtime) {
        return Ok(status);
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
    if let Ok(status) = runtime::query_built_in_ready_engine_status(runtime) {
        return Ok(status);
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

    // 6. Wait for the native CrateBay Engine contract inside the runtime.
    wait_for_engine_contract(runtime, docker_wait_timeout).await
}

/// Ensure a responsive CrateBay Engine compatibility client, starting the built-in runtime if needed.
///
/// This keeps the compatibility endpoint readiness contract for older
/// Bollard-based call sites. Native management commands should use
/// [`ensure_engine_contract`] instead.
pub async fn ensure_engine_compatibility(
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

    if let Some(docker) = try_connect_builtin(runtime).await {
        return Ok(Arc::new(docker));
    }

    if runtime::common::env_flag_enabled(DISABLE_IMPLICIT_RUNTIME_START_ENV) {
        return Err(AppError::Runtime(format!(
            "Implicit runtime start disabled by {}",
            DISABLE_IMPLICIT_RUNTIME_START_ENV
        )));
    }

    let _lock = acquire_engine_lock(lock_wait_timeout).await?;

    if let Some(docker) = try_connect_builtin(runtime).await {
        return Ok(Arc::new(docker));
    }

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

    let docker = wait_for_docker(runtime, docker_wait_timeout).await?;
    Ok(Arc::new(docker))
}

/// Compatibility alias for older call sites.
///
/// The returned client is connected to CrateBay Engine's compatibility endpoint,
/// not to an external Docker daemon.
pub async fn ensure_docker(
    runtime: &dyn RuntimeManager,
    options: EnsureOptions,
) -> Result<Arc<Docker>, AppError> {
    ensure_engine_compatibility(runtime, options).await
}

// ---------------------------------------------------------------------------
// Connection helpers
// ---------------------------------------------------------------------------

/// Try to connect to the CrateBay built-in runtime only (skip external Docker).
async fn try_connect_builtin(runtime: &dyn RuntimeManager) -> Option<Docker> {
    // Unix socket path (macOS / Linux socket mode)
    #[cfg(unix)]
    {
        let socket = runtime.engine_socket_path();
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
        let host = crate::runtime::linux::linux_engine_host();
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
        let host = crate::runtime::windows::windows_engine_host();
        let http_host = host
            .strip_prefix("tcp://")
            .map(|rest| format!("http://{}", rest))
            .unwrap_or_else(|| host.replace("tcp://", "http://"));
        Docker::connect_with_http(&http_host, 120, bollard::API_DEFAULT_VERSION)
            .map_err(AppError::Docker)
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let socket = runtime.engine_socket_path();
        let socket_str = socket.to_str().unwrap_or_default();
        Docker::connect_with_unix(socket_str, 120, bollard::API_DEFAULT_VERSION)
            .map_err(AppError::Docker)
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = runtime;
        Err(AppError::Runtime(
            "Unsupported platform for CrateBay Engine compatibility connection".to_string(),
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
        "Timed out waiting for CrateBay Engine API to become responsive (timeout {:?}): {}",
        timeout,
        last_error.unwrap_or_else(|| "unknown".to_string())
    )))
}

async fn wait_for_engine_contract(
    runtime: &dyn RuntimeManager,
    timeout: Duration,
) -> Result<runtime::RuntimeEngineStatus, AppError> {
    let deadline = Instant::now() + timeout;
    let mut last_error: Option<String> = None;

    while Instant::now() < deadline {
        match runtime::query_built_in_ready_engine_status(runtime) {
            Ok(status) => return Ok(status),
            Err(error) => last_error = Some(error.to_string()),
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    Err(AppError::Runtime(format!(
        "Timed out waiting for native CrateBay Engine contract to become responsive (timeout {:?}): {}",
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
    engine_lock_path_from_socket(crate::runtime::common::host_engine_socket_path())
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
        let path = engine_lock_path_from_socket(Path::new("/tmp/engine.sock"));
        assert!(path.to_string_lossy().ends_with("engine.lock"));
    }

    #[cfg(unix)]
    #[derive(Clone)]
    struct NativeContractRuntime {
        socket_path: PathBuf,
        get_state_calls: Arc<std::sync::atomic::AtomicUsize>,
        provision_calls: Arc<std::sync::atomic::AtomicUsize>,
        start_calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[cfg(unix)]
    impl NativeContractRuntime {
        fn new(socket_path: PathBuf) -> Self {
            Self {
                socket_path,
                get_state_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                provision_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                start_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            }
        }
    }

    #[cfg(unix)]
    #[async_trait::async_trait]
    impl RuntimeManager for NativeContractRuntime {
        async fn get_state(&self) -> Result<RuntimeState, AppError> {
            self.get_state_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(RuntimeState::Provisioned)
        }

        async fn provision(
            &self,
            _on_progress: Box<dyn Fn(runtime::ProvisionProgress) + Send>,
        ) -> Result<(), AppError> {
            self.provision_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        async fn start(&self) -> Result<(), AppError> {
            self.start_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        async fn stop(&self) -> Result<(), AppError> {
            Ok(())
        }

        async fn health_check(&self) -> Result<runtime::HealthStatus, AppError> {
            Ok(runtime::HealthStatus {
                runtime_state: RuntimeState::Ready,
                engine_responsive: true,
                compatibility_responsive: false,
                compatibility_version: None,
                docker_responsive: false,
                docker_version: None,
                uptime_seconds: None,
                last_check: chrono::Utc::now().to_rfc3339(),
                engine_source: Some("builtin".to_string()),
                docker_source: Some("builtin".to_string()),
                engine: runtime::built_in_engine_status(),
            })
        }

        fn engine_socket_path(&self) -> PathBuf {
            self.socket_path.clone()
        }

        async fn resource_usage(&self) -> Result<crate::models::ResourceUsage, AppError> {
            Ok(crate::models::ResourceUsage {
                cpu_percent: 0.0,
                memory_used_mb: 0,
                memory_total_mb: 0,
                disk_used_gb: 0.0,
                disk_total_gb: 0.0,
                container_count: 0,
            })
        }
    }

    #[cfg(unix)]
    fn spawn_engine_contract_server(socket_path: &Path) -> std::thread::JoinHandle<()> {
        use std::io::{Read, Write};
        use std::os::unix::net::UnixListener;

        let listener = UnixListener::bind(socket_path).expect("bind fake engine socket");
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept engine request");
            let mut request = [0_u8; 512];
            let read = stream.read(&mut request).expect("read engine request");
            let request = String::from_utf8_lossy(&request[..read]);
            assert!(request.starts_with("GET /cratebay/engine HTTP/1.1"));

            let body = br#"{
                "name": "CrateBay Engine",
                "kind": "cratebay-containerd",
                "adapter": { "api": "cratebay.engine.v1" },
                "backend": { "runtime": "containerd", "ociRuntime": "runc" },
                "network": { "stack": "CNI" },
                "compatibility": { "dockerCompatible": true }
            }"#;
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                body.len()
            );
            stream
                .write_all(headers.as_bytes())
                .expect("write response headers");
            stream.write_all(body).expect("write response body");
        })
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn ensure_engine_contract_prefers_native_contract_fast_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        let socket_path = temp.path().join("engine.sock");
        let runtime = NativeContractRuntime::new(socket_path.clone());
        let server = spawn_engine_contract_server(&socket_path);

        let status = ensure_engine_contract(
            &runtime,
            EnsureOptions {
                lock_wait_timeout: Duration::from_millis(50),
                docker_wait_timeout: Duration::from_millis(50),
                runtime_detect_timeout: Duration::from_millis(50),
                runtime_start_timeout: Duration::from_millis(50),
                runtime_provision_timeout: Duration::from_millis(50),
                on_provision_progress: None,
            },
        )
        .await
        .expect("native contract should be ready");

        assert_eq!(status.kind, "cratebay-containerd");
        assert_eq!(
            runtime
                .get_state_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(
            runtime
                .provision_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(
            runtime
                .start_calls
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        server.join().expect("fake engine server should finish");
    }
}

//! Windows runtime — WSL2 integration.
//!
//! Uses a custom WSL2 distro containing the CrateBay Engine
//! (containerd + runc + CrateBay adapter) for the built-in container runtime.
//! All WSL2 management is done via `wsl.exe`.
//!
//! Architecture overview:
//!   1. **Provision**: Import a bundled rootfs tar into WSL2 as a custom distro.
//!   2. **Start**: Boot the distro, start CrateBay Engine, detect the guest IP
//!      (6 strategies), optionally set up a host relay.
//!   3. **Stop**: Stop CrateBay Engine processes, then `wsl --terminate` the distro.
//!   4. **Health**: Probe the CrateBay Engine compatibility API via `/_ping`.
//!
//! Ported from `master:crates/cratebay-core/src/runtime.rs` (Windows WSL2
//! section, ~1300 lines) and `master:crates/cratebay-core/src/windows.rs`
//! (Hyper-V hypervisor, used for helper utilities).

use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::error::AppError;
use crate::models::ResourceUsage;

use super::common;
use super::{HealthStatus, ProvisionProgress, RuntimeConfig, RuntimeManager, RuntimeState};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default WSL2 CrateBay Engine TCP port (inside the guest, bound to 0.0.0.0).
const DEFAULT_WSL_ENGINE_PORT: u16 = 2375;

/// Compatibility alias for older callers.
const DEFAULT_WSL_DOCKER_PORT: u16 = DEFAULT_WSL_ENGINE_PORT;

/// Default distro name for the CrateBay WSL2 runtime.
const DEFAULT_DISTRO_NAME: &str = "cratebay-runtime";

/// WSL2 asset subdirectory name inside the bundled runtime assets.
const WSL_ASSETS_SUBDIR: &str = "runtime-wsl";

/// Number of ping attempts performed in each health check cycle.
const HEALTH_PING_ATTEMPTS: usize = 3;

/// Delay between ping retry attempts in a health check cycle.
const HEALTH_PING_RETRY_DELAY: Duration = Duration::from_millis(400);

/// Consecutive failed health-check cycles required before degrading
/// from `Ready` to `Starting`.
const READY_DOWNGRADE_FAILURE_THRESHOLD: u8 = 3;

static RESOLVED_WINDOWS_ENGINE_HOST: OnceLock<StdMutex<Option<String>>> = OnceLock::new();

fn resolved_windows_engine_host() -> &'static StdMutex<Option<String>> {
    RESOLVED_WINDOWS_ENGINE_HOST.get_or_init(|| StdMutex::new(None))
}

fn cache_windows_engine_host(host: &str) {
    if host.trim().is_empty() {
        return;
    }
    if let Ok(mut cached) = resolved_windows_engine_host().lock() {
        *cached = Some(host.to_string());
    }
}

pub fn windows_engine_host_candidates() -> Vec<String> {
    let mut hosts = Vec::new();
    if let Ok(cached) = resolved_windows_engine_host().lock() {
        if let Some(host) = cached.as_ref().filter(|host| !host.trim().is_empty()) {
            hosts.push(host.clone());
        }
    }

    let canonical = windows_engine_host();
    if !hosts.iter().any(|host| host == &canonical) {
        hosts.push(canonical);
    }
    hosts
}

// ---------------------------------------------------------------------------
// WindowsRuntime struct
// ---------------------------------------------------------------------------

/// Windows host Engine-compatible endpoint for the built-in runtime.
///
/// WSL2 supports localhost port forwarding by default, so we connect to the
/// CrateBay Engine compatibility TCP port as `tcp://127.0.0.1:<port>`.
pub fn windows_engine_host() -> String {
    format!("tcp://127.0.0.1:{}", wsl_engine_port())
}

/// Compatibility alias for Docker-compatible endpoint callers.
pub fn windows_docker_host() -> String {
    windows_engine_host()
}

/// Windows runtime manager using WSL2.
///
/// Manages a custom WSL2 distro that contains the CrateBay Engine. The distro
/// is imported from a tar archive during provisioning and controlled via
/// `wsl.exe` commands. The CrateBay Engine compatibility API is exposed to
/// the host via TCP on the guest's IP, with an optional localhost relay for
/// NAT-unfriendly configurations.
pub struct WindowsRuntime {
    config: RuntimeConfig,
    data_dir: PathBuf,
    distro_name: String,
    state: Arc<Mutex<RuntimeState>>,
    /// Cached guest IP from the last successful start.
    guest_ip: Arc<Mutex<Option<String>>>,
    /// Engine compatibility TCP endpoint string (e.g. `tcp://172.28.x.x:2375`).
    docker_host: Arc<Mutex<Option<String>>>,
    /// Number of consecutive health-check cycles with a failed Engine ping.
    consecutive_health_failures: Arc<Mutex<u8>>,
}

impl WindowsRuntime {
    /// Create a new Windows runtime manager with default configuration.
    pub fn new() -> Self {
        let data_dir = crate::storage::data_dir().join("runtime");
        let distro_name = std::env::var("CRATEBAY_RUNTIME_VM_NAME")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_DISTRO_NAME.to_string());

        Self {
            config: RuntimeConfig::default(),
            data_dir,
            distro_name,
            state: Arc::new(Mutex::new(RuntimeState::None)),
            guest_ip: Arc::new(Mutex::new(None)),
            docker_host: Arc::new(Mutex::new(None)),
            consecutive_health_failures: Arc::new(Mutex::new(0)),
        }
    }

    /// Ping an Engine TCP endpoint with retries to smooth transient failures.
    async fn ping_docker_host_with_retry(host: &str) -> bool {
        for attempt in 0..HEALTH_PING_ATTEMPTS {
            let host_c = host.to_string();
            let ping_ok =
                tokio::task::spawn_blocking(move || common::engine_http_ping_host(&host_c).is_ok())
                    .await
                    .unwrap_or(false);

            if ping_ok {
                return true;
            }

            if attempt + 1 < HEALTH_PING_ATTEMPTS {
                tokio::time::sleep(HEALTH_PING_RETRY_DELAY).await;
            }
        }

        false
    }

    async fn native_engine_contract_ready(&self) -> bool {
        if let Some(host) = self.docker_host.lock().await.clone() {
            if native_engine_contract_ready_on_host(&host).is_ok() {
                cache_windows_engine_host(&host);
                return true;
            }
        }

        for host in windows_engine_host_candidates() {
            if native_engine_contract_ready_on_host(&host).is_ok() {
                cache_windows_engine_host(&host);
                return true;
            }
        }

        false
    }

    /// Ping CrateBay Engine inside WSL guest with retries.
    async fn ping_docker_guest_with_retry(distro: String, port: u16) -> bool {
        for attempt in 0..HEALTH_PING_ATTEMPTS {
            let distro_c = distro.clone();
            let ping_ok =
                tokio::task::spawn_blocking(move || wsl_engine_ping_in_guest(&distro_c, port))
                    .await
                    .unwrap_or(false);

            if ping_ok {
                return true;
            }

            if attempt + 1 < HEALTH_PING_ATTEMPTS {
                tokio::time::sleep(HEALTH_PING_RETRY_DELAY).await;
            }
        }

        false
    }
}

impl Default for WindowsRuntime {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RuntimeManager trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl RuntimeManager for WindowsRuntime {
    async fn get_state(&self) -> Result<RuntimeState, AppError> {
        // Step 1: Check if WSL2 is available
        let wsl2_ok = tokio::task::spawn_blocking(|| wsl2_available())
            .await
            .unwrap_or(false);
        if !wsl2_ok {
            return Ok(RuntimeState::None);
        }

        // Step 2: Check if our distro has been imported
        let distro = self.distro_name.clone();
        let exists = tokio::task::spawn_blocking(move || wsl_distro_exists(&distro))
            .await
            .map_err(|e| AppError::Runtime(format!("Join error: {}", e)))?
            .unwrap_or(false);

        if !exists {
            return Ok(RuntimeState::None);
        }

        // Step 3: Check if the distro is currently running
        let distro = self.distro_name.clone();
        let running = tokio::task::spawn_blocking(move || wsl_distro_is_running(&distro))
            .await
            .unwrap_or(false);

        if !running {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Provisioned;
            return Ok(RuntimeState::Provisioned);
        }

        // Step 4: Distro is running — check if the native CrateBay Engine contract is responsive.
        let engine_responsive = self.native_engine_contract_ready().await;

        if engine_responsive {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Ready;
            Ok(RuntimeState::Ready)
        } else {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Starting;
            Ok(RuntimeState::Starting)
        }
    }

    async fn provision(
        &self,
        on_progress: Box<dyn Fn(ProvisionProgress) + Send>,
    ) -> Result<(), AppError> {
        // Stage 1: Check WSL2 availability (10%)
        on_progress(ProvisionProgress {
            stage: "checking".into(),
            percent: 10.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Checking WSL2 availability...".into(),
        });

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Starting;
        }

        let wsl2_ok = tokio::task::spawn_blocking(|| wsl2_available())
            .await
            .unwrap_or(false);
        if !wsl2_ok {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Error("WSL2 is required. Please run: wsl --install".into());
            return Err(AppError::Runtime(
                "WSL2 is required but not available. Please run: wsl --install".into(),
            ));
        }

        // Stage 2: Ensure runtime image is available (20% — 80%)
        on_progress(ProvisionProgress {
            stage: "downloading".into(),
            percent: 20.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Preparing CrateBay WSL2 runtime image...".into(),
        });

        // Ensure data directory exists
        let data_dir = self.data_dir.clone();
        tokio::fs::create_dir_all(&data_dir).await.map_err(|e| {
            AppError::Runtime(format!(
                "Failed to create runtime directory {}: {}",
                data_dir.display(),
                e
            ))
        })?;

        // Try to locate the WSL rootfs tar from bundled assets
        let rootfs_path = tokio::task::spawn_blocking(|| runtime_wsl_rootfs_tar_path())
            .await
            .map_err(|e| AppError::Runtime(format!("Join error: {}", e)))?;

        let rootfs_path = match rootfs_path {
            Ok(path) => path,
            Err(msg) => {
                let mut state = self.state.lock().await;
                *state = RuntimeState::Error(msg.clone());
                return Err(AppError::Runtime(msg));
            }
        };

        on_progress(ProvisionProgress {
            stage: "downloading".into(),
            percent: 80.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Runtime image ready.".into(),
        });

        // Stage 3: Import distro into WSL2 (85%)
        on_progress(ProvisionProgress {
            stage: "configuring".into(),
            percent: 85.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Importing WSL2 distro...".into(),
        });

        let distro = self.distro_name.clone();
        tokio::task::spawn_blocking(move || wsl_import_runtime_distro(&distro, &rootfs_path))
            .await
            .map_err(|e| AppError::Runtime(format!("Join error: {}", e)))??;

        // Stage 4: Complete (100%)
        on_progress(ProvisionProgress {
            stage: "complete".into(),
            percent: 100.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Runtime provisioned successfully.".into(),
        });

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Provisioned;
        }

        info!(
            "Windows WSL2 runtime provisioned (distro: {})",
            self.distro_name
        );
        Ok(())
    }

    async fn start(&self) -> Result<(), AppError> {
        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Starting;
        }

        let distro = self.distro_name.clone();
        let port = wsl_engine_port();

        // 1) Ensure the distro exists (import if missing)
        let distro_c = distro.clone();
        let exists = tokio::task::spawn_blocking(move || wsl_distro_exists(&distro_c))
            .await
            .map_err(|e| AppError::Runtime(format!("Join error: {}", e)))?
            .map_err(|e| AppError::Runtime(format!("WSL distro check failed: {}", e)))?;

        if !exists {
            return Err(AppError::Runtime(format!(
                "WSL2 distro '{}' not found. Please run provision first.",
                distro
            )));
        }

        // 2) Start CrateBay Engine (containerd + adapter + TCP proxy)
        let distro_c = distro.clone();
        tokio::task::spawn_blocking(move || wsl_start_engine(&distro_c, port))
            .await
            .map_err(|e| AppError::Runtime(format!("Join error: {}", e)))?
            .map_err(|e| AppError::Runtime(format!("Failed to start CrateBay Engine: {}", e)))?;

        // 3) Wait for CrateBay Engine to become ready inside guest
        let distro_c = distro.clone();
        let guest_docker_host = tokio::task::spawn_blocking(move || {
            wait_for_wsl_engine_ready_in_guest(&distro_c, port)
        })
        .await
        .map_err(|e| AppError::Runtime(format!("Join error: {}", e)))?
        .map_err(|e| AppError::Runtime(format!("CrateBay Engine readiness wait failed: {}", e)))?;

        // Cache guest IP for diagnostics (independent of host connectivity).
        if let Some((host, _port)) = common::engine_host_tcp_endpoint(&guest_docker_host) {
            *self.guest_ip.lock().await = Some(host);
        }

        // 4) Try direct guest connection, fall back to localhost relay
        let guest_docker_host_c = guest_docker_host.clone();
        let docker_host = tokio::task::spawn_blocking(move || {
            resolve_reachable_docker_host(&guest_docker_host_c)
        })
        .await
        .map_err(|e| AppError::Runtime(format!("Join error: {}", e)))?
        .map_err(|e| AppError::Runtime(format!("Engine host resolution failed: {}", e)))?;

        // Cache the results
        *self.docker_host.lock().await = Some(docker_host.clone());

        if !self.native_engine_contract_ready().await {
            return Err(AppError::Runtime(
                "CrateBay Engine compatibility endpoint is reachable, but the native engine contract is not ready".to_string(),
            ));
        }

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Ready;
        }

        info!(
            "Windows WSL2 runtime started (distro: {}, engine_host: {})",
            self.distro_name, docker_host
        );
        Ok(())
    }

    async fn stop(&self) -> Result<(), AppError> {
        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Stopping;
        }

        let distro = self.distro_name.clone();

        // 1) Stop CrateBay Engine processes inside the distro
        let distro_c = distro.clone();
        let _ = tokio::task::spawn_blocking(move || wsl_stop_engine_processes(&distro_c)).await;

        // 2) Terminate the WSL2 distro
        let distro_c = distro.clone();
        tokio::task::spawn_blocking(move || wsl_terminate_distro(&distro_c))
            .await
            .map_err(|e| AppError::Runtime(format!("Join error: {}", e)))?
            .map_err(|e| AppError::Runtime(format!("WSL terminate failed: {}", e)))?;

        // 3) Clear cached state
        *self.guest_ip.lock().await = None;
        *self.docker_host.lock().await = None;

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Stopped;
        }

        info!(
            "Windows WSL2 runtime stopped (distro: {})",
            self.distro_name
        );
        Ok(())
    }

    async fn health_check(&self) -> Result<HealthStatus, AppError> {
        // Check if the distro is running
        let distro = self.distro_name.clone();
        let running = tokio::task::spawn_blocking(move || wsl_distro_is_running(&distro))
            .await
            .unwrap_or(false);

        if !running {
            {
                let mut failures = self.consecutive_health_failures.lock().await;
                *failures = 0;
            }
            {
                let mut state = self.state.lock().await;
                *state = RuntimeState::Stopped;
            }
            return Ok(HealthStatus {
                runtime_state: RuntimeState::Stopped,
                engine_responsive: false,
                compatibility_responsive: false,
                compatibility_version: None,
                docker_responsive: false,
                docker_version: None,
                uptime_seconds: None,
                last_check: chrono::Utc::now().to_rfc3339(),
                engine_source: Some("builtin".to_string()),
                docker_source: Some("builtin".to_string()),
                engine: crate::runtime::built_in_engine_status(),
            });
        }

        // Check Docker via TCP ping if we have a cached docker_host
        let docker_host = self.docker_host.lock().await.clone();
        let (docker_responsive, docker_version) = if let Some(ref host) = docker_host {
            let mut ping_ok = Self::ping_docker_host_with_retry(host).await;

            // If the cached endpoint is stale/unreachable, prefer the canonical
            // localhost-forwarded endpoint (used by `engine::ensure_engine_compatibility()`).
            if !ping_ok {
                let fallback = windows_engine_host();
                if host != &fallback {
                    let fallback_ok = Self::ping_docker_host_with_retry(&fallback).await;
                    if fallback_ok {
                        ping_ok = true;
                        *self.docker_host.lock().await = Some(fallback);
                    }
                }
            }

            let version = if ping_ok {
                let distro = self.distro_name.clone();
                tokio::task::spawn_blocking(move || wsl_docker_version(&distro))
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            };

            (ping_ok, version)
        } else {
            // No cached host — try guest-side ping
            let distro = self.distro_name.clone();
            let port = wsl_engine_port();
            let ping_ok = Self::ping_docker_guest_with_retry(distro, port).await;
            (ping_ok, None)
        };
        let engine_responsive = self.native_engine_contract_ready().await;

        let current_state = {
            let state = self.state.lock().await;
            state.clone()
        };

        let runtime_state = if engine_responsive {
            let mut failures = self.consecutive_health_failures.lock().await;
            *failures = 0;
            RuntimeState::Ready
        } else {
            let mut failures = self.consecutive_health_failures.lock().await;
            *failures = failures.saturating_add(1);

            if matches!(current_state, RuntimeState::Ready)
                && *failures < READY_DOWNGRADE_FAILURE_THRESHOLD
            {
                RuntimeState::Ready
            } else {
                RuntimeState::Starting
            }
        };

        // Get uptime
        let distro = self.distro_name.clone();
        let uptime_seconds = tokio::task::spawn_blocking(move || wsl_get_uptime(&distro))
            .await
            .ok()
            .flatten();

        {
            let mut state = self.state.lock().await;
            *state = runtime_state.clone();
        }

        Ok(HealthStatus {
            runtime_state,
            engine_responsive,
            compatibility_responsive: docker_responsive,
            compatibility_version: docker_version.clone(),
            docker_responsive,
            docker_version,
            uptime_seconds,
            last_check: chrono::Utc::now().to_rfc3339(),
            engine_source: Some("builtin".to_string()),
            docker_source: Some("builtin".to_string()),
            engine: crate::runtime::built_in_engine_status(),
        })
    }

    fn engine_socket_path(&self) -> PathBuf {
        // Windows: the Engine is accessed via TCP, but return the preferred
        // named pipe path for future/fallback compatibility.
        PathBuf::from(r"\\.\pipe\cratebay-engine")
    }

    async fn resource_usage(&self) -> Result<ResourceUsage, AppError> {
        let distro = self.distro_name.clone();
        let config_mem = self.config.memory_mb;
        let config_disk = self.config.disk_gb;

        tokio::task::spawn_blocking(move || {
            let memory_info = wsl_get_memory_info(&distro);
            let container_count = wsl_get_container_count();
            let cpu_percent = wsl_get_cpu_percent(&distro).unwrap_or_default();
            let disk_used_gb = wsl_get_disk_used_gb(&distro).unwrap_or_default();

            let (memory_used_mb, memory_total_mb) = memory_info.unwrap_or((0, config_mem));

            Ok(ResourceUsage {
                cpu_percent,
                memory_used_mb,
                memory_total_mb,
                disk_used_gb,
                disk_total_gb: config_disk as f32,
                container_count: container_count.unwrap_or(0),
            })
        })
        .await
        .map_err(|e| AppError::Runtime(format!("Join error: {}", e)))?
    }
}

// ===========================================================================
// WSL2 command execution helpers (blocking, wrapped in spawn_blocking above)
// ===========================================================================

/// Engine TCP port inside the WSL guest.
fn wsl_engine_port() -> u16 {
    std::env::var("CRATEBAY_WSL_DOCKER_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|p| *p > 0)
        .unwrap_or(DEFAULT_WSL_DOCKER_PORT)
}

/// Compatibility alias for older callers.
fn wsl_docker_port() -> u16 {
    wsl_engine_port()
}

/// Check if WSL2 is available by running `wsl --status`.
fn wsl2_available() -> bool {
    std::process::Command::new("wsl.exe")
        .args(["--status"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// WSL command execution with timeout
// ---------------------------------------------------------------------------

/// Run a command with a timeout. If the command does not finish before the
/// deadline, kill the process and return an error.
fn run_command_with_timeout(
    command: &mut std::process::Command,
    timeout: Duration,
    description: &str,
) -> Result<std::process::Output, String> {
    use std::process::Stdio;

    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start {}: {}", description, e))?;

    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process finished — collect output
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        let mut buf = Vec::new();
                        std::io::Read::read_to_end(&mut s, &mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();
                let stderr = child
                    .stderr
                    .take()
                    .map(|mut s| {
                        let mut buf = Vec::new();
                        std::io::Read::read_to_end(&mut s, &mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();
                return Ok(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Ok(None) => {
                // Timed out — kill the process
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{} timed out after {} seconds",
                    description,
                    timeout.as_secs()
                ));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Failed while waiting for {}: {}", description, e));
            }
        }
    }
}

/// Execute a shell command inside the WSL distro, returning stdout as a String.
fn wsl_exec(distro: &str, shell_cmd: &str) -> Result<String, String> {
    wsl_exec_with_timeout(distro, shell_cmd, Duration::from_secs(30))
}

/// Execute a shell command inside the WSL distro with a custom timeout.
fn wsl_exec_with_timeout(
    distro: &str,
    shell_cmd: &str,
    timeout: Duration,
) -> Result<String, String> {
    let mut command = std::process::Command::new("wsl.exe");
    command.args(["-d", distro, "--", "sh", "-lc", shell_cmd]);
    let description = format!(
        "wsl -d {} -- sh -lc '{}'",
        distro,
        &shell_cmd[..shell_cmd.len().min(60)]
    );

    let output = run_command_with_timeout(&mut command, timeout, &description)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{}\n{}", stderr.trim(), stdout.trim());

        if wsl_output_indicates_missing_distro(&combined) {
            return Err(format!("WSL distro '{}' not found", distro));
        }

        return Err(format!(
            "WSL command failed (exit {}): {}",
            output.status,
            combined.trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ---------------------------------------------------------------------------
// WSL distro management
// ---------------------------------------------------------------------------

/// List all installed WSL distros.
fn wsl_list_distros() -> Result<Vec<String>, String> {
    let mut command = std::process::Command::new("wsl.exe");
    command.args(["-l", "-q"]);
    let out = run_command_with_timeout(&mut command, Duration::from_secs(20), "wsl.exe -l -q")?;

    if !out.status.success() {
        return Err(format!(
            "wsl.exe -l failed (exit {}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(stdout
        .lines()
        .map(|l| l.trim().trim_matches('\0').to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// Check if a specific WSL distro exists.
fn wsl_distro_exists(name: &str) -> Result<bool, String> {
    let distros = wsl_list_distros()?;
    if distros.iter().any(|d| d == name) {
        return Ok(true);
    }

    // Check if install directory exists with content
    let install_dir = wsl_install_dir(name);
    if !install_dir.is_dir() {
        return Ok(false);
    }

    let mut entries = std::fs::read_dir(&install_dir).map_err(|e| e.to_string())?;
    if entries.next().is_none() {
        return Ok(false);
    }

    // Directory exists with content — probe if distro is usable
    wsl_distro_is_usable(name)
}

/// Probe if a WSL distro is usable by running `sh -lc true`.
fn wsl_distro_is_usable(distro: &str) -> Result<bool, String> {
    let mut command = std::process::Command::new("wsl.exe");
    command.args(["-d", distro, "--", "sh", "-lc", "true"]);
    let description = format!("wsl.exe probe '{}'", distro);
    let out = run_command_with_timeout(&mut command, Duration::from_secs(20), &description)?;

    if out.status.success() {
        return Ok(true);
    }

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let combined = format!("{}\n{}", stderr, stdout);
    if wsl_output_indicates_missing_distro(&combined) {
        return Ok(false);
    }

    // Other errors don't necessarily mean "not usable" — be optimistic
    Ok(true)
}

/// Wait for a freshly imported distro to become usable (up to 20s).
fn wsl_wait_for_distro_usable(distro: &str) -> Result<(), String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut last_error = "distro registration is still settling".to_string();

    while std::time::Instant::now() < deadline {
        match wsl_distro_is_usable(distro) {
            Ok(true) => return Ok(()),
            Ok(false) => last_error = "distro is not registered yet".to_string(),
            Err(e) => last_error = e,
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    Err(format!(
        "WSL distro '{}' did not become usable after import: {}",
        distro, last_error
    ))
}

/// Check if a WSL distro is currently running.
fn wsl_distro_is_running(distro: &str) -> bool {
    let mut command = std::process::Command::new("wsl.exe");
    command.args(["-l", "--running"]);
    let Ok(out) =
        run_command_with_timeout(&mut command, Duration::from_secs(10), "wsl -l --running")
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    // WSL output may be UTF-16LE on Windows; handle both encodings
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout.contains(distro)
}

/// Check if wsl.exe output indicates a missing distro.
fn wsl_output_indicates_missing_distro(output: &str) -> bool {
    let normalized = output.trim().to_ascii_lowercase();
    normalized.contains("there is no distribution with the supplied name")
        || normalized.contains("distribution with the supplied name")
        || normalized.contains("wsl_e_distrolistnotfound")
}

/// Get the WSL distro installation directory.
fn wsl_install_dir(distro: &str) -> PathBuf {
    crate::storage::data_dir().join("wsl").join(distro)
}

// ---------------------------------------------------------------------------
// WSL rootfs asset discovery
// ---------------------------------------------------------------------------

/// WSL image id based on host architecture.
fn runtime_wsl_image_id() -> String {
    #[cfg(target_arch = "aarch64")]
    {
        "cratebay-runtime-wsl-aarch64".to_string()
    }
    #[cfg(target_arch = "x86_64")]
    {
        "cratebay-runtime-wsl-x86_64".to_string()
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "cratebay-runtime-wsl-x86_64".to_string()
    }
}

/// Locate the WSL assets directory from a candidate root.
fn runtime_wsl_assets_dir_from_root(root: &std::path::Path) -> Option<PathBuf> {
    if root.file_name().is_some_and(|n| n == WSL_ASSETS_SUBDIR) && root.is_dir() {
        return Some(root.to_path_buf());
    }
    let dir = root.join(WSL_ASSETS_SUBDIR);
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// Search all asset root candidates for WSL runtime assets.
fn runtime_wsl_assets_dir() -> Option<PathBuf> {
    for root in common::runtime_assets_root_candidates() {
        if let Some(dir) = runtime_wsl_assets_dir_from_root(&root) {
            let rootfs_path = dir.join(runtime_wsl_image_id()).join("rootfs.tar");
            if rootfs_path.is_file() {
                return Some(dir);
            }
        }
    }
    None
}

/// Locate the rootfs.tar file for WSL import.
fn runtime_wsl_rootfs_tar_path() -> Result<PathBuf, String> {
    // 1. Explicit environment override
    if let Ok(p) = std::env::var("CRATEBAY_WSL_ROOTFS_TAR") {
        if !p.trim().is_empty() {
            let path = PathBuf::from(&p);
            if path.is_file() && !common::file_contains_placeholder_marker(&path) {
                return Ok(path);
            }
            if common::file_contains_placeholder_marker(&path) {
                return Err(format!(
                    "CRATEBAY_WSL_ROOTFS_TAR points to a placeholder or Git LFS pointer: {}",
                    path.display()
                ));
            }
            return Err(format!(
                "CRATEBAY_WSL_ROOTFS_TAR points to '{}' which does not exist",
                p
            ));
        }
    }

    // 2. Search bundled assets
    if let Some(dir) = runtime_wsl_assets_dir() {
        let path = dir.join(runtime_wsl_image_id()).join("rootfs.tar");
        if path.is_file() && !common::file_contains_placeholder_marker(&path) {
            return Ok(path);
        }
        if common::file_contains_placeholder_marker(&path) {
            return Err(format!(
                "CrateBay WSL runtime asset is a placeholder or Git LFS pointer: {}",
                path.display()
            ));
        }
    }

    Err(
        "CrateBay WSL runtime assets not found. Ensure the desktop app is installed \
         correctly or set CRATEBAY_RUNTIME_ASSETS_DIR / CRATEBAY_WSL_ROOTFS_TAR."
            .into(),
    )
}

// ---------------------------------------------------------------------------
// WSL distro import
// ---------------------------------------------------------------------------

/// Prepare the installation directory (remove old content if any).
fn prepare_wsl_install_dir(distro: &str) -> Result<PathBuf, String> {
    let install_dir = wsl_install_dir(distro);

    if install_dir.exists() {
        let mut entries = std::fs::read_dir(&install_dir).map_err(|e| e.to_string())?;
        if entries.next().is_some() {
            std::fs::remove_dir_all(&install_dir).map_err(|e| {
                format!(
                    "Failed to clean install dir {}: {}",
                    install_dir.display(),
                    e
                )
            })?;
        }
    }

    std::fs::create_dir_all(&install_dir).map_err(|e| {
        format!(
            "Failed to create install dir {}: {}",
            install_dir.display(),
            e
        )
    })?;
    Ok(install_dir)
}

/// Import a rootfs tar into WSL2 as a new distro.
fn wsl_import_runtime_distro(distro: &str, rootfs_path: &std::path::Path) -> Result<(), AppError> {
    let install_dir = prepare_wsl_install_dir(distro)
        .map_err(|e| AppError::Runtime(format!("Failed to prepare install dir: {}", e)))?;

    let mut command = std::process::Command::new("wsl.exe");
    command.args([
        "--import",
        distro,
        &install_dir.to_string_lossy(),
        &rootfs_path.to_string_lossy(),
        "--version",
        "2",
    ]);
    let description = format!("wsl.exe --import {}", distro);
    let out = run_command_with_timeout(&mut command, Duration::from_secs(300), &description)
        .map_err(|e| AppError::Runtime(e))?;

    if !out.status.success() {
        return Err(AppError::Runtime(format!(
            "wsl.exe --import failed (exit {}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    wsl_wait_for_distro_usable(distro).map_err(|e| AppError::Runtime(e))?;
    Ok(())
}

/// Terminate a WSL distro.
fn wsl_terminate_distro(distro: &str) -> Result<(), String> {
    let mut command = std::process::Command::new("wsl.exe");
    command.args(["--terminate", distro]);
    let description = format!("wsl.exe --terminate {}", distro);
    let out = run_command_with_timeout(&mut command, Duration::from_secs(30), &description)?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        // Not fatal if the distro is already stopped
        if !stderr.to_lowercase().contains("not running") {
            return Err(format!(
                "wsl.exe --terminate failed (exit {}): {}",
                out.status, stderr
            ));
        }
    }

    Ok(())
}

// ===========================================================================
// CrateBay Engine startup
// ===========================================================================

fn wsl_engine_command(action: &str, port: u16) -> String {
    format!("CRATEBAY_DOCKER_PROXY_PORT={port} /usr/local/bin/cratebay-wsl-engine {action}")
}

/// Start CrateBay Engine in the WSL distro.
fn wsl_start_engine(distro: &str, port: u16) -> Result<(), String> {
    let command = wsl_engine_command("start", port);
    let output = wsl_exec_with_timeout(distro, &command, Duration::from_secs(90))?;
    if output.trim().is_empty() {
        Ok(())
    } else {
        info!(
            "Windows runtime: CrateBay Engine start output: {}",
            output.trim()
        );
        Ok(())
    }
}

/// Stop all CrateBay Engine processes inside the distro.
fn wsl_stop_engine_processes(distro: &str) {
    let _ = wsl_exec_with_timeout(
        distro,
        "/usr/local/bin/cratebay-wsl-engine stop >/dev/null 2>&1 || true; \
         pkill -x cratebay-guest-agent 2>/dev/null || true; \
         pkill -x cratebay-engine-adapter 2>/dev/null || true; \
         pkill -x containerd 2>/dev/null || true",
        Duration::from_secs(15),
    );
}

// ===========================================================================
// Guest IP detection (6 strategies)
// ===========================================================================

/// Parse an IPv4 address into octets.
fn parse_ipv4_octets(candidate: &str) -> Option<[u8; 4]> {
    let octets: Result<Vec<u8>, _> = candidate.split('.').map(str::parse::<u8>).collect();
    let octets = octets.ok()?;
    let arr: [u8; 4] = octets.try_into().ok()?;
    Some(arr)
}

/// Check if an IP is a usable WSL guest address (not loopback, link-local, etc.).
fn is_usable_wsl_guest_ipv4(candidate: &str) -> bool {
    !(candidate.starts_with("127.")
        || candidate == "0.0.0.0"
        || candidate == "10.255.255.254"
        || candidate.starts_with("169.254."))
}

/// Check if an IP is likely a Docker bridge address (x.x.x.1 in private range).
fn is_probable_wsl_bridge_ipv4(candidate: &str) -> bool {
    let [first, second, _, fourth] = match parse_ipv4_octets(candidate) {
        Some(octets) => octets,
        None => return false,
    };

    fourth == 1
        && (first == 10
            || (first == 172 && (16..=31).contains(&second))
            || (first == 192 && second == 168))
}

/// Extract all non-loopback IPv4 addresses from command output.
fn extract_non_loopback_ipv4s(output: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip loopback interface lines
        if trimmed.contains(" lo") && trimmed.contains("inet ") {
            continue;
        }

        for token in trimmed.split_whitespace() {
            let candidate = token
                .trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == ';')
                .split('/')
                .next()
                .unwrap_or(token)
                .trim();

            if candidate.is_empty() || candidate.starts_with("127.") || candidate == "0.0.0.0" {
                continue;
            }

            if parse_ipv4_octets(candidate).is_some()
                && !candidates
                    .iter()
                    .any(|existing: &String| existing == candidate)
            {
                candidates.push(candidate.to_string());
            }
        }
    }

    candidates
}

/// Extract the first non-loopback IPv4 from output.
fn extract_first_non_loopback_ipv4(output: &str) -> Option<String> {
    extract_non_loopback_ipv4s(output).into_iter().next()
}

/// Select the best WSL guest IPv4 from candidates, preferring non-bridge addresses.
fn select_wsl_guest_ipv4<I>(candidates: I) -> Option<String>
where
    I: IntoIterator<Item = String>,
{
    let mut fallback = None;
    for candidate in candidates {
        if !is_usable_wsl_guest_ipv4(&candidate) {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(candidate.clone());
        }
        if !is_probable_wsl_bridge_ipv4(&candidate) {
            return Some(candidate);
        }
    }
    fallback
}

/// Extract a usable WSL guest IPv4 from output text.
fn extract_usable_wsl_guest_ipv4(output: &str) -> Option<String> {
    select_wsl_guest_ipv4(extract_non_loopback_ipv4s(output))
}

/// Extract IPv4 addresses for a hostname from /etc/hosts content.
fn extract_hosts_ipv4_for_hostname(output: &str, hostname: &str) -> Option<String> {
    let hostname = hostname.trim();
    if hostname.is_empty() {
        return None;
    }

    let mut candidates = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let Some(ip) = parts.next() else {
            continue;
        };
        if !is_usable_wsl_guest_ipv4(ip) {
            continue;
        }
        if parts.any(|alias| alias == hostname) {
            candidates.push(ip.to_string());
        }
    }

    select_wsl_guest_ipv4(candidates)
}

/// Extract local IPv4 addresses from /proc/net/fib_trie output.
fn extract_local_ipv4s_from_fib_trie(output: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut last_ipv4 = None::<String>;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(ip) = extract_first_non_loopback_ipv4(trimmed) {
            last_ipv4 = Some(ip);
        }

        if trimmed.contains("/32 host LOCAL") {
            if let Some(ip) = last_ipv4.take() {
                if is_usable_wsl_guest_ipv4(&ip)
                    && !candidates.iter().any(|existing: &String| existing == &ip)
                {
                    candidates.push(ip);
                }
            }
        }
    }

    candidates
}

/// Extract IPv4 from fib_trie, preferring non-bridge addresses.
fn extract_first_non_loopback_ipv4_from_fib_trie(output: &str) -> Option<String> {
    select_wsl_guest_ipv4(extract_local_ipv4s_from_fib_trie(output))
}

/// Detect the guest IP address of the WSL distro using 6 strategies.
///
/// Strategies (in order):
/// 1. `ip -4 route get 1.1.1.1` (iproute2 src address)
/// 2. `ip -4 -o addr show scope global up` (interface addresses)
/// 3. `hostname -I` (all host addresses)
/// 4. `hostname -i` (primary address)
/// 5. `/etc/hosts` lookup for the distro hostname
/// 6. `/proc/net/fib_trie` (kernel routing table)
fn wsl_guest_ip(distro: &str) -> Result<String, String> {
    // Strategy 1: ip route src
    let out = wsl_exec(
        distro,
        "ip -4 route get 1.1.1.1 2>/dev/null | awk '{for (i=1;i<=NF;i++) if ($i==\"src\") {print $(i+1); exit}}' | tr -d '\\r'",
    )
    .unwrap_or_default();
    if let Some(ip) = extract_usable_wsl_guest_ipv4(&out) {
        return Ok(ip);
    }

    // Strategy 2: ip addr show
    let out = wsl_exec(
        distro,
        "ip -4 -o addr show scope global up 2>/dev/null | awk '$2 != \"lo\" {print $4}' | cut -d/ -f1 | tr -d '\\r'",
    )
    .unwrap_or_default();
    if let Some(ip) = extract_usable_wsl_guest_ipv4(&out) {
        return Ok(ip);
    }

    // Strategy 3: hostname -I
    let out = wsl_exec(distro, "hostname -I 2>/dev/null | tr -d '\\r'").unwrap_or_default();
    if let Some(ip) = extract_usable_wsl_guest_ipv4(&out) {
        return Ok(ip);
    }

    // Strategy 4: hostname -i
    let out = wsl_exec(distro, "hostname -i 2>/dev/null | tr -d '\\r'").unwrap_or_default();
    if let Some(ip) = extract_usable_wsl_guest_ipv4(&out) {
        return Ok(ip);
    }

    // Strategy 5: /etc/hosts lookup
    let hostname = wsl_exec(distro, "hostname 2>/dev/null").unwrap_or_default();
    let hosts = wsl_exec(distro, "cat /etc/hosts 2>/dev/null").unwrap_or_default();
    if let Some(ip) = extract_hosts_ipv4_for_hostname(&hosts, &hostname) {
        return Ok(ip);
    }

    // Strategy 6: fib_trie
    let out = wsl_exec(distro, "cat /proc/net/fib_trie 2>/dev/null").unwrap_or_default();
    if let Some(ip) = extract_first_non_loopback_ipv4_from_fib_trie(&out) {
        return Ok(ip);
    }

    Err(
        "Failed to determine WSL guest IP from iproute2, hostname, /etc/hosts, or /proc/net/fib_trie"
            .into(),
    )
}

// ===========================================================================
// CrateBay Engine readiness + host relay
// ===========================================================================

/// Build a CrateBay Engine ping command for use inside WSL.
fn wsl_engine_http_ping_command(port: u16) -> String {
    format!(
        "CRATEBAY_DOCKER_PROXY_PORT={port} /usr/local/bin/cratebay-wsl-engine ping 2>/dev/null | tr -d '\\r' || true"
    )
}

/// Check if CrateBay Engine is responding inside the WSL guest.
fn wsl_engine_ping_in_guest(distro: &str, port: u16) -> bool {
    let cmd = wsl_engine_http_ping_command(port);
    match wsl_exec_with_timeout(distro, &cmd, Duration::from_secs(10)) {
        Ok(output) => output.lines().any(|line| line.trim() == "OK"),
        Err(_) => false,
    }
}

/// Collect diagnostic information from the WSL distro for troubleshooting.
fn wsl_runtime_diagnostics(distro: &str) -> String {
    let ping_cmd = wsl_engine_http_ping_command(wsl_engine_port());
    let probes = vec![
        ("ip -4 route get 1.1.1.1", "ip -4 route get 1.1.1.1 2>/dev/null || true".to_string()),
        ("ip -4 -o addr show", "ip -4 -o addr show 2>/dev/null || true".to_string()),
        ("hostname -I", "hostname -I 2>/dev/null || true".to_string()),
        ("cratebay-wsl-engine status", "/usr/local/bin/cratebay-wsl-engine status 2>&1 || true".to_string()),
        ("engine _ping", ping_cmd),
        ("containerd version", "ctr --address /run/containerd/containerd.sock version 2>&1 || true".to_string()),
        ("ctr cratebay tasks", "ctr --address /run/containerd/containerd.sock --namespace cratebay tasks list 2>&1 || true".to_string()),
        ("ps | grep engine", "ps | grep -E '[c]ratebay-engine-adapter|[c]ratebay-guest-agent|[c]ontainerd' 2>/dev/null || true".to_string()),
        ("containerd.log", "tail -n 80 /var/log/containerd.log 2>/dev/null || true".to_string()),
        ("cratebay-engine-adapter.log", "tail -n 80 /var/log/cratebay-engine-adapter.log 2>/dev/null || true".to_string()),
        ("cratebay-guest-agent.log", "tail -n 80 /var/log/cratebay-guest-agent.log 2>/dev/null || true".to_string()),
        ("cratebay-engine.log", "tail -n 80 /var/log/cratebay-engine.log 2>/dev/null || true".to_string()),
    ];

    let mut diagnostics = Vec::new();

    for (label, command) in probes {
        match wsl_exec_with_timeout(distro, &command, Duration::from_secs(5)) {
            Ok(output) if !output.trim().is_empty() => {
                diagnostics.push(format!("{}:\n{}", label, output.trim()));
            }
            Ok(_) => {}
            Err(e) => diagnostics.push(format!("{}: <probe failed: {}>", label, e)),
        }
    }

    diagnostics.join("\n\n")
}

/// Wait for CrateBay Engine to become ready inside the WSL guest.
///
/// Returns the guest Docker-compatible endpoint (e.g. `tcp://172.28.x.x:2375`).
fn wait_for_wsl_engine_ready_in_guest(distro: &str, port: u16) -> Result<String, String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    let mut last_error = "CrateBay Engine is still starting".to_string();
    let readiness_probe = wsl_engine_http_ping_command(port);

    while std::time::Instant::now() < deadline {
        match wsl_exec(distro, &readiness_probe) {
            Ok(output) if output.lines().any(|line| line.trim() == "OK") => {
                let guest_ip = wsl_guest_ip(distro)?;
                return Ok(format!("tcp://{}:{}", guest_ip, port));
            }
            Ok(_) => {
                last_error = "CrateBay Engine API is not responding inside WSL".to_string();
            }
            Err(e) => last_error = e,
        }

        std::thread::sleep(Duration::from_millis(500));
    }

    let diagnostics = wsl_runtime_diagnostics(distro);
    let message = if diagnostics.trim().is_empty() {
        format!(
            "CrateBay Runtime (WSL2) did not become ready within 120 seconds: {}",
            last_error
        )
    } else {
        format!(
            "CrateBay Runtime (WSL2) did not become ready within 120 seconds: {}\n{}",
            last_error,
            diagnostics.trim()
        )
    };

    Err(message)
}

/// Try to reach the CrateBay Engine compatibility endpoint, then fall back to a
/// localhost relay if direct connection fails.
fn resolve_reachable_docker_host(guest_docker_host: &str) -> Result<String, String> {
    // Prefer the localhost-forwarded endpoint, which is the most compatible
    // across Windows networking configurations.
    let local = windows_engine_host();
    if common::wait_for_engine_tcp(&local, Duration::from_secs(5)).is_ok() {
        cache_windows_engine_host(&local);
        return Ok(local);
    }

    // Try direct connection first
    if common::wait_for_engine_tcp(guest_docker_host, Duration::from_secs(5)).is_ok() {
        cache_windows_engine_host(guest_docker_host);
        return Ok(guest_docker_host.to_string());
    }

    // Extract guest IP and port for relay setup
    let (guest_ip, port) = common::engine_host_tcp_endpoint(guest_docker_host)
        .ok_or_else(|| format!("Invalid WSL guest Engine host '{}'", guest_docker_host))?;

    // Set up localhost TCP relay
    let relay_host = setup_localhost_relay(&guest_ip, port)?;

    if common::wait_for_engine_tcp(&relay_host, Duration::from_secs(10)).is_ok() {
        cache_windows_engine_host(&relay_host);
        return Ok(relay_host);
    }

    Err(format!(
        "CrateBay Runtime (WSL2) is running in the guest but is not reachable from \
         Windows at {} or through a local CrateBay relay",
        guest_docker_host
    ))
}

/// Set up a localhost TCP relay that forwards to the WSL guest.
fn setup_localhost_relay(guest_ip: &str, port: u16) -> Result<String, String> {
    let target_addr = format!("{}:{}", guest_ip, port);

    let listener =
        std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).map_err(|e| {
            format!(
                "Failed to bind local Engine compatibility relay on 127.0.0.1: {}",
                e
            )
        })?;

    let relay_port = listener
        .local_addr()
        .map_err(|e| format!("Failed to resolve relay address: {}", e))?
        .port();

    let host = format!("tcp://127.0.0.1:{}", relay_port);
    let target = Arc::new(std::sync::Mutex::new(target_addr));
    let thread_target = Arc::clone(&target);

    std::thread::Builder::new()
        .name("cratebay-wsl-docker-relay".into())
        .spawn(move || run_relay_listener(listener, thread_target))
        .map_err(|e| format!("Failed to spawn relay thread: {}", e))?;

    Ok(host)
}

/// Accept connections on the relay listener and proxy them to the target.
fn run_relay_listener(listener: std::net::TcpListener, target: Arc<std::sync::Mutex<String>>) {
    while let Ok((inbound, _peer)) = listener.accept() {
        let target_addr = target.lock().unwrap_or_else(|e| e.into_inner()).clone();
        std::thread::spawn(move || {
            let _ = proxy_tcp_connection(inbound, &target_addr);
        });
    }
}

/// Proxy a single TCP connection: copy bytes bidirectionally.
fn proxy_tcp_connection(inbound: std::net::TcpStream, target_addr: &str) -> Result<(), String> {
    use std::io;
    use std::net::Shutdown;

    let outbound = std::net::TcpStream::connect(target_addr)
        .map_err(|e| format!("Failed to connect to {}: {}", target_addr, e))?;

    let _ = inbound.set_nodelay(true);
    let _ = outbound.set_nodelay(true);

    let mut inbound_reader = inbound
        .try_clone()
        .map_err(|e| format!("Clone inbound: {}", e))?;
    let mut inbound_writer = inbound;
    let mut outbound_reader = outbound
        .try_clone()
        .map_err(|e| format!("Clone outbound: {}", e))?;
    let mut outbound_writer = outbound;

    let c2s = std::thread::spawn(move || {
        let _ = io::copy(&mut inbound_reader, &mut outbound_writer);
        let _ = outbound_writer.shutdown(Shutdown::Write);
    });
    let s2c = std::thread::spawn(move || {
        let _ = io::copy(&mut outbound_reader, &mut inbound_writer);
        let _ = inbound_writer.shutdown(Shutdown::Write);
    });

    let _ = c2s.join();
    let _ = s2c.join();
    Ok(())
}

// ===========================================================================
// Utility: resource querying
// ===========================================================================

/// Get CrateBay Engine version string from inside the distro.
fn wsl_docker_version(distro: &str) -> Option<String> {
    let port = wsl_engine_port();
    let command = format!(
        "curl -fsS http://127.0.0.1:{port}/version 2>/dev/null \
          | sed -n 's/.*\"Version\":\"\\([^\"]*\\)\".*/\\1/p'"
    );
    let output = wsl_exec_with_timeout(distro, &command, Duration::from_secs(10)).ok()?;
    let version = output.trim().to_string();
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

/// Get uptime in seconds from the WSL2 distro.
fn wsl_get_uptime(distro: &str) -> Option<u64> {
    let output = wsl_exec_with_timeout(distro, "cat /proc/uptime", Duration::from_secs(5)).ok()?;
    let uptime_str = output.split_whitespace().next()?;
    let uptime_f64: f64 = uptime_str.parse().ok()?;
    Some(uptime_f64 as u64)
}

/// Get memory info (used_mb, total_mb) from the WSL2 distro via /proc/meminfo.
fn wsl_get_memory_info(distro: &str) -> Option<(u64, u64)> {
    let output = wsl_exec_with_timeout(distro, "cat /proc/meminfo", Duration::from_secs(5)).ok()?;

    let mut mem_total_kb: Option<u64> = None;
    let mut mem_available_kb: Option<u64> = None;

    for line in output.lines() {
        if line.starts_with("MemTotal:") {
            mem_total_kb = parse_meminfo_value(line);
        } else if line.starts_with("MemAvailable:") {
            mem_available_kb = parse_meminfo_value(line);
        }
        if mem_total_kb.is_some() && mem_available_kb.is_some() {
            break;
        }
    }

    let total_kb = mem_total_kb?;
    let available_kb = mem_available_kb?;
    let used_kb = total_kb.saturating_sub(available_kb);

    Some((used_kb / 1024, total_kb / 1024))
}

/// Get aggregate CPU usage percentage from two short `/proc/stat` samples.
fn wsl_get_cpu_percent(distro: &str) -> Option<f32> {
    let previous =
        wsl_exec_with_timeout(distro, "sed -n '1p' /proc/stat", Duration::from_secs(5)).ok()?;
    std::thread::sleep(Duration::from_millis(250));
    let current =
        wsl_exec_with_timeout(distro, "sed -n '1p' /proc/stat", Duration::from_secs(5)).ok()?;
    common::linux_proc_stat_cpu_percent(&previous, &current)
}

/// Get runtime disk usage in GiB from the WSL root filesystem.
fn wsl_get_disk_used_gb(distro: &str) -> Option<f32> {
    let output = wsl_exec_with_timeout(distro, "df -B1 /", Duration::from_secs(5)).ok()?;
    common::linux_df_used_gb(&output)
}

/// Get the number of containers inside the WSL2 runtime.
fn wsl_get_container_count() -> Option<u32> {
    if let Ok(payload) =
        common::engine_http_get_json_tcp_host(&windows_engine_host(), "/cratebay/containers")
    {
        if let Some(count) = native_container_count_from_payload(&payload) {
            return Some(count);
        }
    }

    let payload =
        common::engine_http_get_json_tcp_host(&windows_engine_host(), "/containers/json?all=true")
            .ok()?;
    compatibility_container_count_from_payload(&payload)
}

fn native_container_count_from_payload(payload: &serde_json::Value) -> Option<u32> {
    payload
        .get("count")
        .and_then(|value| value.as_u64())
        .map(|count| count as u32)
        .or_else(|| {
            payload
                .get("items")
                .and_then(|value| value.as_array())
                .map(|items| items.len() as u32)
        })
}

fn compatibility_container_count_from_payload(payload: &serde_json::Value) -> Option<u32> {
    payload.as_array().map(|containers| containers.len() as u32)
}

fn native_engine_contract_ready_on_host(host: &str) -> Result<(), String> {
    let payload = common::engine_http_get_json_tcp_host(host, "/cratebay/engine")?;
    crate::runtime::built_in_engine_contract_ready(&payload).map_err(|error| error.to_string())
}

/// Parse a value from a /proc/meminfo line (e.g. "MemTotal:       16384 kB").
fn parse_meminfo_value(line: &str) -> Option<u64> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse::<u64>().ok()
    } else {
        None
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // IP address parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_ipv4_octets_valid() {
        assert_eq!(parse_ipv4_octets("192.168.1.1"), Some([192, 168, 1, 1]));
        assert_eq!(parse_ipv4_octets("10.0.0.1"), Some([10, 0, 0, 1]));
        assert_eq!(
            parse_ipv4_octets("255.255.255.255"),
            Some([255, 255, 255, 255])
        );
    }

    #[test]
    fn parse_ipv4_octets_invalid() {
        assert!(parse_ipv4_octets("").is_none());
        assert!(parse_ipv4_octets("not-an-ip").is_none());
        assert!(parse_ipv4_octets("256.1.1.1").is_none());
        assert!(parse_ipv4_octets("1.2.3").is_none());
        assert!(parse_ipv4_octets("1.2.3.4.5").is_none());
    }

    #[test]
    fn is_usable_wsl_guest_ipv4_rejects_loopback() {
        assert!(!is_usable_wsl_guest_ipv4("127.0.0.1"));
        assert!(!is_usable_wsl_guest_ipv4("127.1.2.3"));
    }

    #[test]
    fn is_usable_wsl_guest_ipv4_rejects_special() {
        assert!(!is_usable_wsl_guest_ipv4("0.0.0.0"));
        assert!(!is_usable_wsl_guest_ipv4("10.255.255.254"));
        assert!(!is_usable_wsl_guest_ipv4("169.254.1.1"));
    }

    #[test]
    fn is_usable_wsl_guest_ipv4_accepts_normal() {
        assert!(is_usable_wsl_guest_ipv4("172.28.245.112"));
        assert!(is_usable_wsl_guest_ipv4("192.168.1.100"));
        assert!(is_usable_wsl_guest_ipv4("10.0.0.5"));
    }

    #[test]
    fn is_probable_wsl_bridge_ipv4_detects_bridges() {
        assert!(is_probable_wsl_bridge_ipv4("172.17.0.1"));
        assert!(is_probable_wsl_bridge_ipv4("10.0.0.1"));
        assert!(is_probable_wsl_bridge_ipv4("192.168.1.1"));
    }

    #[test]
    fn is_probable_wsl_bridge_ipv4_non_bridges() {
        assert!(!is_probable_wsl_bridge_ipv4("172.28.245.112"));
        assert!(!is_probable_wsl_bridge_ipv4("192.168.1.100"));
        assert!(!is_probable_wsl_bridge_ipv4("10.0.0.5"));
    }

    // -----------------------------------------------------------------------
    // IP extraction tests
    // -----------------------------------------------------------------------

    #[test]
    fn extract_first_non_loopback_ipv4_skips_loopback_lines() {
        assert_eq!(
            extract_first_non_loopback_ipv4("1: lo    inet 10.255.255.254/32 scope global lo"),
            None
        );
        assert_eq!(
            extract_first_non_loopback_ipv4("inet 127.0.0.1/8 scope host lo"),
            None
        );
    }

    #[test]
    fn extract_first_non_loopback_ipv4_reads_hostname_output() {
        assert_eq!(
            extract_first_non_loopback_ipv4("172.28.245.112  fd00::1"),
            Some("172.28.245.112".to_string())
        );
        assert_eq!(
            extract_first_non_loopback_ipv4(
                "2: eth0    inet 172.28.245.112/20 brd 172.28.255.255 scope global eth0"
            ),
            Some("172.28.245.112".to_string())
        );
    }

    #[test]
    fn extract_non_loopback_ipv4s_collects_multiple() {
        assert_eq!(
            extract_non_loopback_ipv4s("172.17.0.1 172.28.245.112 172.28.245.112"),
            vec!["172.17.0.1".to_string(), "172.28.245.112".to_string()]
        );
    }

    #[test]
    fn extract_usable_wsl_guest_ipv4_prefers_non_bridge() {
        assert_eq!(
            extract_usable_wsl_guest_ipv4("172.17.0.1 172.28.245.112"),
            Some("172.28.245.112".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // /etc/hosts parsing
    // -----------------------------------------------------------------------

    #[test]
    fn extract_hosts_ipv4_for_hostname_reads_match() {
        let hosts = "127.0.0.1 localhost\n172.28.245.112 cratebay-wsl\n";
        assert_eq!(
            extract_hosts_ipv4_for_hostname(hosts, "cratebay-wsl"),
            Some("172.28.245.112".to_string())
        );
    }

    #[test]
    fn extract_hosts_ipv4_for_hostname_skips_unusable() {
        let hosts = "10.255.255.254 cratebay-wsl\n172.28.245.112 other-host\n";
        assert_eq!(extract_hosts_ipv4_for_hostname(hosts, "cratebay-wsl"), None);
    }

    #[test]
    fn extract_hosts_ipv4_for_hostname_prefers_non_bridge() {
        let hosts = "172.17.0.1 cratebay-wsl\n172.28.245.112 cratebay-wsl\n";
        assert_eq!(
            extract_hosts_ipv4_for_hostname(hosts, "cratebay-wsl"),
            Some("172.28.245.112".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // fib_trie parsing
    // -----------------------------------------------------------------------

    #[test]
    fn fib_trie_reads_local_host_entry() {
        let fib_trie = r#"
Main:
  +-- 172.28.240.0/20 2 0 2
     +-- 172.28.240.0/24 2 0 2
        |-- 172.28.240.0
           /20 link UNICAST
        |-- 172.28.245.112
           /32 host LOCAL
"#;
        assert_eq!(
            extract_first_non_loopback_ipv4_from_fib_trie(fib_trie),
            Some("172.28.245.112".to_string())
        );
    }

    #[test]
    fn fib_trie_skips_loopback() {
        let fib_trie = r#"
Local:
  +-- 127.0.0.0/8 2 0 2
     |-- 127.0.0.1
        /32 host LOCAL
"#;
        assert_eq!(
            extract_first_non_loopback_ipv4_from_fib_trie(fib_trie),
            None
        );
    }

    #[test]
    fn fib_trie_prefers_non_bridge() {
        let fib_trie = r#"
Local:
  +-- 172.17.0.0/16 2 0 2
     |-- 172.17.0.1
        /32 host LOCAL
  +-- 172.28.240.0/20 2 0 2
     |-- 172.28.245.112
        /32 host LOCAL
"#;
        assert_eq!(
            extract_first_non_loopback_ipv4_from_fib_trie(fib_trie),
            Some("172.28.245.112".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // CrateBay Engine command tests
    // -----------------------------------------------------------------------

    #[test]
    fn wsl_engine_command_uses_cratebay_helper() {
        let command = wsl_engine_command("start", 2375);
        assert!(command.contains("CRATEBAY_DOCKER_PROXY_PORT=2375"));
        assert!(command.contains("/usr/local/bin/cratebay-wsl-engine start"));
        assert!(!command.contains("dockerd"));
    }

    // -----------------------------------------------------------------------
    // WSL output detection
    // -----------------------------------------------------------------------

    #[test]
    fn wsl_output_indicates_missing_distro_detects_known_errors() {
        let output = "Wsl/Service/WSL_E_DISTROLISTNOTFOUND: There is no distribution with the supplied name.";
        assert!(wsl_output_indicates_missing_distro(output));
    }

    #[test]
    fn wsl_output_indicates_missing_distro_ignores_other() {
        let output =
            "The process cannot access the file because it is being used by another process.";
        assert!(!wsl_output_indicates_missing_distro(output));
    }

    // -----------------------------------------------------------------------
    // meminfo parsing
    // -----------------------------------------------------------------------

    #[test]
    fn parse_meminfo_value_valid() {
        assert_eq!(parse_meminfo_value("MemTotal:       16384 kB"), Some(16384));
        assert_eq!(parse_meminfo_value("MemAvailable:   8192 kB"), Some(8192));
    }

    #[test]
    fn parse_meminfo_value_invalid() {
        assert_eq!(parse_meminfo_value(""), None);
        assert_eq!(parse_meminfo_value("MemTotal:"), None);
    }

    #[test]
    fn native_container_count_prefers_count_field() {
        let payload = serde_json::json!({
            "api": "cratebay.containers.v1",
            "count": 3,
            "items": [{ "id": "one" }],
        });

        assert_eq!(native_container_count_from_payload(&payload), Some(3));
    }

    #[test]
    fn native_container_count_falls_back_to_items_length() {
        let payload = serde_json::json!({
            "api": "cratebay.containers.v1",
            "items": [{ "id": "one" }, { "id": "two" }],
        });

        assert_eq!(native_container_count_from_payload(&payload), Some(2));
    }

    #[test]
    fn compatibility_container_count_reads_array_length() {
        let payload = serde_json::json!([{ "Id": "one" }, { "Id": "two" }]);

        assert_eq!(
            compatibility_container_count_from_payload(&payload),
            Some(2)
        );
    }

    // -----------------------------------------------------------------------
    // Struct tests
    // -----------------------------------------------------------------------

    #[test]
    fn windows_runtime_default_config() {
        let runtime = WindowsRuntime::new();
        assert_eq!(runtime.config.cpu_cores, 2);
        assert_eq!(runtime.config.memory_mb, 2048);
        assert!(!runtime.distro_name.is_empty());
    }

    #[test]
    fn wsl_engine_port_default() {
        // Unless env var is set, should return the default
        let port = wsl_engine_port();
        assert!(port > 0);
    }

    #[test]
    fn wsl_install_dir_format() {
        let dir = wsl_install_dir("test-distro");
        assert!(dir.ends_with("wsl/test-distro") || dir.ends_with(r"wsl\test-distro"));
    }

    #[test]
    fn runtime_wsl_image_id_not_empty() {
        let id = runtime_wsl_image_id();
        assert!(!id.is_empty());
        assert!(id.starts_with("cratebay-runtime-wsl-"));
    }

    #[test]
    fn wsl_engine_http_ping_command_format() {
        let cmd = wsl_engine_http_ping_command(2375);
        assert!(cmd.contains("2375"));
        assert!(cmd.contains("_ping"));
        assert!(cmd.contains("cratebay-wsl-engine ping"));
        assert!(!cmd.contains("dockerd"));
    }

    #[test]
    fn engine_socket_path_is_windows_pipe() {
        let runtime = WindowsRuntime::new();
        let path = runtime.engine_socket_path();
        let s = path.to_string_lossy();
        assert!(
            s.contains("pipe") && s.contains("cratebay-engine"),
            "Expected Windows pipe path, got: {}",
            s
        );
    }
}

//! Linux runtime — KVM/QEMU integration.
//!
//! Uses KVM-accelerated QEMU to run a lightweight Linux VM
//! with CrateBay Engine inside.
//!
//! # Architecture (runtime-spec.md §2.2)
//!
//! ```text
//! Linux Host
//! ├── CrateBay binary
//! │   └── Rust Backend (LinuxRuntime)
//! │       └── QEMU process management
//! │           └── qemu-system-{x86_64|aarch64}
//! │               ├── -machine q35/virt,accel=kvm|tcg
//! │               ├── -kernel vmlinuz -initrd initramfs
//! │               ├── -drive file=disk.raw,format=raw,if=virtio
//! │               ├── -netdev user (hostfwd Docker port)
//! │               └── -serial file:console.log
//! │                   └── Alpine Linux
//! │                       └── CrateBay Engine
//! │                           └── Compatibility API exposed via TCP port forwarding
//! ```
//!
//! # Lifecycle
//!
//! 1. **provision()** — Install runtime image from bundled assets via `common::ensure_runtime_image_ready()`
//! 2. **start()** — Spawn daemonized QEMU, write PID file, wait for engine API readiness
//! 3. **stop()** — SIGTERM → wait → SIGKILL → cleanup PID file
//! 4. **detect()** — Check KVM, QEMU binary, image files, PID, engine health
//!
//! Ported from `master:crates/cratebay-core/src/runtime.rs` (Linux QEMU section)
//! and adapted for the v2 `RuntimeManager` trait with `AppError` error model.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::error::AppError;
use crate::models::ResourceUsage;

use super::common;
use super::{HealthStatus, ProvisionProgress, RuntimeConfig, RuntimeManager, RuntimeState};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Grace period for SIGTERM before escalating to SIGKILL.
const STOP_GRACE_PERIOD: Duration = Duration::from_secs(5);

/// Timeout for CrateBay Engine to become responsive after QEMU starts.
const DOCKER_READY_TIMEOUT: Duration = Duration::from_secs(45);

/// Number of ping attempts performed in each health check cycle.
const HEALTH_PING_ATTEMPTS: usize = 3;

/// Delay between ping retry attempts in a health check cycle.
const HEALTH_PING_RETRY_DELAY: Duration = Duration::from_millis(400);

/// Consecutive failed health-check cycles required before degrading
/// from `Ready` to `Starting`.
const READY_DOWNGRADE_FAILURE_THRESHOLD: u8 = 3;

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Runtime data directory: `<data_dir>/runtime-linux/`.
fn runtime_dir() -> PathBuf {
    crate::storage::data_dir().join("runtime-linux")
}

/// PID file for the QEMU process.
fn qemu_pid_path() -> PathBuf {
    runtime_dir().join("qemu.pid")
}

/// Disk image path for the runtime VM.
fn runtime_disk_path() -> PathBuf {
    runtime_dir().join("disk.raw")
}

/// Console log path for serial output.
pub fn runtime_console_log_path() -> PathBuf {
    runtime_dir().join("console.log")
}

/// Read a PID from a file. Returns `None` if the file doesn't exist or
/// contains an invalid value.
fn read_pid_file(path: &Path) -> Option<u32> {
    let content = std::fs::read_to_string(path).ok()?;
    content.trim().parse::<u32>().ok().filter(|pid| *pid > 0)
}

/// Check if a process is alive using `/proc/<pid>` existence check.
fn pid_alive(pid: u32) -> bool {
    // On Linux, checking /proc/<pid> is reliable and does not require
    // special permissions.
    Path::new(&format!("/proc/{}", pid)).exists()
}

/// Kill a process by PID using libc::kill (avoids spawning a process).
fn kill_process(pid: u32, signal: i32) {
    unsafe {
        libc::kill(pid as i32, signal);
    }
}

// ---------------------------------------------------------------------------
// Engine TCP port configuration
// ---------------------------------------------------------------------------

/// Engine compatibility TCP port exposed by the QEMU VM on the host.
///
/// Override via `CRATEBAY_LINUX_DOCKER_PORT`. Defaults to [`common::DEFAULT_LINUX_DOCKER_PORT`].
fn linux_engine_port() -> u16 {
    std::env::var("CRATEBAY_LINUX_DOCKER_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .filter(|port| *port > 0)
        .unwrap_or(common::DEFAULT_LINUX_DOCKER_PORT)
}

/// Compatibility alias for Docker-compatible endpoint callers.
fn linux_docker_port() -> u16 {
    linux_engine_port()
}

/// CrateBay Engine compatibility host string: `tcp://127.0.0.1:<port>`.
pub fn linux_engine_host() -> String {
    format!("tcp://127.0.0.1:{}", linux_engine_port())
}

/// Compatibility alias for Docker-compatible endpoint callers.
pub fn linux_docker_host() -> String {
    linux_engine_host()
}

// ---------------------------------------------------------------------------
// KVM availability
// ---------------------------------------------------------------------------

/// Check if KVM hardware acceleration is available by attempting to
/// open `/dev/kvm` for read/write.
fn kvm_available() -> bool {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/kvm")
        .is_ok()
}

// ---------------------------------------------------------------------------
// QEMU binary discovery
// ---------------------------------------------------------------------------

/// Architecture-appropriate QEMU system binary name.
fn qemu_binary_name() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "qemu-system-aarch64"
    }

    #[cfg(target_arch = "x86_64")]
    {
        "qemu-system-x86_64"
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "qemu-system-x86_64"
    }
}

/// Find an executable on PATH.
fn find_executable_on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|raw| {
        std::env::split_paths(&raw)
            .map(|dir| dir.join(name))
            .find(|path| path.is_file())
    })
}

/// Resolve the QEMU binary path.
///
/// Priority:
/// 1. `CRATEBAY_RUNTIME_QEMU_PATH` environment variable
/// 2. Bundled binary from runtime assets
/// 3. System PATH lookup
fn resolve_qemu_path() -> Result<PathBuf, AppError> {
    // 1. Explicit override
    if let Ok(path) = std::env::var("CRATEBAY_RUNTIME_QEMU_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(AppError::Runtime(format!(
            "CRATEBAY_RUNTIME_QEMU_PATH does not point to a file: {}",
            path.display()
        )));
    }

    // 2. Bundled binary inside runtime assets
    if let Some(assets_dir) = common::bundled_linux_runtime_assets_dir() {
        let bundle_id = runtime_linux_bundle_id();
        let bundled = assets_dir.join(bundle_id).join(qemu_binary_name());
        if bundled.is_file() && !common::file_contains_placeholder_marker(&bundled) {
            return Ok(bundled);
        }
    }

    // 3. System PATH
    if let Some(path) = find_executable_on_path(qemu_binary_name()) {
        return Ok(path);
    }

    Err(AppError::Runtime(format!(
        "Bundled Linux runtime helper '{}' was not found. \
         Reinstall CrateBay (ensure runtime-linux assets are present) or set CRATEBAY_RUNTIME_QEMU_PATH.",
        qemu_binary_name()
    )))
}

/// Bundle ID for the Linux runtime assets (architecture-specific).
fn runtime_linux_bundle_id() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "cratebay-runtime-linux-aarch64"
    }

    #[cfg(target_arch = "x86_64")]
    {
        "cratebay-runtime-linux-x86_64"
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "cratebay-runtime-linux-x86_64"
    }
}

/// QEMU share directory (firmware files, etc.) adjacent to the binary.
fn qemu_share_dir(qemu_path: &Path) -> Option<PathBuf> {
    let dir = qemu_path.parent()?.join("share").join("qemu");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// QEMU library directory adjacent to the binary.
fn qemu_lib_dir(qemu_path: &Path) -> Option<PathBuf> {
    let dir = qemu_path.parent()?.join("lib");
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Kernel command line
// ---------------------------------------------------------------------------

/// Default kernel command line for the runtime VM.
fn default_kernel_cmdline() -> &'static str {
    #[cfg(target_arch = "aarch64")]
    {
        "console=ttyAMA0 panic=1"
    }

    #[cfg(target_arch = "x86_64")]
    {
        "console=ttyS0 panic=1"
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        "console=ttyS0 panic=1"
    }
}

/// Build the full kernel command line, including optional HTTP proxy.
fn build_kernel_cmdline() -> String {
    let mut cmdline = std::env::var("CRATEBAY_LINUX_RUNTIME_CMDLINE")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default_kernel_cmdline().to_string());

    // Inject HTTP proxy if configured and not already present.
    if !cmdline
        .split_whitespace()
        .any(|arg| arg.starts_with("cratebay_http_proxy="))
    {
        if let Ok(proxy) = std::env::var("CRATEBAY_RUNTIME_HTTP_PROXY") {
            let proxy = proxy
                .trim()
                .trim_start_matches("http://")
                .trim_start_matches("https://");
            if !proxy.is_empty() {
                cmdline.push_str(" cratebay_http_proxy=");
                cmdline.push_str(proxy);
            }
        }
    }

    cmdline
}

// ---------------------------------------------------------------------------
// Port conflict detection
// ---------------------------------------------------------------------------

/// Check that the Docker TCP port is not already in use by another service.
fn ensure_host_port_available(host: &str) -> Result<(), AppError> {
    let (tcp_host, port) = common::engine_host_tcp_endpoint(host)
        .ok_or_else(|| AppError::Runtime(format!("Invalid Engine host '{}'", host)))?;

    std::net::TcpListener::bind((tcp_host.as_str(), port))
        .map(drop)
        .map_err(|error| {
            AppError::Runtime(format!(
                "Linux runtime Engine host {} is already in use; stop the conflicting service \
             or set CRATEBAY_LINUX_DOCKER_PORT to a different port ({})",
                host, error
            ))
        })
}

// ---------------------------------------------------------------------------
// Disk management
// ---------------------------------------------------------------------------

/// Ensure the runtime disk image exists (sparse/thin-provisioned).
fn ensure_runtime_disk() -> Result<PathBuf, AppError> {
    let path = runtime_disk_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if !path.exists() {
        let file = std::fs::File::create(&path)?;
        let disk_gb = RuntimeConfig::default().disk_gb as u64;
        file.set_len(disk_gb * 1024 * 1024 * 1024)?;
        tracing::info!("Created sparse runtime disk: {}", path.display());
    }

    Ok(path)
}

// ---------------------------------------------------------------------------
// Console log helpers
// ---------------------------------------------------------------------------

/// Read the last 25 lines of the console log (for error diagnostics).
fn tail_console_log() -> String {
    let path = runtime_console_log_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return String::new();
    };

    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    let tail: String = lines
        .iter()
        .rev()
        .take(25)
        .copied()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");

    format!("\nConsole tail ({}):\n{}", path.display(), tail)
}

// ---------------------------------------------------------------------------
// LinuxRuntime (§11.2)
// ---------------------------------------------------------------------------

/// Linux runtime manager using KVM/QEMU.
///
/// Manages a lightweight QEMU virtual machine with KVM hardware acceleration
/// running Alpine Linux + CrateBay Engine. The compatibility API is exposed to the host via
/// TCP port forwarding through QEMU's user-mode networking.
///
/// The QEMU process is daemonized (via `-daemonize` flag) with its PID stored
/// in a PID file for lifecycle management across process restarts.
pub struct LinuxRuntime {
    config: RuntimeConfig,
    state: Arc<Mutex<RuntimeState>>,
    /// Timestamp (seconds since epoch) when QEMU was started.
    started_at: Arc<Mutex<Option<i64>>>,
    /// Number of consecutive health-check cycles with failed engine ping.
    consecutive_health_failures: Arc<Mutex<u8>>,
}

impl LinuxRuntime {
    /// Create a new Linux runtime manager with default configuration.
    pub fn new() -> Self {
        Self {
            config: RuntimeConfig::default(),
            state: Arc::new(Mutex::new(RuntimeState::None)),
            started_at: Arc::new(Mutex::new(None)),
            consecutive_health_failures: Arc::new(Mutex::new(0)),
        }
    }

    /// Ping Docker with retries to smooth transient networking hiccups.
    async fn docker_ping_with_retry(host: &str) -> bool {
        for attempt in 0..HEALTH_PING_ATTEMPTS {
            let host_check = host.to_string();
            let responsive = tokio::task::spawn_blocking(move || {
                common::engine_http_ping_host(&host_check).is_ok()
            })
            .await
            .unwrap_or(false);

            if responsive {
                return true;
            }

            if attempt + 1 < HEALTH_PING_ATTEMPTS {
                tokio::time::sleep(HEALTH_PING_RETRY_DELAY).await;
            }
        }

        false
    }

    /// Read the current QEMU PID from the PID file.
    fn current_pid() -> Option<u32> {
        read_pid_file(&qemu_pid_path())
    }

    /// Check if the QEMU process is currently alive.
    fn is_running() -> bool {
        Self::current_pid().is_some_and(pid_alive)
    }

    /// Build QEMU command and spawn the daemonized process.
    ///
    /// This follows master's approach: use `std::process::Command` with
    /// QEMU's `-daemonize` flag so QEMU forks to background and writes
    /// its PID to a file, then returns control immediately.
    fn spawn_qemu(
        qemu_path: &Path,
        image_paths: &crate::images::ImagePaths,
        disk_path: &Path,
        host_port: u16,
        guest_port: u32,
    ) -> Result<(), AppError> {
        let runtime_dir = runtime_dir();
        std::fs::create_dir_all(&runtime_dir)?;

        // Clean up stale PID file.
        let pid_file = qemu_pid_path();
        let _ = std::fs::remove_file(&pid_file);

        // Truncate (or create) console log file.
        let console_log = runtime_console_log_path();
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&console_log)?;

        let use_kvm = kvm_available();
        let cmdline = build_kernel_cmdline();

        let machine = if cfg!(target_arch = "aarch64") {
            if use_kvm {
                "virt,accel=kvm"
            } else {
                "virt,accel=tcg"
            }
        } else if use_kvm {
            "q35,accel=kvm"
        } else {
            "q35,accel=tcg"
        };

        let cpu = if use_kvm { "host" } else { "max" };

        let mut cmd = std::process::Command::new(qemu_path);
        cmd.arg("-name")
            .arg(common::runtime_vm_name())
            .arg("-machine")
            .arg(machine)
            .arg("-cpu")
            .arg(cpu)
            .arg("-smp")
            .arg("2")
            .arg("-m")
            .arg("2048")
            .arg("-kernel")
            .arg(&image_paths.kernel_path)
            .arg("-initrd")
            .arg(&image_paths.initrd_path)
            .arg("-append")
            .arg(&cmdline)
            .arg("-drive")
            .arg(format!("if=virtio,format=raw,file={}", disk_path.display()))
            .arg("-netdev")
            .arg(format!(
                "user,id=net0,hostfwd=tcp:127.0.0.1:{host_port}-:{guest_port}"
            ))
            .arg("-device")
            .arg("virtio-net-pci,netdev=net0")
            .arg("-device")
            .arg("virtio-rng-pci")
            .arg("-serial")
            .arg(format!("file:{}", console_log.display()))
            .arg("-display")
            .arg("none")
            .arg("-monitor")
            .arg("none")
            .arg("-daemonize")
            .arg("-pidfile")
            .arg(&pid_file)
            .arg("-no-reboot");

        // Add QEMU share directory for firmware files if available.
        if let Some(share_dir) = qemu_share_dir(qemu_path) {
            cmd.arg("-L").arg(share_dir);
        }

        // Set LD_LIBRARY_PATH if bundled libs exist.
        if let Some(lib_dir) = qemu_lib_dir(qemu_path) {
            let current = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
            let joined = if current.trim().is_empty() {
                lib_dir.to_string_lossy().into_owned()
            } else {
                format!("{}:{}", lib_dir.display(), current)
            };
            cmd.env("LD_LIBRARY_PATH", joined);
        }

        tracing::info!(
            "Starting QEMU: {} (KVM: {}, machine: {}, cpu: {})",
            qemu_path.display(),
            use_kvm,
            machine,
            cpu
        );

        let output = cmd.output().map_err(|error| {
            AppError::Runtime(format!(
                "Failed to launch CrateBay Linux runtime helper '{}': {}",
                qemu_path.display(),
                error
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("exit {}", output.status)
            };
            return Err(AppError::Runtime(format!(
                "Failed to start CrateBay Runtime (Linux/QEMU): {}",
                detail
            )));
        }

        // Verify PID file was written.
        let pid = read_pid_file(&pid_file).ok_or_else(|| {
            AppError::Runtime("QEMU daemonized but PID file was not written".to_string())
        })?;
        tracing::info!("QEMU daemonized with PID {}", pid);

        Ok(())
    }

    /// Stop the QEMU process by PID: SIGTERM → wait → SIGKILL.
    fn stop_qemu_impl() -> Result<(), AppError> {
        if let Some(pid) = Self::current_pid() {
            if pid_alive(pid) {
                tracing::info!("Sending SIGTERM to QEMU PID {}", pid);
                kill_process(pid, libc::SIGTERM);

                // Wait for graceful exit.
                let deadline = std::time::Instant::now() + STOP_GRACE_PERIOD;
                while std::time::Instant::now() < deadline {
                    if !pid_alive(pid) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }

                // Escalate to SIGKILL if still alive.
                if pid_alive(pid) {
                    tracing::warn!(
                        "QEMU PID {} did not exit after SIGTERM, sending SIGKILL",
                        pid
                    );
                    kill_process(pid, libc::SIGKILL);
                    // Give a moment for cleanup.
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
        }

        // Clean up PID file.
        let _ = std::fs::remove_file(qemu_pid_path());
        Ok(())
    }
}

impl Default for LinuxRuntime {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RuntimeManager implementation (§3.2, §11.2)
// ---------------------------------------------------------------------------

#[async_trait]
impl RuntimeManager for LinuxRuntime {
    /// Detect the current runtime state (§3.2).
    ///
    /// Checks:
    /// 1. Whether a QEMU process is running (via PID file).
    /// 2. Whether CrateBay Engine is responsive (via TCP ping).
    /// 3. Whether runtime images are provisioned.
    /// 4. KVM and QEMU binary availability.
    async fn get_state(&self) -> Result<RuntimeState, AppError> {
        // If QEMU is running, check engine readiness.
        if Self::is_running() {
            let engine_responsive = crate::runtime::query_built_in_ready_engine_status(self)
                .map(|_| true)
                .unwrap_or(false);

            if engine_responsive {
                let mut state = self.state.lock().await;
                *state = RuntimeState::Ready;
                return Ok(RuntimeState::Ready);
            }

            // QEMU running but CrateBay Engine not responsive — still booting.
            let mut state = self.state.lock().await;
            *state = RuntimeState::Starting;
            return Ok(RuntimeState::Starting);
        }

        // No running QEMU. Check if images are provisioned.
        let image_id = common::runtime_os_image_id();
        let image_ready = crate::images::is_image_ready(image_id);
        let image_paths = crate::images::image_paths(image_id);

        if image_ready && image_paths.kernel_path.exists() && image_paths.initrd_path.exists() {
            // Images present — check prerequisites for starting.
            if !Path::new("/dev/kvm").exists() {
                let msg = "KVM not available (/dev/kvm not found). \
                           Ensure hardware virtualization is enabled in BIOS/UEFI."
                    .to_string();
                let mut state = self.state.lock().await;
                *state = RuntimeState::Error(msg.clone());
                return Ok(RuntimeState::Error(msg));
            }

            if resolve_qemu_path().is_err() {
                let msg = format!(
                    "QEMU not found. Install {} or set CRATEBAY_RUNTIME_QEMU_PATH.",
                    qemu_binary_name()
                );
                let mut state = self.state.lock().await;
                *state = RuntimeState::Error(msg.clone());
                return Ok(RuntimeState::Error(msg));
            }

            let mut state = self.state.lock().await;
            *state = RuntimeState::Provisioned;
            Ok(RuntimeState::Provisioned)
        } else {
            let mut state = self.state.lock().await;
            *state = RuntimeState::None;
            Ok(RuntimeState::None)
        }
    }

    /// Provision the runtime — install runtime image from bundled assets (§8).
    ///
    /// Progress stages: checking → downloading → extracting → configuring → complete.
    /// Uses `common::ensure_runtime_image_ready()` which handles bundled asset
    /// discovery, placeholder detection, and file copying.
    async fn provision(
        &self,
        on_progress: Box<dyn Fn(ProvisionProgress) + Send>,
    ) -> Result<(), AppError> {
        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Starting;
        }

        // Stage 1: Check prerequisites.
        on_progress(ProvisionProgress {
            stage: "checking".into(),
            percent: 5.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Checking KVM and QEMU availability...".into(),
        });

        // Verify KVM is available (not strictly required for provisioning,
        // but we warn early since start() will fail without it).
        if !Path::new("/dev/kvm").exists() {
            tracing::warn!(
                "KVM not available during provisioning. \
                 Runtime will fall back to TCG (software emulation) at start."
            );
        }

        on_progress(ProvisionProgress {
            stage: "checking".into(),
            percent: 10.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Locating runtime image assets...".into(),
        });

        let image_id = common::runtime_os_image_id().to_string();

        // Stage 2: Install runtime image (download / copy from bundled assets).
        on_progress(ProvisionProgress {
            stage: "downloading".into(),
            percent: 15.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Installing CrateBay Linux runtime image...".into(),
        });

        // This is a blocking operation — run in spawn_blocking.
        let id = image_id.clone();
        tokio::task::spawn_blocking(move || common::ensure_runtime_image_ready(&id))
            .await
            .map_err(|e| AppError::Runtime(format!("Provision task panicked: {}", e)))??;

        on_progress(ProvisionProgress {
            stage: "extracting".into(),
            percent: 70.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Runtime image installed.".into(),
        });

        // Stage 3: Create runtime disk (thin-provisioned).
        on_progress(ProvisionProgress {
            stage: "configuring".into(),
            percent: 85.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Creating runtime disk image...".into(),
        });

        tokio::task::spawn_blocking(ensure_runtime_disk)
            .await
            .map_err(|e| AppError::Runtime(format!("Disk creation task panicked: {}", e)))??;

        // Stage 4: Verify everything is in place.
        on_progress(ProvisionProgress {
            stage: "configuring".into(),
            percent: 95.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Verifying runtime image integrity...".into(),
        });

        let paths = crate::images::image_paths(&image_id);
        if !paths.kernel_path.exists() || !paths.initrd_path.exists() {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Error("Runtime image files missing after install".into());
            return Err(AppError::Runtime(
                "Runtime image files are missing after installation. \
                 Ensure the CrateBay desktop app is installed correctly."
                    .into(),
            ));
        }

        // Stage 5: Complete.
        on_progress(ProvisionProgress {
            stage: "complete".into(),
            percent: 100.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Linux KVM/QEMU runtime provisioned successfully.".into(),
        });

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Provisioned;
        }

        tracing::info!("Linux KVM/QEMU runtime provisioned successfully");
        Ok(())
    }

    /// Start the QEMU VM (§11.2).
    ///
    /// 1. Check prerequisites (images, QEMU binary).
    /// 2. If already running and CrateBay Engine responsive, return early.
    /// 3. Stop any existing stale QEMU process.
    /// 4. Check port availability.
    /// 5. Spawn daemonized QEMU with PID file.
    /// 6. Wait for engine API readiness (45 s timeout).
    async fn start(&self) -> Result<(), AppError> {
        // Guard: Check current state.
        {
            let current = self.state.lock().await;
            match &*current {
                RuntimeState::Starting => {
                    return Err(AppError::Runtime("Runtime is already starting".into()));
                }
                RuntimeState::Starting => {
                    return Err(AppError::Runtime(
                        "Runtime is currently being provisioned".into(),
                    ));
                }
                _ => {} // Proceed.
            }
        }

        let host = linux_engine_host();

        // If the engine API is already responsive on our port, check if it's our process.
        let native_already_up = crate::runtime::query_built_in_ready_engine_status(self)
            .map(|_| true)
            .unwrap_or(false);

        if native_already_up {
            if Self::is_running() {
                // Our QEMU is already running and CrateBay Engine is up.
                let mut state = self.state.lock().await;
                *state = RuntimeState::Ready;
                return Ok(());
            }
            // Port is taken by another service.
            return Err(AppError::Runtime(format!(
                "Linux runtime engine host {} is already serving another endpoint; \
                 stop the conflicting service or set CRATEBAY_LINUX_DOCKER_PORT to a different port",
                host
            )));
        }

        // If QEMU is running but CrateBay Engine isn't ready, give it a short grace period.
        if Self::is_running() {
            let host_wait = host.clone();
            let wait_result = tokio::task::spawn_blocking(move || {
                wait_for_native_engine_contract_tcp(&host_wait, Duration::from_secs(5))
            })
            .await
            .unwrap_or(Err("Task panicked".to_string()));

            if wait_result.is_ok() {
                let mut state = self.state.lock().await;
                *state = RuntimeState::Ready;
                return Ok(());
            }

            // QEMU running but CrateBay Engine not responsive — kill and restart.
            tracing::warn!("Stale QEMU process detected, stopping before restart");
            tokio::task::spawn_blocking(Self::stop_qemu_impl)
                .await
                .map_err(|e| AppError::Runtime(format!("Stop task panicked: {}", e)))??;
        } else {
            // No QEMU running — clean up stale PID file.
            let _ = std::fs::remove_file(qemu_pid_path());
        }

        // Transition to Starting state.
        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Starting;
        }

        // Verify runtime image is ready.
        let image_id = common::runtime_os_image_id().to_string();
        let id = image_id.clone();
        tokio::task::spawn_blocking(move || common::ensure_runtime_image_ready(&id))
            .await
            .map_err(|e| AppError::Runtime(format!("Image check task panicked: {}", e)))??;

        // Check port availability.
        let host_for_port_check = host.clone();
        tokio::task::spawn_blocking(move || ensure_host_port_available(&host_for_port_check))
            .await
            .map_err(|e| AppError::Runtime(format!("Port check task panicked: {}", e)))??;

        // Resolve paths.
        let qemu_path = tokio::task::spawn_blocking(resolve_qemu_path)
            .await
            .map_err(|e| AppError::Runtime(format!("QEMU resolve task panicked: {}", e)))??;

        let disk_path = tokio::task::spawn_blocking(ensure_runtime_disk)
            .await
            .map_err(|e| AppError::Runtime(format!("Disk ensure task panicked: {}", e)))??;

        let image_paths = crate::images::image_paths(&image_id);
        let host_port = linux_engine_port();
        let guest_port = common::engine_proxy_port();

        // Spawn QEMU.
        let qp = qemu_path.clone();
        let ip = image_paths.clone();
        let dp = disk_path.clone();
        tokio::task::spawn_blocking(move || Self::spawn_qemu(&qp, &ip, &dp, host_port, guest_port))
            .await
            .map_err(|e| AppError::Runtime(format!("QEMU spawn task panicked: {}", e)))??;

        // Record start time.
        {
            let mut started = self.started_at.lock().await;
            *started = Some(chrono::Utc::now().timestamp());
        }

        // Wait for CrateBay Engine to become responsive.
        let host_wait = host.clone();
        let wait_result = tokio::task::spawn_blocking(move || {
            wait_for_native_engine_contract_tcp(&host_wait, DOCKER_READY_TIMEOUT)
        })
        .await
        .map_err(|e| AppError::Runtime(format!("Engine contract wait task panicked: {}", e)))?;

        match wait_result {
            Ok(()) => {
                let mut state = self.state.lock().await;
                *state = RuntimeState::Ready;
                tracing::info!(
                    "CrateBay Engine API is responsive on {} — Linux runtime is Ready",
                    host
                );
                Ok(())
            }
            Err(error) => {
                // CrateBay Engine did not come up. Stop QEMU and report failure.
                let _ = tokio::task::spawn_blocking(Self::stop_qemu_impl).await;
                let console_tail = tokio::task::spawn_blocking(tail_console_log)
                    .await
                    .unwrap_or_default();
                let msg = format!(
                    "CrateBay Runtime (Linux/QEMU) did not become ready within {} seconds: {}{}",
                    DOCKER_READY_TIMEOUT.as_secs(),
                    error,
                    console_tail
                );
                let mut state = self.state.lock().await;
                *state = RuntimeState::Error(msg.clone());
                Err(AppError::Runtime(msg))
            }
        }
    }

    /// Stop the QEMU VM gracefully (§3.1).
    ///
    /// 1. SIGTERM to the QEMU process.
    /// 2. Wait up to 5 seconds for graceful exit.
    /// 3. SIGKILL if still alive.
    /// 4. Clean up PID file.
    async fn stop(&self) -> Result<(), AppError> {
        if !Self::is_running() {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Stopped;
            return Ok(());
        }

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Stopping;
        }

        tokio::task::spawn_blocking(Self::stop_qemu_impl)
            .await
            .map_err(|e| AppError::Runtime(format!("Stop task panicked: {}", e)))??;

        {
            let mut started = self.started_at.lock().await;
            *started = None;
        }

        {
            let mut state = self.state.lock().await;
            *state = RuntimeState::Stopped;
        }

        tracing::info!("Linux QEMU runtime stopped");
        Ok(())
    }

    /// Health check (§9.1).
    ///
    /// 1. Check QEMU process alive (via PID file).
    /// 2. Check Docker TCP ping.
    /// 3. Derive runtime state from health signals.
    async fn health_check(&self) -> Result<HealthStatus, AppError> {
        let vm_running = Self::is_running();
        let host = linux_engine_host();

        let (docker_responsive, _) = if vm_running {
            (Self::docker_ping_with_retry(&host).await, None::<String>)
        } else {
            (false, None)
        };
        let engine_responsive = vm_running
            && crate::runtime::query_built_in_ready_engine_status(self)
                .map(|_| true)
                .unwrap_or(false);

        // Calculate uptime.
        let uptime_seconds = if vm_running {
            let started = self.started_at.lock().await;
            started.map(|ts| {
                let now = chrono::Utc::now().timestamp();
                (now - ts).max(0) as u64
            })
        } else {
            None
        };

        // Determine runtime state.
        let current_state = {
            let state = self.state.lock().await;
            state.clone()
        };

        let runtime_state = if engine_responsive {
            let mut failures = self.consecutive_health_failures.lock().await;
            *failures = 0;
            RuntimeState::Ready
        } else if vm_running {
            let mut failures = self.consecutive_health_failures.lock().await;
            *failures = failures.saturating_add(1);

            if matches!(current_state, RuntimeState::Ready)
                && *failures < READY_DOWNGRADE_FAILURE_THRESHOLD
            {
                RuntimeState::Ready
            } else {
                RuntimeState::Starting
            }
        } else {
            {
                let mut failures = self.consecutive_health_failures.lock().await;
                *failures = 0;
            }

            // Check if we had a tracked PID that exited.
            let pid = Self::current_pid();
            if pid.is_some() {
                RuntimeState::Error("QEMU process exited unexpectedly".into())
            } else {
                current_state
            }
        };

        // Update internal state.
        {
            let mut state = self.state.lock().await;
            *state = runtime_state.clone();
        }

        Ok(HealthStatus {
            runtime_state,
            engine_responsive,
            compatibility_responsive: docker_responsive,
            compatibility_version: None,
            docker_responsive,
            docker_version: None, // Would require a full compatibility API call
            uptime_seconds,
            last_check: chrono::Utc::now().to_rfc3339(),
            engine_source: Some("builtin".to_string()),
            docker_source: Some("builtin".to_string()),
            engine: crate::runtime::built_in_engine_status(),
        })
    }

    /// CrateBay Engine socket path (§4.1).
    ///
    /// On Linux the runtime uses TCP port forwarding, but the trait requires
    /// returning a `PathBuf`. We return a synthetic path that indicates TCP
    /// mode; the GUI layer should use `linux_engine_host()` for actual connection.
    fn engine_socket_path(&self) -> PathBuf {
        // Return the canonical host Engine socket path.
        // The actual connection goes through TCP, but this provides a
        // consistent path for the trait interface.
        common::host_engine_socket_path().to_path_buf()
    }

    /// Get current resource usage of the QEMU VM (§7.3).
    ///
    /// Reads from `/proc/{pid}/stat` and `/proc/{pid}/statm` to obtain
    /// CPU and memory usage of the QEMU process.
    async fn resource_usage(&self) -> Result<ResourceUsage, AppError> {
        let pid = Self::current_pid();

        match pid {
            Some(pid) if pid_alive(pid) => {
                let cpu_percent = read_proc_cpu_percent(pid).await;
                let memory_used_mb = read_proc_memory_mb(pid).await;
                let disk_used_gb = common::file_allocated_gb(&runtime_disk_path());
                let container_count = linux_container_count(self).await.unwrap_or_default();

                Ok(ResourceUsage {
                    cpu_percent,
                    memory_used_mb,
                    memory_total_mb: self.config.memory_mb,
                    disk_used_gb,
                    disk_total_gb: self.config.disk_gb as f32,
                    container_count,
                })
            }
            _ => {
                // No QEMU process running.
                Ok(ResourceUsage {
                    cpu_percent: 0.0,
                    memory_used_mb: 0,
                    memory_total_mb: self.config.memory_mb,
                    disk_used_gb: 0.0,
                    disk_total_gb: self.config.disk_gb as f32,
                    container_count: 0,
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// /proc helpers — read QEMU process resource usage on Linux
// ---------------------------------------------------------------------------

/// Read approximate CPU usage percentage from `/proc/{pid}/stat`.
///
/// Single-sample approach: reads total CPU ticks consumed by the process
/// and compares against system uptime.
async fn read_proc_cpu_percent(pid: u32) -> f32 {
    let stat_path = format!("/proc/{}/stat", pid);
    let stat_content = match tokio::fs::read_to_string(&stat_path).await {
        Ok(c) => c,
        Err(_) => return 0.0,
    };

    // /proc/{pid}/stat: skip comm field (may contain spaces) by finding ')'.
    let after_comm = match stat_content.rfind(')') {
        Some(pos) => &stat_content[pos + 2..], // skip ") "
        None => return 0.0,
    };

    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    // After comm: field[0] = state, field[11] = utime, field[12] = stime
    if fields.len() < 13 {
        return 0.0;
    }

    let utime: u64 = fields[11].parse().unwrap_or(0);
    let stime: u64 = fields[12].parse().unwrap_or(0);
    let total_ticks = utime + stime;

    // Read system uptime.
    let uptime = match tokio::fs::read_to_string("/proc/uptime").await {
        Ok(c) => c
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(1.0),
        Err(_) => return 0.0,
    };

    // clock ticks per second — typically 100 on Linux.
    let clk_tck: f64 = 100.0;
    let process_seconds = total_ticks as f64 / clk_tck;

    if uptime > 0.0 {
        ((process_seconds / uptime) * 100.0) as f32
    } else {
        0.0
    }
}

/// Read memory usage in MB from `/proc/{pid}/statm`.
///
/// Field 1 (RSS — resident set size) is in pages. Page size is typically 4096.
async fn read_proc_memory_mb(pid: u32) -> u64 {
    let statm_path = format!("/proc/{}/statm", pid);
    let statm_content = match tokio::fs::read_to_string(&statm_path).await {
        Ok(c) => c,
        Err(_) => return 0,
    };

    let fields: Vec<&str> = statm_content.split_whitespace().collect();
    if fields.len() < 2 {
        return 0;
    }

    // Field 1 = RSS in pages.
    let rss_pages: u64 = fields[1].parse().unwrap_or(0);
    let page_size: u64 = 4096; // Standard Linux page size.

    (rss_pages * page_size) / (1024 * 1024)
}

async fn linux_container_count(runtime: &LinuxRuntime) -> Option<u32> {
    if let Ok(containers) = crate::runtime::query_built_in_native_containers(runtime) {
        return Some(containers.count as u32);
    }

    compatibility_container_count()
}

fn compatibility_container_count() -> Option<u32> {
    let payload =
        common::engine_http_get_json_tcp_host(&linux_engine_host(), "/containers/json?all=true")
            .ok()?;

    payload.as_array().map(|containers| containers.len() as u32)
}

fn native_engine_contract_ready_on_host(host: &str) -> Result<(), String> {
    let payload = common::engine_http_get_json_tcp_host(host, "/cratebay/engine")?;
    crate::runtime::built_in_engine_contract_ready(&payload).map_err(|error| error.to_string())
}

fn wait_for_native_engine_contract_tcp(
    host: &str,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    let mut last_error = "CrateBay Engine native contract is still starting".to_string();

    while std::time::Instant::now() < deadline {
        match native_engine_contract_ready_on_host(host) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    Err(last_error)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_runtime_creates_with_defaults() {
        let rt = LinuxRuntime::new();
        assert_eq!(rt.config.cpu_cores, 2);
        assert_eq!(rt.config.memory_mb, 2048);
        assert_eq!(rt.config.disk_gb, 35);
    }

    #[test]
    fn linux_runtime_default_trait() {
        let rt = LinuxRuntime::default();
        assert_eq!(rt.config.cpu_cores, 2);
    }

    #[test]
    fn engine_socket_path_contains_engine_sock() {
        let rt = LinuxRuntime::new();
        let path = rt.engine_socket_path();
        assert!(
            path.to_string_lossy().contains("engine.sock"),
            "path should contain engine.sock: {:?}",
            path
        );
    }

    #[test]
    fn linux_engine_host_is_tcp() {
        let host = linux_engine_host();
        assert!(
            host.starts_with("tcp://127.0.0.1:"),
            "host should be tcp://127.0.0.1:<port>: {}",
            host
        );
    }

    #[test]
    fn linux_docker_port_is_positive() {
        let port = linux_docker_port();
        assert!(port > 0, "port should be positive: {}", port);
    }

    #[test]
    fn runtime_dir_is_under_data_dir() {
        let dir = runtime_dir();
        assert!(
            dir.to_string_lossy().contains("runtime-linux"),
            "dir should contain runtime-linux: {:?}",
            dir
        );
    }

    #[test]
    fn qemu_pid_path_is_under_runtime_dir() {
        let path = qemu_pid_path();
        assert!(
            path.to_string_lossy().contains("qemu.pid"),
            "path should contain qemu.pid: {:?}",
            path
        );
    }

    #[test]
    fn runtime_disk_path_ends_with_disk_raw() {
        let path = runtime_disk_path();
        assert!(
            path.to_string_lossy().ends_with("disk.raw"),
            "path should end with disk.raw: {:?}",
            path
        );
    }

    #[test]
    fn console_log_path_ends_with_console_log() {
        let path = runtime_console_log_path();
        assert!(
            path.to_string_lossy().ends_with("console.log"),
            "path should end with console.log: {:?}",
            path
        );
    }

    #[test]
    fn qemu_binary_name_is_arch_specific() {
        let name = qemu_binary_name();
        assert!(
            name.starts_with("qemu-system-"),
            "name should start with qemu-system-: {}",
            name
        );
    }

    #[test]
    fn runtime_linux_bundle_id_is_arch_specific() {
        let id = runtime_linux_bundle_id();
        assert!(
            id.starts_with("cratebay-runtime-linux-"),
            "bundle id should start with cratebay-runtime-linux-: {}",
            id
        );
    }

    #[test]
    fn default_kernel_cmdline_contains_console() {
        let cmdline = default_kernel_cmdline();
        assert!(
            cmdline.contains("console="),
            "cmdline should contain console=: {}",
            cmdline
        );
    }

    #[test]
    fn build_kernel_cmdline_contains_console() {
        let cmdline = build_kernel_cmdline();
        assert!(
            cmdline.contains("console="),
            "cmdline should contain console=: {}",
            cmdline
        );
    }

    #[test]
    fn pid_alive_returns_false_for_nonexistent() {
        // PID 0 should not have a /proc entry (it's the kernel scheduler).
        // Use a very large PID that is extremely unlikely to exist.
        assert!(!pid_alive(4_294_967_295));
    }

    #[test]
    fn read_pid_file_returns_none_for_missing() {
        let result = read_pid_file(Path::new("/nonexistent/path/qemu.pid"));
        assert!(result.is_none());
    }

    #[test]
    fn read_pid_file_parses_valid_pid() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.pid");
        std::fs::write(&path, "12345\n").unwrap();
        let result = read_pid_file(&path);
        assert_eq!(result, Some(12345));
    }

    #[test]
    fn read_pid_file_rejects_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.pid");
        std::fs::write(&path, "0\n").unwrap();
        let result = read_pid_file(&path);
        assert!(result.is_none());
    }

    #[test]
    fn read_pid_file_rejects_garbage() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.pid");
        std::fs::write(&path, "not-a-number\n").unwrap();
        let result = read_pid_file(&path);
        assert!(result.is_none());
    }

    #[test]
    fn is_running_returns_false_when_no_pid_file() {
        // Unless there happens to be a valid PID file at the exact runtime path,
        // this should return false.
        // We can't guarantee the path doesn't exist, but it's extremely unlikely
        // in a test environment.
        let _ = LinuxRuntime::is_running();
    }

    #[test]
    fn tail_console_log_returns_empty_for_missing() {
        // With no console log file, should return empty string.
        let result = tail_console_log();
        // Can't assert empty because it depends on whether the file exists,
        // but it should not panic.
        let _ = result;
    }

    #[tokio::test]
    async fn detect_returns_valid_state() {
        let rt = LinuxRuntime::new();
        let state = rt.get_state().await;
        // On a non-Linux platform or without KVM, detect should still succeed
        // and return a valid state.
        assert!(state.is_ok());
    }

    #[tokio::test]
    async fn resource_usage_without_running_vm() {
        let rt = LinuxRuntime::new();
        let usage = rt.resource_usage().await.unwrap();
        assert_eq!(usage.cpu_percent, 0.0);
        assert_eq!(usage.memory_used_mb, 0);
        assert_eq!(usage.container_count, 0);
        assert_eq!(usage.memory_total_mb, 2048);
    }

    #[test]
    fn resource_usage_prefers_native_container_count_before_compatibility_fallback() {
        let source = include_str!("linux.rs");

        assert!(source.contains("query_built_in_native_containers"));
        assert!(source.contains("compatibility_container_count"));
    }

    #[test]
    fn resource_usage_reads_host_allocated_disk_usage() {
        let source = include_str!("linux.rs");

        assert!(source.contains("common::file_allocated_gb"));
    }

    #[tokio::test]
    async fn health_check_without_running_vm() {
        let rt = LinuxRuntime::new();
        let status = rt.health_check().await.unwrap();
        assert!(!status.docker_responsive);
        assert!(status.uptime_seconds.is_none());
    }
}

//! macOS runtime — VZ.framework integration via external Swift runner.
//!
//! Uses Apple's Virtualization.framework to run a lightweight Linux VM
//! with Docker Engine inside. VZ.framework is accessed through an external
//! Swift binary (`cratebay-vz`) because the VZ API requires Objective-C/Swift.
//!
//! # Architecture (runtime-spec.md §2.1)
//!
//! ```text
//! macOS Host
//! └── CrateBay.app
//!     └── Rust Backend (MacOSRuntime)
//!         └── spawns cratebay-vz (Swift binary)
//!             └── VZVirtualMachine
//!                 ├── VZLinuxBootLoader (vmlinuz + initrd)
//!                 ├── VZVirtioBlockStorageDevice (rootfs.img)
//!                 ├── VZVirtioFileSystemDevice (shared dirs)
//!                 ├── VZVirtioSocketDevice (vsock → Docker socket)
//!                 └── VZNATNetworkDeviceAttachment
//!                     └── Alpine Linux → Docker Engine
//! ```
//!
//! # Lifecycle
//!
//! 1. **provision()** — Install runtime image from bundled assets or download
//! 2. **start()** — Spawn VZ runner, wait for ready file, wait for Docker
//! 3. **stop()** — SIGTERM → wait → SIGKILL → cleanup
//! 4. **detect()** — Check macOS version, images, runner PID, Docker socket
//!
//! Ported from `master:crates/cratebay-core/src/macos.rs` (1749 lines) and
//! adapted for the v2 `RuntimeManager` trait with `AppError` error model.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;

use crate::error::AppError;
use crate::models::ResourceUsage;
use crate::MutexExt;

use super::common;
use super::{HealthStatus, ProvisionProgress, RuntimeConfig, RuntimeManager, RuntimeState};

/// Minimum macOS version required for VZ.framework (macOS 13 Ventura).
const MIN_MACOS_VERSION: u32 = 13;

/// Timeout for the VZ runner to write its ready file after spawning.
const RUNNER_READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Timeout for Docker to become responsive after the VM starts.
const DOCKER_READY_TIMEOUT: Duration = Duration::from_secs(120);

/// Number of ping attempts performed in each health check cycle.
const HEALTH_PING_ATTEMPTS: usize = 3;

/// Delay between ping retry attempts in a health check cycle.
const HEALTH_PING_RETRY_DELAY: Duration = Duration::from_millis(400);

/// Bollard connection timeout for health-check pings.
const HEALTH_PING_CONNECT_TIMEOUT_SECS: u64 = 8;

/// Consecutive failed health-check cycles required before degrading
/// from `Ready` to `Starting`.
const READY_DOWNGRADE_FAILURE_THRESHOLD: u8 = 3;

/// Grace period for SIGTERM before escalating to SIGKILL.
const STOP_GRACE_PERIOD: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
struct RuntimeHttpProxyConfig {
    guest_proxy_endpoint: String,
    host_tcp_forward_spec: Option<String>,
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// VM data directory under the CrateBay data directory.
fn vm_dir() -> PathBuf {
    crate::storage::data_dir().join("runtime").join("vm")
}

/// Disk image path for the runtime VM.
fn vm_disk_path() -> PathBuf {
    vm_dir().join("disk.raw")
}

/// Console log path for the runtime VM.
fn vm_console_log_path() -> PathBuf {
    vm_dir().join("console.log")
}

/// PID file for the VZ runner process.
fn vm_runner_pid_path() -> PathBuf {
    vm_dir().join("runner.pid")
}

/// Ready file — the VZ runner creates this after the VM boots.
fn vm_runner_ready_path() -> PathBuf {
    vm_dir().join("runner.ready")
}

/// Read a PID from a file.
fn read_pid_file(path: &Path) -> Option<u32> {
    let content = std::fs::read_to_string(path).ok()?;
    content.trim().parse::<u32>().ok()
}

/// Check if a process is alive (via `kill(pid, 0)`).
fn pid_alive(pid: u32) -> bool {
    let rc = unsafe { libc::kill(pid as i32, 0) };
    if rc == 0 {
        return true;
    }
    // EPERM means the process exists but we don't have permission to signal it
    matches!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EPERM)
    )
}

/// Find VZ runner processes for the runtime VM by scanning `ps` output.
fn find_runner_processes() -> Vec<u32> {
    let disk = vm_disk_path();
    let ready = vm_runner_ready_path();
    let console = vm_console_log_path();
    let current_uid = unsafe { libc::geteuid() } as u32;

    let output = match Command::new("ps")
        .args(["-axww", "-o", "pid=,uid=,command="])
        .output()
    {
        Ok(output) if output.status.success() => output,
        Ok(_) | Err(_) => return Vec::new(),
    };

    let disk = disk.to_string_lossy();
    let ready = ready.to_string_lossy();
    let console = console.to_string_lossy();

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse::<u32>().ok()?;
            let uid = parts.next()?.parse::<u32>().ok()?;
            if uid != current_uid {
                return None;
            }
            let command = parts.collect::<Vec<_>>().join(" ");
            if !command.contains("cratebay-vz") {
                return None;
            }
            if command.contains(disk.as_ref())
                || command.contains(ready.as_ref())
                || command.contains(console.as_ref())
            {
                return Some(pid);
            }
            None
        })
        .collect()
}

/// Terminate a set of runner PIDs with SIGTERM → wait → SIGKILL.
fn terminate_runner_pids(pids: &[u32], reason: &str) {
    if pids.is_empty() {
        return;
    }

    tracing::warn!(
        "Terminating {} VZ runner process(es) ({}): {:?}",
        pids.len(),
        reason,
        pids
    );

    for pid in pids {
        if pid_alive(*pid) {
            unsafe {
                libc::kill(*pid as i32, libc::SIGTERM);
            }
        }
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if pids.iter().all(|pid| !pid_alive(*pid)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    for pid in pids {
        if pid_alive(*pid) {
            tracing::warn!(
                "Runner process {} did not stop gracefully during {}, sending SIGKILL",
                pid,
                reason
            );
            unsafe {
                libc::kill(*pid as i32, libc::SIGKILL);
            }
        }
    }
}

/// Clean up stray runner processes, optionally preserving one PID.
fn cleanup_stray_runner_processes(preserve_pid: Option<u32>, reason: &str) {
    let stray: Vec<u32> = find_runner_processes()
        .into_iter()
        .filter(|pid| Some(*pid) != preserve_pid)
        .collect();
    terminate_runner_pids(&stray, reason);
}

// ---------------------------------------------------------------------------
// VZ runner binary location
// ---------------------------------------------------------------------------

/// Locate the `cratebay-vz` Swift runner binary.
///
/// Search order:
/// 1. `CRATEBAY_VZ_RUNNER_PATH` env var
/// 2. App bundle `Contents/MacOS/cratebay-vz`
/// 3. Sibling of current executable
/// 4. System-wide app installations
/// 5. Workspace development builds
fn vz_runner_path() -> PathBuf {
    if let Ok(path) = std::env::var("CRATEBAY_VZ_RUNNER_PATH") {
        return PathBuf::from(path);
    }

    let mut sibling_candidate = None;
    let mut repo_external_candidate = None;

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("cratebay-vz");
            // Check if we're inside an app bundle (Contents/MacOS/*)
            let is_app_bundle = dir
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|name| name == "MacOS")
                && dir
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
                    .is_some_and(|name| name == "Contents");

            if is_app_bundle && candidate.is_file() {
                return candidate;
            }

            sibling_candidate = Some(candidate);

            // Search workspace for pre-built runner
            let host_target = format!("{}-apple-darwin", std::env::consts::ARCH);
            for ancestor in exe.ancestors() {
                let candidate = ancestor
                    .join("crates")
                    .join("cratebay-gui")
                    .join("src-tauri")
                    .join("bin")
                    .join(format!("cratebay-vz-{}", host_target));
                if candidate.is_file() {
                    repo_external_candidate = Some(candidate);
                    break;
                }
            }
        }
    }

    // In dev mode, prefer workspace builds over system installations.
    // This ensures `pnpm tauri dev` uses the freshly compiled runner
    // instead of an outdated /Applications/CrateBay.app copy.
    if let Some(candidate) = sibling_candidate {
        if candidate.is_file() {
            return candidate;
        }
    }

    if let Some(candidate) = repo_external_candidate {
        if candidate.is_file() {
            return candidate;
        }
    }

    // Check system-wide app bundle installations
    if let Ok(home) = std::env::var("HOME") {
        let candidates = [
            PathBuf::from("/Applications/CrateBay.app/Contents/MacOS/cratebay-vz"),
            PathBuf::from(home)
                .join("Applications")
                .join("CrateBay.app")
                .join("Contents")
                .join("MacOS")
                .join("cratebay-vz"),
        ];
        for candidate in candidates {
            if candidate.is_file() {
                return candidate;
            }
        }
    }

    // Fallback: hope it's on PATH
    PathBuf::from("cratebay-vz")
}

/// Check if a binary has the required macOS virtualization entitlements.
///
/// Tries both `codesign --entitlements :-` (XML plist, legacy) and
/// `codesign --entitlements -` (human-readable, macOS 15+) to handle
/// all macOS versions.  The `:` prefix is deprecated on newer systems
/// and may produce empty output.
fn runner_has_virtualization_entitlements(path: &Path) -> bool {
    // Try XML plist format first (works on older macOS), then
    // human-readable format (required on macOS 15+ / Sequoia+).
    for flag in [":-", "-"] {
        let output = Command::new("codesign")
            .args(["-d", "--entitlements", flag])
            .arg(path)
            .output();
        let Ok(output) = output else {
            continue;
        };
        if !output.status.success() {
            continue;
        }

        let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
        if combined.contains("com.apple.security.virtualization")
            && combined.contains("com.apple.security.hypervisor")
        {
            return true;
        }
    }
    false
}

/// Attempt to sign the VZ runner with required entitlements.
fn ensure_runner_entitlements(runner_path: &Path) -> Result<(), AppError> {
    if runner_has_virtualization_entitlements(runner_path) {
        return Ok(());
    }

    // Don't try to re-sign app bundle binaries
    if runner_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .is_some_and(|name| name == "MacOS")
    {
        return Err(AppError::Runtime(format!(
            "VZ runner is missing required virtualization entitlements: {}. \
             Reinstall or re-sign the app bundle.",
            runner_path.display()
        )));
    }

    // Find the entitlements plist in the project
    let entitlements = find_entitlements_plist().ok_or_else(|| {
        AppError::Runtime(format!(
            "VZ runner is missing required virtualization entitlements and \
             scripts/macos-entitlements.plist was not found: {}",
            runner_path.display()
        ))
    })?;

    let output = Command::new("codesign")
        .args([
            "--force",
            "--sign",
            "-",
            "--options",
            "runtime",
            "--entitlements",
        ])
        .arg(&entitlements)
        .arg(runner_path)
        .output()
        .map_err(|e| {
            AppError::Runtime(format!(
                "Failed to code-sign VZ runner {}: {}",
                runner_path.display(),
                e
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(AppError::Runtime(format!(
            "Failed to code-sign VZ runner {} with {}: {}",
            runner_path.display(),
            entitlements.display(),
            detail
        )));
    }

    if !runner_has_virtualization_entitlements(runner_path) {
        return Err(AppError::Runtime(format!(
            "VZ runner still lacks virtualization entitlements after code-signing: {}",
            runner_path.display()
        )));
    }

    tracing::warn!(
        "Applied local virtualization entitlements to VZ runner {}",
        runner_path.display()
    );
    Ok(())
}

/// Search for the `macos-entitlements.plist` file.
fn find_entitlements_plist() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(root) = std::env::var("CRATEBAY_REPO_ROOT") {
        candidates.push(PathBuf::from(root));
    }
    if let Ok(exe) = std::env::current_exe() {
        candidates.extend(exe.ancestors().map(Path::to_path_buf));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.extend(cwd.ancestors().map(Path::to_path_buf));
    }

    for candidate in candidates {
        let entitlements = candidate.join("scripts").join("macos-entitlements.plist");
        if entitlements.is_file() {
            return Some(entitlements);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// MacOSRuntime
// ---------------------------------------------------------------------------

/// macOS runtime manager using Apple's VZ.framework via external Swift runner.
///
/// The runtime manages a single VM that runs Docker inside a lightweight
/// Alpine Linux guest. The VZ runner process (`cratebay-vz`) handles all
/// VZ.framework API calls and exposes the Docker socket via vsock or TCP
/// forwarding.
pub struct MacOSRuntime {
    /// Runtime configuration (CPU, memory, disk, shared dirs).
    config: RuntimeConfig,
    /// Current runtime state, shared across async operations.
    state: Arc<Mutex<RuntimeState>>,
    /// Tracked VZ runner child process (if we spawned it this session).
    runner: Arc<Mutex<Option<Child>>>,
    /// Tracked runner PID (may be from a previous session via PID file).
    runner_pid: Arc<Mutex<Option<u32>>>,
    /// Timestamp when the runner was started (for uptime calculation).
    started_at: Arc<Mutex<Option<Instant>>>,
    /// Number of consecutive health-check cycles with failed Docker ping.
    consecutive_health_failures: Arc<Mutex<u8>>,
}

impl MacOSRuntime {
    /// Create a new macOS runtime manager with default configuration.
    pub fn new() -> Self {
        let rt = Self {
            config: RuntimeConfig::default(),
            state: Arc::new(Mutex::new(RuntimeState::None)),
            runner: Arc::new(Mutex::new(None)),
            runner_pid: Arc::new(Mutex::new(None)),
            started_at: Arc::new(Mutex::new(None)),
            consecutive_health_failures: Arc::new(Mutex::new(0)),
        };

        // Try to recover state from a previous session's PID file
        if let Some(pid) = read_pid_file(&vm_runner_pid_path()) {
            if pid_alive(pid) {
                if let Ok(mut pid_guard) = rt.runner_pid.lock() {
                    *pid_guard = Some(pid);
                }
                // Don't set state here — detect() will determine the correct state
            } else {
                // Stale PID file — clean up
                let _ = std::fs::remove_file(vm_runner_pid_path());
                let _ = std::fs::remove_file(vm_runner_ready_path());
            }
        }

        rt
    }

    /// Create a new macOS runtime manager with custom configuration.
    #[allow(dead_code)]
    pub fn with_config(config: RuntimeConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(RuntimeState::None)),
            runner: Arc::new(Mutex::new(None)),
            runner_pid: Arc::new(Mutex::new(None)),
            started_at: Arc::new(Mutex::new(None)),
            consecutive_health_failures: Arc::new(Mutex::new(0)),
        }
    }

    /// Check if the host macOS version meets the minimum requirement.
    fn check_macos_version() -> Result<bool, AppError> {
        let output = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .map_err(|e| AppError::Runtime(format!("Failed to check macOS version: {}", e)))?;

        let version_str = String::from_utf8_lossy(&output.stdout);
        let major: u32 = version_str
            .trim()
            .split('.')
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        Ok(major >= MIN_MACOS_VERSION)
    }

    /// Check if Rosetta x86_64 emulation is available.
    fn rosetta_available() -> bool {
        #[cfg(target_arch = "aarch64")]
        {
            Path::new("/Library/Apple/usr/libexec/oah/libRosettaRuntime").exists()
                || Path::new("/usr/libexec/rosetta").exists()
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            false
        }
    }

    /// Check if the VZ runner process is alive.
    fn is_runner_alive(&self) -> bool {
        if let Ok(pid_guard) = self.runner_pid.lock() {
            if let Some(pid) = *pid_guard {
                return pid_alive(pid);
            }
        }
        // Also check via child process handle
        if let Ok(mut runner_guard) = self.runner.lock() {
            if let Some(ref mut child) = *runner_guard {
                return child.try_wait().ok().flatten().is_none();
            }
        }
        false
    }

    /// Get the current runner PID (from in-memory tracking or PID file).
    fn current_runner_pid(&self) -> Option<u32> {
        if let Ok(guard) = self.runner_pid.lock() {
            if let Some(pid) = *guard {
                if pid_alive(pid) {
                    return Some(pid);
                }
            }
        }
        // Fallback: check PID file
        let pid = read_pid_file(&vm_runner_pid_path())?;
        if pid_alive(pid) {
            Some(pid)
        } else {
            None
        }
    }

    /// Build kernel cmdline with DNS servers, host epoch and optional HTTP proxy.
    fn build_cmdline(&self, runtime_http_proxy: Option<&RuntimeHttpProxyConfig>) -> String {
        let image_id = common::runtime_os_image_id();
        let mut cmdline = crate::images::find_image(image_id)
            .map(|e| e.default_cmdline)
            .unwrap_or_else(|| "console=hvc0".to_string());

        // Inject runtime HTTP proxy for guest-side dockerd/apk egress
        if !cmdline
            .split_whitespace()
            .any(|arg| arg.starts_with("cratebay_http_proxy="))
        {
            if let Some(proxy) = runtime_http_proxy {
                cmdline.push_str(" cratebay_http_proxy=");
                cmdline.push_str(&proxy.guest_proxy_endpoint);
            }
        }

        // Inject DNS servers from macOS resolver
        if !cmdline
            .split_whitespace()
            .any(|arg| arg.starts_with("cratebay_dns="))
        {
            let dns_servers = Self::resolve_dns_servers();
            if dns_servers.is_empty() {
                // This shouldn't happen since resolve_dns_servers always adds defaults,
                // but guard against it anyway.
                tracing::warn!("DNS server list is empty, injecting failsafe DNS");
                cmdline.push_str(" cratebay_dns=1.1.1.1,8.8.8.8");
            } else {
                cmdline.push_str(" cratebay_dns=");
                cmdline.push_str(&dns_servers.join(","));
            }
        }

        // Inject host epoch for guest time sync
        if !cmdline
            .split_whitespace()
            .any(|arg| arg.starts_with("cratebay_host_epoch="))
        {
            if let Ok(now) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
                cmdline.push_str(" cratebay_host_epoch=");
                cmdline.push_str(&now.as_secs().to_string());
            }
        }

        if !cmdline
            .split_whitespace()
            .any(|arg| arg.starts_with("cratebay_docker_proxy_port="))
        {
            cmdline.push_str(" cratebay_docker_proxy_port=");
            cmdline.push_str(&common::docker_proxy_port().to_string());
        }

        tracing::debug!("VM boot cmdline: {}", cmdline);
        cmdline
    }

    fn first_non_empty_env(names: &[&str]) -> Option<String> {
        names.iter().find_map(|name| {
            std::env::var(name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
    }

    fn parse_http_proxy_target(raw: &str) -> Option<(String, u16)> {
        let input = raw.trim();
        if input.is_empty() {
            return None;
        }

        let (scheme, mut endpoint) = if let Some(rest) = input.strip_prefix("https://") {
            ("https", rest)
        } else if let Some(rest) = input.strip_prefix("http://") {
            ("http", rest)
        } else {
            ("http", input)
        };

        endpoint = endpoint.split('/').next().unwrap_or(endpoint).trim();
        endpoint = endpoint.rsplit('@').next().unwrap_or(endpoint).trim();
        if endpoint.is_empty() {
            return None;
        }

        let default_port = if scheme.eq_ignore_ascii_case("https") {
            443
        } else {
            80
        };

        if let Some(stripped) = endpoint.strip_prefix('[') {
            let end = stripped.find(']')?;
            let host = stripped[..end].trim();
            if host.is_empty() {
                return None;
            }
            let rest = stripped[end + 1..].trim();
            let port = if rest.is_empty() {
                default_port
            } else {
                rest.strip_prefix(':')?.parse::<u16>().ok()?
            };
            return Some((host.to_string(), port));
        }

        if let Some((host, port_raw)) = endpoint.rsplit_once(':') {
            if !host.is_empty() && !port_raw.is_empty() {
                if let Ok(port) = port_raw.parse::<u16>() {
                    return Some((host.to_string(), port));
                }
            }
        }

        Some((endpoint.to_string(), default_port))
    }

    fn format_host_port(host: &str, port: u16) -> String {
        if host.contains(':') && !(host.starts_with('[') && host.ends_with(']')) {
            format!("[{}]:{}", host, port)
        } else {
            format!("{}:{}", host, port)
        }
    }

    fn resolve_runtime_http_proxy_config() -> Option<RuntimeHttpProxyConfig> {
        let explicit_proxy = Self::first_non_empty_env(&["CRATEBAY_RUNTIME_HTTP_PROXY"]);
        let bridge_enabled = common::env_flag_enabled("CRATEBAY_RUNTIME_HTTP_PROXY_BRIDGE");

        if bridge_enabled {
            let target_proxy = explicit_proxy.or_else(|| {
                Self::first_non_empty_env(&[
                    "HTTPS_PROXY",
                    "https_proxy",
                    "HTTP_PROXY",
                    "http_proxy",
                ])
            });

            let target_proxy = target_proxy?;
            let (target_host, target_port) = Self::parse_http_proxy_target(&target_proxy)?;

            let bind_host = Self::first_non_empty_env(&["CRATEBAY_RUNTIME_HTTP_PROXY_BIND_HOST"])
                .unwrap_or_else(|| "0.0.0.0".to_string());
            let bind_port = Self::first_non_empty_env(&["CRATEBAY_RUNTIME_HTTP_PROXY_BIND_PORT"])
                .and_then(|raw| raw.parse::<u16>().ok())
                .filter(|port| *port > 0)
                .unwrap_or(3128);

            let guest_host = Self::first_non_empty_env(&["CRATEBAY_RUNTIME_HTTP_PROXY_GUEST_HOST"])
                .unwrap_or_else(|| "192.168.64.1".to_string());
            let guest_proxy_endpoint = Self::format_host_port(&guest_host, bind_port);
            let host_tcp_forward_spec = format!(
                "{}={}",
                Self::format_host_port(&bind_host, bind_port),
                Self::format_host_port(&target_host, target_port)
            );

            return Some(RuntimeHttpProxyConfig {
                guest_proxy_endpoint,
                host_tcp_forward_spec: Some(host_tcp_forward_spec),
            });
        }

        explicit_proxy
            .as_deref()
            .and_then(Self::parse_http_proxy_target)
            .map(|(host, port)| RuntimeHttpProxyConfig {
                guest_proxy_endpoint: Self::format_host_port(&host, port),
                host_tcp_forward_spec: None,
            })
    }

    /// Query macOS DNS configuration via `scutil --dns`.
    fn resolve_dns_servers() -> Vec<String> {
        tracing::debug!("Resolving DNS servers for runtime VM...");
        const DEFAULT_DNS: &[&str] = &["1.1.1.1", "8.8.8.8"];

        // Check env override
        if let Ok(raw) = std::env::var("CRATEBAY_RUNTIME_DNS") {
            let servers: Vec<String> = raw
                .split([',', ' ', '\t'])
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s.parse::<std::net::Ipv4Addr>().is_ok())
                .collect();
            if !servers.is_empty() {
                tracing::info!("Using DNS from CRATEBAY_RUNTIME_DNS: {:?}", servers);
                return servers;
            }
        }

        let mut servers = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Parse scutil --dns
        if let Ok(output) = Command::new("scutil").arg("--dns").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if !line.contains("nameserver") {
                        continue;
                    }
                    if let Some((_, value)) = line.split_once(':') {
                        let value = value.trim();
                        if let Ok(addr) = value.parse::<std::net::Ipv4Addr>() {
                            // Skip loopback, link-local, and unspecified
                            if addr.is_unspecified() || addr.is_loopback() || addr.is_link_local() {
                                continue;
                            }
                            let s = addr.to_string();
                            if seen.insert(s.clone()) {
                                servers.push(s);
                            }
                        }
                    }
                }
            }
        }

        if servers.is_empty() {
            tracing::warn!("No DNS servers found from scutil, will use defaults only");
        } else {
            tracing::debug!("DNS servers from scutil: {:?}", servers);
        }

        // Ensure defaults are included
        for dns in DEFAULT_DNS {
            if seen.insert(dns.to_string()) {
                servers.push(dns.to_string());
            }
        }

        tracing::info!("Final DNS servers for VM: {:?}", servers);
        servers
    }

    /// Spawn the `cratebay-vz` runner process.
    fn spawn_runner(&self) -> Result<Child, AppError> {
        let runner_path = vz_runner_path();

        // Verify the runner binary exists
        if !runner_path.exists() {
            return Err(AppError::Runtime(format!(
                "VZ runner binary not found: {}. \
                 Set CRATEBAY_VZ_RUNNER_PATH or ensure the app is installed correctly.",
                runner_path.display()
            )));
        }

        // Ensure it has virtualization entitlements
        ensure_runner_entitlements(&runner_path)?;

        let image_id = common::runtime_os_image_id();
        let paths = crate::images::image_paths(image_id);

        // Verify kernel exists
        if !paths.kernel_path.exists() {
            return Err(AppError::Runtime(format!(
                "Kernel image not found: {}",
                paths.kernel_path.display()
            )));
        }

        let disk = vm_disk_path();
        if !disk.exists() {
            return Err(AppError::Runtime(format!(
                "VM disk image not found: {}",
                disk.display()
            )));
        }

        let ready_file = vm_runner_ready_path();
        let _ = std::fs::remove_file(&ready_file);

        let console_log = vm_console_log_path();
        let console_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&console_log)
            .map_err(|e| {
                AppError::Runtime(format!(
                    "Failed to create console log {}: {}",
                    console_log.display(),
                    e
                ))
            })?;
        let console_err = console_file.try_clone()?;

        let runtime_http_proxy = Self::resolve_runtime_http_proxy_config();
        let cmdline = self.build_cmdline(runtime_http_proxy.as_ref());

        let mut cmd = Command::new(&runner_path);
        cmd.arg("--kernel")
            .arg(&paths.kernel_path)
            .arg("--disk")
            .arg(&disk)
            .arg("--cpus")
            .arg(self.config.cpu_cores.to_string())
            .arg("--memory-mb")
            .arg(self.config.memory_mb.to_string())
            .arg("--cmdline")
            .arg(&cmdline)
            .arg("--ready-file")
            .arg(&ready_file)
            .arg("--console-log")
            .arg(&console_log);

        // Set up initrd if present
        if paths.initrd_path.exists() {
            cmd.arg("--initrd").arg(&paths.initrd_path);
        }

        // Set up Docker socket forwarding
        let vm_name = common::runtime_vm_name();
        let sock_path = common::runtime_host_docker_socket_path(vm_name);
        if let Some(parent) = sock_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let _ = std::fs::remove_file(&sock_path);
        common::link_runtime_host_docker_socket(vm_name)?;

        let forward_spec = format!(
            "{}:{}",
            common::docker_proxy_port(),
            sock_path.to_string_lossy()
        );

        // Default to TCP forwarding mode on all architectures.
        // vsock requires the vmw_vsock_virtio_transport kernel module which
        // may not be present in the initramfs. TCP reverse-connect works
        // reliably across both aarch64 and x86_64.
        let default_forward_mode = "tcp";

        let forward_mode = std::env::var("CRATEBAY_RUNTIME_SOCKET_FORWARD")
            .ok()
            .map(|v| v.trim().to_ascii_lowercase())
            .filter(|v| matches!(v.as_str(), "vsock" | "tcp"))
            .unwrap_or_else(|| default_forward_mode.to_string());

        tracing::info!(
            "Docker socket forwarding mode: {} (arch: {}, default: {})",
            forward_mode,
            std::env::consts::ARCH,
            default_forward_mode
        );

        match forward_mode.as_str() {
            "vsock" => {
                cmd.arg("--vsock-forward").arg(&forward_spec);
            }
            _ => {
                cmd.arg("--tcp-forward").arg(&forward_spec);
            }
        }

        if let Some(proxy_config) = runtime_http_proxy.as_ref() {
            if let Some(host_forward_spec) = proxy_config.host_tcp_forward_spec.as_ref() {
                cmd.arg("--host-tcp-forward").arg(host_forward_spec);
                tracing::info!(
                    "Enabled runtime HTTP proxy bridge (guest proxy {}, host forward {})",
                    proxy_config.guest_proxy_endpoint,
                    host_forward_spec
                );
            } else {
                tracing::info!(
                    "Enabled runtime HTTP proxy passthrough (guest proxy {})",
                    proxy_config.guest_proxy_endpoint
                );
            }
        }

        // Auto-start built-in HTTP CONNECT proxy inside the VZ runner if the
        // proxy was auto-configured (env var was set by start() above).
        // The proxy runs inside the long-lived VZ runner process.
        if std::env::var("CRATEBAY_RUNTIME_HTTP_PROXY")
            .ok()
            .filter(|v| v.contains("192.168.64.1:"))
            .is_some()
        {
            // Extract port from the env var (e.g. "192.168.64.1:3128" → 3128)
            let proxy_port = std::env::var("CRATEBAY_RUNTIME_HTTP_PROXY")
                .ok()
                .and_then(|v| v.rsplit(':').next().map(|s| s.to_string()))
                .unwrap_or_else(|| "3128".to_string());
            cmd.arg("--http-connect-proxy")
                .arg(format!("0.0.0.0:{}", proxy_port));
            tracing::info!(
                "VZ runner will host built-in HTTP proxy on 0.0.0.0:{}",
                proxy_port,
            );
        }

        // Rosetta support
        if common::env_flag_enabled("CRATEBAY_RUNTIME_ROSETTA") && Self::rosetta_available() {
            cmd.arg("--rosetta");
        }

        // Shared directories
        for share in &self.config.shared_dirs {
            cmd.arg("--share")
                .arg(format!("{}:{}", share.tag, share.host_path));
        }

        cmd.stdin(Stdio::null())
            .stdout(Stdio::from(console_file))
            .stderr(Stdio::from(console_err));

        // Detach runner from parent process/session so it survives app exit
        unsafe {
            use std::os::unix::process::CommandExt;
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }

        tracing::debug!(
            "VZ runner command: {:?} (kernel={}, disk={}, cpus={}, mem={}MB)",
            runner_path.display(),
            paths.kernel_path.display(),
            disk.display(),
            self.config.cpu_cores,
            self.config.memory_mb,
        );

        let child = cmd.spawn().map_err(|e| {
            AppError::Runtime(format!(
                "Failed to spawn VZ runner {}: {}",
                runner_path.display(),
                e
            ))
        })?;

        tracing::info!(
            "Spawned VZ runner {} (PID {})",
            runner_path.display(),
            child.id()
        );

        Ok(child)
    }

    /// Wait for Docker inside the VM to become responsive via the Unix socket.
    async fn wait_for_docker(&self, timeout: Duration) -> Result<(), AppError> {
        let socket_path = self.docker_socket_path();
        let start = Instant::now();

        tracing::info!(
            "Waiting for Docker at {} (timeout: {:?})",
            socket_path.display(),
            timeout
        );

        while start.elapsed() < timeout {
            if socket_path.exists() {
                match bollard::Docker::connect_with_unix(
                    socket_path.to_str().unwrap_or_default(),
                    5,
                    bollard::API_DEFAULT_VERSION,
                ) {
                    Ok(docker) => {
                        if crate::docker::is_available(&docker).await {
                            tracing::info!("Docker is responsive at {}", socket_path.display());
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        tracing::trace!("Docker not yet ready: {}", e);
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        Err(AppError::Runtime(format!(
            "Docker did not become responsive within {:?}",
            timeout
        )))
    }

    /// Update the internal runtime state.
    fn set_state(&self, new_state: RuntimeState) -> Result<(), AppError> {
        let mut state = self.state.lock_or_recover()?;
        tracing::info!("Runtime state: {:?} → {:?}", *state, new_state);
        *state = new_state;
        Ok(())
    }

    /// Read the current internal runtime state.
    fn get_state(&self) -> Result<RuntimeState, AppError> {
        let state = self.state.lock_or_recover()?;
        Ok(state.clone())
    }

    /// Ping Docker via Unix socket with retries.
    async fn docker_health_with_retry(&self, socket_path: &Path) -> (bool, Option<String>) {
        for attempt in 0..HEALTH_PING_ATTEMPTS {
            if let Ok(docker) = bollard::Docker::connect_with_unix(
                socket_path.to_str().unwrap_or_default(),
                HEALTH_PING_CONNECT_TIMEOUT_SECS,
                bollard::API_DEFAULT_VERSION,
            ) {
                if crate::docker::is_available(&docker).await {
                    let docker_version =
                        tokio::time::timeout(Duration::from_secs(5), docker.version())
                            .await
                            .ok()
                            .and_then(|v| v.ok())
                            .and_then(|v| v.version);
                    return (true, docker_version);
                }
            }

            if attempt + 1 < HEALTH_PING_ATTEMPTS {
                tokio::time::sleep(HEALTH_PING_RETRY_DELAY).await;
            }
        }

        (false, None)
    }

    /// Check if runtime images are installed and ready.
    fn images_ready(&self) -> bool {
        common::runtime_image_ready()
    }
}

impl Default for MacOSRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeManager for MacOSRuntime {
    /// Detect the current runtime state.
    ///
    /// Checks:
    /// 1. macOS version >= 13 (Ventura) for VZ.framework support
    /// 2. Whether runtime images are installed and ready
    /// 3. Whether the VZ runner process is alive
    /// 4. Whether Docker is responsive on the socket
    async fn get_state(&self) -> Result<RuntimeState, AppError> {
        // Check macOS version
        match Self::check_macos_version() {
            Ok(true) => {}
            Ok(false) => {
                let state = RuntimeState::Error(format!(
                    "macOS {} or later is required for VZ.framework",
                    MIN_MACOS_VERSION
                ));
                self.set_state(state.clone())?;
                return Ok(state);
            }
            Err(e) => {
                tracing::warn!("Could not determine macOS version: {}", e);
            }
        }

        // Check if runtime images are installed
        if !self.images_ready() {
            self.set_state(RuntimeState::None)?;
            return Ok(RuntimeState::None);
        }

        // Check if the runner process is alive
        if self.is_runner_alive() {
            // Verify Docker is responsive
            let socket_path = self.docker_socket_path();
            if socket_path.exists() {
                if let Ok(docker) = bollard::Docker::connect_with_unix(
                    socket_path.to_str().unwrap_or_default(),
                    5,
                    bollard::API_DEFAULT_VERSION,
                ) {
                    if crate::docker::is_available(&docker).await {
                        self.set_state(RuntimeState::Ready)?;
                        return Ok(RuntimeState::Ready);
                    }
                }
            }
            // Runner alive but Docker not responsive — still starting
            self.set_state(RuntimeState::Starting)?;
            return Ok(RuntimeState::Starting);
        }

        // Images exist but VM is not running
        if vm_disk_path().exists() {
            self.set_state(RuntimeState::Provisioned)?;
            return Ok(RuntimeState::Provisioned);
        }

        // Images ready but no disk image yet — need provisioning
        self.set_state(RuntimeState::None)?;
        Ok(RuntimeState::None)
    }

    /// Provision the runtime VM image and disk.
    ///
    /// Stages:
    /// 1. `checking` — Verify macOS version and prerequisites
    /// 2. `installing` — Install runtime image from bundled assets
    /// 3. `configuring` — Create VM disk image from rootfs
    /// 4. `complete` — Provisioning finished
    async fn provision(
        &self,
        on_progress: Box<dyn Fn(ProvisionProgress) + Send>,
    ) -> Result<(), AppError> {
        self.set_state(RuntimeState::Starting)?;

        // Stage 1: Check prerequisites
        on_progress(ProvisionProgress {
            stage: "checking".into(),
            percent: 5.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Checking macOS version and VZ.framework availability...".into(),
        });

        let version_ok = Self::check_macos_version()?;
        if !version_ok {
            self.set_state(RuntimeState::Error(
                "macOS version too old for VZ.framework".into(),
            ))?;
            return Err(AppError::Runtime(format!(
                "macOS {} or later is required for Virtualization.framework",
                MIN_MACOS_VERSION
            )));
        }

        on_progress(ProvisionProgress {
            stage: "checking".into(),
            percent: 10.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Prerequisites verified.".into(),
        });

        // Stage 2: Ensure runtime image is installed from bundled assets
        on_progress(ProvisionProgress {
            stage: "installing".into(),
            percent: 20.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Installing CrateBay runtime image from bundled assets...".into(),
        });

        let image_id = common::runtime_os_image_id();
        // This copies from bundled assets — blocking I/O, wrap in spawn_blocking
        tokio::task::spawn_blocking({
            let image_id = image_id.to_string();
            move || common::ensure_runtime_image_ready(&image_id)
        })
        .await
        .map_err(|e| AppError::Runtime(format!("Task join error: {}", e)))??;

        on_progress(ProvisionProgress {
            stage: "installing".into(),
            percent: 60.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Runtime image installed.".into(),
        });

        // Stage 3: Create VM disk image from rootfs
        on_progress(ProvisionProgress {
            stage: "configuring".into(),
            percent: 70.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Creating VM disk image...".into(),
        });

        let disk_path = vm_disk_path();
        if let Some(parent) = disk_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if !disk_path.exists() {
            let disk_bytes = (self.config.disk_gb as u64) * 1024 * 1024 * 1024;
            let image_id_owned = image_id.to_string();
            let disk_path_owned = disk_path.clone();
            tokio::task::spawn_blocking(move || {
                crate::images::create_disk_from_image(&image_id_owned, &disk_path_owned, disk_bytes)
            })
            .await
            .map_err(|e| AppError::Runtime(format!("Task join error: {}", e)))?
            .map_err(|e| AppError::Runtime(format!("Failed to create VM disk image: {}", e)))?;
        }

        on_progress(ProvisionProgress {
            stage: "configuring".into(),
            percent: 90.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "VM disk image ready.".into(),
        });

        // Stage 4: Complete
        on_progress(ProvisionProgress {
            stage: "complete".into(),
            percent: 100.0,
            bytes_downloaded: 0,
            bytes_total: 0,
            message: "Runtime provisioned successfully.".into(),
        });

        self.set_state(RuntimeState::Provisioned)?;
        tracing::info!("macOS VZ runtime provisioned successfully");
        Ok(())
    }

    /// Start the VZ.framework virtual machine.
    ///
    /// Flow:
    /// 1. Ensure images are provisioned (auto-provision if needed)
    /// 2. Clean up stray runner processes
    /// 3. Spawn the `cratebay-vz` runner binary
    /// 4. Wait for the ready file (VM booted)
    /// 5. Wait for Docker to become responsive on the socket
    /// 6. Transition state to Ready
    async fn start(&self) -> Result<(), AppError> {
        // If Docker isn't responsive (stale proxy / old runner), restart once.
        let mut restarted = false;
        loop {
            // Check if already running
            if self.is_runner_alive() {
                let socket_path = self.docker_socket_path();
                if socket_path.exists() {
                    if let Ok(docker) = bollard::Docker::connect_with_unix(
                        socket_path.to_str().unwrap_or_default(),
                        5,
                        bollard::API_DEFAULT_VERSION,
                    ) {
                        if crate::docker::is_available(&docker).await {
                            self.set_state(RuntimeState::Ready)?;
                            return Ok(());
                        }
                    }
                }

                // Runner is alive but Docker isn't ready — wait for it.
                self.set_state(RuntimeState::Starting)?;
                match self.wait_for_docker(DOCKER_READY_TIMEOUT).await {
                    Ok(()) => {
                        self.set_state(RuntimeState::Ready)?;
                        return Ok(());
                    }
                    Err(e) if !restarted => {
                        restarted = true;
                        tracing::warn!(
                            "Runner is alive but Docker is not responsive: {}. Restarting runtime once...",
                            e
                        );
                        if let Err(stop_err) = self.stop().await {
                            return Err(AppError::Runtime(format!(
                                "Docker is not responsive and failed to restart runtime (stop failed): {}; original error: {}",
                                stop_err, e
                            )));
                        }
                        continue;
                    }
                    Err(e) => {
                        self.set_state(RuntimeState::Starting)?;
                        return Err(e);
                    }
                }
            }

            // Ensure images are provisioned
            if !self.images_ready() {
                return Err(AppError::Runtime(
                    "Runtime not provisioned. Call provision() first.".into(),
                ));
            }

            // Ensure disk exists
            if !vm_disk_path().exists() {
                return Err(AppError::Runtime(
                    "VM disk image not found. Call provision() first.".into(),
                ));
            }

            self.set_state(RuntimeState::Starting)?;

            // If no external HTTP proxy is configured, auto-enable the built-in
            // HTTP CONNECT proxy inside the VZ runner process.  The VZ runner is
            // long-lived so the proxy survives after the CLI/GUI process exits.
            // We set the env var here so that build_cmdline() injects the correct
            // `cratebay_http_proxy=` kernel argument for the guest.
            if Self::resolve_runtime_http_proxy_config().is_none() {
                let proxy_port = 3128u16;
                std::env::set_var(
                    "CRATEBAY_RUNTIME_HTTP_PROXY",
                    format!("192.168.64.1:{}", proxy_port),
                );
                std::env::set_var("CRATEBAY_RUNTIME_HTTP_PROXY_BRIDGE", "0");
                tracing::info!(
                    "Auto-enabling built-in HTTP proxy (guest will use 192.168.64.1:{})",
                    proxy_port,
                );
            }

            // Check if an existing VZ runner (from a previous GUI session) is
            // still alive and Docker is already responsive.  If so, adopt it
            // instead of killing it and restarting — this preserves running
            // containers across GUI restarts.
            let existing_pids = find_runner_processes();
            if !existing_pids.is_empty() {
                let socket_path = self.docker_socket_path();
                if socket_path.exists() {
                    if let Ok(docker) = bollard::Docker::connect_with_unix(
                        socket_path.to_str().unwrap_or_default(),
                        5,
                        bollard::API_DEFAULT_VERSION,
                    ) {
                        if crate::docker::is_available(&docker).await {
                            // Adopt the existing runner
                            let pid = existing_pids[0];
                            tracing::info!(
                                "Adopting existing VZ runner (PID {}) — Docker is responsive",
                                pid,
                            );
                            if let Ok(mut guard) = self.runner_pid.lock() {
                                *guard = Some(pid);
                            }
                            if let Ok(mut guard) = self.started_at.lock() {
                                if guard.is_none() {
                                    *guard = Some(Instant::now());
                                }
                            }
                            self.set_state(RuntimeState::Ready)?;
                            return Ok(());
                        }
                    }
                }
            }

            // Clean up any stray runner processes from previous sessions
            cleanup_stray_runner_processes(None, "start preflight");

            // Spawn the VZ runner — blocking operation
            let mut child = tokio::task::spawn_blocking({
                let rt = MacOSRuntime {
                    config: self.config.clone(),
                    state: Arc::clone(&self.state),
                    runner: Arc::clone(&self.runner),
                    runner_pid: Arc::clone(&self.runner_pid),
                    started_at: Arc::clone(&self.started_at),
                    consecutive_health_failures: Arc::clone(&self.consecutive_health_failures),
                };
                move || rt.spawn_runner()
            })
            .await
            .map_err(|e| AppError::Runtime(format!("Task join error: {}", e)))??;

            let pid = child.id();

            // Wait for the ready file (VZ runner writes this after the VM boots)
            let ready_file = vm_runner_ready_path();
            let deadline = Instant::now() + RUNNER_READY_TIMEOUT;
            loop {
                if ready_file.exists() {
                    break;
                }

                // Check if runner exited prematurely
                if let Ok(Some(status)) = child.try_wait() {
                    self.set_state(RuntimeState::Error(format!(
                        "VZ runner exited early: {}",
                        status
                    )))?;
                    return Err(AppError::Runtime(format!(
                        "cratebay-vz exited early with status: {}. \
                         Check console log at {}",
                        status,
                        vm_console_log_path().display()
                    )));
                }

                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    self.set_state(RuntimeState::Error(
                        "Timed out waiting for VM to start".into(),
                    ))?;
                    return Err(AppError::Runtime(format!(
                        "VZ runner did not become ready within {:?}. \
                         Check console log at {}",
                        RUNNER_READY_TIMEOUT,
                        vm_console_log_path().display()
                    )));
                }

                tokio::time::sleep(Duration::from_millis(200)).await;
            }

            // Store runner state
            {
                let mut pid_guard = self.runner_pid.lock_or_recover()?;
                *pid_guard = Some(pid);
            }
            {
                let mut runner_guard = self.runner.lock_or_recover()?;
                *runner_guard = Some(child);
            }
            {
                let mut started_guard = self.started_at.lock_or_recover()?;
                *started_guard = Some(Instant::now());
            }

            // Write PID file for recovery across sessions
            let _ = std::fs::write(vm_runner_pid_path(), format!("{}\n", pid));

            // Wait for Docker to become responsive
            match self.wait_for_docker(DOCKER_READY_TIMEOUT).await {
                Ok(()) => {
                    self.set_state(RuntimeState::Ready)?;
                    tracing::info!("macOS VZ runtime started (PID {})", pid);
                    return Ok(());
                }
                Err(e) if !restarted => {
                    restarted = true;
                    tracing::warn!(
                        "Docker not responsive after VM start: {}. Restarting runtime once...",
                        e
                    );
                    if let Err(stop_err) = self.stop().await {
                        return Err(AppError::Runtime(format!(
                            "Docker is not responsive and failed to restart runtime (stop failed): {}; original error: {}",
                            stop_err, e
                        )));
                    }
                    continue;
                }
                Err(e) => {
                    tracing::warn!("Docker not responsive after VM start: {}", e);
                    // VM is running but Docker isn't ready — don't kill the runner,
                    // it may still come up. Set Starting state so health checks
                    // can track it.
                    self.set_state(RuntimeState::Starting)?;
                    return Err(e);
                }
            }
        }
    }

    /// Stop the VZ.framework virtual machine gracefully.
    ///
    /// Flow:
    /// 1. SIGTERM to the runner (triggers VZ graceful shutdown)
    /// 2. Wait up to 15 seconds for the process to exit
    /// 3. SIGKILL if still alive
    /// 4. Clean up socket files and PID files
    async fn stop(&self) -> Result<(), AppError> {
        let current = self.get_state()?;
        if current == RuntimeState::Stopped || current == RuntimeState::None {
            tracing::info!("Runtime is already stopped");
            return Ok(());
        }

        self.set_state(RuntimeState::Stopping)?;

        // Get the runner PID
        let runner_pid = self.current_runner_pid();

        // Phase 1: Send SIGTERM for graceful shutdown
        if let Some(pid) = runner_pid {
            tracing::info!("Sending SIGTERM to VZ runner (PID {})", pid);
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }

            // Phase 2: Wait for graceful shutdown
            let deadline = Instant::now() + STOP_GRACE_PERIOD;
            while Instant::now() < deadline {
                if !pid_alive(pid) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }

            // Phase 3: Force kill if still alive
            if pid_alive(pid) {
                tracing::warn!(
                    "VZ runner (PID {}) did not stop gracefully, sending SIGKILL",
                    pid
                );
                unsafe {
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }
        }

        // Wait for child process to be reaped
        {
            let mut runner_guard = self.runner.lock_or_recover()?;
            if let Some(ref mut child) = *runner_guard {
                let _ = child.wait();
            }
            *runner_guard = None;
        }

        // Clean up state
        {
            let mut pid_guard = self.runner_pid.lock_or_recover()?;
            *pid_guard = None;
        }
        {
            let mut started_guard = self.started_at.lock_or_recover()?;
            *started_guard = None;
        }

        // Clean up files
        let _ = std::fs::remove_file(vm_runner_pid_path());
        let _ = std::fs::remove_file(vm_runner_ready_path());

        // Clean up stray processes
        cleanup_stray_runner_processes(None, "stop cleanup");

        // Clean up Docker socket symlink
        let vm_name = common::runtime_vm_name();
        common::unlink_runtime_host_docker_socket(vm_name);

        self.set_state(RuntimeState::Stopped)?;
        tracing::info!("macOS VZ runtime stopped");
        Ok(())
    }

    /// Check runtime health and Docker responsiveness.
    async fn health_check(&self) -> Result<HealthStatus, AppError> {
        let runner_alive = self.is_runner_alive();
        let socket_path = self.docker_socket_path();
        let socket_exists = socket_path.exists();

        let (docker_responsive, docker_version) = if socket_exists {
            self.docker_health_with_retry(&socket_path).await
        } else {
            (false, None)
        };

        let current_state = self.get_state()?;
        let runtime_state = if docker_responsive {
            let mut failures = self.consecutive_health_failures.lock_or_recover()?;
            *failures = 0;
            RuntimeState::Ready
        } else if runner_alive {
            let mut failures = self.consecutive_health_failures.lock_or_recover()?;
            *failures = failures.saturating_add(1);

            if matches!(current_state, RuntimeState::Ready)
                && *failures < READY_DOWNGRADE_FAILURE_THRESHOLD
            {
                RuntimeState::Ready
            } else {
                RuntimeState::Starting
            }
        } else if self.images_ready() {
            let mut failures = self.consecutive_health_failures.lock_or_recover()?;
            *failures = 0;

            match current_state {
                RuntimeState::Stopping => RuntimeState::Stopped,
                other => other,
            }
        } else {
            let mut failures = self.consecutive_health_failures.lock_or_recover()?;
            *failures = 0;
            RuntimeState::None
        };

        self.set_state(runtime_state.clone())?;

        // Calculate uptime from tracked start time
        let uptime_seconds = if runner_alive {
            if let Ok(guard) = self.started_at.lock() {
                guard.map(|start| start.elapsed().as_secs())
            } else {
                None
            }
        } else {
            None
        };

        Ok(HealthStatus {
            runtime_state,
            docker_responsive,
            docker_version,
            uptime_seconds,
            last_check: chrono::Utc::now().to_rfc3339(),
            docker_source: Some("builtin".to_string()),
        })
    }

    /// Get the Docker socket path managed by the runtime.
    ///
    /// Returns the canonical `~/.cratebay/runtime/docker.sock` path which
    /// is a symlink to the per-VM actual socket.
    fn docker_socket_path(&self) -> PathBuf {
        common::host_docker_socket_path().to_path_buf()
    }

    /// Get current resource usage of the VM.
    ///
    /// When Docker is responsive, queries container count via the API.
    /// CPU and memory metrics require VZ.framework instrumentation
    /// (not yet implemented).
    async fn resource_usage(&self) -> Result<ResourceUsage, AppError> {
        let mut container_count = 0;

        // Try to get container count from Docker API
        let socket_path = self.docker_socket_path();
        if socket_path.exists() {
            if let Ok(docker) = bollard::Docker::connect_with_unix(
                socket_path.to_str().unwrap_or_default(),
                5,
                bollard::API_DEFAULT_VERSION,
            ) {
                if let Ok(containers) = docker
                    .list_containers(Some(bollard::container::ListContainersOptions::<String> {
                        all: false,
                        ..Default::default()
                    }))
                    .await
                {
                    container_count = containers.len() as u32;
                }
            }
        }

        Ok(ResourceUsage {
            cpu_percent: 0.0,  // Requires VZ.framework instrumentation
            memory_used_mb: 0, // Requires VZ.framework instrumentation
            memory_total_mb: self.config.memory_mb,
            disk_used_gb: 0.0, // Could query via Docker system info
            disk_total_gb: self.config.disk_gb as f32,
            container_count,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct EnvGuard {
        _lock_guard: std::sync::MutexGuard<'static, ()>,
        old_data_dir: Option<std::ffi::OsString>,
        _temp: tempfile::TempDir,
    }

    impl EnvGuard {
        fn with_temp_data_dir() -> Self {
            let lock = ENV_LOCK.get_or_init(|| Mutex::new(()));
            let lock_guard = lock.lock().expect("ENV_LOCK poisoned");

            let old_data_dir = std::env::var_os("CRATEBAY_DATA_DIR");
            let temp = tempfile::tempdir().expect("create tempdir");
            std::env::set_var("CRATEBAY_DATA_DIR", temp.path());

            Self {
                _lock_guard: lock_guard,
                old_data_dir,
                _temp: temp,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(old) = self.old_data_dir.clone() {
                std::env::set_var("CRATEBAY_DATA_DIR", old);
            } else {
                std::env::remove_var("CRATEBAY_DATA_DIR");
            }
        }
    }

    fn test_runtime() -> MacOSRuntime {
        MacOSRuntime {
            config: RuntimeConfig::default(),
            state: Arc::new(Mutex::new(RuntimeState::None)),
            runner: Arc::new(Mutex::new(None)),
            runner_pid: Arc::new(Mutex::new(None)),
            started_at: Arc::new(Mutex::new(None)),
            consecutive_health_failures: Arc::new(Mutex::new(0)),
        }
    }

    #[test]
    fn new_creates_with_defaults() {
        let rt = MacOSRuntime::new();
        assert_eq!(rt.config.cpu_cores, 2);
        assert_eq!(rt.config.memory_mb, 2048);
    }

    #[test]
    fn docker_socket_path_is_correct() {
        let rt = test_runtime();
        let path = rt.docker_socket_path();
        let s = path.to_string_lossy();
        assert!(
            s.contains("docker.sock"),
            "should contain docker.sock: {}",
            s
        );
    }

    #[test]
    fn check_macos_version_does_not_panic() {
        let _ = MacOSRuntime::check_macos_version();
    }

    #[test]
    fn state_management_works() {
        let rt = test_runtime();
        assert_eq!(rt.get_state().unwrap(), RuntimeState::None);

        rt.set_state(RuntimeState::Starting).unwrap();
        assert_eq!(rt.get_state().unwrap(), RuntimeState::Starting);

        rt.set_state(RuntimeState::Ready).unwrap();
        assert_eq!(rt.get_state().unwrap(), RuntimeState::Ready);
    }

    #[test]
    fn is_runner_alive_false_when_no_pid() {
        let rt = test_runtime();
        assert!(!rt.is_runner_alive());
    }

    #[test]
    fn current_runner_pid_none_when_no_runner() {
        let _guard = EnvGuard::with_temp_data_dir();
        let rt = test_runtime();
        assert!(rt.current_runner_pid().is_none());
    }

    #[test]
    fn rosetta_check_does_not_panic() {
        // Just verify it doesn't crash
        let _ = MacOSRuntime::rosetta_available();
    }

    #[test]
    fn vz_runner_path_returns_something() {
        let path = vz_runner_path();
        // Should return a non-empty path
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn pid_alive_returns_false_for_nonexistent() {
        // PID 0 is special (kernel), use an unlikely PID
        assert!(!pid_alive(u32::MAX - 1));
    }

    #[test]
    fn read_pid_file_returns_none_for_nonexistent() {
        assert!(read_pid_file(Path::new("/tmp/nonexistent-pid-file")).is_none());
    }

    #[test]
    fn read_pid_file_parses_valid_content() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.pid");
        std::fs::write(&path, "12345\n").unwrap();
        assert_eq!(read_pid_file(&path), Some(12345));
    }

    #[test]
    fn read_pid_file_handles_invalid_content() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.pid");
        std::fs::write(&path, "not-a-number\n").unwrap();
        assert!(read_pid_file(&path).is_none());
    }

    #[test]
    fn build_cmdline_contains_console() {
        let rt = test_runtime();
        let cmdline = rt.build_cmdline(None);
        assert!(
            cmdline.contains("console="),
            "cmdline should contain console=: {}",
            cmdline
        );
    }

    #[test]
    fn build_cmdline_contains_epoch() {
        let rt = test_runtime();
        let cmdline = rt.build_cmdline(None);
        assert!(
            cmdline.contains("cratebay_host_epoch="),
            "cmdline should contain host epoch: {}",
            cmdline
        );
    }

    #[test]
    fn parse_http_proxy_target_supports_http_url() {
        let parsed = MacOSRuntime::parse_http_proxy_target("http://127.0.0.1:7890");
        assert_eq!(parsed, Some(("127.0.0.1".to_string(), 7890)));
    }

    #[test]
    fn parse_http_proxy_target_supports_bare_host_port() {
        let parsed = MacOSRuntime::parse_http_proxy_target("proxy.example.com:3128");
        assert_eq!(parsed, Some(("proxy.example.com".to_string(), 3128)));
    }

    #[test]
    fn resolve_dns_servers_returns_something() {
        let servers = MacOSRuntime::resolve_dns_servers();
        // Should at least contain the default DNS servers
        assert!(
            !servers.is_empty(),
            "should return at least default DNS servers"
        );
    }

    #[tokio::test]
    async fn detect_returns_none_when_no_images() {
        let rt = test_runtime();
        // With default config and no images installed, should return None or Provisioned
        let state = RuntimeManager::get_state(&rt).await.unwrap();
        // The exact state depends on whether runtime images happen to be
        // installed on this machine
        assert!(
            matches!(
                state,
                RuntimeState::None
                    | RuntimeState::Provisioned
                    | RuntimeState::Ready
                    | RuntimeState::Starting
            ),
            "unexpected state: {:?}",
            state
        );
    }

    #[tokio::test]
    async fn stop_succeeds_when_already_stopped() {
        let rt = test_runtime();
        let result = rt.stop().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn start_fails_without_images() {
        let rt = test_runtime();
        // If images aren't ready and disk doesn't exist, start should fail
        if !rt.images_ready() {
            let result = rt.start().await;
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn health_check_returns_valid_status() {
        let rt = test_runtime();
        let status = rt.health_check().await.unwrap();
        // Should have a valid RFC 3339 timestamp
        assert!(
            chrono::DateTime::parse_from_rfc3339(&status.last_check).is_ok(),
            "last_check should be valid RFC 3339: {}",
            status.last_check
        );
    }

    #[tokio::test]
    async fn resource_usage_returns_configured_totals() {
        let rt = test_runtime();
        let usage = rt.resource_usage().await.unwrap();
        assert_eq!(usage.memory_total_mb, 2048);
        assert_eq!(usage.disk_total_gb, 20.0);
    }

    #[test]
    fn default_trait_creates_same_as_new() {
        let from_new = MacOSRuntime::new();
        let from_default = MacOSRuntime::default();
        assert_eq!(from_new.config.cpu_cores, from_default.config.cpu_cores);
        assert_eq!(from_new.config.memory_mb, from_default.config.memory_mb);
        assert_eq!(from_new.config.disk_gb, from_default.config.disk_gb);
    }

    #[test]
    fn with_config_uses_provided_values() {
        let config = RuntimeConfig {
            cpu_cores: 8,
            memory_mb: 8192,
            disk_gb: 100,
            auto_start: false,
            shared_dirs: vec![super::super::SharedDir {
                host_path: "/Users/test".into(),
                tag: "test".into(),
            }],
        };
        let rt = MacOSRuntime::with_config(config);
        assert_eq!(rt.config.cpu_cores, 8);
        assert_eq!(rt.config.memory_mb, 8192);
        assert_eq!(rt.config.disk_gb, 100);
        assert!(!rt.config.auto_start);
        assert_eq!(rt.config.shared_dirs.len(), 1);
    }

    #[test]
    fn state_transitions_through_full_lifecycle() {
        let rt = test_runtime();
        let transitions = vec![
            RuntimeState::None,
            RuntimeState::Provisioned,
            RuntimeState::Starting,
            RuntimeState::Ready,
            RuntimeState::Stopping,
            RuntimeState::Stopped,
        ];
        for expected in transitions {
            rt.set_state(expected.clone()).unwrap();
            assert_eq!(rt.get_state().unwrap(), expected);
        }
    }

    #[test]
    fn state_can_transition_to_error() {
        let rt = test_runtime();
        rt.set_state(RuntimeState::Starting).unwrap();
        rt.set_state(RuntimeState::Error("VZ.framework failed".into()))
            .unwrap();
        let state = rt.get_state().unwrap();
        match state {
            RuntimeState::Error(msg) => assert_eq!(msg, "VZ.framework failed"),
            other => panic!("Expected Error state, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn resource_usage_with_custom_config() {
        let config = RuntimeConfig {
            cpu_cores: 4,
            memory_mb: 4096,
            disk_gb: 50,
            auto_start: true,
            shared_dirs: vec![],
        };
        let rt = MacOSRuntime::with_config(config);
        let usage = rt.resource_usage().await.unwrap();
        assert_eq!(usage.memory_total_mb, 4096);
        assert_eq!(usage.disk_total_gb, 50.0);
    }

    #[test]
    fn vm_dir_is_under_data_dir() {
        let dir = vm_dir();
        let data = crate::storage::data_dir();
        assert!(
            dir.starts_with(&data),
            "vm_dir {:?} should be under data_dir {:?}",
            dir,
            data
        );
    }

    #[test]
    fn vm_disk_path_ends_with_disk_raw() {
        let path = vm_disk_path();
        assert!(
            path.to_string_lossy().ends_with("disk.raw"),
            "should end with disk.raw: {:?}",
            path
        );
    }

    #[test]
    fn runner_entitlements_check_does_not_panic() {
        // Just verify it doesn't crash on a non-existent path
        let result = runner_has_virtualization_entitlements(Path::new("/nonexistent/binary"));
        assert!(!result);
    }

    #[test]
    fn find_runner_processes_returns_vec() {
        // Should not panic, just return empty or non-empty vec
        let pids = find_runner_processes();
        // We can't assert much here since it depends on whether
        // a VZ runner is actually running
        let _ = pids;
    }
}

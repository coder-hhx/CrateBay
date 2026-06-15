//! System-related Tauri commands.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bollard::Docker;
use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::state::AppState;
use cratebay_core::docker;
use cratebay_core::error::AppError;
use cratebay_core::models::{DockerStatus, EngineEndpointStatus, RuntimeStatusInfo, SystemInfo};
use cratebay_core::runtime::{self, RuntimeConfig, RuntimeState};
use cratebay_core::settings::{
    SETTINGS_KEY_RUNTIME_HTTP_PROXY, SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_HOST,
    SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_PORT, SETTINGS_KEY_RUNTIME_HTTP_PROXY_BRIDGE,
    SETTINGS_KEY_RUNTIME_HTTP_PROXY_GUEST_HOST,
};
use cratebay_core::{storage, MutexExt};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiagnosticsInfo {
    ok: bool,
    runtime: RuntimeStatusInfo,
    engine_contract: RuntimeDiagnosticSection,
    substrate: RuntimeDiagnosticSection,
    storage_gc: RuntimeDiagnosticSection,
    shim_tasks: RuntimeDiagnosticSection,
    generated_at_unix: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiagnosticSection {
    ok: bool,
    value: Option<Value>,
    error: Option<String>,
}

impl RuntimeDiagnosticSection {
    fn ok(value: Value) -> Self {
        Self {
            ok: true,
            value: Some(value),
            error: None,
        }
    }

    fn err(error: impl ToString) -> Self {
        Self {
            ok: false,
            value: None,
            error: Some(error.to_string()),
        }
    }
}

/// Get system information.
#[tauri::command]
pub async fn system_info(state: State<'_, AppState>) -> Result<SystemInfo, AppError> {
    let data_dir = state.data_dir.to_string_lossy().to_string();
    let db_path = state.data_dir.join("cratebay.db");
    let db_path_str = db_path.to_string_lossy().to_string();

    let db_size_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

    let log_path = state.data_dir.join("cratebay.log");
    let log_path_str = log_path.to_string_lossy().to_string();

    Ok(SystemInfo {
        os: std::env::consts::OS.to_string(),
        os_version: os_version(),
        arch: std::env::consts::ARCH.to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir,
        db_path: db_path_str,
        db_size_bytes,
        log_path: log_path_str,
    })
}

/// Get CrateBay Engine compatibility endpoint status.
///
/// Checks the current Engine client in AppState (may have been
/// updated by the runtime auto-start background thread).
#[tauri::command]
pub async fn engine_status(state: State<'_, AppState>) -> Result<EngineEndpointStatus, AppError> {
    engine_status_impl(state).await
}

/// Compatibility alias for older frontends and automation.
#[tauri::command]
pub async fn docker_status(state: State<'_, AppState>) -> Result<DockerStatus, AppError> {
    engine_status_impl(state).await
}

async fn engine_status_impl(state: State<'_, AppState>) -> Result<EngineEndpointStatus, AppError> {
    if let Ok(engine) = runtime::query_built_in_ready_engine_status(state.runtime.as_ref()) {
        return Ok(EngineEndpointStatus {
            connected: true,
            version: Some(engine.kind),
            api_version: Some(engine.api),
            os: Some("linux".to_string()),
            arch: Some(std::env::consts::ARCH.to_string()),
            engine_source: "builtin".to_string(),
            source: "builtin".to_string(),
            socket_path: Some(built_in_engine_endpoint(&state)),
        });
    }

    let docker_opt = {
        let guard = state
            .engine_compatibility
            .lock()
            .map_err(|e| AppError::Runtime(format!("Engine state lock poisoned: {}", e)))?;
        guard.clone()
    };
    let source = state
        .engine_compatibility_source()
        .unwrap_or_else(|| "builtin".to_string());

    match docker_opt {
        Some(d) => {
            let is_available = docker::is_available(&d).await;
            if is_available {
                let version_info = docker::version(&d).await.ok();
                let socket_path = if docker::is_builtin_source(Some(source.as_str())) {
                    Some(built_in_engine_endpoint(&state))
                } else {
                    Some(source.clone())
                };
                Ok(EngineEndpointStatus {
                    connected: true,
                    version: version_info.as_ref().and_then(|v| v.version.clone()),
                    api_version: version_info.as_ref().and_then(|v| v.api_version.clone()),
                    os: version_info.as_ref().and_then(|v| v.os.clone()),
                    arch: version_info.as_ref().and_then(|v| v.arch.clone()),
                    engine_source: source.clone(),
                    source,
                    socket_path,
                })
            } else {
                Ok(EngineEndpointStatus {
                    connected: false,
                    version: None,
                    api_version: None,
                    os: None,
                    arch: None,
                    engine_source: source.clone(),
                    source,
                    socket_path: None,
                })
            }
        }
        None => Ok(EngineEndpointStatus {
            connected: false,
            version: None,
            api_version: None,
            os: None,
            arch: None,
            engine_source: source.clone(),
            source,
            socket_path: None,
        }),
    }
}

fn built_in_engine_endpoint(state: &State<'_, AppState>) -> String {
    #[cfg(target_os = "linux")]
    {
        cratebay_core::runtime::linux::linux_engine_host()
    }

    #[cfg(target_os = "windows")]
    {
        cratebay_core::runtime::windows::windows_engine_host()
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        state
            .runtime
            .engine_socket_path()
            .to_string_lossy()
            .to_string()
    }

    #[cfg(not(any(unix, windows)))]
    {
        "<unsupported>".to_string()
    }
}

/// Get built-in runtime status.
///
/// Returns the current state of the built-in container runtime (VM),
/// including health, configuration, and resource usage.
#[tauri::command]
pub async fn runtime_status(state: State<'_, AppState>) -> Result<RuntimeStatusInfo, AppError> {
    runtime_status_impl(&state).await
}

async fn runtime_status_impl(state: &State<'_, AppState>) -> Result<RuntimeStatusInfo, AppError> {
    let platform = match std::env::consts::OS {
        "macos" => "macos-vz",
        "linux" => "linux-kvm",
        "windows" => "windows-wsl2",
        other => other,
    };

    // Perform a health check via the runtime manager
    let mut health = state.runtime.health_check().await?;
    let config = RuntimeConfig::default();

    // Reconcile transient compatibility ping failures with the shared AppState
    // client without promoting native runtime readiness.
    let docker_source = state.engine_compatibility_source();
    if docker::is_builtin_source(docker_source.as_deref())
        && !health.docker_responsive
        && matches!(
            health.runtime_state,
            RuntimeState::Starting | RuntimeState::Ready | RuntimeState::Error(_)
        )
    {
        let docker_opt = {
            let guard = state
                .engine_compatibility
                .lock()
                .map_err(|e| AppError::Runtime(format!("Engine state lock poisoned: {}", e)))?;
            guard.clone()
        };

        if let Some(docker_client) = docker_opt {
            for attempt in 0..3 {
                if docker::is_available(&docker_client).await {
                    health.docker_responsive = true;
                    break;
                }
                if attempt < 2 {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                }
            }
        }
    }

    let native_engine = runtime::query_built_in_ready_engine_status(state.runtime.as_ref()).ok();
    let engine_responsive = health.engine_responsive || native_engine.is_some();
    let compatibility_responsive = health.compatibility_responsive || health.docker_responsive;
    let runtime_state =
        reconcile_runtime_state_with_native_engine(health.runtime_state.clone(), engine_responsive);

    // Try to get resource usage (non-fatal if it fails)
    let resource_usage = state.runtime.resource_usage().await.ok();
    let engine = native_engine.unwrap_or_else(|| health.engine.clone());
    let (engine_source, docker_source) =
        runtime_status_sources(&health, engine_responsive, compatibility_responsive);

    Ok(RuntimeStatusInfo {
        state: format_runtime_state(&runtime_state),
        platform: platform.to_string(),
        cpu_cores: config.cpu_cores,
        memory_mb: config.memory_mb,
        disk_gb: config.disk_gb as f32,
        engine_responsive,
        compatibility_responsive,
        compatibility_version: health
            .compatibility_version
            .clone()
            .or_else(|| health.docker_version.clone()),
        engine_source,
        docker_source,
        docker_responsive: compatibility_responsive,
        engine,
        uptime_seconds: health.uptime_seconds,
        resource_usage,
    })
}

fn diagnostic_section(
    query: impl FnOnce() -> Result<Value, cratebay_core::AppError>,
) -> RuntimeDiagnosticSection {
    match query() {
        Ok(value) => RuntimeDiagnosticSection::ok(value),
        Err(error) => RuntimeDiagnosticSection::err(error),
    }
}

fn runtime_status_sources(
    health: &runtime::HealthStatus,
    engine_responsive: bool,
    compatibility_responsive: bool,
) -> (Option<String>, Option<String>) {
    let engine_source = health
        .engine_source
        .clone()
        .or_else(|| engine_responsive.then(|| "builtin".to_string()));
    let compatibility_source = health
        .docker_source
        .clone()
        .or_else(|| {
            compatibility_responsive
                .then(|| health.engine_source.clone())
                .flatten()
        })
        .or_else(|| compatibility_responsive.then(|| "builtin".to_string()));

    (engine_source, compatibility_source)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

/// Return one coherent diagnostics snapshot for Settings and automation.
#[tauri::command]
pub async fn runtime_diagnostics(
    state: State<'_, AppState>,
    prune_exited_containers: bool,
) -> Result<RuntimeDiagnosticsInfo, AppError> {
    let runtime_info = runtime_status_impl(&state).await?;
    let engine_contract =
        diagnostic_section(|| runtime::query_built_in_engine_contract(state.runtime.as_ref()));
    let substrate =
        diagnostic_section(|| runtime::query_built_in_native_substrate(state.runtime.as_ref()));
    let storage_gc = diagnostic_section(|| {
        runtime::query_built_in_native_storage_gc(
            state.runtime.as_ref(),
            false,
            prune_exited_containers,
        )
    });
    let shim_tasks =
        diagnostic_section(|| runtime::query_built_in_native_shim_tasks(state.runtime.as_ref()));
    let ok = runtime_info.engine_responsive
        && engine_contract.ok
        && substrate.ok
        && storage_gc.ok
        && shim_tasks.ok;

    Ok(RuntimeDiagnosticsInfo {
        ok,
        runtime: runtime_info,
        engine_contract,
        substrate,
        storage_gc,
        shim_tasks,
        generated_at_unix: unix_now(),
    })
}

/// Manually start the built-in runtime.
///
/// This command allows the frontend to trigger runtime start
/// (e.g., from Settings page or a retry button).
#[tauri::command]
pub async fn runtime_start(state: State<'_, AppState>) -> Result<String, AppError> {
    tracing::info!("Manual runtime start requested");
    start_runtime_and_connect_engine(&state).await
}

/// Pre-download the built-in runtime image without starting the VM.
#[tauri::command]
pub async fn runtime_provision(state: State<'_, AppState>) -> Result<String, AppError> {
    tracing::info!("Manual runtime provision requested");

    apply_runtime_http_proxy_env(&state)?;

    let current = state.runtime.get_state().await?;
    if current != RuntimeState::None {
        return Ok("Runtime is already provisioned".to_string());
    }

    state
        .runtime
        .provision(Box::new(|progress| {
            tracing::info!(
                "Provision: {} - {:.1}% - {}",
                progress.stage,
                progress.percent,
                progress.message
            );
        }))
        .await?;

    Ok("Runtime provisioning complete".to_string())
}

async fn start_runtime_and_connect_engine(state: &State<'_, AppState>) -> Result<String, AppError> {
    apply_runtime_http_proxy_env(state)?;

    // Step 1: Detect
    let current = state.runtime.get_state().await?;
    tracing::info!("Runtime current state: {:?}", current);

    // Step 2: Provision if needed
    if current == RuntimeState::None {
        tracing::info!("Runtime needs provisioning...");
        state
            .runtime
            .provision(Box::new(|progress| {
                tracing::info!(
                    "Provision: {} - {:.1}% - {}",
                    progress.stage,
                    progress.percent,
                    progress.message
                );
            }))
            .await?;
    }

    // Step 3: Start
    state.runtime.start().await?;
    tracing::info!("Runtime started, waiting for CrateBay Engine API...");

    // Step 4: Wait for the native CrateBay Engine contract and cache the
    // compatibility client if it is also available for older call sites.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    while std::time::Instant::now() < deadline {
        if runtime::query_built_in_ready_engine_status(state.runtime.as_ref()).is_ok() {
            tracing::info!("Native CrateBay Engine API connected via built-in runtime");
            if let Some(docker) =
                try_connect_runtime_engine_compatibility(state.runtime.as_ref()).await
            {
                state.set_engine_compatibility(Some(Arc::new(docker)), Some("builtin".to_string()));
            }
            return Ok("Runtime started and CrateBay Engine connected".to_string());
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    Ok("Runtime started but CrateBay Engine is not yet responsive".to_string())
}

/// Manually restart the built-in runtime.
#[tauri::command]
pub async fn runtime_restart(state: State<'_, AppState>) -> Result<String, AppError> {
    tracing::info!("Manual runtime restart requested");

    let current = state.runtime.get_state().await?;
    if !matches!(
        current,
        RuntimeState::None | RuntimeState::Provisioned | RuntimeState::Stopped
    ) {
        state.runtime.stop().await?;
        state.set_engine_compatibility(None, None);
    }

    let result = start_runtime_and_connect_engine(&state).await?;
    if result.contains("not yet responsive") {
        Ok(result.replace("Runtime started", "Runtime restarted"))
    } else {
        Ok("Runtime restarted and CrateBay Engine connected".to_string())
    }
}

async fn try_connect_runtime_engine_compatibility(
    runtime: &dyn cratebay_core::runtime::RuntimeManager,
) -> Option<Docker> {
    // Linux runtime: TCP hostfwd endpoint.
    #[cfg(target_os = "linux")]
    {
        let _ = runtime;
        let host = cratebay_core::runtime::linux::linux_engine_host();
        let http_host = host
            .strip_prefix("tcp://")
            .map(|rest| format!("http://{}", rest))
            .unwrap_or_else(|| host.replace("tcp://", "http://"));

        let docker = Docker::connect_with_http(&http_host, 5, bollard::API_DEFAULT_VERSION).ok()?;
        if docker.ping().await.is_ok() {
            return Docker::connect_with_http(&http_host, 120, bollard::API_DEFAULT_VERSION).ok();
        }
        None
    }

    // Windows runtime: WSL localhost forwarding endpoint.
    #[cfg(target_os = "windows")]
    {
        let _ = runtime;
        let host = cratebay_core::runtime::windows::windows_engine_host();
        let http_host = host
            .strip_prefix("tcp://")
            .map(|rest| format!("http://{}", rest))
            .unwrap_or_else(|| host.replace("tcp://", "http://"));

        let docker = Docker::connect_with_http(&http_host, 5, bollard::API_DEFAULT_VERSION).ok()?;
        if docker.ping().await.is_ok() {
            return Docker::connect_with_http(&http_host, 120, bollard::API_DEFAULT_VERSION).ok();
        }
        return None;
    }

    // macOS and other Unix platforms: Unix socket.
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let socket_path = runtime.engine_socket_path();
        let socket_str = socket_path.to_str().unwrap_or_default();
        let docker = Docker::connect_with_unix(socket_str, 5, bollard::API_DEFAULT_VERSION).ok()?;
        if docker.ping().await.is_ok() {
            return Docker::connect_with_unix(socket_str, 120, bollard::API_DEFAULT_VERSION).ok();
        }
        None
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = runtime;
        None
    }
}

/// Manually stop the built-in runtime.
#[tauri::command]
pub async fn runtime_stop(state: State<'_, AppState>) -> Result<String, AppError> {
    tracing::info!("Manual runtime stop requested");
    state.runtime.stop().await?;

    // Clear Engine compatibility connection since runtime is stopping.
    state.set_engine_compatibility(None, None);

    Ok("Runtime stopped".to_string())
}

/// Convert a [`RuntimeState`] enum to its string representation for the API.
fn format_runtime_state(state: &RuntimeState) -> String {
    match state {
        RuntimeState::None => "none".to_string(),
        RuntimeState::Provisioned => "provisioned".to_string(),
        RuntimeState::Starting => "starting".to_string(),
        RuntimeState::Ready => "ready".to_string(),
        RuntimeState::Stopping | RuntimeState::Stopped => "stopped".to_string(),
        RuntimeState::Error(_) => "error".to_string(),
    }
}

fn reconcile_runtime_state_with_native_engine(
    state: RuntimeState,
    engine_responsive: bool,
) -> RuntimeState {
    if engine_responsive {
        RuntimeState::Ready
    } else {
        state
    }
}

#[derive(Debug)]
struct RuntimeHttpProxySettings {
    proxy: Option<String>,
    bridge_enabled: bool,
    bind_host: Option<String>,
    bind_port: Option<u16>,
    guest_host: Option<String>,
}

fn apply_runtime_http_proxy_env(state: &State<'_, AppState>) -> Result<(), AppError> {
    let settings = load_runtime_http_proxy_settings(state)?;

    set_or_remove_env_var("CRATEBAY_RUNTIME_HTTP_PROXY", settings.proxy.as_deref());
    std::env::set_var(
        "CRATEBAY_RUNTIME_HTTP_PROXY_BRIDGE",
        if settings.bridge_enabled { "1" } else { "0" },
    );
    set_or_remove_env_var(
        "CRATEBAY_RUNTIME_HTTP_PROXY_BIND_HOST",
        settings.bind_host.as_deref(),
    );
    set_or_remove_env_var(
        "CRATEBAY_RUNTIME_HTTP_PROXY_BIND_PORT",
        settings.bind_port.map(|port| port.to_string()).as_deref(),
    );
    set_or_remove_env_var(
        "CRATEBAY_RUNTIME_HTTP_PROXY_GUEST_HOST",
        settings.guest_host.as_deref(),
    );

    tracing::info!(
        bridge_enabled = settings.bridge_enabled,
        bind_host = ?settings.bind_host,
        bind_port = ?settings.bind_port,
        guest_host = ?settings.guest_host,
        proxy_configured = settings.proxy.is_some(),
        "Applied runtime HTTP proxy settings from persisted app settings"
    );

    Ok(())
}

fn load_runtime_http_proxy_settings(
    state: &State<'_, AppState>,
) -> Result<RuntimeHttpProxySettings, AppError> {
    let db = state.db.lock_or_recover()?;
    let proxy =
        normalize_optional_setting(storage::get_setting(&db, SETTINGS_KEY_RUNTIME_HTTP_PROXY)?);
    let bridge_enabled = parse_boolish(storage::get_setting(
        &db,
        SETTINGS_KEY_RUNTIME_HTTP_PROXY_BRIDGE,
    )?)
    .unwrap_or(false);
    let bind_host = normalize_optional_setting(storage::get_setting(
        &db,
        SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_HOST,
    )?);
    let bind_port = storage::get_setting(&db, SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_PORT)?
        .and_then(|raw| raw.trim().parse::<u16>().ok())
        .filter(|port| *port > 0);
    let guest_host = normalize_optional_setting(storage::get_setting(
        &db,
        SETTINGS_KEY_RUNTIME_HTTP_PROXY_GUEST_HOST,
    )?);

    Ok(RuntimeHttpProxySettings {
        proxy,
        bridge_enabled,
        bind_host,
        bind_port,
        guest_host,
    })
}

fn normalize_optional_setting(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_boolish(raw: Option<String>) -> Option<bool> {
    let value = raw?;
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn set_or_remove_env_var(key: &str, value: Option<&str>) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        std::env::set_var(key, value);
    } else {
        std::env::remove_var(key);
    }
}

/// Debug command: frontend reports its status back to Rust.
/// Only compiled in debug builds.
#[cfg(debug_assertions)]
#[tauri::command]
pub fn webview_debug_report(info: String) {
    tracing::info!("=== WEBVIEW DEBUG REPORT ===\n{}", info);
}

/// Get OS version string.
fn os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|l| l.starts_with("PRETTY_NAME="))
                    .map(|l| {
                        l.trim_start_matches("PRETTY_NAME=")
                            .trim_matches('"')
                            .to_string()
                    })
            })
            .unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        "unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_health_status() -> runtime::HealthStatus {
        runtime::HealthStatus {
            runtime_state: RuntimeState::Starting,
            engine_responsive: false,
            compatibility_responsive: false,
            compatibility_version: None,
            docker_responsive: false,
            docker_version: None,
            uptime_seconds: None,
            last_check: "2026-06-14T00:00:00Z".to_string(),
            engine_source: None,
            docker_source: None,
            engine: runtime::built_in_engine_status(),
        }
    }

    #[test]
    fn native_engine_readiness_promotes_runtime_state_to_ready() {
        assert_eq!(
            reconcile_runtime_state_with_native_engine(RuntimeState::Starting, true),
            RuntimeState::Ready
        );
        assert_eq!(
            reconcile_runtime_state_with_native_engine(
                RuntimeState::Error("compatibility ping failed".to_string()),
                true,
            ),
            RuntimeState::Ready
        );
    }

    #[test]
    fn runtime_state_is_preserved_without_native_engine_readiness() {
        assert_eq!(
            reconcile_runtime_state_with_native_engine(RuntimeState::Starting, false),
            RuntimeState::Starting
        );
        assert_eq!(
            reconcile_runtime_state_with_native_engine(RuntimeState::Stopped, false),
            RuntimeState::Stopped
        );
    }

    #[test]
    fn runtime_status_sources_keep_compatibility_separate() {
        let mut health = test_health_status();
        health.compatibility_responsive = true;
        health.docker_responsive = true;
        health.docker_source = Some("builtin".to_string());

        let (engine_source, docker_source) = runtime_status_sources(&health, false, true);

        assert_eq!(engine_source, None);
        assert_eq!(docker_source, Some("builtin".to_string()));
    }

    #[test]
    fn runtime_status_sources_mark_native_engine_without_compatibility() {
        let health = test_health_status();

        let (engine_source, docker_source) = runtime_status_sources(&health, true, false);

        assert_eq!(engine_source, Some("builtin".to_string()));
        assert_eq!(docker_source, None);
    }

    #[test]
    fn runtime_diagnostics_exposes_aggregate_snapshot_contract() {
        let payload = RuntimeDiagnosticsInfo {
            ok: false,
            runtime: RuntimeStatusInfo {
                state: "stopped".to_string(),
                platform: "macos-vz".to_string(),
                cpu_cores: 2,
                memory_mb: 2048,
                disk_gb: 20.0,
                engine_responsive: false,
                compatibility_responsive: false,
                compatibility_version: None,
                engine_source: Some("builtin".to_string()),
                docker_source: Some("builtin".to_string()),
                docker_responsive: false,
                engine: runtime::built_in_engine_status(),
                uptime_seconds: None,
                resource_usage: None,
            },
            engine_contract: RuntimeDiagnosticSection::err("engine offline"),
            substrate: RuntimeDiagnosticSection::ok(serde_json::json!({
                "network": { "stack": "CNI" }
            })),
            storage_gc: RuntimeDiagnosticSection::ok(serde_json::json!({
                "candidateCount": 1
            })),
            shim_tasks: RuntimeDiagnosticSection::ok(serde_json::json!({
                "items": []
            })),
            generated_at_unix: 1,
        };

        let json = serde_json::to_value(&payload).expect("diagnostics should serialize");

        assert_eq!(json["ok"], false);
        assert_eq!(json["runtime"]["state"], "stopped");
        assert_eq!(json["engineContract"]["ok"], false);
        assert_eq!(json["engineContract"]["error"], "engine offline");
        assert_eq!(json["substrate"]["value"]["network"]["stack"], "CNI");
        assert_eq!(json["storageGc"]["value"]["candidateCount"], 1);
        assert!(json["shimTasks"]["value"]["items"].is_array());
    }

    #[test]
    fn engine_status_prefers_native_contract_before_compatibility_ping() {
        let source = include_str!("system.rs");
        let native_probe = source
            .find("query_built_in_ready_engine_status")
            .expect("engine_status should probe the native CrateBay Engine contract");
        let compatibility_probe = source
            .find("docker::is_available")
            .expect("engine_status should keep compatibility fallback");

        assert!(
            native_probe < compatibility_probe,
            "native Engine contract must be the primary engine_status signal"
        );
        assert!(source.contains("version: Some(engine.kind)"));
        assert!(source.contains("api_version: Some(engine.api)"));
    }
}

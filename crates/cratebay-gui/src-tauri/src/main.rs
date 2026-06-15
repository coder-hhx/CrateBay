//! CrateBay Desktop App — Tauri v2 entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod events;
mod state;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use serde_json::{json, Value};

#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{Emitter, Manager};

use cratebay_core::settings::{
    SETTINGS_KEY_RUNTIME_HTTP_PROXY, SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_HOST,
    SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_PORT, SETTINGS_KEY_RUNTIME_HTTP_PROXY_BRIDGE,
    SETTINGS_KEY_RUNTIME_HTTP_PROXY_GUEST_HOST,
};
use cratebay_core::{storage, MutexExt};

use state::AppState;

/// Check whether the shared CrateBay Engine compatibility client is responsive.
///
/// This uses the already-connected client instead of creating a new connection
/// each time, and retries briefly to smooth transient socket jitter.
///
/// Returns `Some(Arc<Docker>)` if the compatibility client is responsive, `None` otherwise.
async fn get_responsive_shared_engine_client(
    app_handle: &tauri::AppHandle,
) -> Option<Arc<bollard::Docker>> {
    let engine_client = {
        let state = app_handle.state::<AppState>();
        let guard = match state.engine_compatibility.lock() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::warn!(
                    "Failed to lock Engine endpoint state for health reconciliation: {}",
                    e
                );
                return None;
            }
        };
        guard.clone()
    }?;

    // 5 retries at 200 ms gives ~800 ms total — enough to absorb brief socket
    // proxy restarts without meaningfully delaying the health event.
    for attempt in 0..5u8 {
        if engine_client.ping().await.is_ok() {
            return Some(engine_client);
        }
        if attempt < 4 {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    None
}

fn runtime_health_event_payload(
    mut health: cratebay_core::runtime::HealthStatus,
    native_engine: Option<cratebay_core::runtime::RuntimeEngineStatus>,
) -> Value {
    let engine_responsive = health.engine_responsive || native_engine.is_some();
    if let Some(engine) = native_engine {
        health.runtime_state = cratebay_core::runtime::RuntimeState::Ready;
        health.engine = engine;
    }
    let compatibility_responsive = health.compatibility_responsive || health.docker_responsive;
    let compatibility_version = health
        .compatibility_version
        .clone()
        .or_else(|| health.docker_version.clone());

    json!({
        "runtime_state": health.runtime_state,
        "engine_responsive": engine_responsive,
        "compatibility_responsive": compatibility_responsive,
        "compatibility_version": compatibility_version,
        "docker_responsive": compatibility_responsive,
        "docker_version": health.docker_version,
        "uptime_seconds": health.uptime_seconds,
        "last_check": health.last_check,
        "engine_source": health.engine_source,
        "docker_source": health.docker_source,
        "engine": health.engine,
    })
}

fn reconcile_runtime_health_sources(
    health: &mut cratebay_core::runtime::HealthStatus,
    has_native_engine: bool,
) {
    if has_native_engine && health.engine_source.is_none() {
        health.engine_source = Some("builtin".to_string());
    }
    if (health.compatibility_responsive || health.docker_responsive)
        && health.docker_source.is_none()
    {
        health.docker_source = Some("builtin".to_string());
    }
}

/// Start runtime health monitor in Tauri async runtime.
///
/// Strategy (shared-client-first):
/// 1. Try to ping the **shared** Engine compatibility client from AppState first.
///    - If it responds, broadcast `Ready` immediately.
/// 2. Only fall back to `runtime.health_check()` when the shared client is
///    unresponsive or absent.
fn start_runtime_health_monitor(
    app_handle: tauri::AppHandle,
    runtime: Arc<dyn cratebay_core::runtime::RuntimeManager>,
) {
    tauri::async_runtime::spawn(async move {
        // 20-second interval — faster feedback without excessive overhead.
        let mut interval = tokio::time::interval(Duration::from_secs(20));
        loop {
            interval.tick().await;

            // ── Fast path: shared compatibility client is alive ───────────
            if let Some(_engine_client) = get_responsive_shared_engine_client(&app_handle).await {
                let native_engine =
                    cratebay_core::runtime::query_built_in_ready_engine_status(runtime.as_ref())
                        .ok();
                if let Some(engine) = native_engine {
                    tracing::debug!(
                        "Health monitor: shared Engine compatibility client and native contract responsive; emitting Ready"
                    );
                    let state = app_handle.state::<AppState>();
                    let source = state
                        .engine_compatibility_source()
                        .unwrap_or_else(|| "builtin".to_string());
                    let health = cratebay_core::runtime::HealthStatus {
                        runtime_state: cratebay_core::runtime::RuntimeState::Ready,
                        engine_responsive: true,
                        compatibility_responsive: true,
                        compatibility_version: None,
                        docker_responsive: true,
                        docker_version: None,
                        uptime_seconds: None,
                        last_check: Utc::now().to_rfc3339(),
                        engine_source: Some(source.clone()),
                        docker_source: Some(source),
                        engine,
                    };
                    let payload = runtime_health_event_payload(health, None);
                    let _ = app_handle.emit(events::event_names::RUNTIME_HEALTH, &payload);
                    continue;
                }
            }

            // ── Slow path: shared client absent/unresponsive — full check ──
            tracing::debug!(
                "Health monitor: shared Engine compatibility client unresponsive; running full health_check"
            );
            let mut health = match runtime.health_check().await {
                Ok(status) => status,
                Err(e) => {
                    tracing::warn!("Health check failed: {}", e);
                    cratebay_core::runtime::HealthStatus {
                        runtime_state: cratebay_core::runtime::RuntimeState::Error(e.to_string()),
                        engine_responsive: false,
                        compatibility_responsive: false,
                        compatibility_version: None,
                        docker_responsive: false,
                        docker_version: None,
                        uptime_seconds: None,
                        last_check: Utc::now().to_rfc3339(),
                        engine_source: Some("builtin".to_string()),
                        docker_source: Some("builtin".to_string()),
                        engine: cratebay_core::runtime::built_in_engine_status(),
                    }
                }
            };

            let native_engine =
                cratebay_core::runtime::query_built_in_ready_engine_status(runtime.as_ref()).ok();
            reconcile_runtime_health_sources(&mut health, native_engine.is_some());
            let payload = runtime_health_event_payload(health, native_engine);
            let _ = app_handle.emit(events::event_names::RUNTIME_HEALTH, &payload);
        }
    });
}

#[derive(Debug)]
struct RuntimeHttpProxySettings {
    proxy: Option<String>,
    bridge_enabled: bool,
    bind_host: Option<String>,
    bind_port: Option<u16>,
    guest_host: Option<String>,
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

fn load_runtime_http_proxy_settings(
    app_handle: &tauri::AppHandle,
) -> Option<RuntimeHttpProxySettings> {
    let state = app_handle.state::<AppState>();
    let db = state.db.lock_or_recover().ok()?;

    let proxy = normalize_optional_setting(
        storage::get_setting(&db, SETTINGS_KEY_RUNTIME_HTTP_PROXY)
            .ok()
            .flatten(),
    );
    let bridge_enabled = parse_boolish(
        storage::get_setting(&db, SETTINGS_KEY_RUNTIME_HTTP_PROXY_BRIDGE)
            .ok()
            .flatten(),
    )
    .unwrap_or(false);
    let bind_host = normalize_optional_setting(
        storage::get_setting(&db, SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_HOST)
            .ok()
            .flatten(),
    );
    let bind_port = storage::get_setting(&db, SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_PORT)
        .ok()
        .flatten()
        .and_then(|raw| raw.trim().parse::<u16>().ok())
        .filter(|port| *port > 0);
    let guest_host = normalize_optional_setting(
        storage::get_setting(&db, SETTINGS_KEY_RUNTIME_HTTP_PROXY_GUEST_HOST)
            .ok()
            .flatten(),
    );

    Some(RuntimeHttpProxySettings {
        proxy,
        bridge_enabled,
        bind_host,
        bind_port,
        guest_host,
    })
}

fn apply_runtime_http_proxy_env(app_handle: &tauri::AppHandle) {
    let Some(settings) = load_runtime_http_proxy_settings(app_handle) else {
        return;
    };

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
        "Applied runtime HTTP proxy settings for runtime auto-start"
    );
}

fn main() {
    // Initialize tracing
    let env_filter = tracing_subscriber::EnvFilter::from_default_env();
    let env_filter = match "cratebay=info".parse() {
        Ok(directive) => env_filter.add_directive(directive),
        Err(_) => env_filter,
    };
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    // Initialize database
    let db_path = match cratebay_core::storage::default_db_path() {
        Ok(path) => path,
        Err(e) => {
            tracing::error!("Failed to determine database path: {}", e);
            eprintln!("Fatal: Failed to determine database path: {}", e);
            std::process::exit(1);
        }
    };
    let conn = match cratebay_core::storage::init(&db_path) {
        Ok(conn) => conn,
        Err(e) => {
            tracing::error!("Failed to initialize database: {}", e);
            eprintln!(
                "Fatal: Failed to initialize database at {}: {}",
                db_path.display(),
                e
            );
            std::process::exit(1);
        }
    };
    tracing::info!("Database initialized at {}", db_path.display());

    // Create platform-specific runtime manager
    let runtime: Arc<dyn cratebay_core::runtime::RuntimeManager> =
        Arc::from(cratebay_core::runtime::create_runtime_manager());
    tracing::info!("Runtime manager initialized for {}", std::env::consts::OS);

    // Attempt an existing CrateBay Engine compatibility connection without
    // blocking app launch. If unavailable, the runtime auto-start in Tauri setup
    // will handle it.
    let (engine_compatibility, engine_compatibility_source) = {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("Failed to create tokio runtime for Engine check: {}", e);
                eprintln!("Fatal: Failed to create tokio runtime: {}", e);
                std::process::exit(1);
            }
        };
        match rt.block_on(cratebay_core::docker::try_connect()) {
            Some(d) => {
                tracing::info!("CrateBay Engine API connected (existing runtime or explicit host)");
                (Some(Arc::new(d)), Some("builtin".to_string()))
            }
            None => {
                tracing::info!(
                    "CrateBay Engine API not available yet — runtime auto-start will attempt connection"
                );
                (None, None)
            }
        }
    };

    let data_dir = db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();

    let app_state = AppState {
        engine_compatibility: Arc::new(Mutex::new(engine_compatibility)),
        engine_compatibility_source: Arc::new(Mutex::new(engine_compatibility_source)),
        engine_init_lock: Arc::new(tokio::sync::Mutex::new(())),
        db: Arc::new(Mutex::new(conn)),
        data_dir,
        runtime: runtime.clone(),
        terminal_sessions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    };

    tauri::Builder::default()
        .manage(app_state)
        .setup(move |app| {
            // macOS: hide title text, show overlay traffic light buttons
            #[cfg(target_os = "macos")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_title("");
                    let _ = window.set_title_bar_style(TitleBarStyle::Overlay);
                }
            }

            // Apply persisted runtime HTTP proxy settings early so both:
            // - runtime auto-start
            // - host-side registry search fallbacks
            // can use the configured proxy without requiring a manual runtime restart.
            apply_runtime_http_proxy_env(app.handle());

            // Start periodic health monitor (every 30s)
            let app_handle = app.handle().clone();
            let health_runtime = runtime.clone();
            start_runtime_health_monitor(app_handle, health_runtime);
            tracing::info!("Runtime health monitor started");

            if cratebay_core::runtime::common::env_flag_enabled(
                "CRATEBAY_DISABLE_RUNTIME_AUTO_START",
            ) {
                tracing::info!("Runtime auto-start disabled by CRATEBAY_DISABLE_RUNTIME_AUTO_START");
            } else {
                // ── Runtime auto-start (background, non-blocking) ────────
                // If the native Engine API is not yet connected, try to start the
                // built-in runtime and then cache the compatibility client if it is available.
                let auto_start_handle = app.handle().clone();
                let auto_start_runtime = runtime.clone();
                std::thread::Builder::new()
                    .name("runtime-auto-start".to_string())
                    .spawn(move || {
                        let rt = match tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                        {
                            Ok(rt) => rt,
                            Err(e) => {
                                tracing::error!("Failed to create runtime auto-start tokio runtime: {}", e);
                                return;
                            }
                        };

                        rt.block_on(async {
                            // Check if the native Engine API is already available.
                            {
                                let state = auto_start_handle.state::<AppState>();
                                if cratebay_core::runtime::query_built_in_ready_engine_status(
                                    state.runtime.as_ref(),
                                )
                                .is_ok()
                                {
                                    tracing::info!(
                                        "CrateBay Engine API already connected, skipping runtime auto-start"
                                    );
                                    return;
                                }
                            }

                            tracing::info!("Starting container engine auto-start sequence...");

                            // Apply persisted runtime HTTP proxy settings so the VM can reach registries
                            // when started automatically (without the user clicking "Start Runtime").
                            apply_runtime_http_proxy_env(&auto_start_handle);

                            // Provision progress callback that emits Tauri events
                            let handle_clone = auto_start_handle.clone();
                            let progress_cb: Box<
                                dyn Fn(cratebay_core::runtime::ProvisionProgress) + Send,
                            > = Box::new(move |progress| {
                                tracing::info!(
                                    "Provision progress: {} - {:.1}% - {}",
                                    progress.stage,
                                    progress.percent,
                                    progress.message
                                );
                                let _ = handle_clone
                                    .emit(events::event_names::RUNTIME_PROVISION, &progress);
                                // Backward-compatible alias (deprecated)
                                let _ = handle_clone.emit("runtime:provision-progress", &progress);
                            });

                            let options = cratebay_core::engine::EnsureOptions {
                                on_provision_progress: Some(progress_cb),
                                ..Default::default()
                            };

                            match cratebay_core::engine::ensure_engine_contract(
                                auto_start_runtime.as_ref(),
                                options,
                            )
                            .await
                            {
                                Ok(_) => {
                                    tracing::info!("Native CrateBay Engine API connected via ensured container engine");
                                    let _ = auto_start_handle
                                        .emit(events::event_names::ENGINE_CONNECTED, true);
                                    // Backward-compatible alias for older frontends.
                                    let _ = auto_start_handle.emit("docker:connected", true);

                                    // Preload bundled images on this background thread.
                                    let bundle_dir = auto_start_handle
                                        .path()
                                        .resource_dir()
                                        .ok()
                                        .map(|dir| dir.join("bundle-images"))
                                        .filter(|dir| dir.is_dir())
                                        .or_else(cratebay_core::bundle_images::find_bundle_image_dir);
                                    let results = match bundle_dir {
                                        Some(bundle_dir) => {
                                            cratebay_core::bundle_images::load_bundle_images_from_dir_native(
                                                auto_start_runtime.as_ref(),
                                                &bundle_dir,
                                            )
                                            .await
                                        }
                                        None => Vec::new(),
                                    };
                                    let loaded_count =
                                        results.iter().filter(|result| result.loaded).count();
                                    for result in results
                                        .iter()
                                        .filter(|result| !result.loaded && !result.skipped)
                                    {
                                        tracing::warn!(
                                            "Failed to preload bundle image {}: {}",
                                            result.image_name,
                                            result.message
                                        );
                                    }
                                    if loaded_count > 0 {
                                        tracing::info!(
                                            "Preloaded {} bundle images: {:?}",
                                            loaded_count,
                                            results
                                        );
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Engine auto-start failed: {}", e);
                                }
                            }
                        });
                    })
                    .ok(); // JoinHandle is dropped — the thread runs independently.
            }

            // Debug: check WebView status
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").expect("main window not found");
                let window_clone = window.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(5));

                    // Read WebView URL
                    match window_clone.url() {
                        Ok(url) => tracing::info!("WebView URL: {}", url),
                        Err(e) => tracing::warn!("Failed to get URL: {}", e),
                    }
                    match window_clone.title() {
                        Ok(title) => tracing::info!("WebView title: {}", title),
                        Err(e) => tracing::warn!("Failed to get title: {}", e),
                    }
                    match window_clone.inner_size() {
                        Ok(size) => tracing::info!("WebView inner size: {:?}", size),
                        Err(e) => tracing::warn!("Failed to get size: {}", e),
                    }

                    // Inject JS that calls our debug command via __TAURI_INTERNALS__
                    let _ = window_clone.eval(r#"
                        (function() {
                            try {
                                var rootEl = document.getElementById('root');
                                var rootLen = rootEl ? rootEl.innerHTML.length : -1;
                                var rootSnippet = rootEl ? rootEl.innerHTML.substring(0, 2000) : 'NO_ROOT';
                                var errs = window.__CRATEBAY_ERRORS || [];
                                var hasTauri = typeof window.__TAURI_INTERNALS__ !== 'undefined';
                                var scripts = Array.from(document.scripts).map(function(s) { return (s.src || 'inline').substring(0, 100); });
                                var stylesheets = Array.from(document.styleSheets).length;
                                
                                var info = 'TAURI=' + hasTauri + 
                                    ' | READY=' + document.readyState + 
                                    ' | ROOT_LEN=' + rootLen + 
                                    ' | ERRORS=' + errs.length + 
                                    ' | SCRIPTS=' + scripts.length + 
                                    ' | STYLES=' + stylesheets +
                                    '\nSCRIPT_SRCS=' + JSON.stringify(scripts) +
                                    '\nERRORS=' + JSON.stringify(errs) +
                                    '\nROOT_FULL=' + rootSnippet;
                                
                                if (hasTauri) {
                                    window.__TAURI_INTERNALS__.invoke('webview_debug_report', { info: info });
                                }
                            } catch(ex) {
                                if (window.__TAURI_INTERNALS__) {
                                    window.__TAURI_INTERNALS__.invoke('webview_debug_report', { info: 'EXCEPTION: ' + ex.message + '\n' + ex.stack });
                                }
                            }
                        })();
                    "#);
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Container
            commands::container::container_templates,
            commands::container::container_list,
            commands::container::container_create,
            commands::container::container_run,
            commands::container::container_start,
            commands::container::container_stop,
            commands::container::container_delete,
            commands::container::container_exec,
            commands::container::container_exec_stream,
            commands::container::container_terminal_open,
            commands::container::container_terminal_input,
            commands::container::container_terminal_resize,
            commands::container::container_terminal_close,
            commands::container::container_logs,
            commands::container::container_inspect,
            commands::container::container_stats,
            commands::container::image_list,
            commands::container::image_search,
            commands::container::image_inspect,
            commands::container::image_remove,
            commands::container::image_tag,
            commands::container::image_pack_container,
            commands::container::image_export,
            commands::container::image_import,
            commands::container::image_preload_bundled,
            commands::container::image_pull,
            // Pods
            commands::pod::pod_list,
            commands::pod::pod_create,
            commands::pod::pod_inspect,
            commands::pod::pod_delete,
            commands::pod::pod_add_container,
            commands::pod::pod_remove_container,
            // Volumes
            commands::volume::volume_list,
            commands::volume::volume_create,
            commands::volume::volume_inspect,
            commands::volume::volume_delete,
            // Networks
            commands::network::network_list,
            commands::network::network_create,
            commands::network::network_inspect,
            commands::network::network_delete,
            // Storage
            commands::storage::settings_get,
            commands::storage::settings_update,
            // System
            commands::system::system_info,
            commands::system::engine_status,
            commands::system::docker_status,
            commands::system::runtime_status,
            commands::system::runtime_diagnostics,
            commands::system::runtime_start,
            commands::system::runtime_provision,
            commands::system::runtime_stop,
            commands::system::runtime_restart,
            commands::engine::engine_contract,
            commands::engine::engine_substrate,
            commands::engine::engine_storage_gc,
            commands::engine::engine_shim_tasks,
            commands::engine::engine_shim_reap_task,
            commands::update::app_update_check,
            commands::update::app_update_install,
            commands::update::app_restart,
            // Debug
            #[cfg(debug_assertions)]
            commands::system::webview_debug_report,
        ])
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            tracing::error!("Failed to run CrateBay: {}", e);
            eprintln!("Fatal: Failed to run CrateBay: {}", e);
            std::process::exit(1);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_health_status() -> cratebay_core::runtime::HealthStatus {
        cratebay_core::runtime::HealthStatus {
            runtime_state: cratebay_core::runtime::RuntimeState::Starting,
            engine_responsive: false,
            compatibility_responsive: false,
            compatibility_version: None,
            docker_responsive: false,
            docker_version: None,
            uptime_seconds: None,
            last_check: "2026-06-14T00:00:00Z".to_string(),
            engine_source: None,
            docker_source: None,
            engine: cratebay_core::runtime::built_in_engine_status(),
        }
    }

    #[test]
    fn runtime_health_event_payload_prefers_native_engine_readiness() {
        let payload = runtime_health_event_payload(
            cratebay_core::runtime::HealthStatus {
                runtime_state: cratebay_core::runtime::RuntimeState::Starting,
                engine_responsive: false,
                compatibility_responsive: false,
                compatibility_version: None,
                docker_responsive: false,
                docker_version: None,
                uptime_seconds: Some(7),
                last_check: "2026-06-14T00:00:00Z".to_string(),
                engine_source: Some("builtin".to_string()),
                docker_source: Some("builtin".to_string()),
                engine: cratebay_core::runtime::built_in_engine_status(),
            },
            Some(cratebay_core::runtime::built_in_engine_status()),
        );

        assert_eq!(payload["runtime_state"], "Ready");
        assert_eq!(payload["engine_responsive"], true);
        assert_eq!(payload["compatibility_responsive"], false);
        assert_eq!(payload["docker_responsive"], false);
        assert_eq!(payload["engine"]["kind"], "cratebay-containerd");
    }

    #[test]
    fn runtime_health_event_payload_preserves_compatibility_state_without_native_engine() {
        let payload = runtime_health_event_payload(
            cratebay_core::runtime::HealthStatus {
                runtime_state: cratebay_core::runtime::RuntimeState::Starting,
                engine_responsive: false,
                compatibility_responsive: true,
                compatibility_version: Some("25.0.0".to_string()),
                docker_responsive: true,
                docker_version: Some("25.0.0".to_string()),
                uptime_seconds: None,
                last_check: "2026-06-14T00:00:00Z".to_string(),
                engine_source: Some("builtin".to_string()),
                docker_source: Some("builtin".to_string()),
                engine: cratebay_core::runtime::built_in_engine_status(),
            },
            None,
        );

        assert_eq!(payload["runtime_state"], "Starting");
        assert_eq!(payload["engine_responsive"], false);
        assert_eq!(payload["compatibility_responsive"], true);
        assert_eq!(payload["docker_responsive"], true);
        assert_eq!(payload["docker_version"], "25.0.0");
    }

    #[test]
    fn runtime_health_source_reconciliation_keeps_compatibility_separate() {
        let mut health = test_health_status();
        health.compatibility_responsive = true;

        reconcile_runtime_health_sources(&mut health, false);

        assert_eq!(health.engine_source, None);
        assert_eq!(health.docker_source, Some("builtin".to_string()));
    }

    #[test]
    fn runtime_health_source_reconciliation_marks_native_engine_source() {
        let mut health = test_health_status();

        reconcile_runtime_health_sources(&mut health, true);

        assert_eq!(health.engine_source, Some("builtin".to_string()));
        assert_eq!(health.docker_source, None);
    }
}

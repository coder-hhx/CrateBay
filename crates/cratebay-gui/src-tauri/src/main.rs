//! CrateBay Desktop App — Tauri v2 entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod events;
mod state;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;

#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{Emitter, Manager};

use cratebay_core::{storage, MutexExt};

use state::AppState;

/// Check whether the shared Docker client in AppState is currently responsive.
///
/// This uses the already-connected client instead of creating a new connection
/// each time, and retries briefly to smooth transient socket jitter.
///
/// Returns `Some(Arc<Docker>)` if the shared client is responsive, `None` otherwise.
async fn get_responsive_shared_docker(
    app_handle: &tauri::AppHandle,
) -> Option<Arc<bollard::Docker>> {
    let docker = {
        let state = app_handle.state::<AppState>();
        let guard = match state.docker.lock() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::warn!(
                    "Failed to lock Docker state for health reconciliation: {}",
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
        if docker.ping().await.is_ok() {
            return Some(docker);
        }
        if attempt < 4 {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    None
}

/// Start runtime health monitor in Tauri async runtime.
///
/// Strategy (shared-client-first):
/// 1. Try to ping the **shared** Docker client from AppState first.
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

            // ── Fast path: shared client is alive ──────────────────────────
            if let Some(_docker) = get_responsive_shared_docker(&app_handle).await {
                tracing::debug!("Health monitor: shared Docker client responsive — emitting Ready");
                let health = cratebay_core::runtime::HealthStatus {
                    runtime_state: cratebay_core::runtime::RuntimeState::Ready,
                    docker_responsive: true,
                    docker_version: None,
                    uptime_seconds: None,
                    last_check: Utc::now().to_rfc3339(),
                    docker_source: Some("builtin".to_string()),
                };
                let _ = app_handle.emit(events::event_names::RUNTIME_HEALTH, &health);
                continue;
            }

            // ── Slow path: shared client absent/unresponsive — full check ──
            tracing::debug!(
                "Health monitor: shared Docker unresponsive, running full health_check"
            );
            let mut health = match runtime.health_check().await {
                Ok(status) => status,
                Err(e) => {
                    tracing::warn!("Health check failed: {}", e);
                    cratebay_core::runtime::HealthStatus {
                        runtime_state: cratebay_core::runtime::RuntimeState::Error(e.to_string()),
                        docker_responsive: false,
                        docker_version: None,
                        uptime_seconds: None,
                        last_check: Utc::now().to_rfc3339(),
                        docker_source: Some("builtin".to_string()),
                    }
                }
            };

            // Set docker_source to "builtin" if Docker is responsive.
            if health.docker_responsive && health.docker_source.is_none() {
                health.docker_source = Some("builtin".to_string());
            }

            let _ = app_handle.emit(events::event_names::RUNTIME_HEALTH, &health);
        }
    });
}

const SETTINGS_KEY_RUNTIME_HTTP_PROXY: &str = "runtimeHttpProxy";
const SETTINGS_KEY_RUNTIME_HTTP_PROXY_BRIDGE: &str = "runtimeHttpProxyBridge";
const SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_HOST: &str = "runtimeHttpProxyBindHost";
const SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_PORT: &str = "runtimeHttpProxyBindPort";
const SETTINGS_KEY_RUNTIME_HTTP_PROXY_GUEST_HOST: &str = "runtimeHttpProxyGuestHost";

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

    // Attempt Docker connection (non-blocking, optional)
    // Try external Docker first; if unavailable, the runtime auto-start
    // in Tauri setup will handle it.
    let docker = {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("Failed to create tokio runtime for Docker: {}", e);
                eprintln!("Fatal: Failed to create tokio runtime: {}", e);
                std::process::exit(1);
            }
        };
        match rt.block_on(cratebay_core::docker::try_connect()) {
            Some(d) => {
                tracing::info!("Docker connected (external or existing runtime)");
                Some(Arc::new(d))
            }
            None => {
                tracing::info!(
                    "Docker not available yet — runtime auto-start will attempt connection"
                );
                None
            }
        }
    };

    let data_dir = db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();

    // Initialize MCP Manager
    // Load .mcp.json from project root and merge with SQLite-stored servers
    let mcp_manager = {
        use cratebay_core::mcp::{load_mcp_json, merge_server_configs, McpServerDbRow};

        let mcp_json = match load_mcp_json(std::path::Path::new(".")) {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!("Failed to load .mcp.json: {}", e);
                None
            }
        };

        // Load user-configured servers from SQLite
        let db_rows: Vec<McpServerDbRow> = (|| -> Vec<McpServerDbRow> {
            let mut stmt = match conn.prepare(
                "SELECT id, name, command, args, env, working_dir, enabled, notes, auto_start \
                 FROM mcp_servers ORDER BY name",
            ) {
                Ok(stmt) => stmt,
                Err(e) => {
                    tracing::warn!("Failed to prepare MCP servers query: {}", e);
                    return Vec::new();
                }
            };

            stmt.query_map([], |row| {
                let args_json: String = row.get(3)?;
                let env_json: String = row.get(4)?;
                let args: Vec<String> = serde_json::from_str(&args_json).unwrap_or_default();
                let env: Vec<String> = serde_json::from_str(&env_json).unwrap_or_default();

                Ok(McpServerDbRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    command: row.get(2)?,
                    args,
                    env,
                    working_dir: row.get(5)?,
                    enabled: row.get::<_, i32>(6)? != 0,
                    notes: row.get(7)?,
                    auto_start: row.get::<_, i32>(8)? != 0,
                })
            })
            .map(|r| r.filter_map(|row| row.ok()).collect())
            .unwrap_or_default()
        })();

        let configs = merge_server_configs(mcp_json.as_ref(), &db_rows);
        let manager = Arc::new(cratebay_core::mcp::McpManager::new());

        // Load configs and auto-start in a blocking tokio context
        let mgr_clone = manager.clone();
        let mcp_rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("Failed to create tokio runtime for MCP: {}", e);
                eprintln!("Fatal: Failed to create tokio runtime for MCP: {}", e);
                std::process::exit(1);
            }
        };
        mcp_rt.block_on(async {
            mgr_clone.load_configs(configs).await;
            mgr_clone.auto_start_servers().await;
        });

        tracing::info!("MCP manager initialized with {} servers", db_rows.len());
        manager
    };

    let app_state = AppState {
        docker: Arc::new(Mutex::new(docker.clone())),
        docker_init: Arc::new(tokio::sync::OnceCell::new()),
        db: Arc::new(Mutex::new(conn)),
        data_dir,
        llm_cancel_tokens: Arc::new(Mutex::new(std::collections::HashMap::new())),
        runtime: runtime.clone(),
        mcp_manager,
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
            // - host-side Docker Hub fallbacks (image search)
            // can use the configured proxy without requiring a manual runtime restart.
            apply_runtime_http_proxy_env(app.handle());

            // Start periodic health monitor (every 30s)
            let app_handle = app.handle().clone();
            let health_runtime = runtime.clone();
            start_runtime_health_monitor(app_handle, health_runtime);
            tracing::info!("Runtime health monitor started");

            // ── Runtime auto-start (background, non-blocking) ────────
            // If Docker is not yet connected, try to start the built-in
            // runtime and then reconnect Docker through the runtime socket.
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
                        // Check if Docker is already available
                        {
                            let state = auto_start_handle.state::<AppState>();
                            if state.has_docker() {
                                tracing::info!(
                                    "Docker already connected, skipping runtime auto-start"
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
                            let _ = handle_clone.emit(events::event_names::RUNTIME_PROVISION, &progress);
                            // Backward-compatible alias (deprecated)
                            let _ = handle_clone.emit("runtime:provision-progress", &progress);
                        });

                        let options = cratebay_core::engine::EnsureOptions {
                            on_provision_progress: Some(progress_cb),
                            ..Default::default()
                        };

                        match cratebay_core::engine::ensure_docker(
                            auto_start_runtime.as_ref(),
                            options,
                        )
                        .await
                        {
                            Ok(docker) => {
                                tracing::info!("Docker connected via ensured container engine");
                                let state = auto_start_handle.state::<AppState>();
                                state.set_docker(Some(docker.clone()));
                                let _ = auto_start_handle.emit("docker:connected", true);

                                // Preload bundle images in background
                                let resource_dir = auto_start_handle
                                    .path()
                                    .resource_dir()
                                    .unwrap_or_default();
                                let preload_docker = docker.clone();
                                tokio::spawn(async move {
                                    let loaded = commands::container::load_bundle_images(
                                        &preload_docker,
                                        &resource_dir,
                                    )
                                    .await;
                                    if !loaded.is_empty() {
                                        tracing::info!(
                                            "Preloaded {} bundle images: {:?}",
                                            loaded.len(),
                                            loaded
                                        );
                                    }
                                });
                            }
                            Err(e) => {
                                tracing::warn!("Engine auto-start failed: {}", e);
                            }
                        }
                    });
                })
                .ok(); // JoinHandle is dropped — the thread runs independently.

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
            commands::container::container_start,
            commands::container::container_stop,
            commands::container::container_delete,
            commands::container::container_exec,
            commands::container::container_exec_stream,
            commands::container::container_logs,
            commands::container::container_inspect,
            commands::container::container_stats,
            commands::container::image_list,
            commands::container::image_search,
            commands::container::image_inspect,
            commands::container::image_remove,
            commands::container::image_tag,
            commands::container::image_pull,
            // LLM
            commands::llm::llm_proxy_stream,
            commands::llm::llm_proxy_cancel,
            commands::llm::llm_provider_list,
            commands::llm::llm_provider_create,
            commands::llm::llm_provider_update,
            commands::llm::llm_provider_delete,
            commands::llm::llm_provider_test,
            commands::llm::llm_models_fetch,
            commands::llm::llm_models_list,
            commands::llm::llm_models_toggle,
            // Storage
            commands::storage::settings_get,
            commands::storage::settings_update,
            commands::storage::api_key_save,
            commands::storage::api_key_delete,
            commands::storage::conversation_list,
            commands::storage::conversation_get_messages,
            commands::storage::conversation_create,
            commands::storage::conversation_delete,
            commands::storage::conversation_save_message,
            commands::storage::conversation_update_title,
            // MCP
            commands::mcp::mcp_server_list,
            commands::mcp::mcp_server_add,
            commands::mcp::mcp_server_remove,
            commands::mcp::mcp_server_start,
            commands::mcp::mcp_server_stop,
            commands::mcp::mcp_client_call_tool,
            commands::mcp::mcp_client_list_tools,
            commands::mcp::mcp_export_client_config,
            // System
            commands::system::system_info,
            commands::system::docker_status,
            commands::system::runtime_status,
            commands::system::runtime_start,
            commands::system::runtime_stop,
            // Sandbox
            commands::sandbox::sandbox_run_code,
            commands::sandbox::sandbox_install,
            // Debug
            #[cfg(debug_assertions)]
            commands::system::webview_debug_report,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            tracing::error!("Failed to run CrateBay: {}", e);
            eprintln!("Fatal: Failed to run CrateBay: {}", e);
            std::process::exit(1);
        });
}

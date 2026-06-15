//! Runtime management commands.

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

use cratebay_core::models::ResourceUsage;
use cratebay_core::runtime::{self, RuntimeEngineStatus, RuntimeState};
use cratebay_core::{settings, storage};

use super::{print_structured, OutputFormat};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatusPayload {
    state: String,
    engine_responsive: bool,
    compatibility_responsive: bool,
    compatibility_version: Option<String>,
    engine_source: Option<String>,
    docker_source: Option<String>,
    docker_responsive: bool,
    docker_version: Option<String>,
    engine: RuntimeEngineStatus,
    uptime_seconds: Option<u64>,
    resource_usage: Option<ResourceUsage>,
    socket_path: String,
    message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeProxySettings {
    proxy: Option<String>,
    bridge_enabled: bool,
    bind_host: String,
    bind_port: u16,
    guest_host: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeProxyPayload {
    proxy: Option<String>,
    bridge_enabled: bool,
    bind_host: String,
    bind_port: u16,
    guest_host: String,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeDiagnosticsPayload {
    ok: bool,
    runtime: RuntimeStatusPayload,
    engine_contract: DiagnosticSection,
    substrate: DiagnosticSection,
    storage_gc: DiagnosticSection,
    shim_tasks: DiagnosticSection,
    generated_at_unix: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticSection {
    ok: bool,
    value: Option<Value>,
    error: Option<String>,
}

impl RuntimeProxySettings {
    fn defaults() -> Self {
        Self {
            proxy: None,
            bridge_enabled: settings::DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE,
            bind_host: settings::DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST.to_string(),
            bind_port: settings::DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT,
            guest_host: settings::DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST.to_string(),
        }
    }

    fn payload(&self, message: Option<String>) -> RuntimeProxyPayload {
        RuntimeProxyPayload {
            proxy: self.proxy.clone(),
            bridge_enabled: self.bridge_enabled,
            bind_host: self.bind_host.clone(),
            bind_port: self.bind_port,
            guest_host: self.guest_host.clone(),
            message,
        }
    }
}

impl DiagnosticSection {
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

fn state_label(state: &RuntimeState) -> String {
    match state {
        RuntimeState::None => "none",
        RuntimeState::Provisioned => "provisioned",
        RuntimeState::Starting => "starting",
        RuntimeState::Ready => "ready",
        RuntimeState::Stopping => "stopping",
        RuntimeState::Stopped => "stopped",
        RuntimeState::Error(_) => "error",
    }
    .to_string()
}

fn reconcile_runtime_state_with_native_engine(
    state: RuntimeState,
    engine_responsive: bool,
) -> RuntimeState {
    if engine_responsive && !matches!(state, RuntimeState::Stopping | RuntimeState::Error(_)) {
        RuntimeState::Ready
    } else {
        state
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn diagnostic_section(
    query: impl FnOnce() -> Result<Value, cratebay_core::AppError>,
) -> DiagnosticSection {
    match query() {
        Ok(value) => DiagnosticSection::ok(value),
        Err(error) => DiagnosticSection::err(error),
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

async fn runtime_status_payload(
    runtime: &dyn runtime::RuntimeManager,
    message: Option<String>,
) -> Result<RuntimeStatusPayload> {
    let state = runtime.get_state().await?;
    let socket_path = runtime.engine_socket_path().display().to_string();
    let resource_usage = runtime.resource_usage().await.ok();
    let native_engine = runtime::query_built_in_ready_engine_status(runtime).ok();
    if state == RuntimeState::Ready {
        let health = runtime.health_check().await?;
        let engine_responsive = health.engine_responsive || native_engine.is_some();
        let compatibility_responsive = health.compatibility_responsive || health.docker_responsive;
        let runtime_state = reconcile_runtime_state_with_native_engine(
            health.runtime_state.clone(),
            engine_responsive,
        );
        let compatibility_version = health
            .compatibility_version
            .clone()
            .or_else(|| health.docker_version.clone());
        let (engine_source, docker_source) =
            runtime_status_sources(&health, engine_responsive, compatibility_responsive);
        let engine = native_engine.unwrap_or_else(|| health.engine.clone());
        return Ok(RuntimeStatusPayload {
            state: state_label(&runtime_state),
            engine_responsive,
            compatibility_responsive,
            compatibility_version,
            engine_source: engine_source.clone(),
            docker_source,
            docker_responsive: compatibility_responsive,
            docker_version: health.docker_version,
            engine,
            uptime_seconds: health.uptime_seconds,
            resource_usage,
            socket_path,
            message,
        });
    }

    if let Some(engine) = native_engine {
        return Ok(RuntimeStatusPayload {
            state: state_label(&reconcile_runtime_state_with_native_engine(state, true)),
            engine_responsive: true,
            compatibility_responsive: engine.docker_compatible,
            compatibility_version: Some(engine.kind.clone()),
            engine_source: Some("builtin".to_string()),
            docker_source: Some("builtin".to_string()),
            docker_responsive: engine.docker_compatible,
            docker_version: Some(engine.kind.clone()),
            engine,
            uptime_seconds: None,
            resource_usage,
            socket_path,
            message,
        });
    }

    Ok(RuntimeStatusPayload {
        state: state_label(&state),
        engine_responsive: false,
        compatibility_responsive: false,
        compatibility_version: None,
        engine_source: Some("builtin".to_string()),
        docker_source: Some("builtin".to_string()),
        docker_responsive: false,
        docker_version: None,
        engine: runtime::built_in_engine_status(),
        uptime_seconds: None,
        resource_usage,
        socket_path,
        message,
    })
}

async fn runtime_diagnostics_payload(
    runtime: &dyn runtime::RuntimeManager,
    prune_exited_containers: bool,
) -> Result<RuntimeDiagnosticsPayload> {
    let runtime_payload = runtime_status_payload(runtime, None).await?;
    let engine_contract = diagnostic_section(|| runtime::query_built_in_engine_contract(runtime));
    let substrate = diagnostic_section(|| runtime::query_built_in_native_substrate(runtime));
    let storage_gc = diagnostic_section(|| {
        runtime::query_built_in_native_storage_gc(runtime, false, prune_exited_containers)
    });
    let shim_tasks = diagnostic_section(|| runtime::query_built_in_native_shim_tasks(runtime));
    let ok = runtime_payload.engine_responsive
        && engine_contract.ok
        && substrate.ok
        && storage_gc.ok
        && shim_tasks.ok;

    Ok(RuntimeDiagnosticsPayload {
        ok,
        runtime: runtime_payload,
        engine_contract,
        substrate,
        storage_gc,
        shim_tasks,
        generated_at_unix: unix_now(),
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

fn env_var_has_nonempty_value(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn load_runtime_proxy_settings() -> Result<RuntimeProxySettings> {
    let db_path = storage::default_db_path()?;
    let conn = storage::init(&db_path)?;

    let mut settings_payload = RuntimeProxySettings::defaults();
    settings_payload.proxy = normalize_optional_setting(storage::get_setting(
        &conn,
        settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY,
    )?);
    settings_payload.bridge_enabled = parse_boolish(storage::get_setting(
        &conn,
        settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_BRIDGE,
    )?)
    .unwrap_or(settings_payload.bridge_enabled);
    settings_payload.bind_host = normalize_optional_setting(storage::get_setting(
        &conn,
        settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_HOST,
    )?)
    .unwrap_or(settings_payload.bind_host);
    settings_payload.bind_port =
        storage::get_setting(&conn, settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_PORT)?
            .and_then(|raw| raw.trim().parse::<u16>().ok())
            .filter(|port| *port > 0)
            .unwrap_or(settings_payload.bind_port);
    settings_payload.guest_host = normalize_optional_setting(storage::get_setting(
        &conn,
        settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_GUEST_HOST,
    )?)
    .unwrap_or(settings_payload.guest_host);

    Ok(settings_payload)
}

fn persist_runtime_proxy_settings(settings_payload: &RuntimeProxySettings) -> Result<()> {
    let db_path = storage::default_db_path()?;
    let conn = storage::init(&db_path)?;

    storage::set_setting(
        &conn,
        settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY,
        settings_payload.proxy.as_deref().unwrap_or(""),
    )?;
    storage::set_setting(
        &conn,
        settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_BRIDGE,
        if settings_payload.bridge_enabled {
            "true"
        } else {
            "false"
        },
    )?;
    storage::set_setting(
        &conn,
        settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_HOST,
        &settings_payload.bind_host,
    )?;
    storage::set_setting(
        &conn,
        settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_BIND_PORT,
        &settings_payload.bind_port.to_string(),
    )?;
    storage::set_setting(
        &conn,
        settings::SETTINGS_KEY_RUNTIME_HTTP_PROXY_GUEST_HOST,
        &settings_payload.guest_host,
    )?;

    Ok(())
}

fn apply_runtime_proxy_env_with(
    settings_payload: &RuntimeProxySettings,
    mut apply: impl FnMut(&str, Option<&str>),
) {
    apply(
        "CRATEBAY_RUNTIME_HTTP_PROXY",
        settings_payload.proxy.as_deref(),
    );
    apply(
        "CRATEBAY_RUNTIME_HTTP_PROXY_BRIDGE",
        Some(if settings_payload.bridge_enabled {
            "1"
        } else {
            "0"
        }),
    );
    apply(
        "CRATEBAY_RUNTIME_HTTP_PROXY_BIND_HOST",
        Some(settings_payload.bind_host.as_str()),
    );
    let bind_port = settings_payload.bind_port.to_string();
    apply(
        "CRATEBAY_RUNTIME_HTTP_PROXY_BIND_PORT",
        Some(bind_port.as_str()),
    );
    apply(
        "CRATEBAY_RUNTIME_HTTP_PROXY_GUEST_HOST",
        Some(settings_payload.guest_host.as_str()),
    );
}

fn apply_runtime_proxy_env(settings_payload: &RuntimeProxySettings) {
    apply_runtime_proxy_env_with(settings_payload, set_or_remove_env_var);
}

fn apply_runtime_proxy_env_preserving_existing(settings_payload: &RuntimeProxySettings) {
    apply_runtime_proxy_env_with(settings_payload, |key, value| {
        if !env_var_has_nonempty_value(key) {
            set_or_remove_env_var(key, value);
        }
    });
}

pub(crate) fn apply_persisted_runtime_proxy_env() -> Result<()> {
    let settings_payload = load_runtime_proxy_settings()?;
    apply_runtime_proxy_env_preserving_existing(&settings_payload);
    Ok(())
}

async fn provision_if_needed(
    runtime: &dyn runtime::RuntimeManager,
    state: &RuntimeState,
    format: &OutputFormat,
) -> Result<()> {
    if *state != RuntimeState::None {
        return Ok(());
    }

    if matches!(format, OutputFormat::Table) {
        println!("Provisioning runtime image...");
    }
    runtime
        .provision(Box::new(|progress| {
            if progress.percent > 0.0 {
                eprint!("\r  {} — {:.0}%", progress.message, progress.percent);
            } else {
                eprint!("\r  {}", progress.message);
            }
        }))
        .await?;
    eprintln!();
    if matches!(format, OutputFormat::Table) {
        println!("Provisioning complete.");
    }

    Ok(())
}

async fn wait_for_runtime_engine(
    runtime: &dyn runtime::RuntimeManager,
    format: &OutputFormat,
    success_message: &str,
    timeout_message: &str,
) -> Result<RuntimeStatusPayload> {
    if matches!(format, OutputFormat::Table) {
        print!("Waiting for CrateBay Engine...");
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    loop {
        if std::time::Instant::now() >= deadline {
            let payload =
                runtime_status_payload(runtime, Some(timeout_message.to_string())).await?;
            if matches!(format, OutputFormat::Table) {
                println!(" timed out.");
                println!("{timeout_message}");
            }
            return Ok(payload);
        }

        let health = runtime.health_check().await?;
        if health.engine_responsive || runtime::query_built_in_ready_engine_status(runtime).is_ok()
        {
            let payload =
                runtime_status_payload(runtime, Some(success_message.to_string())).await?;
            if matches!(format, OutputFormat::Table) {
                println!(" ready.");
            }
            return Ok(payload);
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if matches!(format, OutputFormat::Table) {
            print!(".");
        }
    }
}

fn print_runtime_status_table(payload: &RuntimeStatusPayload) {
    let state_str = match payload.state.as_str() {
        "none" => "not provisioned",
        "provisioned" => "provisioned (stopped)",
        "ready" => "ready",
        "error" => "error",
        other => other,
    };

    println!("Runtime: {}", state_str);
    if payload.state == "ready" {
        println!(
            "Engine: {} ({}, {})",
            payload.engine.name, payload.engine.kind, payload.engine.api
        );
        if payload.engine_responsive {
            println!("Native Engine: responsive");
        } else {
            println!("Native Engine: not responsive");
        }
        if let Some(source) = &payload.engine_source {
            println!("Source: {}", source);
        }
        if payload.compatibility_responsive {
            println!("Compatibility API: responsive");
            if let Some(version) = &payload.compatibility_version {
                println!("Compatibility version: {}", version);
            }
        } else {
            println!("Compatibility API: not responsive");
        }
        if let Some(uptime) = payload.uptime_seconds {
            let mins = uptime / 60;
            let secs = uptime % 60;
            println!("Uptime: {}m {}s", mins, secs);
        }
    }
    if let Some(usage) = &payload.resource_usage {
        println!(
            "Resource usage: CPU {:.1}%, memory {} / {} MB, disk {:.1} / {:.1} GB, containers {}",
            usage.cpu_percent,
            usage.memory_used_mb,
            usage.memory_total_mb,
            usage.disk_used_gb,
            usage.disk_total_gb,
            usage.container_count
        );
    }
    println!("Socket: {}", payload.socket_path);
}

fn value_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn str_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    value_path(value, path).and_then(Value::as_str)
}

fn u64_path(value: &Value, path: &[&str]) -> Option<u64> {
    value_path(value, path).and_then(Value::as_u64)
}

fn section_value(section: &DiagnosticSection) -> Option<&Value> {
    section.value.as_ref()
}

fn print_section_error(label: &str, section: &DiagnosticSection) {
    if !section.ok {
        println!(
            "{label}: unavailable{}",
            section
                .error
                .as_ref()
                .map(|error| format!(" - {error}"))
                .unwrap_or_default()
        );
    }
}

fn print_runtime_diagnostics_table(payload: &RuntimeDiagnosticsPayload) {
    println!(
        "Runtime diagnostics: {}",
        if payload.ok { "ok" } else { "attention needed" }
    );
    print_runtime_status_table(&payload.runtime);

    if let Some(contract) = section_value(&payload.engine_contract) {
        println!(
            "Engine contract: {} ({})",
            str_path(contract, &["name"]).unwrap_or("CrateBay Engine"),
            str_path(contract, &["adapter", "api"]).unwrap_or("cratebay.engine.v1")
        );
        if let Some(namespace) = str_path(contract, &["backend", "namespace"]) {
            println!("Namespace: {namespace}");
        }
    } else {
        print_section_error("Engine contract", &payload.engine_contract);
    }

    if let Some(substrate) = section_value(&payload.substrate) {
        println!(
            "Substrate: {} / {} / {}",
            str_path(substrate, &["shim", "backend"]).unwrap_or("containerd task service"),
            str_path(substrate, &["network", "stack"]).unwrap_or("CNI"),
            str_path(substrate, &["storage", "manager"]).unwrap_or("cratebay-storage")
        );
        if let Some(endpoint) = str_path(substrate, &["daemon", "compatibilityEndpoint"]) {
            println!("Compatibility endpoint: {endpoint}");
        }
    } else {
        print_section_error("Substrate", &payload.substrate);
    }

    if let Some(storage_gc) = section_value(&payload.storage_gc) {
        println!(
            "Storage GC dry run: {} candidates, {} bytes reclaimable",
            u64_path(storage_gc, &["candidateCount"]).unwrap_or_default(),
            u64_path(storage_gc, &["reclaimableBytes"]).unwrap_or_default()
        );
    } else {
        print_section_error("Storage GC dry run", &payload.storage_gc);
    }

    if let Some(shim_tasks) = section_value(&payload.shim_tasks) {
        let count = u64_path(shim_tasks, &["count"])
            .or_else(|| {
                value_path(shim_tasks, &["items"])
                    .and_then(Value::as_array)
                    .map(|items| items.len() as u64)
            })
            .unwrap_or_default();
        println!("Shim tasks: {count}");
    } else {
        print_section_error("Shim tasks", &payload.shim_tasks);
    }
}

/// Show current runtime status.
pub async fn status(format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime_status_payload(runtime.as_ref(), None).await?;

    match format {
        OutputFormat::Table => {
            if let RuntimeState::Error(msg) = runtime.get_state().await? {
                println!("Runtime: error - {}", msg);
            } else {
                print_runtime_status_table(&payload);
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

/// Show an aggregated runtime and native Engine diagnostics snapshot.
pub async fn diagnostics(prune_exited_containers: bool, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime_diagnostics_payload(runtime.as_ref(), prune_exited_containers).await?;

    match format {
        OutputFormat::Table => print_runtime_diagnostics_table(&payload),
        _ => print_structured(&payload, format)?,
    }
    Ok(())
}

/// Start the built-in runtime (provision if needed).
pub async fn start(format: &OutputFormat) -> Result<()> {
    apply_persisted_runtime_proxy_env()?;
    let runtime = runtime::create_runtime_manager();

    let state = runtime.get_state().await?;
    if state == RuntimeState::Ready {
        let payload =
            runtime_status_payload(runtime.as_ref(), Some("Runtime is already running.".into()))
                .await?;
        match format {
            OutputFormat::Table => println!("Runtime is already running."),
            _ => print_structured(&payload, format)?,
        }
        return Ok(());
    }

    if matches!(format, OutputFormat::Table) {
        println!("Starting CrateBay runtime...");
    }

    provision_if_needed(runtime.as_ref(), &state, format).await?;

    // Start
    runtime.start().await?;
    if matches!(format, OutputFormat::Table) {
        println!("Runtime started.");
    }

    let payload = wait_for_runtime_engine(
        runtime.as_ref(),
        format,
        "Runtime started.",
        "Runtime is running but CrateBay Engine is not yet responsive.",
    )
    .await?;
    if !matches!(format, OutputFormat::Table) {
        print_structured(&payload, format)?;
    }
    Ok(())
}

/// Stop the built-in runtime.
pub async fn stop(format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();

    let state = runtime.get_state().await?;
    match state {
        RuntimeState::None | RuntimeState::Provisioned | RuntimeState::Stopped => {
            let payload =
                runtime_status_payload(runtime.as_ref(), Some("Runtime is not running.".into()))
                    .await?;
            match format {
                OutputFormat::Table => println!("Runtime is not running."),
                _ => print_structured(&payload, format)?,
            }
            return Ok(());
        }
        _ => {}
    }

    if matches!(format, OutputFormat::Table) {
        println!("Stopping CrateBay runtime...");
    }
    runtime.stop().await?;
    let payload = runtime_status_payload(runtime.as_ref(), Some("Runtime stopped.".into())).await?;
    match format {
        OutputFormat::Table => println!("Runtime stopped."),
        _ => print_structured(&payload, format)?,
    }
    Ok(())
}

/// Restart the built-in runtime.
pub async fn restart(format: &OutputFormat) -> Result<()> {
    apply_persisted_runtime_proxy_env()?;
    let runtime = runtime::create_runtime_manager();

    let state = runtime.get_state().await?;
    if matches!(format, OutputFormat::Table) {
        println!("Restarting CrateBay runtime...");
    }

    if !matches!(
        state,
        RuntimeState::None | RuntimeState::Provisioned | RuntimeState::Stopped
    ) {
        if matches!(format, OutputFormat::Table) {
            println!("Stopping CrateBay runtime...");
        }
        runtime.stop().await?;
        if matches!(format, OutputFormat::Table) {
            println!("Runtime stopped.");
        }
    }

    provision_if_needed(runtime.as_ref(), &state, format).await?;

    if matches!(format, OutputFormat::Table) {
        println!("Starting CrateBay runtime...");
    }
    runtime.start().await?;
    if matches!(format, OutputFormat::Table) {
        println!("Runtime started.");
    }

    let payload = wait_for_runtime_engine(
        runtime.as_ref(),
        format,
        "Runtime restarted.",
        "Runtime restarted, but CrateBay Engine is not yet responsive.",
    )
    .await?;
    match format {
        OutputFormat::Table if payload.message.as_deref() == Some("Runtime restarted.") => {
            println!("Runtime restarted.")
        }
        OutputFormat::Table => {}
        _ => print_structured(&payload, format)?,
    }
    Ok(())
}

/// Show persisted runtime HTTP proxy settings.
pub async fn proxy_show(format: &OutputFormat) -> Result<()> {
    let settings_payload = load_runtime_proxy_settings()?;
    let payload = settings_payload.payload(None);

    match format {
        OutputFormat::Table => print_runtime_proxy_table(&payload),
        _ => print_structured(&payload, format)?,
    }
    Ok(())
}

/// Persist runtime HTTP proxy settings.
pub async fn proxy_set(
    proxy: Option<String>,
    bridge: bool,
    no_bridge: bool,
    bind_host: Option<String>,
    bind_port: Option<u16>,
    guest_host: Option<String>,
    format: &OutputFormat,
) -> Result<()> {
    if !bridge
        && !no_bridge
        && proxy.is_none()
        && bind_host.is_none()
        && bind_port.is_none()
        && guest_host.is_none()
    {
        anyhow::bail!("No runtime proxy setting changes were provided.");
    }

    let mut settings_payload = load_runtime_proxy_settings()?;
    if let Some(proxy) = proxy {
        settings_payload.proxy = normalize_optional_setting(Some(proxy));
    }
    if bridge {
        settings_payload.bridge_enabled = true;
    }
    if no_bridge {
        settings_payload.bridge_enabled = false;
    }
    if let Some(bind_host) = normalize_optional_setting(bind_host) {
        settings_payload.bind_host = bind_host;
    }
    if let Some(bind_port) = bind_port {
        if bind_port == 0 {
            anyhow::bail!("Runtime proxy bind port must be greater than 0.");
        }
        settings_payload.bind_port = bind_port;
    }
    if let Some(guest_host) = normalize_optional_setting(guest_host) {
        settings_payload.guest_host = guest_host;
    }

    persist_runtime_proxy_settings(&settings_payload)?;
    apply_runtime_proxy_env(&settings_payload);
    let payload = settings_payload.payload(Some("Runtime proxy settings saved.".to_string()));
    match format {
        OutputFormat::Table => {
            println!("Runtime proxy settings saved.");
            print_runtime_proxy_table(&payload);
        }
        _ => print_structured(&payload, format)?,
    }
    Ok(())
}

/// Clear persisted runtime HTTP proxy settings.
pub async fn proxy_clear(format: &OutputFormat) -> Result<()> {
    let settings_payload = RuntimeProxySettings::defaults();
    persist_runtime_proxy_settings(&settings_payload)?;
    apply_runtime_proxy_env(&settings_payload);
    let payload = settings_payload.payload(Some("Runtime proxy settings cleared.".to_string()));

    match format {
        OutputFormat::Table => {
            println!("Runtime proxy settings cleared.");
            print_runtime_proxy_table(&payload);
        }
        _ => print_structured(&payload, format)?,
    }
    Ok(())
}

fn print_runtime_proxy_table(payload: &RuntimeProxyPayload) {
    println!(
        "Runtime HTTP proxy: {}",
        payload.proxy.as_deref().unwrap_or("disabled")
    );
    println!(
        "Bridge: {}",
        if payload.bridge_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("Bind host: {}", payload.bind_host);
    println!("Bind port: {}", payload.bind_port);
    println!("Guest host: {}", payload.guest_host);
}

/// Pre-download runtime image without starting.
pub async fn provision(format: &OutputFormat) -> Result<()> {
    apply_persisted_runtime_proxy_env()?;
    let runtime = runtime::create_runtime_manager();

    let state = runtime.get_state().await?;
    if state != RuntimeState::None {
        let payload = runtime_status_payload(
            runtime.as_ref(),
            Some("Runtime is already provisioned.".into()),
        )
        .await?;
        match format {
            OutputFormat::Table => println!("Runtime is already provisioned."),
            _ => print_structured(&payload, format)?,
        }
        return Ok(());
    }

    if matches!(format, OutputFormat::Table) {
        println!("Downloading runtime image...");
    }
    runtime
        .provision(Box::new(|progress| {
            if progress.percent > 0.0 {
                eprint!("\r  {} — {:.0}%", progress.message, progress.percent);
            } else {
                eprint!("\r  {}", progress.message);
            }
        }))
        .await?;
    eprintln!(); // newline after progress
    let payload =
        runtime_status_payload(runtime.as_ref(), Some("Provisioning complete.".into())).await?;
    match format {
        OutputFormat::Table => println!("Provisioning complete."),
        _ => print_structured(&payload, format)?,
    }
    Ok(())
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
    fn runtime_status_json_exposes_engine_responsive_alias() {
        let payload = RuntimeStatusPayload {
            state: "ready".to_string(),
            engine_responsive: true,
            compatibility_responsive: true,
            compatibility_version: Some("cratebay-containerd".to_string()),
            engine_source: Some("builtin".to_string()),
            docker_source: Some("builtin".to_string()),
            docker_responsive: true,
            docker_version: Some("cratebay-containerd".to_string()),
            engine: runtime::built_in_engine_status(),
            uptime_seconds: Some(42),
            resource_usage: Some(ResourceUsage {
                cpu_percent: 12.5,
                memory_used_mb: 768,
                memory_total_mb: 2048,
                disk_used_gb: 6.5,
                disk_total_gb: 35.0,
                container_count: 3,
            }),
            socket_path: "/tmp/cratebay/engine.sock".to_string(),
            message: None,
        };

        let json = serde_json::to_value(&payload).expect("runtime payload should serialize");

        assert_eq!(json["engineResponsive"], true);
        assert_eq!(json["compatibilityResponsive"], true);
        assert_eq!(json["compatibilityVersion"], "cratebay-containerd");
        assert_eq!(json["engineSource"], "builtin");
        assert_eq!(json["dockerSource"], "builtin");
        assert_eq!(json["dockerResponsive"], true);
        assert_eq!(json["engine"]["kind"], "cratebay-containerd");
        assert_eq!(json["resourceUsage"]["cpuPercent"], 12.5);
        assert_eq!(json["resourceUsage"]["memoryUsedMb"], 768);
        assert_eq!(json["resourceUsage"]["diskUsedGb"], 6.5);
        assert_eq!(json["resourceUsage"]["containerCount"], 3);
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
    fn runtime_status_table_distinguishes_engine_from_compatibility_source() {
        let source = include_str!("runtime.rs");
        assert!(source.contains("query_built_in_ready_engine_status"));
        assert!(source.contains("Native Engine: responsive"));
        assert!(source.contains("compatibility_responsive"));
        assert!(source.contains("Resource usage: CPU"));
        assert!(source.contains("container_count"));
    }

    #[test]
    fn runtime_status_reconciles_stale_state_when_native_engine_is_ready() {
        assert_eq!(
            reconcile_runtime_state_with_native_engine(RuntimeState::Provisioned, true),
            RuntimeState::Ready
        );
        assert_eq!(
            reconcile_runtime_state_with_native_engine(RuntimeState::Starting, true),
            RuntimeState::Ready
        );
        assert_eq!(
            reconcile_runtime_state_with_native_engine(RuntimeState::Stopping, true),
            RuntimeState::Stopping
        );
        assert_eq!(
            reconcile_runtime_state_with_native_engine(RuntimeState::Provisioned, false),
            RuntimeState::Provisioned
        );
    }

    #[test]
    fn runtime_diagnostics_json_matches_desktop_maintenance_snapshot_shape() {
        let payload = RuntimeDiagnosticsPayload {
            ok: false,
            runtime: RuntimeStatusPayload {
                state: "stopped".to_string(),
                engine_responsive: false,
                compatibility_responsive: false,
                compatibility_version: None,
                engine_source: Some("builtin".to_string()),
                docker_source: Some("builtin".to_string()),
                docker_responsive: false,
                docker_version: None,
                engine: runtime::built_in_engine_status(),
                uptime_seconds: None,
                resource_usage: None,
                socket_path: "/tmp/cratebay/engine.sock".to_string(),
                message: None,
            },
            engine_contract: DiagnosticSection::err("engine offline"),
            substrate: DiagnosticSection::ok(serde_json::json!({
                "engine": "CrateBay Engine",
                "shim": { "backend": "containerd task service" },
                "network": { "stack": "CNI" },
                "storage": { "manager": "cratebay-storage" }
            })),
            storage_gc: DiagnosticSection::ok(serde_json::json!({
                "applied": false,
                "candidateCount": 2,
                "reclaimableBytes": 42
            })),
            shim_tasks: DiagnosticSection::ok(serde_json::json!({
                "count": 0,
                "items": []
            })),
            generated_at_unix: 1,
        };

        let json = serde_json::to_value(&payload).expect("diagnostics payload should serialize");

        assert_eq!(json["ok"], false);
        assert_eq!(json["runtime"]["state"], "stopped");
        assert_eq!(json["engineContract"]["ok"], false);
        assert_eq!(json["engineContract"]["error"], "engine offline");
        assert_eq!(json["substrate"]["value"]["network"]["stack"], "CNI");
        assert_eq!(json["storageGc"]["value"]["candidateCount"], 2);
        assert_eq!(json["shimTasks"]["value"]["count"], 0);
    }

    #[test]
    fn runtime_proxy_settings_parse_desktop_storage_formats() {
        assert_eq!(normalize_optional_setting(Some("  ".into())), None);
        assert_eq!(
            normalize_optional_setting(Some(" http://127.0.0.1:7890 ".into())),
            Some("http://127.0.0.1:7890".into())
        );
        assert_eq!(parse_boolish(Some("on".into())), Some(true));
        assert_eq!(parse_boolish(Some("0".into())), Some(false));
        assert_eq!(parse_boolish(Some("maybe".into())), None);
    }

    #[test]
    fn runtime_proxy_defaults_match_desktop_settings_defaults() {
        let defaults = RuntimeProxySettings::defaults();
        assert_eq!(defaults.proxy, None);
        assert!(!defaults.bridge_enabled);
        assert_eq!(defaults.bind_host, "0.0.0.0");
        assert_eq!(defaults.bind_port, 3128);
        assert_eq!(defaults.guest_host, "192.168.64.1");
    }

    struct EnvGuard {
        saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvGuard {
        fn new(keys: &[&'static str]) -> Self {
            Self {
                saved: keys
                    .iter()
                    .map(|key| (*key, std::env::var_os(key)))
                    .collect(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.saved {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }

    #[test]
    fn persisted_runtime_proxy_env_preserves_explicit_bind_port_override() {
        let _guard = EnvGuard::new(&[
            "CRATEBAY_RUNTIME_HTTP_PROXY",
            "CRATEBAY_RUNTIME_HTTP_PROXY_BRIDGE",
            "CRATEBAY_RUNTIME_HTTP_PROXY_BIND_HOST",
            "CRATEBAY_RUNTIME_HTTP_PROXY_BIND_PORT",
            "CRATEBAY_RUNTIME_HTTP_PROXY_GUEST_HOST",
        ]);
        std::env::remove_var("CRATEBAY_RUNTIME_HTTP_PROXY");
        std::env::remove_var("CRATEBAY_RUNTIME_HTTP_PROXY_BRIDGE");
        std::env::remove_var("CRATEBAY_RUNTIME_HTTP_PROXY_BIND_HOST");
        std::env::set_var("CRATEBAY_RUNTIME_HTTP_PROXY_BIND_PORT", "54346");
        std::env::remove_var("CRATEBAY_RUNTIME_HTTP_PROXY_GUEST_HOST");

        apply_runtime_proxy_env_preserving_existing(&RuntimeProxySettings::defaults());

        assert_eq!(
            std::env::var("CRATEBAY_RUNTIME_HTTP_PROXY_BIND_PORT").as_deref(),
            Ok("54346")
        );
    }
}

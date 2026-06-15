//! Container management Tauri commands.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::time::{sleep, Duration};

use crate::state::{AppState, ContainerTerminalSession};
use cratebay_core::error::AppError;
use cratebay_core::models::AuditAction;
use cratebay_core::models::{
    ContainerCreateRequest, ContainerDetail, ContainerInfo, ContainerListFilters,
    ContainerRunRequest, ContainerRunResult, ContainerState, ContainerStats, ContainerStatus,
    ExecResult, ImageInspectInfo, ImageSearchResult, LocalImageInfo, LogEntry, LogOptions,
    PortMapping, VolumeMount,
};
use cratebay_core::MutexExt;
use cratebay_core::{audit, bundle_images, container, runtime, storage, validation};

/// List available container templates.
#[tauri::command]
pub async fn container_templates(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let db = state.db.lock_or_recover()?;
    storage::list_templates(&db)
}

/// List all containers, optionally filtered.
#[tauri::command]
pub async fn container_list(
    state: State<'_, AppState>,
    filters: Option<ContainerListFilters>,
) -> Result<Vec<ContainerInfo>, AppError> {
    ensure_native_engine(&state).await?;
    let payload = runtime::query_built_in_native_containers(state.runtime.as_ref())?;
    Ok(filter_native_containers(
        payload
            .items
            .into_iter()
            .map(container_info_from_native_summary)
            .collect(),
        filters.as_ref(),
    ))
}

/// Create a new container.
///
/// The caller should ensure the image is available locally before calling.
/// Use `image_pull` to pull images before creation.
#[tauri::command]
pub async fn container_create(
    state: State<'_, AppState>,
    request: ContainerCreateRequest,
) -> Result<ContainerInfo, AppError> {
    validation::validate_container_name(&request.name)?;
    if let (Some(cpu), Some(mem)) = (request.cpu_cores, request.memory_mb) {
        validation::validate_resource_limits(cpu, mem)?;
    }

    ensure_native_engine(&state).await?;
    let payload = native_container_create_payload(&request)?;
    let result = runtime::query_built_in_native_container_create(state.runtime.as_ref(), &payload)?;
    let id = result
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or(request.name.as_str());
    let result = inspect_native_container(&state, id)?.info;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerCreate, &result.id, None, "user")?;

    Ok(result)
}

/// Run a one-shot container, collect output, and optionally remove it.
#[tauri::command]
pub async fn container_run(
    state: State<'_, AppState>,
    request: ContainerRunRequest,
) -> Result<ContainerRunResult, AppError> {
    validate_container_run_request(&request)?;
    ensure_native_engine(&state).await?;

    let payload = native_container_run_payload(&request)?;
    let create_payload =
        runtime::query_built_in_native_container_create(state.runtime.as_ref(), &payload)?;
    let id = create_payload
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| create_payload.get("name").and_then(Value::as_str))
        .unwrap_or("cratebay-run")
        .to_string();
    let name = create_payload
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| payload.get("name").and_then(Value::as_str))
        .unwrap_or(id.as_str())
        .to_string();
    let image = create_payload
        .get("image")
        .and_then(Value::as_str)
        .unwrap_or(request.image.as_str())
        .to_string();

    let result: Result<ContainerRunResult, AppError> = (|| {
        let wait_timeout = request.timeout_secs.filter(|timeout| *timeout > 0);
        let wait_payload = runtime::query_built_in_native_container_wait(
            state.runtime.as_ref(),
            &id,
            wait_timeout,
        )?;
        let timed_out = wait_payload
            .get("timedOut")
            .or_else(|| wait_payload.get("timed_out"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if timed_out {
            let _ =
                runtime::query_built_in_native_container_stop(state.runtime.as_ref(), &id, Some(1));
        }

        let logs_payload = runtime::query_built_in_native_container_logs(
            state.runtime.as_ref(),
            &id,
            None,
            false,
        )?;
        Ok(container_run_result_from_native_payload(
            &id,
            &name,
            &image,
            &wait_payload,
            &logs_payload,
            request.max_output_bytes,
            timed_out,
        ))
    })();

    if request.remove {
        if let Err(error) =
            runtime::query_built_in_native_container_remove(state.runtime.as_ref(), &id, true)
        {
            tracing::warn!("Failed to clean up one-shot container '{}': {}", id, error);
        }
    }

    let result = result?;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerExec, &id, None, "user")?;

    Ok(result)
}

/// Start a stopped container.
#[tauri::command]
pub async fn container_start(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_container_start(state.runtime.as_ref(), &id)?;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerStart, &id, None, "user")?;
    Ok(())
}

/// Stop a running container.
#[tauri::command]
pub async fn container_stop(
    state: State<'_, AppState>,
    id: String,
    timeout: Option<u32>,
) -> Result<(), AppError> {
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_container_stop(
        state.runtime.as_ref(),
        &id,
        timeout.map(u64::from),
    )?;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerStop, &id, None, "user")?;
    Ok(())
}

/// Remove a container.
#[tauri::command]
pub async fn container_delete(
    state: State<'_, AppState>,
    id: String,
    force: Option<bool>,
) -> Result<(), AppError> {
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_container_remove(
        state.runtime.as_ref(),
        &id,
        force.unwrap_or(false),
    )?;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerDelete, &id, None, "user")?;
    Ok(())
}

/// Execute a command inside a running container.
#[tauri::command]
pub async fn container_exec(
    state: State<'_, AppState>,
    id: String,
    cmd: Vec<String>,
    working_dir: Option<String>,
    timeout: Option<u64>,
    max_output_bytes: Option<u64>,
) -> Result<ExecResult, AppError> {
    ensure_native_engine(&state).await?;
    let result = runtime::query_built_in_native_container_exec(
        state.runtime.as_ref(),
        &id,
        cmd,
        working_dir,
        timeout.filter(|value| *value > 0),
        max_output_bytes,
    )?;
    let result = exec_result_from_native_payload(result);

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerExec, &id, None, "user")?;

    Ok(result)
}

/// Get container logs.
#[tauri::command]
pub async fn container_logs(
    state: State<'_, AppState>,
    id: String,
    options: Option<LogOptions>,
) -> Result<Vec<LogEntry>, AppError> {
    ensure_native_engine(&state).await?;
    let tail = options
        .as_ref()
        .and_then(|options| options.tail)
        .map(u64::from);
    let timestamps = options
        .as_ref()
        .and_then(|options| options.timestamps)
        .unwrap_or(false);
    let payload = runtime::query_built_in_native_container_logs(
        state.runtime.as_ref(),
        &id,
        tail,
        timestamps,
    )?;
    Ok(logs_from_native_payload(payload, timestamps))
}

/// Inspect a container for detailed information.
#[tauri::command]
pub async fn container_inspect(
    state: State<'_, AppState>,
    id: String,
) -> Result<ContainerDetail, AppError> {
    ensure_native_engine(&state).await?;
    inspect_native_container(&state, &id)
}

/// Get real-time resource usage snapshot for a container.
#[tauri::command]
pub async fn container_stats(
    state: State<'_, AppState>,
    id: String,
) -> Result<ContainerStats, AppError> {
    ensure_native_engine(&state).await?;
    let payload = runtime::query_built_in_native_container_stats(state.runtime.as_ref(), &id)?;
    Ok(container_stats_from_native_payload(payload))
}

async fn ensure_native_engine(state: &AppState) -> Result<(), AppError> {
    state.ensure_native_engine_once().await
}

fn inspect_native_container(state: &AppState, id: &str) -> Result<ContainerDetail, AppError> {
    let payload = runtime::query_built_in_native_container_inspect(state.runtime.as_ref(), id)?;
    container_detail_from_native_payload(payload)
}

fn container_detail_from_native_payload(payload: Value) -> Result<ContainerDetail, AppError> {
    let item = payload.get("item").unwrap_or(&payload);
    let info = container_info_from_native_inspect_item(item);
    let state = container_state_from_native_item(item);
    let network_settings = item
        .get("networkSettings")
        .or_else(|| item.get("NetworkSettings"))
        .cloned()
        .unwrap_or_else(|| json!({ "Networks": {} }));
    let mounts = item
        .get("mounts")
        .or_else(|| item.get("Mounts"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(ContainerDetail {
        info,
        network_settings,
        mounts,
        state,
    })
}

fn container_info_from_native_summary(summary: runtime::NativeContainerSummary) -> ContainerInfo {
    let labels = string_map_from_value(Some(&summary.labels));
    let status = container_status_from_state(&summary.state, &summary.status);
    ContainerInfo {
        short_id: short_container_id(&summary.id),
        id: summary.id,
        name: summary.name,
        image: summary.image,
        status,
        state: summary.state,
        created_at: String::new(),
        ports: Vec::new(),
        cpu_cores: cpu_cores_from_labels(&labels).or_else(|| cpu_cores_from_value(None)),
        memory_mb: memory_mb_from_labels(&labels).or_else(|| memory_mb_from_value(None)),
        labels,
    }
}

fn container_info_from_native_inspect_item(item: &Value) -> ContainerInfo {
    let id = optional_string_value(item.get("id").or_else(|| item.get("Id"))).unwrap_or_default();
    let name = optional_string_value(item.get("name").or_else(|| item.get("Name")))
        .map(|name| name.trim_start_matches('/').to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    let image = optional_string_value(item.get("image").or_else(|| item.get("Image")))
        .or_else(|| nested_string_value(item, &["config", "Image"]))
        .or_else(|| nested_string_value(item, &["Config", "Image"]))
        .unwrap_or_default();
    let state_text = native_state_text(item);
    let status_text = native_status_text(item).unwrap_or_else(|| state_text.clone());
    let config = item.get("config").or_else(|| item.get("Config"));
    let host_config = item.get("hostConfig").or_else(|| item.get("HostConfig"));
    let labels = string_map_from_value(
        config
            .and_then(|config| config.get("labels").or_else(|| config.get("Labels")))
            .or_else(|| item.get("labels").or_else(|| item.get("Labels"))),
    );

    ContainerInfo {
        short_id: short_container_id(&id),
        id,
        name,
        image,
        status: container_status_from_state(&state_text, &status_text),
        state: state_text,
        created_at: optional_string_value(
            item.get("createdAt")
                .or_else(|| item.get("CreatedAt"))
                .or_else(|| item.get("Created")),
        )
        .unwrap_or_default(),
        ports: port_mappings_from_host_config(host_config),
        cpu_cores: cpu_cores_from_labels(&labels).or_else(|| cpu_cores_from_value(host_config)),
        memory_mb: memory_mb_from_labels(&labels).or_else(|| memory_mb_from_value(host_config)),
        labels,
    }
}

fn container_state_from_native_item(item: &Value) -> ContainerState {
    let state = item.get("state").or_else(|| item.get("State"));
    let status = native_state_text(item);
    ContainerState {
        running: bool_field(state, &["running", "Running"]).unwrap_or(status == "running"),
        status,
        started_at: string_field(state, &["startedAt", "StartedAt"]),
        finished_at: string_field(state, &["finishedAt", "FinishedAt"]),
        exit_code: i64_field(state, &["exitCode", "ExitCode"]),
        error: string_field(state, &["error", "Error"]),
        pid: u64_field(state, &["pid", "Pid"]),
    }
}

fn filter_native_containers(
    mut containers: Vec<ContainerInfo>,
    filters: Option<&ContainerListFilters>,
) -> Vec<ContainerInfo> {
    if let Some(filters) = filters {
        if let Some(statuses) = filters.status.as_ref() {
            containers.retain(|container| statuses.contains(&container.status));
        }
        if let Some(name) = filters
            .name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            containers.retain(|container| container.name.contains(name));
        }
        if let Some(image) = filters
            .image
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            containers.retain(|container| container.image.contains(image));
        }
        if let Some(label) = filters.label.as_ref().filter(|labels| !labels.is_empty()) {
            containers.retain(|container| {
                label
                    .iter()
                    .all(|(key, value)| container.labels.get(key) == Some(value))
            });
        }
    }
    containers.sort_by(|a, b| a.name.cmp(&b.name));
    containers
}

fn native_container_create_payload(request: &ContainerCreateRequest) -> Result<Value, AppError> {
    let pod = trimmed_option(request.pod.as_deref());
    let network = trimmed_option(request.network.as_deref());
    if pod.is_some() && network.is_some() {
        return Err(AppError::Validation(
            "Pod and network cannot both be set for one container".to_string(),
        ));
    }

    let mut labels = request.labels.clone().unwrap_or_default();
    labels.insert("com.cratebay.managed".to_string(), "true".to_string());
    if let Some(cpu) = request.cpu_cores {
        labels.insert("com.cratebay.cpu_cores".to_string(), cpu.to_string());
    }
    if let Some(memory) = request.memory_mb {
        labels.insert("com.cratebay.memory_mb".to_string(), memory.to_string());
    }
    if let Some(template_id) = trimmed_option(request.template_id.as_deref()) {
        labels.insert(
            "com.cratebay.template_id".to_string(),
            template_id.to_string(),
        );
    }
    if let Some(pod) = pod {
        labels.insert("com.cratebay.pod_name".to_string(), pod.to_string());
    }

    Ok(json!({
        "name": request.name,
        "image": request.image,
        "entrypoint": trimmed_option(request.entrypoint.as_deref()),
        "command": trimmed_option(request.command.as_deref()),
        "workingDir": trimmed_option(request.working_dir.as_deref()),
        "env": request.env.clone().unwrap_or_default(),
        "publish": native_publish_specs(request.ports.as_deref())?,
        "volume": native_volume_specs(request.volumes.as_deref())?,
        "pod": pod,
        "network": network,
        "user": trimmed_option(request.user.as_deref()),
        "readOnly": request.read_only_rootfs.unwrap_or(false),
        "noStart": !request.auto_start.unwrap_or(true),
        "cpu": request.cpu_cores,
        "memory": request.memory_mb,
        "labels": labels,
        "registryMirrors": normalized_registry_mirrors(request.registry_mirrors.as_deref()),
        "tty": true,
    }))
}

fn validate_container_run_request(request: &ContainerRunRequest) -> Result<(), AppError> {
    if request.image.trim().is_empty() {
        return Err(AppError::Validation("Image must not be empty".to_string()));
    }
    if request.command.is_empty() || request.command.iter().all(|item| item.trim().is_empty()) {
        return Err(AppError::Validation(
            "Command must not be empty".to_string(),
        ));
    }
    if let Some(name) = trimmed_option(request.name.as_deref()) {
        validation::validate_container_name(name)?;
    }
    if let Some(cpu) = request.cpu_cores {
        if cpu == 0 || cpu > 16 {
            return Err(AppError::Validation("CPU cores must be 1-16".to_string()));
        }
    }
    if let Some(memory) = request.memory_mb {
        if !(256..=65536).contains(&memory) {
            return Err(AppError::Validation(
                "Memory must be 256-65536 MB".to_string(),
            ));
        }
    }
    if trimmed_option(request.pod.as_deref()).is_some()
        && trimmed_option(request.network.as_deref()).is_some()
    {
        return Err(AppError::Validation(
            "Pod and network cannot both be set for one container".to_string(),
        ));
    }
    Ok(())
}

fn native_container_run_payload(request: &ContainerRunRequest) -> Result<Value, AppError> {
    let pod = trimmed_option(request.pod.as_deref());
    let network = trimmed_option(request.network.as_deref());
    let name = trimmed_option(request.name.as_deref())
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("cratebay-run-{}", uuid::Uuid::new_v4().simple()));
    let image = request.image.trim().to_string();

    let mut labels = HashMap::new();
    labels.insert("com.cratebay.managed".to_string(), "true".to_string());
    labels.insert("com.cratebay.run".to_string(), "true".to_string());
    if let Some(cpu) = request.cpu_cores {
        labels.insert("com.cratebay.cpu_cores".to_string(), cpu.to_string());
    }
    if let Some(memory) = request.memory_mb {
        labels.insert("com.cratebay.memory_mb".to_string(), memory.to_string());
    }
    if let Some(pod) = pod {
        labels.insert("com.cratebay.pod_name".to_string(), pod.to_string());
    }

    Ok(json!({
        "name": name,
        "image": image,
        "entrypoint": trimmed_option(request.entrypoint.as_deref()),
        "command": request.command,
        "workingDir": trimmed_option(request.working_dir.as_deref()),
        "env": request.env.clone().unwrap_or_default(),
        "publish": native_publish_specs(request.ports.as_deref())?,
        "volume": native_volume_specs(request.volumes.as_deref())?,
        "pod": pod,
        "network": network,
        "user": trimmed_option(request.user.as_deref()),
        "readOnly": request.read_only_rootfs.unwrap_or(false),
        "noPull": !request.pull,
        "autoStart": true,
        "cpu": request.cpu_cores,
        "memory": request.memory_mb,
        "labels": labels,
        "registryMirrors": normalized_registry_mirrors(request.registry_mirrors.as_deref()),
        "tty": false,
    }))
}

fn normalized_registry_mirrors(mirrors: Option<&[String]>) -> Vec<String> {
    mirrors
        .unwrap_or_default()
        .iter()
        .map(|mirror| mirror.trim().to_string())
        .filter(|mirror| !mirror.is_empty())
        .collect()
}

fn container_run_result_from_native_payload(
    id: &str,
    name: &str,
    image: &str,
    wait_payload: &Value,
    logs_payload: &Value,
    max_output_bytes: Option<u64>,
    timed_out: bool,
) -> ContainerRunResult {
    let max_output_bytes = max_output_bytes.unwrap_or(0);
    let (stdout, stdout_truncated) = truncate_text(
        logs_payload
            .get("stdout")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        max_output_bytes,
    );
    let (stderr, stderr_truncated) = truncate_text(
        logs_payload
            .get("stderr")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        max_output_bytes,
    );
    let exit_code = wait_payload
        .get("exitCode")
        .or_else(|| wait_payload.get("exit_code"))
        .and_then(Value::as_i64)
        .unwrap_or(if timed_out { 124 } else { -1 });

    ContainerRunResult {
        id: id.to_string(),
        name: name.to_string(),
        image: image.to_string(),
        exit_code,
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
        timed_out,
    }
}

fn truncate_text(value: &str, max_bytes: u64) -> (String, bool) {
    if max_bytes == 0 || value.len() <= max_bytes as usize {
        return (value.to_string(), false);
    }
    let mut end = max_bytes as usize;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_string(), true)
}

fn native_publish_specs(ports: Option<&[PortMapping]>) -> Result<Vec<String>, AppError> {
    let mut specs = Vec::new();
    for port in ports.unwrap_or_default() {
        if port.host_port == 0 || port.container_port == 0 {
            return Err(AppError::Validation(
                "Port mappings must use ports in the range 1-65535".to_string(),
            ));
        }
        let protocol = port.protocol.trim().to_ascii_lowercase();
        if !matches!(protocol.as_str(), "tcp" | "udp" | "sctp") {
            return Err(AppError::Validation(format!(
                "Unsupported port protocol '{}'; expected tcp, udp, or sctp",
                port.protocol
            )));
        }
        specs.push(format!(
            "{}:{}/{}",
            port.host_port, port.container_port, protocol
        ));
    }
    Ok(specs)
}

fn native_volume_specs(volumes: Option<&[VolumeMount]>) -> Result<Vec<String>, AppError> {
    let mut specs = Vec::new();
    for volume in volumes.unwrap_or_default() {
        let host_path = volume.host_path.trim();
        let container_path = volume.container_path.trim();
        if host_path.is_empty() || container_path.is_empty() {
            return Err(AppError::Validation(
                "Volume mounts must include both host and container paths".to_string(),
            ));
        }
        if !container_path.starts_with('/') {
            return Err(AppError::Validation(format!(
                "Container mount path '{}' must be absolute",
                container_path
            )));
        }
        let mode = if volume.read_only.unwrap_or(false) {
            "ro"
        } else {
            "rw"
        };
        specs.push(format!("{host_path}:{container_path}:{mode}"));
    }
    Ok(specs)
}

fn logs_from_native_payload(payload: Value, timestamps: bool) -> Vec<LogEntry> {
    let mut entries = Vec::new();
    append_log_entries(
        &mut entries,
        "stdout",
        payload.get("stdout").and_then(Value::as_str).unwrap_or(""),
        timestamps,
    );
    append_log_entries(
        &mut entries,
        "stderr",
        payload.get("stderr").and_then(Value::as_str).unwrap_or(""),
        timestamps,
    );
    if entries.is_empty() {
        append_log_entries(
            &mut entries,
            "stdout",
            payload.get("logs").and_then(Value::as_str).unwrap_or(""),
            timestamps,
        );
    }
    entries
}

fn append_log_entries(entries: &mut Vec<LogEntry>, stream: &str, text: &str, timestamps: bool) {
    for line in text.lines() {
        let (timestamp, message) = if timestamps {
            split_log_timestamp(line)
        } else {
            (None, line.to_string())
        };
        entries.push(LogEntry {
            stream: stream.to_string(),
            message,
            timestamp,
        });
    }
}

fn split_log_timestamp(line: &str) -> (Option<String>, String) {
    let Some((prefix, rest)) = line.split_once(' ') else {
        return (None, line.to_string());
    };
    if prefix.contains('T') && (prefix.ends_with('Z') || prefix.contains('+')) {
        (Some(prefix.to_string()), rest.to_string())
    } else {
        (None, line.to_string())
    }
}

fn exec_result_from_native_payload(payload: Value) -> ExecResult {
    ExecResult {
        exit_code: payload
            .get("exitCode")
            .or_else(|| payload.get("exit_code"))
            .and_then(Value::as_i64)
            .unwrap_or(-1),
        stdout: payload
            .get("stdout")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        stderr: payload
            .get("stderr")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        stdout_truncated: payload
            .get("stdoutTruncated")
            .or_else(|| payload.get("stdout_truncated"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        stderr_truncated: payload
            .get("stderrTruncated")
            .or_else(|| payload.get("stderr_truncated"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        timed_out: payload
            .get("timedOut")
            .or_else(|| payload.get("timed_out"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn container_stats_from_native_payload(payload: Value) -> ContainerStats {
    let cpu = payload.get("cpu").unwrap_or(&Value::Null);
    let memory = payload.get("memory").unwrap_or(&Value::Null);
    let memory_used_mb = number_f64(memory.get("usedMb").or_else(|| memory.get("used_mb")))
        .or_else(|| {
            numeric_u64(
                memory
                    .get("usedBytes")
                    .or_else(|| memory.get("used_bytes"))
                    .or_else(|| memory.get("usage")),
            )
            .map(|bytes| bytes as f64 / 1024.0 / 1024.0)
        })
        .unwrap_or_default();
    let memory_limit_mb = number_f64(memory.get("limitMb").or_else(|| memory.get("limit_mb")))
        .or_else(|| {
            numeric_u64(
                memory
                    .get("limitBytes")
                    .or_else(|| memory.get("limit_bytes"))
                    .or_else(|| memory.get("limit")),
            )
            .map(|bytes| bytes as f64 / 1024.0 / 1024.0)
        })
        .unwrap_or_default();
    let memory_percent = number_f64(memory.get("percent")).unwrap_or_else(|| {
        if memory_limit_mb > 0.0 {
            (memory_used_mb / memory_limit_mb) * 100.0
        } else {
            0.0
        }
    });
    let cpu_percent = number_f64(cpu.get("percent")).unwrap_or_default();

    ContainerStats {
        id: optional_string_value(payload.get("id").or_else(|| payload.get("Id")))
            .unwrap_or_default(),
        name: optional_string_value(payload.get("name").or_else(|| payload.get("Name")))
            .map(|name| name.trim_start_matches('/').to_string())
            .unwrap_or_default(),
        read_at: optional_string_value(
            payload
                .get("readAt")
                .or_else(|| payload.get("read_at"))
                .or_else(|| payload.get("read")),
        )
        .unwrap_or_default(),
        cpu_percent,
        cpu_cores_used: number_f64(cpu.get("coresUsed").or_else(|| cpu.get("cores_used")))
            .unwrap_or(cpu_percent / 100.0),
        memory_used_mb,
        memory_limit_mb,
        memory_percent,
    }
}

fn native_state_text(item: &Value) -> String {
    let state = item.get("state").or_else(|| item.get("State"));
    match state {
        Some(Value::String(value)) => normalize_state(value),
        Some(Value::Object(_)) => string_field(state, &["status", "Status"])
            .map(|value| normalize_state(&value))
            .unwrap_or_else(|| "created".to_string()),
        _ => "created".to_string(),
    }
}

fn native_status_text(item: &Value) -> Option<String> {
    let state = item.get("state").or_else(|| item.get("State"));
    string_field(state, &["statusText", "StatusText", "status", "Status"])
        .or_else(|| optional_string_value(item.get("status").or_else(|| item.get("Status"))))
}

fn container_status_from_state(state: &str, status: &str) -> ContainerStatus {
    let value = if state.trim().is_empty() {
        status
    } else {
        state
    };
    match normalize_state(value).as_str() {
        "running" => ContainerStatus::Running,
        "paused" => ContainerStatus::Paused,
        "restarting" => ContainerStatus::Restarting,
        "removing" => ContainerStatus::Removing,
        "dead" => ContainerStatus::Dead,
        "exited" => ContainerStatus::Exited,
        "created" => ContainerStatus::Created,
        "stopped" => ContainerStatus::Stopped,
        _ => ContainerStatus::Stopped,
    }
}

fn normalize_state(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        "created".to_string()
    } else if normalized == "running" || normalized.starts_with("up") || normalized.contains(" up ")
    {
        "running".to_string()
    } else if normalized.contains("paused") {
        "paused".to_string()
    } else if normalized.contains("restarting") {
        "restarting".to_string()
    } else if normalized.contains("removing") {
        "removing".to_string()
    } else if normalized.contains("dead") {
        "dead".to_string()
    } else if normalized.contains("exited") {
        "exited".to_string()
    } else if normalized.contains("stopped") {
        "stopped".to_string()
    } else if normalized.contains("created") {
        "created".to_string()
    } else {
        normalized
    }
}

fn short_container_id(id: &str) -> String {
    id.chars().take(12).collect()
}

fn trimmed_option(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn string_map_from_value(value: Option<&Value>) -> HashMap<String, String> {
    let Some(Value::Object(map)) = value else {
        return HashMap::new();
    };
    map.iter()
        .filter_map(|(key, value)| value_to_label_string(value).map(|value| (key.clone(), value)))
        .collect()
}

fn value_to_label_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn optional_string_value(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        }
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn string_array_value(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| optional_string_value(Some(item)))
            .collect(),
        Some(Value::String(text)) if !text.trim().is_empty() => vec![text.trim().to_string()],
        _ => Vec::new(),
    }
}

fn nested_string_value(root: &Value, path: &[&str]) -> Option<String> {
    let mut current = root;
    for key in path {
        current = current.get(*key)?;
    }
    optional_string_value(Some(current))
}

fn string_field(value: Option<&Value>, keys: &[&str]) -> Option<String> {
    let value = value?;
    keys.iter()
        .find_map(|key| optional_string_value(value.get(*key)))
}

fn bool_field(value: Option<&Value>, keys: &[&str]) -> Option<bool> {
    let value = value?;
    keys.iter().find_map(|key| match value.get(*key)? {
        Value::Bool(value) => Some(*value),
        Value::Number(number) => Some(number.as_i64().unwrap_or_default() != 0),
        Value::String(text) => Some(matches!(
            text.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )),
        _ => None,
    })
}

fn i64_field(value: Option<&Value>, keys: &[&str]) -> Option<i64> {
    let value = value?;
    keys.iter().find_map(|key| numeric_i64(value.get(*key)))
}

fn u64_field(value: Option<&Value>, keys: &[&str]) -> Option<u64> {
    let value = value?;
    keys.iter().find_map(|key| numeric_u64(value.get(*key)))
}

fn numeric_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().map(|value| value as i64)),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn numeric_u64(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(number) => number.as_u64().or_else(|| {
            number
                .as_i64()
                .filter(|value| *value >= 0)
                .map(|v| v as u64)
        }),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn number_f64(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number
            .as_f64()
            .or_else(|| number.as_i64().map(|value| value as f64))
            .or_else(|| number.as_u64().map(|value| value as f64)),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn cpu_cores_from_labels(labels: &HashMap<String, String>) -> Option<u32> {
    labels
        .get("com.cratebay.cpu_cores")
        .and_then(|value| value.parse().ok())
}

fn memory_mb_from_labels(labels: &HashMap<String, String>) -> Option<u64> {
    labels
        .get("com.cratebay.memory_mb")
        .and_then(|value| value.parse().ok())
}

fn cpu_cores_from_value(host_config: Option<&Value>) -> Option<u32> {
    let nano_cpus = host_config.and_then(|host_config| {
        numeric_u64(
            host_config
                .get("NanoCpus")
                .or_else(|| host_config.get("nanoCpus")),
        )
    })?;
    if nano_cpus == 0 {
        None
    } else {
        Some(((nano_cpus as f64) / 1_000_000_000.0).ceil() as u32)
    }
}

fn memory_mb_from_value(host_config: Option<&Value>) -> Option<u64> {
    let bytes = host_config.and_then(|host_config| {
        numeric_u64(
            host_config
                .get("Memory")
                .or_else(|| host_config.get("memory")),
        )
    })?;
    if bytes == 0 {
        None
    } else {
        Some(bytes / 1024 / 1024)
    }
}

fn port_mappings_from_host_config(host_config: Option<&Value>) -> Vec<PortMapping> {
    let Some(Value::Object(bindings)) = host_config.and_then(|host_config| {
        host_config
            .get("PortBindings")
            .or_else(|| host_config.get("portBindings"))
    }) else {
        return Vec::new();
    };

    let mut ports = Vec::new();
    for (container, host_bindings) in bindings {
        let Some((container_port, protocol)) = parse_container_port_key(container) else {
            continue;
        };
        let Some(items) = host_bindings.as_array() else {
            continue;
        };
        for item in items {
            let Some(host_port) = optional_string_value(
                item.get("HostPort")
                    .or_else(|| item.get("hostPort"))
                    .or_else(|| item.get("host_port")),
            )
            .and_then(|port| port.parse::<u16>().ok())
            .filter(|port| *port > 0) else {
                continue;
            };
            ports.push(PortMapping {
                host_port,
                container_port,
                protocol: protocol.to_string(),
            });
        }
    }
    ports
}

fn parse_container_port_key(value: &str) -> Option<(u16, &str)> {
    let (port, protocol) = value.split_once('/').unwrap_or((value, "tcp"));
    let port = port.parse::<u16>().ok().filter(|port| *port > 0)?;
    Some((port, protocol))
}

fn image_info_from_native_summary(summary: runtime::NativeImageSummary) -> LocalImageInfo {
    let size_bytes = summary.size_bytes;
    let repo_tags = image_tags_from_native_summary(&summary);
    LocalImageInfo {
        id: summary.id,
        repo_tags,
        size: size_bytes.min(i64::MAX as u64) as i64,
        size_bytes,
        size_human: format_bytes_human(size_bytes),
        created: summary.created,
    }
}

fn image_inspect_from_native_payload(payload: Value, fallback_id: &str) -> ImageInspectInfo {
    let inspect = payload.get("inspect").unwrap_or(&payload);
    ImageInspectInfo {
        id: optional_string_value(payload.get("id").or_else(|| inspect.get("Id")))
            .unwrap_or_else(|| fallback_id.to_string()),
        repo_tags: string_array_value(
            payload
                .get("repoTags")
                .or_else(|| payload.get("repo_tags"))
                .or_else(|| inspect.get("RepoTags")),
        ),
        size_bytes: numeric_u64(
            payload
                .get("sizeBytes")
                .or_else(|| payload.get("size_bytes"))
                .or_else(|| inspect.get("Size")),
        )
        .unwrap_or_default(),
        created: optional_string_value(
            payload
                .get("created")
                .or_else(|| inspect.get("Created"))
                .or_else(|| inspect.get("CreatedAt")),
        )
        .unwrap_or_default(),
        architecture: optional_string_value(
            payload
                .get("architecture")
                .or_else(|| inspect.get("Architecture")),
        )
        .unwrap_or_else(|| "unknown".to_string()),
        os: optional_string_value(payload.get("os").or_else(|| inspect.get("Os")))
            .unwrap_or_else(|| "unknown".to_string()),
        docker_version: optional_string_value(
            payload
                .get("runtimeVersion")
                .or_else(|| payload.get("dockerVersion"))
                .or_else(|| inspect.get("DockerVersion")),
        )
        .unwrap_or_else(|| "cratebay-containerd".to_string()),
        layers: numeric_u64(payload.get("layers"))
            .or_else(|| {
                inspect
                    .get("RootFS")
                    .and_then(|root| root.get("Layers"))
                    .and_then(Value::as_array)
                    .map(|layers| layers.len() as u64)
            })
            .unwrap_or_default()
            .min(u32::MAX as u64) as u32,
    }
}

fn image_tags_from_native_summary(summary: &runtime::NativeImageSummary) -> Vec<String> {
    let mut tags = summary
        .tags
        .iter()
        .filter(|tag| !tag.trim().is_empty() && tag.as_str() != "<none>:<none>")
        .cloned()
        .collect::<Vec<_>>();

    if tags.is_empty() && !summary.repository.trim().is_empty() {
        if summary.tag.trim().is_empty() || summary.tag == "<none>" {
            tags.push(summary.repository.clone());
        } else {
            tags.push(format!("{}:{}", summary.repository, summary.tag));
        }
    }

    tags.sort();
    tags.dedup();
    tags
}

fn primary_image_reference(image: &LocalImageInfo) -> &str {
    image
        .repo_tags
        .iter()
        .find(|tag| !tag.trim().is_empty())
        .map(String::as_str)
        .unwrap_or(image.id.as_str())
}

fn image_import_messages_from_native_payload(payload: &Value) -> Vec<String> {
    let mut messages = payload
        .get("images")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if messages.is_empty() {
        if let Some(stdout) = payload.get("stdout").and_then(Value::as_str) {
            messages.extend(
                stdout
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(ToString::to_string),
            );
        }
    }

    messages
}

fn terminal_output_chunks_from_native_payload(payload: &Value) -> Vec<String> {
    payload
        .get("chunks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|chunk| chunk.get("data").and_then(Value::as_str))
        .filter(|data| !data.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn format_bytes_human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// Execute a command with streaming output via Tauri Events.
///
/// Output is emitted as events on `exec:stream:{channel_id}`.
#[tauri::command]
pub async fn container_exec_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    cmd: Vec<String>,
    channel_id: String,
    working_dir: Option<String>,
) -> Result<(), AppError> {
    ensure_native_engine(&state).await?;
    let event_name = format!("exec:stream:{}", channel_id);
    let result = runtime::query_built_in_native_container_exec(
        state.runtime.as_ref(),
        &id,
        cmd,
        working_dir,
        None,
        None,
    )?;
    if let Some(stdout) = result.get("stdout").and_then(Value::as_str) {
        if !stdout.is_empty() {
            let _ = app.emit(
                &event_name,
                &json!({
                    "type": "stdout",
                    "data": stdout,
                }),
            );
        }
    }
    if let Some(stderr) = result.get("stderr").and_then(Value::as_str) {
        if !stderr.is_empty() {
            let _ = app.emit(
                &event_name,
                &json!({
                    "type": "stderr",
                    "data": stderr,
                }),
            );
        }
    }
    let _ = app.emit(
        &event_name,
        &json!({
            "type": "done",
            "exitCode": result.get("exitCode").and_then(Value::as_i64).unwrap_or_default(),
        }),
    );

    // Audit
    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerExec, &id, None, "user")?;

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
enum TerminalStreamChunk {
    Output { data: String },
    Done { exit_code: i64 },
    Error { message: String },
}

/// Open an interactive TTY-backed shell inside a running container.
#[tauri::command]
pub async fn container_terminal_open(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    session_id: String,
    cols: Option<u16>,
    rows: Option<u16>,
) -> Result<(), AppError> {
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_container_terminal_open(
        state.runtime.as_ref(),
        &id,
        &session_id,
        cols,
        rows,
        None,
        None,
    )?;
    let event_name = format!("terminal:stream:{}", session_id);
    let closed = Arc::new(AtomicBool::new(false));

    state.terminal_sessions.lock().await.insert(
        session_id.clone(),
        ContainerTerminalSession {
            container_id: id.clone(),
            closed: closed.clone(),
        },
    );

    let app_handle = app.clone();
    let sessions = state.terminal_sessions.clone();
    let runtime = state.runtime.clone();
    let container_id = id.clone();
    let session_key = session_id.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            if closed.load(Ordering::SeqCst) {
                break;
            }

            match runtime::query_built_in_native_container_terminal_read(
                runtime.as_ref(),
                &container_id,
                &session_key,
            ) {
                Ok(payload) => {
                    for data in terminal_output_chunks_from_native_payload(&payload) {
                        let _ = app_handle.emit(&event_name, TerminalStreamChunk::Output { data });
                    }
                    if let Some(exit_code) = payload.get("exitCode").and_then(Value::as_i64) {
                        let _ =
                            app_handle.emit(&event_name, TerminalStreamChunk::Done { exit_code });
                        break;
                    }
                }
                Err(error) => {
                    let _ = app_handle.emit(
                        &event_name,
                        TerminalStreamChunk::Error {
                            message: error.to_string(),
                        },
                    );
                    break;
                }
            }
            sleep(Duration::from_millis(80)).await;
        }

        sessions.lock().await.remove(&session_key);
    });

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerExec, &id, None, "user")?;
    Ok(())
}

/// Write raw terminal input to an interactive container shell.
#[tauri::command]
pub async fn container_terminal_input(
    state: State<'_, AppState>,
    session_id: String,
    data: String,
) -> Result<(), AppError> {
    let container_id = {
        let sessions = state.terminal_sessions.lock().await;
        sessions
            .get(&session_id)
            .map(|session| session.container_id.clone())
            .ok_or_else(|| AppError::NotFound {
                entity: "container terminal session".to_string(),
                id: session_id.clone(),
            })?
    };
    runtime::query_built_in_native_container_terminal_input(
        state.runtime.as_ref(),
        &container_id,
        &session_id,
        &data,
    )?;
    Ok(())
}

/// Resize the TTY allocated for an interactive container shell.
#[tauri::command]
pub async fn container_terminal_resize(
    state: State<'_, AppState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), AppError> {
    let container_id = {
        let sessions = state.terminal_sessions.lock().await;
        sessions
            .get(&session_id)
            .map(|session| session.container_id.clone())
            .ok_or_else(|| AppError::NotFound {
                entity: "container terminal session".to_string(),
                id: session_id.clone(),
            })?
    };
    runtime::query_built_in_native_container_terminal_resize(
        state.runtime.as_ref(),
        &container_id,
        &session_id,
        cols,
        rows,
    )?;
    Ok(())
}

/// Ask the interactive shell to exit and close its stdin stream.
#[tauri::command]
pub async fn container_terminal_close(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), AppError> {
    let mut sessions = state.terminal_sessions.lock().await;
    if let Some(session) = sessions.remove(&session_id) {
        session.closed.store(true, Ordering::SeqCst);
        let _ = runtime::query_built_in_native_container_terminal_close(
            state.runtime.as_ref(),
            &session.container_id,
            &session_id,
        );
    }
    Ok(())
}

/// List local engine images.
#[tauri::command]
pub async fn image_list(state: State<'_, AppState>) -> Result<Vec<LocalImageInfo>, AppError> {
    ensure_native_engine(&state).await?;
    let payload = runtime::query_built_in_native_images(state.runtime.as_ref())?;
    let mut images = payload
        .items
        .into_iter()
        .map(image_info_from_native_summary)
        .collect::<Vec<_>>();
    images.sort_by(|a, b| {
        primary_image_reference(a)
            .cmp(primary_image_reference(b))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(images)
}

/// Search images from registries without touching the runtime.
#[tauri::command]
pub async fn image_search(
    _state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<ImageSearchResult>, AppError> {
    let term = query.trim();
    let limit = limit.map(u64::from);

    container::image_search_dockerhub(term, limit).await
}

/// Inspect a local image by id or reference.
#[tauri::command]
pub async fn image_inspect(
    state: State<'_, AppState>,
    id: String,
) -> Result<ImageInspectInfo, AppError> {
    ensure_native_engine(&state).await?;
    let payload = runtime::query_built_in_native_image_inspect(state.runtime.as_ref(), &id)?;
    Ok(image_inspect_from_native_payload(payload, &id))
}

/// Remove a local image.
#[tauri::command]
pub async fn image_remove(
    state: State<'_, AppState>,
    id: String,
    force: Option<bool>,
) -> Result<(), AppError> {
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_image_remove(
        state.runtime.as_ref(),
        &id,
        force.unwrap_or(false),
    )?;
    Ok(())
}

/// Tag a local image with a new `repo:tag`.
#[tauri::command]
pub async fn image_tag(
    state: State<'_, AppState>,
    source: String,
    target: String,
) -> Result<(), AppError> {
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_image_tag(state.runtime.as_ref(), &source, &target)?;
    Ok(())
}

/// Commit a container into a new local image tag.
#[tauri::command]
pub async fn image_pack_container(
    state: State<'_, AppState>,
    container: String,
    image: String,
) -> Result<String, AppError> {
    ensure_native_engine(&state).await?;
    let payload = runtime::query_built_in_native_image_pack_container(
        state.runtime.as_ref(),
        &container,
        &image,
    )?;
    Ok(payload
        .get("imageRef")
        .or_else(|| payload.get("image"))
        .and_then(Value::as_str)
        .unwrap_or(image.as_str())
        .to_string())
}

/// Export one or more local images to a tar archive.
#[tauri::command]
pub async fn image_export(
    state: State<'_, AppState>,
    images: Vec<String>,
    output: String,
) -> Result<u64, AppError> {
    ensure_native_engine(&state).await?;
    let archive = runtime::query_built_in_native_image_export(state.runtime.as_ref(), &images)?;
    if let Some(parent) = std::path::Path::new(&output)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, &archive)?;
    Ok(archive.len() as u64)
}

/// Import images from a tar archive.
#[tauri::command]
pub async fn image_import(
    state: State<'_, AppState>,
    input: String,
) -> Result<Vec<String>, AppError> {
    ensure_native_engine(&state).await?;
    let payload = runtime::query_built_in_native_image_import_file(
        state.runtime.as_ref(),
        Path::new(&input),
    )?;
    Ok(image_import_messages_from_native_payload(&payload))
}

/// Load bundled CrateBay container images into the built-in runtime.
#[tauri::command]
pub async fn image_preload_bundled(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<bundle_images::BundleImageLoadResult>, AppError> {
    ensure_native_engine(&state).await?;
    let bundle_dir = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("bundle-images"))
        .filter(|dir| dir.is_dir())
        .or_else(bundle_images::find_bundle_image_dir)
        .ok_or_else(|| {
            AppError::Runtime(
                "No bundle-images directory found. Set CRATEBAY_BUNDLE_IMAGES_DIR or include bundle-images in the app resources."
                    .to_string(),
            )
        })?;

    Ok(
        bundle_images::load_bundle_images_from_dir_native(state.runtime.as_ref(), &bundle_dir)
            .await,
    )
}

/// Pull an OCI/Docker image (non-blocking).
///
/// Spawns the pull operation in the background so it doesn't block other Tauri commands.
/// Progress and completion are reported via `image:pull:{channel_id}` events.
///
/// Returns immediately with the channel_id for the frontend to listen on.
#[tauri::command]
pub async fn image_pull(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    image: String,
    mirrors: Option<Vec<String>>,
    channel_id: Option<String>,
) -> Result<String, AppError> {
    let channel_id = channel_id.unwrap_or_else(|| format!("pull-{}", uuid::Uuid::new_v4()));
    let ch_id = channel_id.clone();
    let app_handle = app.clone();
    let image_clone = image.clone();
    let mirrors = mirrors
        .unwrap_or_default()
        .into_iter()
        .map(|mirror| mirror.trim().to_string())
        .filter(|mirror| !mirror.is_empty())
        .collect::<Vec<_>>();

    // Emit start event
    emit_image_pull_progress(
        &app,
        &crate::events::image_pull_progress_event(&channel_id),
        0,
        format!("Starting image pull {}", &image),
        false,
        None,
    );

    ensure_native_engine(&state).await?;
    let runtime = state.runtime.clone();

    // Spawn background task — does NOT block the Tauri command handler.
    tauri::async_runtime::spawn_blocking(move || {
        let event_name = crate::events::image_pull_progress_event(&ch_id);
        let result = native_image_pull_with_mirrors(
            runtime.as_ref(),
            &app_handle,
            &event_name,
            &image_clone,
            &mirrors,
        );

        match result {
            Ok(()) => {
                emit_image_pull_progress(
                    &app_handle,
                    &event_name,
                    100,
                    format!("Image {} pull completed", &image_clone),
                    true,
                    None,
                );
            }
            Err(e) => {
                let error_msg = e.to_string();
                tracing::error!(
                    "Native image pull failed for {}: {}",
                    image_clone,
                    error_msg
                );
                emit_image_pull_progress(
                    &app_handle,
                    &event_name,
                    0,
                    format!("Image pull failed: {}", error_msg),
                    true,
                    Some(error_msg),
                );
            }
        }
    });

    // Return immediately with the channel_id
    Ok(channel_id)
}

fn native_image_pull_with_mirrors(
    runtime: &dyn runtime::RuntimeManager,
    app: &AppHandle,
    event_name: &str,
    image: &str,
    mirrors: &[String],
) -> Result<(), AppError> {
    if mirrors.is_empty() {
        emit_image_pull_progress(
            app,
            event_name,
            10,
            format!("CrateBay Engine pulling image {}", image),
            false,
            None,
        );
        runtime::query_built_in_native_image_pull(runtime, image, None)?;
        return Ok(());
    }

    let total = mirrors.len();
    for (index, mirror) in mirrors.iter().enumerate() {
        emit_image_pull_progress(
            app,
            event_name,
            5 + ((index as u32 * 65) / total as u32),
            format!("Trying mirror {}/{}: {}", index + 1, total, mirror),
            false,
            None,
        );

        let mirror_ref = rewrite_image_for_native_mirror(image, mirror);
        match runtime::query_built_in_native_image_pull(runtime, &mirror_ref, None) {
            Ok(_) => {
                if mirror_ref != image {
                    emit_image_pull_progress(
                        app,
                        event_name,
                        85,
                        format!("Mirror pull completed, tagging as {}", image),
                        false,
                        None,
                    );
                    if let Err(error) =
                        runtime::query_built_in_native_image_tag(runtime, &mirror_ref, image)
                    {
                        tracing::warn!(
                            "Failed to re-tag native mirror image '{}' to '{}': {}",
                            mirror_ref,
                            image,
                            error
                        );
                        emit_image_pull_progress(
                            app,
                            event_name,
                            0,
                            format!("Mirror {} tag failed, trying next...", mirror),
                            false,
                            None,
                        );
                        continue;
                    }
                    if let Err(error) =
                        runtime::query_built_in_native_image_remove(runtime, &mirror_ref, true)
                    {
                        tracing::warn!(
                            "Failed to remove native mirror image tag '{}': {}",
                            mirror_ref,
                            error
                        );
                    }
                }
                return Ok(());
            }
            Err(error) => {
                tracing::warn!(
                    "Native mirror '{}' failed for '{}': {}",
                    mirror,
                    image,
                    error
                );
                emit_image_pull_progress(
                    app,
                    event_name,
                    0,
                    format!("Mirror {} failed, trying next...", mirror),
                    false,
                    None,
                );
            }
        }
    }

    emit_image_pull_progress(
        app,
        event_name,
        75,
        "All mirrors failed, trying direct pull...".to_string(),
        false,
        None,
    );
    runtime::query_built_in_native_image_pull(runtime, image, None)?;
    Ok(())
}

fn rewrite_image_for_native_mirror(image: &str, mirror: &str) -> String {
    let image = image.trim();
    let mirror = normalize_native_registry_mirror(mirror);

    if image.is_empty() || mirror.is_empty() {
        return image.to_string();
    }

    if let Some(first_slash_pos) = image.find('/') {
        let before_slash = &image[..first_slash_pos];
        if before_slash.contains('.') || before_slash.contains(':') {
            return image.to_string();
        }
        return format!("{}/{}", mirror, image);
    }

    format!("{}/library/{}", mirror, image)
}

fn normalize_native_registry_mirror(mirror: &str) -> String {
    mirror
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string()
}

fn emit_image_pull_progress(
    app: &AppHandle,
    event_name: &str,
    progress_percent: u32,
    status: String,
    complete: bool,
    error: Option<String>,
) {
    let _ = app.emit(
        event_name,
        &crate::events::ImagePullProgress {
            current_layer: 0,
            total_layers: 0,
            progress_percent,
            status,
            complete,
            error,
            current_bytes: 0,
            total_bytes: 0,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_mirror_rewrite_keeps_docker_hub_rules() {
        let mirror = "https://mirror.example.com/";

        assert_eq!(
            rewrite_image_for_native_mirror("node:20-alpine", mirror),
            "mirror.example.com/library/node:20-alpine"
        );
        assert_eq!(
            rewrite_image_for_native_mirror("library/node:20-alpine", mirror),
            "mirror.example.com/library/node:20-alpine"
        );
        assert_eq!(
            rewrite_image_for_native_mirror("myuser/myapp:latest", mirror),
            "mirror.example.com/myuser/myapp:latest"
        );
    }

    #[test]
    fn native_mirror_rewrite_leaves_explicit_registries_unchanged() {
        assert_eq!(
            rewrite_image_for_native_mirror("gcr.io/project/image:tag", "mirror.local"),
            "gcr.io/project/image:tag"
        );
        assert_eq!(
            rewrite_image_for_native_mirror(
                "registry.example.com:5000/team/image:tag",
                "mirror.local"
            ),
            "registry.example.com:5000/team/image:tag"
        );
    }

    #[test]
    fn native_list_payload_maps_and_filters_to_gui_containers() {
        let containers = vec![
            container_info_from_native_summary(runtime::NativeContainerSummary {
                id: "abcdef1234567890".to_string(),
                name: "sandbox-a".to_string(),
                image: "alpine:latest".to_string(),
                state: "running".to_string(),
                status: "Up 3 seconds".to_string(),
                labels: json!({ "com.cratebay.managed": "true" }),
                managed_by: "cratebay".to_string(),
            }),
            container_info_from_native_summary(runtime::NativeContainerSummary {
                id: "deadbeef".to_string(),
                name: "worker-b".to_string(),
                image: "ubuntu:24.04".to_string(),
                state: "exited".to_string(),
                status: "Exited (0)".to_string(),
                labels: json!({ "com.cratebay.managed": "true" }),
                managed_by: "cratebay".to_string(),
            }),
        ];
        let filtered = filter_native_containers(
            containers,
            Some(&ContainerListFilters {
                status: Some(vec![ContainerStatus::Running]),
                name: Some("sandbox".to_string()),
                image: Some("alpine".to_string()),
                label: None,
            }),
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].short_id, "abcdef123456");
        assert_eq!(filtered[0].status, ContainerStatus::Running);
    }

    #[test]
    fn native_inspect_payload_maps_to_container_detail() {
        let detail = container_detail_from_native_payload(json!({
            "api": "cratebay.container.inspect.v1",
            "item": {
                "id": "abc123",
                "name": "sandbox",
                "image": "alpine:latest",
                "createdAt": "2026-06-03T00:00:00Z",
                "state": {
                    "Status": "running",
                    "Running": true,
                    "ExitCode": 0,
                    "Pid": 42
                },
                "config": {
                    "Labels": {
                        "com.cratebay.managed": "true",
                        "com.cratebay.cpu_cores": "2",
                        "com.cratebay.memory_mb": "512"
                    }
                },
                "hostConfig": {
                    "PortBindings": {
                        "80/tcp": [{ "HostPort": "8080" }]
                    }
                },
                "networkSettings": { "Networks": { "demo-pod": {} } },
                "mounts": [{ "Destination": "/workspace" }]
            }
        }))
        .expect("native inspect should map");

        assert_eq!(detail.info.name, "sandbox");
        assert_eq!(detail.info.status, ContainerStatus::Running);
        assert_eq!(detail.info.cpu_cores, Some(2));
        assert_eq!(detail.info.memory_mb, Some(512));
        assert_eq!(detail.info.ports[0].host_port, 8080);
        assert!(detail.state.running);
        assert_eq!(detail.state.pid, Some(42));
        assert_eq!(detail.mounts.len(), 1);
    }

    #[test]
    fn native_logs_payload_maps_stdout_and_stderr_lines() {
        let logs = logs_from_native_payload(
            json!({
                "api": "cratebay.container.logs.v1",
                "stdout": "2026-06-03T00:00:00Z ready\nnext\n",
                "stderr": "warn\n",
                "timestamps": true
            }),
            true,
        );

        assert_eq!(logs.len(), 3);
        assert_eq!(logs[0].stream, "stdout");
        assert_eq!(logs[0].timestamp.as_deref(), Some("2026-06-03T00:00:00Z"));
        assert_eq!(logs[0].message, "ready");
        assert_eq!(logs[2].stream, "stderr");
    }

    #[test]
    fn native_exec_payload_maps_to_exec_result() {
        let result = exec_result_from_native_payload(json!({
            "api": "cratebay.container.exec.v1",
            "exitCode": 7,
            "stdout": "out",
            "stderr": "err",
            "timedOut": true,
            "stdoutTruncated": true,
            "stderrTruncated": false
        }));

        assert_eq!(result.exit_code, 7);
        assert_eq!(result.stdout, "out");
        assert_eq!(result.stderr, "err");
        assert!(result.timed_out);
        assert!(result.stdout_truncated);
        assert!(!result.stderr_truncated);
    }

    #[test]
    fn native_image_import_payload_maps_to_gui_messages() {
        let messages = image_import_messages_from_native_payload(&json!({
            "api": "cratebay.image.import.v1",
            "backend": "containerd",
            "images": ["unpacking docker.io/library/sandbox:latest...done"],
            "stdout": "ignored when images exist\n"
        }));

        assert_eq!(
            messages,
            vec!["unpacking docker.io/library/sandbox:latest...done".to_string()]
        );

        let fallback = image_import_messages_from_native_payload(&json!({
            "api": "cratebay.image.import.v1",
            "backend": "containerd",
            "stdout": "line one\n\nline two\n"
        }));

        assert_eq!(
            fallback,
            vec!["line one".to_string(), "line two".to_string()]
        );
    }

    #[test]
    fn native_terminal_read_payload_maps_to_terminal_output() {
        let chunks = terminal_output_chunks_from_native_payload(&json!({
            "api": "cratebay.container.terminal.read.v1",
            "chunks": [
                { "stream": "stdout", "data": "hello" },
                { "stream": "stderr", "data": "\nwarn" },
                { "stream": "stdout", "data": "" }
            ],
            "running": true
        }));

        assert_eq!(chunks, vec!["hello".to_string(), "\nwarn".to_string()]);
    }

    #[test]
    fn terminal_stream_events_keep_frontend_variant_tags() {
        let output = serde_json::to_value(TerminalStreamChunk::Output {
            data: "ready\n".to_string(),
        })
        .expect("terminal output event should serialize");
        let done = serde_json::to_value(TerminalStreamChunk::Done { exit_code: 0 })
            .expect("terminal done event should serialize");
        let error = serde_json::to_value(TerminalStreamChunk::Error {
            message: "failed".to_string(),
        })
        .expect("terminal error event should serialize");

        assert_eq!(output["type"], "Output");
        assert_eq!(output["data"], "ready\n");
        assert_eq!(done["type"], "Done");
        assert_eq!(done["exit_code"], 0);
        assert_eq!(error["type"], "Error");
        assert_eq!(error["message"], "failed");
    }

    #[test]
    fn native_stats_payload_maps_to_container_stats() {
        let stats = container_stats_from_native_payload(json!({
            "api": "cratebay.container.stats.v1",
            "id": "abc123",
            "name": "sandbox",
            "readAt": "2026-06-03T00:00:00Z",
            "backend": "containerd",
            "cpu": {
                "percent": 12.5,
                "coresUsed": 0.125
            },
            "memory": {
                "usedBytes": 1048576,
                "limitBytes": 4194304,
                "percent": 25.0
            }
        }));

        assert_eq!(stats.id, "abc123");
        assert_eq!(stats.name, "sandbox");
        assert_eq!(stats.cpu_percent, 12.5);
        assert_eq!(stats.cpu_cores_used, 0.125);
        assert_eq!(stats.memory_used_mb, 1.0);
        assert_eq!(stats.memory_limit_mb, 4.0);
        assert_eq!(stats.memory_percent, 25.0);
    }

    #[test]
    fn native_image_summary_maps_to_local_image_info() {
        let image = image_info_from_native_summary(runtime::NativeImageSummary {
            id: "sha256:abc123".to_string(),
            repository: "cratebay-runtime-smoke".to_string(),
            tag: "local".to_string(),
            tags: vec![
                "cratebay-runtime-smoke:local".to_string(),
                "<none>:<none>".to_string(),
            ],
            digests: vec!["cratebay-runtime-smoke@sha256:def456".to_string()],
            size_bytes: 1_572_864,
            created: 1_780_448_112,
            labels: json!({ "com.cratebay.managed": "true" }),
            managed_by: "cratebay".to_string(),
        });
        let fallback = image_info_from_native_summary(runtime::NativeImageSummary {
            id: "sha256:def456".to_string(),
            repository: "alpine".to_string(),
            tag: "3.20".to_string(),
            tags: Vec::new(),
            digests: Vec::new(),
            size_bytes: 512,
            created: 1,
            labels: json!({}),
            managed_by: "cratebay".to_string(),
        });

        assert_eq!(image.repo_tags, vec!["cratebay-runtime-smoke:local"]);
        assert_eq!(image.size, 1_572_864);
        assert_eq!(image.size_bytes, 1_572_864);
        assert_eq!(image.size_human, "1.5 MB");
        assert_eq!(fallback.repo_tags, vec!["alpine:3.20"]);
        assert_eq!(primary_image_reference(&fallback), "alpine:3.20");
    }

    #[test]
    fn native_image_inspect_payload_maps_to_gui_model() {
        let info = image_inspect_from_native_payload(
            json!({
                "api": "cratebay.image.inspect.v1",
                "id": "sha256:abc123",
                "imageRef": "docker.io/library/alpine:3.20@sha256:abc123",
                "repoTags": ["docker.io/library/alpine:3.20"],
                "sizeBytes": 123456,
                "created": "2026-06-03T00:00:00Z",
                "architecture": "x86_64",
                "os": "linux",
                "runtimeVersion": "cratebay-containerd",
                "layers": 2,
                "backend": "containerd"
            }),
            "alpine:3.20",
        );

        assert_eq!(info.id, "sha256:abc123");
        assert_eq!(info.repo_tags, vec!["docker.io/library/alpine:3.20"]);
        assert_eq!(info.size_bytes, 123456);
        assert_eq!(info.created, "2026-06-03T00:00:00Z");
        assert_eq!(info.architecture, "x86_64");
        assert_eq!(info.os, "linux");
        assert_eq!(info.docker_version, "cratebay-containerd");
        assert_eq!(info.layers, 2);
    }

    #[test]
    fn create_request_payload_uses_native_engine_shape() {
        let payload = native_container_create_payload(&ContainerCreateRequest {
            name: "sandbox".to_string(),
            image: "alpine:latest".to_string(),
            entrypoint: None,
            command: Some("sleep 60".to_string()),
            env: Some(vec!["A=B".to_string()]),
            ports: Some(vec![PortMapping {
                host_port: 8080,
                container_port: 80,
                protocol: "tcp".to_string(),
            }]),
            volumes: Some(vec![VolumeMount {
                host_path: "/host".to_string(),
                container_path: "/workspace".to_string(),
                read_only: Some(true),
            }]),
            cpu_cores: Some(2),
            memory_mb: Some(1024),
            working_dir: Some("/workspace".to_string()),
            pod: Some("demo-pod".to_string()),
            network: None,
            user: Some("1000:1000".to_string()),
            read_only_rootfs: Some(true),
            auto_start: Some(false),
            labels: None,
            template_id: Some("template-1".to_string()),
            registry_mirrors: Some(vec![
                " https://mirror.example.com/ ".to_string(),
                "mirror-2.example.com".to_string(),
            ]),
        })
        .expect("native create payload should build");

        assert_eq!(payload["pod"], "demo-pod");
        assert_eq!(payload["publish"][0], "8080:80/tcp");
        assert_eq!(payload["volume"][0], "/host:/workspace:ro");
        assert_eq!(payload["readOnly"], true);
        assert_eq!(payload["noStart"], true);
        assert_eq!(payload["labels"]["com.cratebay.template_id"], "template-1");
        assert_eq!(payload["registryMirrors"][0], "https://mirror.example.com/");
        assert_eq!(payload["registryMirrors"][1], "mirror-2.example.com");
    }

    #[test]
    fn run_request_payload_uses_native_engine_shape() {
        let payload = native_container_run_payload(&ContainerRunRequest {
            name: Some("sandbox-run".to_string()),
            image: "alpine:latest".to_string(),
            entrypoint: Some("/bin/sh".to_string()),
            command: vec!["-lc".to_string(), "echo hello".to_string()],
            env: Some(vec!["A=B".to_string()]),
            ports: Some(vec![PortMapping {
                host_port: 8080,
                container_port: 80,
                protocol: "sctp".to_string(),
            }]),
            volumes: Some(vec![VolumeMount {
                host_path: "/host".to_string(),
                container_path: "/workspace".to_string(),
                read_only: Some(false),
            }]),
            cpu_cores: Some(2),
            memory_mb: Some(1024),
            working_dir: Some("/workspace".to_string()),
            pod: Some("demo-pod".to_string()),
            network: None,
            user: Some("1000:1000".to_string()),
            read_only_rootfs: Some(true),
            pull: false,
            remove: true,
            timeout_secs: Some(30),
            max_output_bytes: Some(16),
            registry_mirrors: Some(vec!["docker.1ms.run".to_string()]),
        })
        .expect("native run payload should build");

        assert_eq!(payload["name"], "sandbox-run");
        assert_eq!(payload["command"][0], "-lc");
        assert_eq!(payload["pod"], "demo-pod");
        assert_eq!(payload["publish"][0], "8080:80/sctp");
        assert_eq!(payload["volume"][0], "/host:/workspace:rw");
        assert_eq!(payload["registryMirrors"][0], "docker.1ms.run");
        assert_eq!(payload["readOnly"], true);
        assert_eq!(payload["noPull"], true);
        assert_eq!(payload["autoStart"], true);
        assert_eq!(payload["tty"], false);
        assert_eq!(payload["labels"]["com.cratebay.run"], "true");
        assert_eq!(payload["labels"]["com.cratebay.pod_name"], "demo-pod");
    }

    #[test]
    fn run_result_truncates_output_on_utf8_boundaries() {
        let result = container_run_result_from_native_payload(
            "abc123",
            "sandbox-run",
            "alpine:latest",
            &json!({ "exitCode": 0, "timedOut": false }),
            &json!({
                "stdout": "hello 世界",
                "stderr": "warning"
            }),
            Some(8),
            false,
        );

        assert_eq!(result.id, "abc123");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "hello ");
        assert!(result.stdout_truncated);
        assert_eq!(result.stderr, "warning");
        assert!(!result.stderr_truncated);
        assert!(!result.timed_out);
    }
}

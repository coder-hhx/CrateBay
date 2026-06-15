//! Built-in container runtime — platform dispatch.
//!
//! macOS: VZ.framework | Linux: KVM/QEMU | Windows: WSL2
//!
//! This module defines the platform-agnostic [`RuntimeManager`] trait and
//! all supporting types for managing the built-in container runtime.

pub mod common;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::ResourceUsage;

pub const BUILT_IN_ENGINE_API: &str = "cratebay.engine.v1";
pub const BUILT_IN_ENGINE_KIND: &str = "cratebay-containerd";

// ---------------------------------------------------------------------------
// Runtime State Machine (§3.1)
// ---------------------------------------------------------------------------

/// Runtime lifecycle state.
///
/// Follows the state machine defined in runtime-spec.md §3.1:
/// `None → Provisioned → Starting → Ready → Stopping → Stopped`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RuntimeState {
    /// No runtime detected, needs provisioning.
    None,
    /// Image ready, VM not started.
    Provisioned,
    /// VM is booting or the CrateBay Engine is initializing.
    Starting,
    /// The native CrateBay Engine contract is available and responsive.
    Ready,
    /// VM is shutting down gracefully.
    Stopping,
    /// VM has been stopped.
    Stopped,
    /// Runtime error with description.
    Error(String),
}

// ---------------------------------------------------------------------------
// Provision Progress (§3.2)
// ---------------------------------------------------------------------------

/// Progress information emitted during the provisioning process.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvisionProgress {
    /// Current stage: "downloading", "extracting", "configuring", "complete".
    pub stage: String,
    /// Progress percentage (0.0 — 100.0).
    pub percent: f32,
    /// Bytes downloaded so far.
    pub bytes_downloaded: u64,
    /// Total bytes to download.
    pub bytes_total: u64,
    /// Human-readable progress message.
    pub message: String,
}

// ---------------------------------------------------------------------------
// Health Status (§9.1)
// ---------------------------------------------------------------------------

/// Health check result for the runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Current runtime state.
    pub runtime_state: RuntimeState,
    /// Whether the native CrateBay Engine contract is responding.
    #[serde(default)]
    pub engine_responsive: bool,
    /// Whether the Engine compatibility endpoint is responding to pings.
    #[serde(default)]
    pub compatibility_responsive: bool,
    /// Engine compatibility endpoint version (if responsive).
    #[serde(default)]
    pub compatibility_version: Option<String>,
    /// Whether the engine compatibility API is responding to pings.
    ///
    /// Kept for older frontend and automation clients. Prefer
    /// `engine_responsive` or `compatibility_responsive`.
    pub docker_responsive: bool,
    /// Engine compatibility API version (if responsive).
    ///
    /// Kept for older frontend and automation clients. Prefer
    /// `compatibility_version`.
    pub docker_version: Option<String>,
    /// VM uptime in seconds (if running).
    pub uptime_seconds: Option<u64>,
    /// Timestamp of this health check (RFC 3339).
    pub last_check: String,
    /// Which engine backend is currently connected (always "builtin" for CrateBay runtime).
    #[serde(default)]
    pub engine_source: Option<String>,
    /// Compatibility alias for older frontend and automation clients.
    pub docker_source: Option<String>,
    /// The underlying CrateBay runtime engine exposed through the compatibility API.
    pub engine: RuntimeEngineStatus,
}

/// Engine metadata for the built-in runtime.
///
/// CrateBay exposes its own engine contract first, with an OCI/Docker
/// compatibility surface for existing automation and Bollard-based clients.
/// The built-in runtime is backed by CrateBay-managed containerd/runc/CNI, not
/// by a guest-side Docker service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEngineStatus {
    pub name: String,
    pub kind: String,
    pub api: String,
    pub backend_runtime: String,
    pub oci_runtime: String,
    pub network_stack: String,
    pub docker_compatible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeContainerSummary {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub labels: serde_json::Value,
    pub managed_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeContainerList {
    pub api: String,
    pub count: usize,
    pub items: Vec<NativeContainerSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeImageSummary {
    pub id: String,
    pub repository: String,
    pub tag: String,
    pub tags: Vec<String>,
    pub digests: Vec<String>,
    pub size_bytes: u64,
    pub created: i64,
    pub labels: serde_json::Value,
    pub managed_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeImageList {
    pub api: String,
    pub count: usize,
    pub items: Vec<NativeImageSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeNetworkSummary {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub internal: bool,
    pub attachable: bool,
    pub labels: serde_json::Value,
    pub containers: serde_json::Value,
    pub managed_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeNetworkList {
    pub api: String,
    pub count: usize,
    pub items: Vec<NativeNetworkSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeVolumeSummary {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created_at: String,
    pub scope: String,
    pub labels: serde_json::Value,
    pub options: serde_json::Value,
    pub managed_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NativeVolumeList {
    pub api: String,
    pub count: usize,
    pub items: Vec<NativeVolumeSummary>,
}

pub fn built_in_engine_status() -> RuntimeEngineStatus {
    RuntimeEngineStatus {
        name: "CrateBay Engine".to_string(),
        kind: BUILT_IN_ENGINE_KIND.to_string(),
        api: BUILT_IN_ENGINE_API.to_string(),
        backend_runtime: "containerd".to_string(),
        oci_runtime: "runc".to_string(),
        network_stack: "CNI".to_string(),
        docker_compatible: true,
    }
}

pub fn engine_status_from_contract(contract: &serde_json::Value) -> RuntimeEngineStatus {
    let backend = &contract["backend"];
    let network = &contract["network"];
    let compatibility = &contract["compatibility"];
    RuntimeEngineStatus {
        name: contract["name"]
            .as_str()
            .unwrap_or("CrateBay Engine")
            .to_string(),
        kind: contract["kind"]
            .as_str()
            .unwrap_or(BUILT_IN_ENGINE_KIND)
            .to_string(),
        api: contract["adapter"]["api"]
            .as_str()
            .or_else(|| contract["api"].as_str())
            .unwrap_or(BUILT_IN_ENGINE_API)
            .to_string(),
        backend_runtime: backend["runtime"]
            .as_str()
            .unwrap_or("containerd")
            .to_string(),
        oci_runtime: backend["ociRuntime"].as_str().unwrap_or("runc").to_string(),
        network_stack: network["stack"]
            .as_str()
            .or_else(|| network["driver"].as_str())
            .unwrap_or("CNI")
            .to_string(),
        docker_compatible: compatibility["dockerCompatible"].as_bool().unwrap_or(true),
    }
}

pub fn built_in_engine_contract_ready(contract: &serde_json::Value) -> Result<(), AppError> {
    let api = contract["adapter"]["api"]
        .as_str()
        .or_else(|| contract["api"].as_str());
    let kind = contract["kind"].as_str();

    if api == Some(BUILT_IN_ENGINE_API) && kind == Some(BUILT_IN_ENGINE_KIND) {
        return Ok(());
    }

    Err(AppError::Runtime(format!(
        "CrateBay Engine contract did not match native runtime api={:?} kind={:?}",
        api, kind
    )))
}

pub fn query_built_in_engine_json(
    runtime: &dyn RuntimeManager,
    path: &str,
) -> Result<serde_json::Value, AppError> {
    #[cfg(target_os = "linux")]
    {
        let _ = runtime;
        return common::engine_http_get_json_tcp_host(&linux::linux_engine_host(), path)
            .map_err(AppError::Runtime);
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let socket = runtime.engine_socket_path();
        if socket.exists() {
            return common::engine_http_get_json_unix_socket(&socket, path)
                .map_err(AppError::Runtime);
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = runtime;
        let mut last_error = None;
        for host in windows::windows_engine_host_candidates() {
            match common::engine_http_get_json_tcp_host(&host, path) {
                Ok(payload) => return Ok(payload),
                Err(error) => last_error = Some(format!("{host}: {error}")),
            }
        }
        return Err(AppError::Runtime(last_error.unwrap_or_else(|| {
            "CrateBay Engine contract endpoint is not reachable".to_string()
        })));
    }

    #[allow(unreachable_code)]
    Err(AppError::Runtime(
        "CrateBay Engine contract endpoint is not reachable".to_string(),
    ))
}

pub fn query_built_in_engine_json_post(
    runtime: &dyn RuntimeManager,
    path: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    #[cfg(target_os = "linux")]
    {
        let _ = runtime;
        return common::engine_http_json_tcp_host(
            &linux::linux_engine_host(),
            "POST",
            path,
            payload,
        )
        .map_err(AppError::Runtime);
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let socket = runtime.engine_socket_path();
        if socket.exists() {
            return common::engine_http_json_unix_socket(&socket, "POST", path, payload)
                .map_err(AppError::Runtime);
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = runtime;
        let mut last_error = None;
        for host in windows::windows_engine_host_candidates() {
            match common::engine_http_json_tcp_host(&host, "POST", path, payload) {
                Ok(payload) => return Ok(payload),
                Err(error) => last_error = Some(format!("{host}: {error}")),
            }
        }
        return Err(AppError::Runtime(last_error.unwrap_or_else(|| {
            "CrateBay Engine contract endpoint is not reachable".to_string()
        })));
    }

    #[allow(unreachable_code)]
    Err(AppError::Runtime(
        "CrateBay Engine contract endpoint is not reachable".to_string(),
    ))
}

pub fn query_built_in_engine_raw(
    runtime: &dyn RuntimeManager,
    method: &str,
    path: &str,
    content_type: &str,
    body: &[u8],
) -> Result<Vec<u8>, AppError> {
    #[cfg(target_os = "linux")]
    {
        let _ = runtime;
        return common::engine_http_raw_tcp_host(
            &linux::linux_engine_host(),
            method,
            path,
            content_type,
            body,
        )
        .map_err(AppError::Runtime);
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let socket = runtime.engine_socket_path();
        if socket.exists() {
            return common::engine_http_raw_unix_socket(&socket, method, path, content_type, body)
                .map_err(AppError::Runtime);
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = runtime;
        let mut last_error = None;
        for host in windows::windows_engine_host_candidates() {
            match common::engine_http_raw_tcp_host(&host, method, path, content_type, body) {
                Ok(payload) => return Ok(payload),
                Err(error) => last_error = Some(format!("{host}: {error}")),
            }
        }
        return Err(AppError::Runtime(last_error.unwrap_or_else(|| {
            "CrateBay Engine raw endpoint is not reachable".to_string()
        })));
    }

    #[allow(unreachable_code)]
    Err(AppError::Runtime(
        "CrateBay Engine raw endpoint is not reachable".to_string(),
    ))
}

pub fn query_built_in_engine_raw_file(
    runtime: &dyn RuntimeManager,
    method: &str,
    path: &str,
    content_type: &str,
    body_path: &Path,
) -> Result<Vec<u8>, AppError> {
    #[cfg(target_os = "linux")]
    {
        let _ = runtime;
        return common::engine_http_raw_file_tcp_host(
            &linux::linux_engine_host(),
            method,
            path,
            content_type,
            body_path,
        )
        .map_err(AppError::Runtime);
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let socket = runtime.engine_socket_path();
        if socket.exists() {
            return common::engine_http_raw_file_unix_socket(
                &socket,
                method,
                path,
                content_type,
                body_path,
            )
            .map_err(AppError::Runtime);
        }
    }

    #[cfg(target_os = "windows")]
    {
        let _ = runtime;
        let mut last_error = None;
        for host in windows::windows_engine_host_candidates() {
            match common::engine_http_raw_file_tcp_host(
                &host,
                method,
                path,
                content_type,
                body_path,
            ) {
                Ok(payload) => return Ok(payload),
                Err(error) => last_error = Some(format!("{host}: {error}")),
            }
        }
        return Err(AppError::Runtime(last_error.unwrap_or_else(|| {
            "CrateBay Engine raw endpoint is not reachable".to_string()
        })));
    }

    #[allow(unreachable_code)]
    Err(AppError::Runtime(
        "CrateBay Engine raw endpoint is not reachable".to_string(),
    ))
}

pub fn query_built_in_engine_contract(
    runtime: &dyn RuntimeManager,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json(runtime, "/cratebay/engine")
}

pub fn query_built_in_engine_status(
    runtime: &dyn RuntimeManager,
) -> Result<RuntimeEngineStatus, AppError> {
    query_built_in_engine_contract(runtime).map(|contract| engine_status_from_contract(&contract))
}

pub fn query_built_in_ready_engine_status(
    runtime: &dyn RuntimeManager,
) -> Result<RuntimeEngineStatus, AppError> {
    let contract = query_built_in_engine_contract(runtime)?;
    built_in_engine_contract_ready(&contract)?;
    Ok(engine_status_from_contract(&contract))
}

pub fn query_built_in_native_substrate(
    runtime: &dyn RuntimeManager,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json(runtime, "/cratebay/substrate")
}

pub fn query_built_in_native_storage_gc(
    runtime: &dyn RuntimeManager,
    apply: bool,
    prune_exited_containers: bool,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        "/cratebay/storage/gc",
        &serde_json::json!({
            "apply": apply,
            "pruneExitedContainers": prune_exited_containers,
        }),
    )
}

pub fn query_built_in_native_shim_tasks(
    runtime: &dyn RuntimeManager,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json(runtime, "/cratebay/shim/tasks")
}

pub fn query_built_in_native_shim_reap_task(
    runtime: &dyn RuntimeManager,
    id: &str,
    apply: bool,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/shim/tasks/{}/reap", engine_path_segment(id)),
        &serde_json::json!({ "apply": apply }),
    )
}

pub fn query_built_in_native_containers(
    runtime: &dyn RuntimeManager,
) -> Result<NativeContainerList, AppError> {
    let payload = query_built_in_engine_json(runtime, "/cratebay/containers")?;
    serde_json::from_value(payload)
        .map_err(|error| AppError::Runtime(format!("Invalid CrateBay containers payload: {error}")))
}

pub fn query_built_in_native_images(
    runtime: &dyn RuntimeManager,
) -> Result<NativeImageList, AppError> {
    let payload = query_built_in_engine_json(runtime, "/cratebay/images")?;
    serde_json::from_value(payload)
        .map_err(|error| AppError::Runtime(format!("Invalid CrateBay images payload: {error}")))
}

pub fn query_built_in_native_image_pull(
    runtime: &dyn RuntimeManager,
    image: &str,
    tag: Option<String>,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        "/cratebay/images/pull",
        &serde_json::json!({
            "image": image,
            "tag": tag,
        }),
    )
}

pub fn query_built_in_native_image_inspect(
    runtime: &dyn RuntimeManager,
    id: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json(
        runtime,
        &format!("/cratebay/images/{}/inspect", engine_path_segment(id)),
    )
}

pub fn query_built_in_native_image_remove(
    runtime: &dyn RuntimeManager,
    id: &str,
    force: bool,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/images/{}/remove", engine_path_segment(id)),
        &serde_json::json!({ "force": force }),
    )
}

pub fn query_built_in_native_image_tag(
    runtime: &dyn RuntimeManager,
    source: &str,
    target: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/images/{}/tag", engine_path_segment(source)),
        &serde_json::json!({ "target": target }),
    )
}

pub fn query_built_in_native_image_pack_container(
    runtime: &dyn RuntimeManager,
    container: &str,
    image: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        "/cratebay/images/pack-container",
        &serde_json::json!({
            "container": container,
            "image": image,
        }),
    )
}

pub fn query_built_in_native_image_export(
    runtime: &dyn RuntimeManager,
    images: &[String],
) -> Result<Vec<u8>, AppError> {
    if images.is_empty() {
        return Err(AppError::Runtime(
            "At least one image is required for CrateBay native image export".to_string(),
        ));
    }
    let query = images
        .iter()
        .map(|image| format!("names={}", engine_path_segment(image)))
        .collect::<Vec<_>>()
        .join("&");
    query_built_in_engine_raw(
        runtime,
        "GET",
        &format!("/cratebay/images/export?{query}"),
        "application/octet-stream",
        &[],
    )
}

pub fn query_built_in_native_image_import(
    runtime: &dyn RuntimeManager,
    archive: &[u8],
) -> Result<serde_json::Value, AppError> {
    let body = query_built_in_engine_raw(
        runtime,
        "POST",
        "/cratebay/images/import",
        "application/x-tar",
        archive,
    )?;
    serde_json::from_slice(&body).map_err(|error| {
        AppError::Runtime(format!("Invalid CrateBay image import payload: {error}"))
    })
}

pub fn query_built_in_native_image_import_file(
    runtime: &dyn RuntimeManager,
    archive_path: &Path,
) -> Result<serde_json::Value, AppError> {
    let body = query_built_in_engine_raw_file(
        runtime,
        "POST",
        "/cratebay/images/import",
        "application/x-tar",
        archive_path,
    )?;
    serde_json::from_slice(&body).map_err(|error| {
        AppError::Runtime(format!("Invalid CrateBay image import payload: {error}"))
    })
}

pub fn query_built_in_native_networks(
    runtime: &dyn RuntimeManager,
) -> Result<NativeNetworkList, AppError> {
    let payload = query_built_in_engine_json(runtime, "/cratebay/networks")?;
    serde_json::from_value(payload)
        .map_err(|error| AppError::Runtime(format!("Invalid CrateBay networks payload: {error}")))
}

pub fn query_built_in_native_network_inspect(
    runtime: &dyn RuntimeManager,
    id: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json(
        runtime,
        &format!("/cratebay/networks/{}", engine_path_segment(id)),
    )
}

pub fn query_built_in_native_network_create(
    runtime: &dyn RuntimeManager,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(runtime, "/cratebay/networks", payload)
}

pub fn query_built_in_native_network_remove(
    runtime: &dyn RuntimeManager,
    id: &str,
    force: bool,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/networks/{}/remove", engine_path_segment(id)),
        &serde_json::json!({ "force": force }),
    )
}

pub fn query_built_in_native_volumes(
    runtime: &dyn RuntimeManager,
) -> Result<NativeVolumeList, AppError> {
    let payload = query_built_in_engine_json(runtime, "/cratebay/volumes")?;
    serde_json::from_value(payload)
        .map_err(|error| AppError::Runtime(format!("Invalid CrateBay volumes payload: {error}")))
}

pub fn query_built_in_native_volume_inspect(
    runtime: &dyn RuntimeManager,
    name: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json(
        runtime,
        &format!("/cratebay/volumes/{}", engine_path_segment(name)),
    )
}

pub fn query_built_in_native_volume_create(
    runtime: &dyn RuntimeManager,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(runtime, "/cratebay/volumes", payload)
}

pub fn query_built_in_native_volume_remove(
    runtime: &dyn RuntimeManager,
    name: &str,
    force: bool,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/volumes/{}/remove", engine_path_segment(name)),
        &serde_json::json!({ "force": force }),
    )
}

pub fn query_built_in_native_pods(
    runtime: &dyn RuntimeManager,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json(runtime, "/cratebay/pods")
}

pub fn query_built_in_native_pod_inspect(
    runtime: &dyn RuntimeManager,
    name: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json(
        runtime,
        &format!("/cratebay/pods/{}", engine_path_segment(name)),
    )
}

pub fn query_built_in_native_pod_create(
    runtime: &dyn RuntimeManager,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(runtime, "/cratebay/pods", payload)
}

pub fn query_built_in_native_pod_remove(
    runtime: &dyn RuntimeManager,
    name: &str,
    force: bool,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/pods/{}/remove", engine_path_segment(name)),
        &serde_json::json!({ "force": force }),
    )
}

pub fn query_built_in_native_pod_attach(
    runtime: &dyn RuntimeManager,
    name: &str,
    container: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/pods/{}/attach", engine_path_segment(name)),
        &serde_json::json!({ "container": container }),
    )
}

pub fn query_built_in_native_pod_detach(
    runtime: &dyn RuntimeManager,
    name: &str,
    container: &str,
    force: bool,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/pods/{}/detach", engine_path_segment(name)),
        &serde_json::json!({
            "container": container,
            "force": force,
        }),
    )
}

pub fn query_built_in_native_container_create(
    runtime: &dyn RuntimeManager,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(runtime, "/cratebay/containers", payload)
}

pub fn query_built_in_native_container_start(
    runtime: &dyn RuntimeManager,
    id: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/containers/{}/start", engine_path_segment(id)),
        &serde_json::json!({}),
    )
}

pub fn query_built_in_native_container_stop(
    runtime: &dyn RuntimeManager,
    id: &str,
    timeout: Option<u64>,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/containers/{}/stop", engine_path_segment(id)),
        &serde_json::json!({ "timeout": timeout }),
    )
}

pub fn query_built_in_native_container_remove(
    runtime: &dyn RuntimeManager,
    id: &str,
    force: bool,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/containers/{}/remove", engine_path_segment(id)),
        &serde_json::json!({ "force": force }),
    )
}

pub fn query_built_in_native_container_inspect(
    runtime: &dyn RuntimeManager,
    id: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json(
        runtime,
        &format!("/cratebay/containers/{}/inspect", engine_path_segment(id)),
    )
}

pub fn query_built_in_native_container_logs(
    runtime: &dyn RuntimeManager,
    id: &str,
    tail: Option<u64>,
    timestamps: bool,
) -> Result<serde_json::Value, AppError> {
    let mut path = format!(
        "/cratebay/containers/{}/logs?timestamps={}",
        engine_path_segment(id),
        if timestamps { "true" } else { "false" }
    );
    if let Some(tail) = tail {
        path.push_str(&format!("&tail={tail}"));
    }
    query_built_in_engine_json(runtime, &path)
}

pub fn query_built_in_native_container_stats(
    runtime: &dyn RuntimeManager,
    id: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json(
        runtime,
        &format!("/cratebay/containers/{}/stats", engine_path_segment(id)),
    )
}

pub fn query_built_in_native_container_wait(
    runtime: &dyn RuntimeManager,
    id: &str,
    timeout: Option<u64>,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/containers/{}/wait", engine_path_segment(id)),
        &serde_json::json!({ "timeout": timeout }),
    )
}

pub fn query_built_in_native_container_exec(
    runtime: &dyn RuntimeManager,
    id: &str,
    command: Vec<String>,
    working_dir: Option<String>,
    timeout: Option<u64>,
    max_output_bytes: Option<u64>,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!("/cratebay/containers/{}/exec", engine_path_segment(id)),
        &serde_json::json!({
            "command": command,
            "workingDir": working_dir,
            "timeout": timeout,
            "maxOutputBytes": max_output_bytes,
        }),
    )
}

pub fn query_built_in_native_container_terminal_open(
    runtime: &dyn RuntimeManager,
    id: &str,
    session_id: &str,
    cols: Option<u16>,
    rows: Option<u16>,
    command: Option<Vec<String>>,
    working_dir: Option<String>,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!(
            "/cratebay/containers/{}/terminal/open",
            engine_path_segment(id)
        ),
        &serde_json::json!({
            "sessionId": session_id,
            "cols": cols,
            "rows": rows,
            "command": command,
            "workingDir": working_dir,
        }),
    )
}

pub fn query_built_in_native_container_terminal_input(
    runtime: &dyn RuntimeManager,
    id: &str,
    session_id: &str,
    data: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!(
            "/cratebay/containers/{}/terminal/input",
            engine_path_segment(id)
        ),
        &serde_json::json!({
            "sessionId": session_id,
            "data": data,
        }),
    )
}

pub fn query_built_in_native_container_terminal_read(
    runtime: &dyn RuntimeManager,
    id: &str,
    session_id: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!(
            "/cratebay/containers/{}/terminal/read",
            engine_path_segment(id)
        ),
        &serde_json::json!({
            "sessionId": session_id,
        }),
    )
}

pub fn query_built_in_native_container_terminal_resize(
    runtime: &dyn RuntimeManager,
    id: &str,
    session_id: &str,
    cols: u16,
    rows: u16,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!(
            "/cratebay/containers/{}/terminal/resize",
            engine_path_segment(id)
        ),
        &serde_json::json!({
            "sessionId": session_id,
            "cols": cols,
            "rows": rows,
        }),
    )
}

pub fn query_built_in_native_container_terminal_close(
    runtime: &dyn RuntimeManager,
    id: &str,
    session_id: &str,
) -> Result<serde_json::Value, AppError> {
    query_built_in_engine_json_post(
        runtime,
        &format!(
            "/cratebay/containers/{}/terminal/close",
            engine_path_segment(id)
        ),
        &serde_json::json!({
            "sessionId": session_id,
        }),
    )
}

fn engine_path_segment(segment: &str) -> String {
    segment
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Runtime Configuration (§7.2)
// ---------------------------------------------------------------------------

/// Configuration for the built-in container runtime VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Number of CPU cores allocated to the VM.
    pub cpu_cores: u32,
    /// Memory allocated to the VM in MB.
    pub memory_mb: u64,
    /// Maximum disk size in GB (thin provisioned).
    pub disk_gb: u32,
    /// Whether to auto-start runtime on app launch.
    pub auto_start: bool,
    /// Shared directories (host → guest).
    pub shared_dirs: Vec<SharedDir>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            cpu_cores: 2,
            memory_mb: 2048,
            disk_gb: 35,
            auto_start: true,
            shared_dirs: default_shared_dirs(),
        }
    }
}

fn default_shared_dirs() -> Vec<SharedDir> {
    #[cfg(target_os = "macos")]
    {
        if std::path::Path::new("/Users").is_dir() {
            return vec![SharedDir {
                host_path: "/Users".to_string(),
                tag: "Users".to_string(),
            }];
        }
    }
    Vec::new()
}

// ---------------------------------------------------------------------------
// Shared Directory (§5)
// ---------------------------------------------------------------------------

/// A host directory shared with the VM via VirtioFS / 9P.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedDir {
    /// Absolute path on the host filesystem.
    pub host_path: String,
    /// Mount tag used inside the VM.
    pub tag: String,
}

// ---------------------------------------------------------------------------
// Port Forwarding (§6)
// ---------------------------------------------------------------------------

/// A port forwarding rule between host and container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortForward {
    /// Port on the host.
    pub host_port: u16,
    /// Port inside the container.
    pub container_port: u16,
    /// Transport protocol.
    pub protocol: Protocol,
}

/// Network protocol for port forwarding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
    Udp,
}

// ---------------------------------------------------------------------------
// RuntimeManager Trait (§3.2)
// ---------------------------------------------------------------------------

/// Platform-agnostic runtime manager trait.
///
/// Each platform (macOS, Linux, Windows) provides a concrete implementation.
/// Consumers use `Box<dyn RuntimeManager>` or `Arc<dyn RuntimeManager>` for
/// dynamic dispatch.
///
/// All async methods use `async-trait` to maintain object-safety.
#[async_trait]
pub trait RuntimeManager: Send + Sync {
    /// Query the current lifecycle state (fast local query, <100ms).
    async fn get_state(&self) -> Result<RuntimeState, AppError>;

    /// Download and prepare the VM image (first-run provisioning).
    ///
    /// The `on_progress` callback is invoked with progress updates during
    /// downloading, extraction, and configuration stages.
    async fn provision(
        &self,
        on_progress: Box<dyn Fn(ProvisionProgress) + Send>,
    ) -> Result<(), AppError>;

    /// Start the runtime VM.
    async fn start(&self) -> Result<(), AppError>;

    /// Stop the runtime VM gracefully.
    async fn stop(&self) -> Result<(), AppError>;

    /// Check if the runtime is healthy and the CrateBay Engine API is responsive.
    async fn health_check(&self) -> Result<HealthStatus, AppError>;

    /// Get the host-exposed CrateBay Engine socket path.
    fn engine_socket_path(&self) -> PathBuf;

    /// Compatibility alias for older call sites that still use Docker-shaped naming.
    fn docker_socket_path(&self) -> PathBuf {
        self.engine_socket_path()
    }

    /// Get current resource usage of the runtime VM.
    async fn resource_usage(&self) -> Result<ResourceUsage, AppError>;
}

// ---------------------------------------------------------------------------
// Factory Function
// ---------------------------------------------------------------------------

/// Create the platform-appropriate runtime manager.
///
/// Returns a boxed trait object that dispatches to:
/// - [`macos::MacOSRuntime`] on macOS
/// - [`linux::LinuxRuntime`] on Linux
/// - [`windows::WindowsRuntime`] on Windows
pub fn create_runtime_manager() -> Box<dyn RuntimeManager> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOSRuntime::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxRuntime::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsRuntime::new())
    }
}

// ---------------------------------------------------------------------------
// Health Monitor (§9.2)
// ---------------------------------------------------------------------------

/// Start a periodic health monitor that checks runtime health every 30 seconds.
///
/// Uses a callback pattern because `cratebay-core` does not depend on Tauri.
/// The GUI layer wraps this callback to emit Tauri events.
///
/// Spawns a dedicated background thread with its own tokio runtime so it can
/// be called from any context (no pre-existing tokio reactor required).
///
/// # Arguments
///
/// * `runtime` — Shared runtime manager instance.
/// * `on_health` — Callback invoked with each health check result.
pub fn start_health_monitor(
    runtime: Arc<dyn RuntimeManager>,
    on_health: impl Fn(HealthStatus) + Send + 'static,
) {
    let spawn_result = std::thread::Builder::new()
        .name("health-monitor".to_string())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to create health monitor runtime: {}", e);
                    return;
                }
            };
            rt.block_on(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    match runtime.health_check().await {
                        Ok(status) => {
                            on_health(status);
                        }
                        Err(e) => {
                            tracing::warn!("Health check failed: {}", e);
                            on_health(HealthStatus {
                                runtime_state: RuntimeState::Error(e.to_string()),
                                engine_responsive: false,
                                compatibility_responsive: false,
                                compatibility_version: None,
                                docker_responsive: false,
                                docker_version: None,
                                uptime_seconds: None,
                                last_check: chrono::Utc::now().to_rfc3339(),
                                engine_source: Some("builtin".to_string()),
                                docker_source: Some("builtin".to_string()),
                                engine: built_in_engine_status(),
                            });
                        }
                    }
                }
            });
        });

    if let Err(e) = spawn_result {
        tracing::error!("Failed to spawn health monitor thread: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_config_default_values() {
        let config = RuntimeConfig::default();
        assert_eq!(config.cpu_cores, 2);
        assert_eq!(config.memory_mb, 2048);
        assert_eq!(config.disk_gb, 35);
        assert!(config.auto_start);
        #[cfg(target_os = "macos")]
        {
            assert_eq!(config.shared_dirs[0].host_path, "/Users");
            assert_eq!(config.shared_dirs[0].tag, "Users");
        }
        #[cfg(not(target_os = "macos"))]
        assert!(config.shared_dirs.is_empty());
    }

    #[test]
    fn runtime_state_serializes() {
        let state = RuntimeState::Ready;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"Ready\"");

        let error = RuntimeState::Error("test error".to_string());
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("test error"));
    }

    #[test]
    fn provision_progress_default() {
        let progress = ProvisionProgress::default();
        assert_eq!(progress.percent, 0.0);
        assert_eq!(progress.bytes_downloaded, 0);
        assert_eq!(progress.bytes_total, 0);
        assert!(progress.stage.is_empty());
    }

    #[test]
    fn protocol_serializes() {
        let tcp = Protocol::Tcp;
        let json = serde_json::to_string(&tcp).unwrap();
        assert_eq!(json, "\"tcp\"");

        let udp = Protocol::Udp;
        let json = serde_json::to_string(&udp).unwrap();
        assert_eq!(json, "\"udp\"");
    }

    #[test]
    fn runtime_state_all_variants_serialize_deserialize() {
        let variants = vec![
            (RuntimeState::None, "\"None\""),
            (RuntimeState::Provisioned, "\"Provisioned\""),
            (RuntimeState::Starting, "\"Starting\""),
            (RuntimeState::Ready, "\"Ready\""),
            (RuntimeState::Stopping, "\"Stopping\""),
            (RuntimeState::Stopped, "\"Stopped\""),
        ];
        for (state, expected_json) in variants {
            let json = serde_json::to_string(&state).unwrap();
            assert_eq!(json, expected_json, "serialize {:?}", state);
            let deserialized: RuntimeState = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, state, "deserialize {:?}", state);
        }

        // Error variant with payload
        let error = RuntimeState::Error("something went wrong".to_string());
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("something went wrong"));
        let deserialized: RuntimeState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, error);
    }

    #[test]
    fn provision_progress_serializes() {
        let progress = ProvisionProgress {
            stage: "downloading".into(),
            percent: 42.5,
            bytes_downloaded: 1024,
            bytes_total: 2048,
            message: "Downloading image...".into(),
        };
        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"downloading\""));
        assert!(json.contains("42.5"));
        assert!(json.contains("1024"));
        assert!(json.contains("2048"));

        let deserialized: ProvisionProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.stage, "downloading");
        assert_eq!(deserialized.percent, 42.5);
        assert_eq!(deserialized.bytes_downloaded, 1024);
        assert_eq!(deserialized.bytes_total, 2048);
        assert_eq!(deserialized.message, "Downloading image...");
    }

    #[test]
    fn health_status_serializes() {
        let status = HealthStatus {
            runtime_state: RuntimeState::Ready,
            engine_responsive: true,
            compatibility_responsive: true,
            compatibility_version: Some("24.0.7".into()),
            docker_responsive: true,
            docker_version: Some("24.0.7".into()),
            uptime_seconds: Some(3600),
            last_check: "2026-03-20T00:00:00Z".into(),
            engine_source: Some("builtin".to_string()),
            docker_source: Some("builtin".to_string()),
            engine: built_in_engine_status(),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"Ready\""));
        assert!(json.contains("true"));
        assert!(json.contains("24.0.7"));
        assert!(json.contains("3600"));

        let deserialized: HealthStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.runtime_state, RuntimeState::Ready);
        assert!(deserialized.engine_responsive);
        assert!(deserialized.compatibility_responsive);
        assert_eq!(deserialized.compatibility_version, Some("24.0.7".into()));
        assert!(deserialized.docker_responsive);
        assert_eq!(deserialized.docker_version, Some("24.0.7".into()));
        assert_eq!(deserialized.uptime_seconds, Some(3600));
        assert_eq!(deserialized.engine_source, Some("builtin".to_string()));
        assert_eq!(deserialized.engine.kind, "cratebay-containerd");
    }

    #[test]
    fn health_status_serializes_with_none_fields() {
        let status = HealthStatus {
            runtime_state: RuntimeState::None,
            engine_responsive: false,
            compatibility_responsive: false,
            compatibility_version: None,
            docker_responsive: false,
            docker_version: None,
            uptime_seconds: None,
            last_check: "2026-03-20T00:00:00Z".into(),
            engine_source: Some("builtin".to_string()),
            docker_source: Some("builtin".to_string()),
            engine: built_in_engine_status(),
        };
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: HealthStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.runtime_state, RuntimeState::None);
        assert!(!deserialized.engine_responsive);
        assert!(!deserialized.compatibility_responsive);
        assert!(deserialized.compatibility_version.is_none());
        assert!(!deserialized.docker_responsive);
        assert!(deserialized.docker_version.is_none());
        assert!(deserialized.uptime_seconds.is_none());
    }

    #[test]
    fn engine_status_maps_from_cratebay_contract() {
        let contract = serde_json::json!({
            "name": "CrateBay Engine",
            "kind": "cratebay-containerd",
            "backend": {
                "runtime": "containerd",
                "ociRuntime": "runc"
            },
            "network": {
                "stack": "CNI",
                "driver": "cratebay-cni"
            },
            "adapter": { "api": "cratebay.engine.v1" },
            "compatibility": {
                "dockerCompatible": true,
                "dockerApiVersion": "1.44"
            }
        });
        let status = engine_status_from_contract(&contract);
        assert_eq!(status.name, "CrateBay Engine");
        assert_eq!(status.kind, "cratebay-containerd");
        assert_eq!(status.api, "cratebay.engine.v1");
        assert_eq!(status.backend_runtime, "containerd");
        assert_eq!(status.oci_runtime, "runc");
        assert_eq!(status.network_stack, "CNI");
        assert!(status.docker_compatible);
    }

    #[test]
    fn built_in_engine_contract_ready_requires_native_api_and_kind() {
        let valid = serde_json::json!({
            "kind": "cratebay-containerd",
            "adapter": { "api": "cratebay.engine.v1" },
        });
        assert!(built_in_engine_contract_ready(&valid).is_ok());

        let compatibility_only = serde_json::json!({
            "ApiVersion": "1.44",
            "Version": "24.0.7",
        });
        assert!(built_in_engine_contract_ready(&compatibility_only).is_err());

        let wrong_kind = serde_json::json!({
            "kind": "docker-engine",
            "adapter": { "api": "cratebay.engine.v1" },
        });
        assert!(built_in_engine_contract_ready(&wrong_kind).is_err());
    }

    #[test]
    fn platform_readiness_source_guards_use_native_engine_contract() {
        let linux = include_str!("linux.rs");
        assert!(linux.contains("query_built_in_ready_engine_status(self)"));
        assert!(linux.contains("wait_for_native_engine_contract_tcp(&host_wait"));
        assert!(!linux.contains("let runtime_state = if docker_responsive"));
        assert!(!linux.contains("let docker_already_up"));

        let macos = include_str!("macos.rs");
        assert!(macos.contains("Self::native_engine_contract_ready(&socket_path)"));
        assert!(macos.contains("query_built_in_ready_engine_status(self)"));
        assert!(!macos.contains("let runtime_state = if docker_responsive"));
        assert!(!macos.contains("Self::compatibility_api_available(&socket_path).await {\n"));

        let windows = include_str!("windows.rs");
        assert!(windows.contains("self.native_engine_contract_ready().await"));
        assert!(windows.contains("native_engine_contract_ready_on_host"));
        assert!(!windows.contains("let runtime_state = if docker_responsive"));

        let cli_runtime = include_str!("../../../cratebay-cli/src/commands/runtime.rs");
        assert!(!cli_runtime.contains(
            "if health.engine_responsive || health.compatibility_responsive || health.docker_responsive"
        ));
        assert!(cli_runtime.contains("query_built_in_ready_engine_status(runtime).is_ok()"));

        let gui_system = include_str!("../../../cratebay-gui/src-tauri/src/commands/system.rs");
        assert!(!gui_system.contains("health.runtime_state = RuntimeState::Ready"));
        assert!(gui_system.contains("query_built_in_ready_engine_status(state.runtime.as_ref())"));

        let gui_state = include_str!("../../../cratebay-gui/src-tauri/src/state.rs");
        assert!(gui_state.contains("query_built_in_ready_engine_status(self.runtime.as_ref())"));
        assert!(
            !gui_state.contains("query_built_in_engine_contract(self.runtime.as_ref()).is_ok()")
        );
    }

    #[test]
    fn native_container_list_deserializes_cratebay_schema() {
        let payload = serde_json::json!({
            "api": "cratebay.containers.v1",
            "count": 1,
            "items": [
                {
                    "id": "abc123",
                    "name": "sandbox-demo",
                    "image": "cratebay-ubuntu-base:v1",
                    "state": "running",
                    "status": "Up 10 seconds",
                    "labels": { "com.cratebay.managed": "true" },
                    "managedBy": "cratebay"
                }
            ]
        });
        let list: NativeContainerList = serde_json::from_value(payload).unwrap();
        assert_eq!(list.api, "cratebay.containers.v1");
        assert_eq!(list.count, 1);
        assert_eq!(list.items[0].name, "sandbox-demo");
        assert_eq!(list.items[0].managed_by, "cratebay");
    }

    #[test]
    fn native_image_list_deserializes_cratebay_schema() {
        let payload = serde_json::json!({
            "api": "cratebay.images.v1",
            "count": 1,
            "items": [
                {
                    "id": "sha256:abc123",
                    "repository": "cratebay-ubuntu-base",
                    "tag": "v1",
                    "tags": ["cratebay-ubuntu-base:v1"],
                    "digests": ["cratebay-ubuntu-base@sha256:def456"],
                    "sizeBytes": 123456,
                    "created": 1700000000,
                    "labels": { "com.cratebay.bundle": "true" },
                    "managedBy": "cratebay"
                }
            ]
        });
        let list: NativeImageList = serde_json::from_value(payload).unwrap();
        assert_eq!(list.api, "cratebay.images.v1");
        assert_eq!(list.count, 1);
        assert_eq!(list.items[0].repository, "cratebay-ubuntu-base");
        assert_eq!(list.items[0].tag, "v1");
        assert_eq!(list.items[0].size_bytes, 123456);
    }

    #[test]
    fn native_network_list_deserializes_cratebay_schema() {
        let payload = serde_json::json!({
            "api": "cratebay.networks.v1",
            "count": 1,
            "items": [
                {
                    "id": "net123",
                    "name": "pod-demo",
                    "driver": "bridge",
                    "scope": "local",
                    "internal": false,
                    "attachable": true,
                    "labels": { "com.cratebay.pod": "true" },
                    "containers": { "abc123": { "Name": "sandbox-demo" } },
                    "managedBy": "cratebay"
                }
            ]
        });
        let list: NativeNetworkList = serde_json::from_value(payload).unwrap();
        assert_eq!(list.api, "cratebay.networks.v1");
        assert_eq!(list.count, 1);
        assert_eq!(list.items[0].name, "pod-demo");
        assert_eq!(list.items[0].driver, "bridge");
        assert!(list.items[0].attachable);
    }

    #[test]
    fn native_volume_list_deserializes_cratebay_schema() {
        let payload = serde_json::json!({
            "api": "cratebay.volumes.v1",
            "count": 1,
            "items": [
                {
                    "name": "workspace-cache",
                    "driver": "local",
                    "mountpoint": "/var/lib/cratebay-engine/volumes/workspace-cache/_data",
                    "createdAt": "2026-06-03T00:00:00Z",
                    "scope": "local",
                    "labels": { "com.cratebay.volume": "true" },
                    "options": {},
                    "managedBy": "cratebay"
                }
            ]
        });
        let list: NativeVolumeList = serde_json::from_value(payload).unwrap();
        assert_eq!(list.api, "cratebay.volumes.v1");
        assert_eq!(list.count, 1);
        assert_eq!(list.items[0].name, "workspace-cache");
        assert_eq!(list.items[0].driver, "local");
        assert_eq!(list.items[0].scope, "local");
    }

    #[test]
    fn engine_path_segment_percent_encodes_native_resource_ids() {
        assert_eq!(engine_path_segment("sandbox demo/1"), "sandbox%20demo%2F1");
        assert_eq!(engine_path_segment("abc-123_~"), "abc-123_~");
    }

    #[test]
    fn port_forward_serializes() {
        let pf = PortForward {
            host_port: 8080,
            container_port: 80,
            protocol: Protocol::Tcp,
        };
        let json = serde_json::to_string(&pf).unwrap();
        assert!(json.contains("8080"));
        assert!(json.contains("80"));
        assert!(json.contains("\"tcp\""));

        let deserialized: PortForward = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.host_port, 8080);
        assert_eq!(deserialized.container_port, 80);
        assert_eq!(deserialized.protocol, Protocol::Tcp);
    }

    #[test]
    fn shared_dir_serializes() {
        let sd = SharedDir {
            host_path: "/Users/test/project".into(),
            tag: "workspace".into(),
        };
        let json = serde_json::to_string(&sd).unwrap();
        assert!(json.contains("/Users/test/project"));
        assert!(json.contains("workspace"));

        let deserialized: SharedDir = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.host_path, "/Users/test/project");
        assert_eq!(deserialized.tag, "workspace");
    }

    #[test]
    fn protocol_deserializes() {
        let tcp: Protocol = serde_json::from_str("\"tcp\"").unwrap();
        assert_eq!(tcp, Protocol::Tcp);

        let udp: Protocol = serde_json::from_str("\"udp\"").unwrap();
        assert_eq!(udp, Protocol::Udp);
    }

    #[test]
    fn runtime_config_serializes() {
        let config = RuntimeConfig {
            cpu_cores: 4,
            memory_mb: 4096,
            disk_gb: 50,
            auto_start: false,
            shared_dirs: vec![SharedDir {
                host_path: "/home/user/code".into(),
                tag: "code".into(),
            }],
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RuntimeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.cpu_cores, 4);
        assert_eq!(deserialized.memory_mb, 4096);
        assert_eq!(deserialized.disk_gb, 50);
        assert!(!deserialized.auto_start);
        assert_eq!(deserialized.shared_dirs.len(), 1);
        assert_eq!(deserialized.shared_dirs[0].tag, "code");
    }

    #[test]
    fn create_runtime_manager_returns_valid_manager() {
        let manager = create_runtime_manager();
        // Verify the manager has the expected engine socket path pattern.
        let socket_path = manager.engine_socket_path();
        assert!(
            socket_path.to_string_lossy().contains("engine.sock")
                || socket_path.to_string_lossy().contains("cratebay-docker"),
            "Engine socket path should contain 'engine.sock' or Windows pipe fallback: {:?}",
            socket_path
        );
    }

    // Explicit compatibility host parsing/connection is implemented in
    // `crate::docker` so it can support tcp/http/npipe endpoints across
    // platforms without making external engines part of the default product path.
}

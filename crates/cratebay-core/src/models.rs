use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::runtime::RuntimeEngineStatus;

// ---------------------------------------------------------------------------
// Container models
// ---------------------------------------------------------------------------

/// Container information returned from CrateBay Engine or its compatibility API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerInfo {
    pub id: String,
    pub short_id: String,
    pub name: String,
    pub image: String,
    pub status: ContainerStatus,
    pub state: String,
    pub created_at: String,
    pub ports: Vec<PortMapping>,
    pub labels: HashMap<String, String>,
    pub cpu_cores: Option<u32>,
    pub memory_mb: Option<u64>,
}

/// Port mapping between host and container.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String,
}

/// Volume mount configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeMount {
    pub host_path: String,
    pub container_path: String,
    pub read_only: Option<bool>,
}

/// Container filter criteria for listing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContainerListFilters {
    pub status: Option<Vec<ContainerStatus>>,
    pub name: Option<String>,
    pub image: Option<String>,
    pub label: Option<HashMap<String, String>>,
}

/// Request to create a new container.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerCreateRequest {
    pub name: String,
    pub image: String,
    pub entrypoint: Option<String>,
    pub command: Option<String>,
    pub env: Option<Vec<String>>,
    pub ports: Option<Vec<PortMapping>>,
    pub volumes: Option<Vec<VolumeMount>>,
    pub cpu_cores: Option<u32>,
    pub memory_mb: Option<u64>,
    pub working_dir: Option<String>,
    pub pod: Option<String>,
    pub network: Option<String>,
    pub user: Option<String>,
    pub read_only_rootfs: Option<bool>,
    pub auto_start: Option<bool>,
    pub labels: Option<HashMap<String, String>>,
    pub template_id: Option<String>,
    pub registry_mirrors: Option<Vec<String>>,
}

/// Request to run a one-shot container and collect its output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerRunRequest {
    pub name: Option<String>,
    pub image: String,
    pub entrypoint: Option<String>,
    pub command: Vec<String>,
    pub env: Option<Vec<String>>,
    pub ports: Option<Vec<PortMapping>>,
    pub volumes: Option<Vec<VolumeMount>>,
    pub cpu_cores: Option<u32>,
    pub memory_mb: Option<u64>,
    pub working_dir: Option<String>,
    pub pod: Option<String>,
    pub network: Option<String>,
    pub user: Option<String>,
    pub read_only_rootfs: Option<bool>,
    pub pull: bool,
    pub remove: bool,
    pub timeout_secs: Option<u64>,
    pub max_output_bytes: Option<u64>,
    pub registry_mirrors: Option<Vec<String>>,
}

/// Result of a one-shot container run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerRunResult {
    pub id: String,
    pub name: String,
    pub image: String,
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timed_out: bool,
}

/// Result of a container exec command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecResult {
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timed_out: bool,
}

/// Exec streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExecStreamChunk {
    Stdout { data: String },
    Stderr { data: String },
    Done { exit_code: i64 },
    Error { message: String },
}

/// Container detail (extended info from inspect).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerDetail {
    pub info: ContainerInfo,
    pub network_settings: serde_json::Value,
    pub mounts: Vec<serde_json::Value>,
    pub state: ContainerState,
}

/// Container state detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerState {
    pub status: String,
    pub running: bool,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub exit_code: Option<i64>,
    pub error: Option<String>,
    pub pid: Option<u64>,
}

/// Log retrieval options.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LogOptions {
    pub tail: Option<u32>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub timestamps: Option<bool>,
}

/// Log entry from container.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub stream: String,
    pub message: String,
    pub timestamp: Option<String>,
}

/// Real-time container resource usage snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStats {
    pub id: String,
    pub name: String,
    pub read_at: String,
    pub cpu_percent: f64,
    pub cpu_cores_used: f64,
    pub memory_used_mb: f64,
    pub memory_limit_mb: f64,
    pub memory_percent: f64,
}

/// OCI image information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub size: i64,
    pub created: i64,
}

/// Compatibility alias for older callers.
pub type DockerImageInfo = ImageInfo;

/// Local image info for the Images page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalImageInfo {
    pub id: String,
    pub repo_tags: Vec<String>,
    /// Compatibility field used by existing container dropdown UI.
    pub size: i64,
    pub size_bytes: u64,
    pub size_human: String,
    pub created: i64,
}

/// Registry image search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSearchResult {
    pub source: String,
    pub reference: String,
    pub description: String,
    pub stars: Option<u64>,
    pub pulls: Option<u64>,
    pub official: bool,
}

/// Image inspection info.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInspectInfo {
    pub id: String,
    pub repo_tags: Vec<String>,
    pub size_bytes: u64,
    pub created: String,
    pub architecture: String,
    pub os: String,
    pub docker_version: String,
    pub layers: u32,
}

/// Pod container membership information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodContainerInfo {
    pub id: String,
    pub name: String,
    pub ipv4_address: Option<String>,
    pub ipv6_address: Option<String>,
}

/// Pod / group information backed by a CrateBay Engine network.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub created_at: Option<String>,
    pub labels: HashMap<String, String>,
    pub containers: Vec<PodContainerInfo>,
}

/// Container lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ContainerStatus {
    Running,
    Stopped,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
    Created,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_result_serializes_timeout_state_with_cli_field_names() {
        let value = ExecResult {
            exit_code: 124,
            stdout: String::new(),
            stderr: "Execution timed out after 1s".to_string(),
            stdout_truncated: false,
            stderr_truncated: false,
            timed_out: true,
        };

        let json = serde_json::to_value(value).expect("exec result should serialize");

        assert_eq!(json["exitCode"], 124);
        assert_eq!(json["stdoutTruncated"], false);
        assert_eq!(json["stderrTruncated"], false);
        assert_eq!(json["timedOut"], true);
    }
}

// ---------------------------------------------------------------------------
// Audit models
// ---------------------------------------------------------------------------

/// Auditable actions for the audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditAction {
    ContainerCreate,
    ContainerStart,
    ContainerStop,
    ContainerDelete,
    ContainerExec,
    SettingsUpdate,
}

impl AuditAction {
    /// Convert to a string representation for database storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditAction::ContainerCreate => "container.create",
            AuditAction::ContainerStart => "container.start",
            AuditAction::ContainerStop => "container.stop",
            AuditAction::ContainerDelete => "container.delete",
            AuditAction::ContainerExec => "container.exec",
            AuditAction::SettingsUpdate => "settings.update",
        }
    }
}

/// CrateBay Engine compatibility endpoint status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineEndpointStatus {
    pub connected: bool,
    pub version: Option<String>,
    pub api_version: Option<String>,
    pub os: Option<String>,
    pub arch: Option<String>,
    pub engine_source: String,
    /// Compatibility alias for older frontend and automation clients.
    pub source: String,
    pub socket_path: Option<String>,
}

/// Compatibility alias for older frontend and automation clients.
///
/// Field names stay Docker-shaped in some payloads for compatibility. They
/// describe CrateBay Engine's compatibility API, not the runtime backend.
pub type DockerStatus = EngineEndpointStatus;

// ---------------------------------------------------------------------------
// Runtime / System models
// ---------------------------------------------------------------------------

/// System-level information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    /// Operating system: "macos", "linux", "windows".
    pub os: String,
    /// OS version string.
    pub os_version: String,
    /// CPU architecture: "x86_64", "aarch64".
    pub arch: String,
    /// CrateBay application version.
    pub app_version: String,
    /// Application data directory (~/.cratebay/).
    pub data_dir: String,
    /// Database file path (~/.cratebay/cratebay.db).
    pub db_path: String,
    /// Database file size in bytes.
    pub db_size_bytes: u64,
    /// Log file path.
    pub log_path: String,
}

/// Built-in runtime status information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatusInfo {
    /// Runtime state: "none", "provisioned", "starting", "ready", "stopped", "error".
    pub state: String,
    /// Platform identifier: "macos-vz", "linux-kvm", "windows-wsl2".
    pub platform: String,
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub disk_gb: f32,
    /// Whether the native CrateBay Engine path is responsive.
    pub engine_responsive: bool,
    /// Whether the CrateBay Engine compatibility endpoint is responsive.
    pub compatibility_responsive: bool,
    /// CrateBay Engine compatibility endpoint version, when available.
    pub compatibility_version: Option<String>,
    /// Which runtime source backs this status.
    pub engine_source: Option<String>,
    /// Compatibility alias for older frontend and automation clients.
    pub docker_source: Option<String>,
    /// Whether the CrateBay Engine compatibility endpoint is responsive.
    ///
    /// Kept for older frontend and automation clients. Prefer
    /// `engineResponsive` or `compatibilityResponsive`.
    pub docker_responsive: bool,
    /// Underlying CrateBay engine metadata.
    pub engine: RuntimeEngineStatus,
    pub uptime_seconds: Option<u64>,
    pub resource_usage: Option<ResourceUsage>,
}

/// Resource usage stats for the runtime VM.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUsage {
    pub cpu_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub disk_used_gb: f32,
    pub disk_total_gb: f32,
    pub container_count: u32,
}

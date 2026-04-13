//! Sandbox operations backed by Docker containers.
//!
//! Uses Docker labels (com.cratebay.sandbox.*) for metadata per mcp-spec.md §2.4.
//! Reuses cratebay-core container and docker modules.

use std::collections::HashMap;

use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, ListContainersOptions,
    RemoveContainerOptions, StopContainerOptions,
};
use bollard::Docker;
use chrono::Utc;
use serde::Serialize;

use crate::error::McpError;
use crate::templates;

/// Label prefix for all CrateBay sandbox metadata.
const LABEL_PREFIX: &str = "com.cratebay.sandbox";

/// Sandbox info DTO returned from list/inspect operations.
#[derive(Debug, Clone, Serialize)]
pub struct SandboxInfo {
    pub id: String,
    pub short_id: String,
    pub name: String,
    pub template_id: String,
    pub owner: String,
    pub created_at: String,
    pub expires_at: String,
    pub ttl_hours: u32,
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub lifecycle_state: String,
    pub is_expired: bool,
}

/// Parameters for creating a sandbox.
#[derive(Debug)]
pub struct CreateSandboxParams {
    pub template_id: String,
    pub name: Option<String>,
    pub image: Option<String>,
    pub command: Option<String>,
    pub env: Option<Vec<String>>,
    pub cpu_cores: Option<u32>,
    pub memory_mb: Option<u64>,
    pub ttl_hours: Option<u32>,
    pub owner: Option<String>,
}

/// List all CrateBay-managed sandboxes.
pub async fn list_sandboxes(
    docker: &Docker,
    status_filter: Option<String>,
) -> Result<Vec<SandboxInfo>, McpError> {
    let mut filters = HashMap::new();
    filters.insert(
        "label".to_string(),
        vec![format!("{}.managed=true", LABEL_PREFIX)],
    );

    let options = ListContainersOptions {
        all: true,
        filters,
        ..Default::default()
    };

    let containers = docker.list_containers(Some(options)).await?;

    let mut sandboxes: Vec<SandboxInfo> = containers
        .into_iter()
        .filter_map(|c| {
            let labels = c.labels.as_ref()?;
            let id = c.id.clone().unwrap_or_default();

            let info = sandbox_info_from_labels(&id, labels, c.state.as_deref());
            Some(info)
        })
        .collect();

    // Apply optional status filter
    if let Some(ref status) = status_filter {
        sandboxes.retain(|s| s.lifecycle_state == *status);
    }

    Ok(sandboxes)
}

/// Inspect a single sandbox by ID.
pub async fn inspect_sandbox(docker: &Docker, sandbox_id: &str) -> Result<SandboxInfo, McpError> {
    let data = docker
        .inspect_container(sandbox_id, Some(InspectContainerOptions { size: false }))
        .await
        .map_err(|e| match e {
            bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            } => McpError::SandboxNotFound(sandbox_id.to_string()),
            other => McpError::Docker(other.to_string()),
        })?;

    let container_id = data.id.unwrap_or_default();
    let labels = data.config.as_ref().and_then(|c| c.labels.as_ref());

    let Some(labels) = labels else {
        return Err(McpError::SandboxNotFound(sandbox_id.to_string()));
    };

    // Verify this is a CrateBay-managed sandbox
    if labels.get(&format!("{}.managed", LABEL_PREFIX)) != Some(&"true".to_string()) {
        return Err(McpError::SandboxNotFound(sandbox_id.to_string()));
    }

    let state_str = data
        .state
        .as_ref()
        .and_then(|s| s.status.as_ref())
        .map(|s| s.to_string());

    Ok(sandbox_info_from_labels(
        &container_id,
        labels,
        state_str.as_deref(),
    ))
}

/// Create a new sandbox from a template.
pub async fn create_sandbox(
    docker: &Docker,
    params: CreateSandboxParams,
) -> Result<SandboxInfo, McpError> {
    let template = templates::find_template(&params.template_id)
        .ok_or_else(|| McpError::TemplateNotFound(params.template_id.clone()))?;

    let image = params.image.as_deref().unwrap_or(&template.image);
    let command = params
        .command
        .as_deref()
        .unwrap_or(&template.default_command);
    let cpu_cores = params.cpu_cores.unwrap_or(template.default_cpu_cores);
    let memory_mb = params.memory_mb.unwrap_or(template.default_memory_mb);
    let ttl_hours = params.ttl_hours.unwrap_or(24);
    let owner = params.owner.as_deref().unwrap_or("mcp_client");

    // Validate resource limits
    if cpu_cores == 0 || cpu_cores > 16 {
        return Err(McpError::InvalidParams(
            "cpu_cores must be 1-16".to_string(),
        ));
    }
    if !(256..=65536).contains(&memory_mb) {
        return Err(McpError::InvalidParams(
            "memory_mb must be 256-65536".to_string(),
        ));
    }
    if ttl_hours == 0 || ttl_hours > 168 {
        return Err(McpError::InvalidParams(
            "ttl_hours must be 1-168".to_string(),
        ));
    }

    let now = Utc::now();
    let expires_at = now + chrono::Duration::hours(ttl_hours as i64);

    let sandbox_name = params.name.unwrap_or_else(|| {
        format!(
            "cratebay-{}-{}",
            params.template_id,
            &uuid::Uuid::new_v4().to_string()[..8]
        )
    });

    // Docker labels for sandbox metadata per mcp-spec.md §2.4
    let mut labels = HashMap::new();
    labels.insert(format!("{}.managed", LABEL_PREFIX), "true".to_string());
    labels.insert(
        format!("{}.template_id", LABEL_PREFIX),
        params.template_id.clone(),
    );
    labels.insert(format!("{}.owner", LABEL_PREFIX), owner.to_string());
    labels.insert(format!("{}.created_at", LABEL_PREFIX), now.to_rfc3339());
    labels.insert(
        format!("{}.expires_at", LABEL_PREFIX),
        expires_at.to_rfc3339(),
    );
    labels.insert(format!("{}.ttl_hours", LABEL_PREFIX), ttl_hours.to_string());
    labels.insert(format!("{}.cpu_cores", LABEL_PREFIX), cpu_cores.to_string());
    labels.insert(format!("{}.memory_mb", LABEL_PREFIX), memory_mb.to_string());

    let host_config = bollard::models::HostConfig {
        memory: Some((memory_mb * 1024 * 1024) as i64),
        nano_cpus: Some((cpu_cores as i64) * 1_000_000_000),
        ..Default::default()
    };

    let config = Config {
        image: Some(image.to_string()),
        cmd: Some(vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            command.to_string(),
        ]),
        env: params.env.clone(),
        host_config: Some(host_config),
        labels: Some(labels),
        ..Default::default()
    };

    let options = CreateContainerOptions {
        name: sandbox_name.as_str(),
        platform: None,
    };

    let response = docker.create_container(Some(options), config).await?;

    // Auto-start the sandbox
    docker.start_container::<String>(&response.id, None).await?;

    // Return the created sandbox info
    inspect_sandbox(docker, &response.id).await
}

/// Start a stopped sandbox.
pub async fn start_sandbox(docker: &Docker, sandbox_id: &str) -> Result<(), McpError> {
    // Verify it's a managed sandbox
    let _info = inspect_sandbox(docker, sandbox_id).await?;

    docker.start_container::<String>(sandbox_id, None).await?;

    Ok(())
}

/// Stop a running sandbox.
pub async fn stop_sandbox(docker: &Docker, sandbox_id: &str) -> Result<(), McpError> {
    // Verify it's a managed sandbox
    let _info = inspect_sandbox(docker, sandbox_id).await?;

    let options = Some(StopContainerOptions { t: 10 });
    docker.stop_container(sandbox_id, options).await?;

    Ok(())
}

/// Delete a sandbox permanently.
pub async fn delete_sandbox(docker: &Docker, sandbox_id: &str) -> Result<(), McpError> {
    // Verify it's a managed sandbox
    let _info = inspect_sandbox(docker, sandbox_id).await?;

    let options = Some(RemoveContainerOptions {
        force: true,
        ..Default::default()
    });
    docker.remove_container(sandbox_id, options).await?;

    Ok(())
}

/// Execute a command inside a running sandbox.
pub async fn exec_in_sandbox(
    docker: &Docker,
    sandbox_id: &str,
    command: &str,
    timeout: Option<u32>,
) -> Result<cratebay_core::ExecResult, McpError> {
    // Verify it's a managed sandbox and running
    let info = inspect_sandbox(docker, sandbox_id).await?;
    if info.lifecycle_state != "running" {
        return Err(McpError::SandboxNotRunning(sandbox_id.to_string()));
    }

    let cmd = vec!["/bin/sh".to_string(), "-c".to_string(), command.to_string()];

    // If timeout is specified, wrap with timeout command
    let final_cmd = if let Some(t) = timeout {
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            format!("timeout {} sh -c '{}'", t, command.replace('\'', "'\\''")),
        ]
    } else {
        cmd
    };

    let result = cratebay_core::container::exec(docker, sandbox_id, final_cmd, None).await?;

    Ok(result)
}

/// Remove all expired sandboxes.
pub async fn cleanup_expired(docker: &Docker) -> Result<Vec<String>, McpError> {
    let sandboxes = list_sandboxes(docker, None).await?;
    let mut removed = Vec::new();

    for sandbox in &sandboxes {
        if sandbox.is_expired {
            if let Err(e) = delete_sandbox(docker, &sandbox.id).await {
                tracing::warn!(
                    "Failed to cleanup expired sandbox {}: {}",
                    sandbox.short_id,
                    e
                );
            } else {
                removed.push(sandbox.short_id.clone());
            }
        }
    }

    Ok(removed)
}

/// Copy file content into a sandbox container.
pub async fn put_path(
    docker: &Docker,
    sandbox_id: &str,
    container_path: &str,
    content: &[u8],
) -> Result<(), McpError> {
    // Verify it's a managed sandbox
    let _info = inspect_sandbox(docker, sandbox_id).await?;

    // Use Docker's put_archive API — we need to create a tar archive
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();

    // Extract directory and filename from container_path
    let path = std::path::Path::new(container_path);
    let filename = path
        .file_name()
        .ok_or_else(|| McpError::InvalidParams("Invalid container path".to_string()))?;
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("/"));

    let mut archive = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut archive);
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        builder
            .append_data(&mut header, filename, content)
            .map_err(|e| McpError::Internal(format!("Failed to create tar archive: {}", e)))?;
        builder
            .finish()
            .map_err(|e| McpError::Internal(format!("Failed to finalize tar archive: {}", e)))?;
    }

    docker
        .upload_to_container(
            sandbox_id,
            Some(bollard::container::UploadToContainerOptions {
                path: parent.to_string_lossy().to_string(),
                ..Default::default()
            }),
            archive.into(),
        )
        .await?;

    Ok(())
}

/// Copy file content from a sandbox container.
pub async fn get_path(
    docker: &Docker,
    sandbox_id: &str,
    container_path: &str,
) -> Result<Vec<u8>, McpError> {
    use futures_util::StreamExt;

    // Verify it's a managed sandbox
    let _info = inspect_sandbox(docker, sandbox_id).await?;

    let stream = docker.download_from_container(
        sandbox_id,
        Some(bollard::container::DownloadFromContainerOptions {
            path: container_path.to_string(),
        }),
    );

    let mut tar_data = Vec::new();
    let mut stream = std::pin::pin!(stream);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| McpError::Docker(e.to_string()))?;
        tar_data.extend_from_slice(&chunk);
    }

    // Extract the file content from the tar archive
    let mut archive = tar::Archive::new(tar_data.as_slice());
    let mut entries = archive
        .entries()
        .map_err(|e| McpError::Internal(format!("Failed to read tar archive: {}", e)))?;

    if let Some(entry) = entries.next() {
        let mut entry =
            entry.map_err(|e| McpError::Internal(format!("Failed to read tar entry: {}", e)))?;
        let mut content = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut content)
            .map_err(|e| McpError::Internal(format!("Failed to read file content: {}", e)))?;
        Ok(content)
    } else {
        Err(McpError::InvalidParams(format!(
            "File not found in container: {}",
            container_path
        )))
    }
}

// ---------------------------------------------------------------------------
// High-level sandbox operations — delegated to cratebay-core::sandbox
// ---------------------------------------------------------------------------

// Re-export types so existing MCP tool code continues to compile.
pub use cratebay_core::sandbox::{InstallParams, InstallResult, RunCodeParams, RunCodeResult};

/// Create a sandbox, write code, execute it, and return the result.
///
/// Delegates to `cratebay_core::sandbox::run_code`.
pub async fn run_code(docker: &Docker, params: RunCodeParams) -> Result<RunCodeResult, McpError> {
    Ok(cratebay_core::sandbox::run_code(docker, params).await?)
}

/// Install packages in an existing sandbox.
///
/// Verifies the sandbox is managed and running before delegating to core.
pub async fn install_packages(
    docker: &Docker,
    params: InstallParams,
) -> Result<InstallResult, McpError> {
    // Verify sandbox is managed and running (MCP-level check)
    let info = inspect_sandbox(docker, &params.sandbox_id).await?;
    if info.lifecycle_state != "running" {
        return Err(McpError::SandboxNotRunning(params.sandbox_id.clone()));
    }

    Ok(cratebay_core::sandbox::install_packages(docker, params).await?)
}

/// Build a SandboxInfo from Docker container labels and state.
fn sandbox_info_from_labels(
    container_id: &str,
    labels: &HashMap<String, String>,
    state: Option<&str>,
) -> SandboxInfo {
    let get_label = |key: &str| -> String {
        labels
            .get(&format!("{}.{}", LABEL_PREFIX, key))
            .cloned()
            .unwrap_or_default()
    };

    let expires_at = get_label("expires_at");
    let is_expired = if expires_at.is_empty() {
        false
    } else {
        chrono::DateTime::parse_from_rfc3339(&expires_at)
            .map(|dt| dt < Utc::now())
            .unwrap_or(false)
    };

    let lifecycle_state = match state {
        Some("running") => {
            if is_expired {
                "expired"
            } else {
                "running"
            }
        }
        Some("exited") => {
            if is_expired {
                "expired"
            } else {
                "stopped"
            }
        }
        Some("created") => "stopped",
        Some(other) => {
            if is_expired {
                "expired"
            } else {
                other
            }
        }
        None => "unknown",
    };

    let name = labels
        .get(&format!("{}.name", LABEL_PREFIX))
        .cloned()
        .unwrap_or_default();

    SandboxInfo {
        id: container_id.to_string(),
        short_id: container_id.chars().take(12).collect(),
        name,
        template_id: get_label("template_id"),
        owner: get_label("owner"),
        created_at: get_label("created_at"),
        expires_at,
        ttl_hours: get_label("ttl_hours").parse().unwrap_or(0),
        cpu_cores: get_label("cpu_cores").parse().unwrap_or(0),
        memory_mb: get_label("memory_mb").parse().unwrap_or(0),
        lifecycle_state: lifecycle_state.to_string(),
        is_expired,
    }
}

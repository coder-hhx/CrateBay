//! Sandbox high-level operations.
//!
//! Provides `run_code` and `install_packages` — the primary functions for
//! AI agents to execute code in isolated containers. Shared by both
//! cratebay-mcp (MCP Server) and cratebay-gui (Tauri commands).

use std::collections::HashMap;

use bollard::container::{Config, CreateContainerOptions, RemoveContainerOptions};
use bollard::Docker;
use serde::Serialize;

use crate::error::AppError;

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

/// A sandbox template definition.
#[derive(Debug, Clone, Serialize)]
pub struct SandboxTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub image: String,
    pub default_command: String,
    pub default_cpu_cores: u32,
    pub default_memory_mb: u64,
    pub tags: Vec<String>,
}

/// Return the 4 built-in sandbox templates.
pub fn builtin_templates() -> Vec<SandboxTemplate> {
    vec![
        SandboxTemplate {
            id: "node-dev".to_string(),
            name: "Node.js Development".to_string(),
            description: "Node.js 20 LTS with npm, yarn, and common dev tools".to_string(),
            image: "node:20-slim".to_string(),
            default_command: "sleep infinity".to_string(),
            default_cpu_cores: 2,
            default_memory_mb: 2048,
            tags: vec![
                "javascript".to_string(),
                "typescript".to_string(),
                "node".to_string(),
            ],
        },
        SandboxTemplate {
            id: "python-dev".to_string(),
            name: "Python Development".to_string(),
            description: "Python 3.12 with pip, venv, and common scientific packages".to_string(),
            image: "python:3.12-slim-bookworm".to_string(),
            default_command: "sleep infinity".to_string(),
            default_cpu_cores: 2,
            default_memory_mb: 2048,
            tags: vec![
                "python".to_string(),
                "data-science".to_string(),
                "ml".to_string(),
            ],
        },
        SandboxTemplate {
            id: "rust-dev".to_string(),
            name: "Rust Development".to_string(),
            description: "Rust stable with cargo, rustfmt, clippy".to_string(),
            image: "rust:1-slim-bookworm".to_string(),
            default_command: "sleep infinity".to_string(),
            default_cpu_cores: 4,
            default_memory_mb: 4096,
            tags: vec!["rust".to_string(), "systems".to_string()],
        },
        SandboxTemplate {
            id: "ubuntu-base".to_string(),
            name: "Ubuntu Base".to_string(),
            description: "Clean Ubuntu 24.04 with basic tools".to_string(),
            image: "ubuntu:24.04".to_string(),
            default_command: "sleep infinity".to_string(),
            default_cpu_cores: 1,
            default_memory_mb: 1024,
            tags: vec!["general".to_string(), "linux".to_string()],
        },
    ]
}

/// Look up a template by ID.
pub fn find_template(id: &str) -> Option<SandboxTemplate> {
    builtin_templates().into_iter().find(|t| t.id == id)
}

// ---------------------------------------------------------------------------
// run_code
// ---------------------------------------------------------------------------

/// Parameters for the run_code operation.
#[derive(Debug)]
pub struct RunCodeParams {
    pub language: String,
    pub code: String,
    pub timeout_seconds: Option<u64>,
    pub environment: Option<HashMap<String, String>>,
    pub cleanup: Option<bool>,
    /// Reuse an existing sandbox instead of creating a new one.
    pub sandbox_id: Option<String>,
}

/// Result of a run_code operation.
#[derive(Debug, Serialize)]
pub struct RunCodeResult {
    pub sandbox_id: String,
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub language: String,
}

/// Language-specific configuration.
struct LangConfig {
    template_id: &'static str,
    file_path: &'static str,
    run_cmd: &'static str,
}

fn lang_config(language: &str) -> Result<LangConfig, AppError> {
    match language {
        "python" => Ok(LangConfig {
            template_id: "python-dev",
            file_path: "/app/run.py",
            run_cmd: "python /app/run.py",
        }),
        "javascript" => Ok(LangConfig {
            template_id: "node-dev",
            file_path: "/app/run.js",
            run_cmd: "node /app/run.js",
        }),
        "bash" => Ok(LangConfig {
            template_id: "ubuntu-base",
            file_path: "/app/run.sh",
            run_cmd: "bash /app/run.sh",
        }),
        "rust" => Ok(LangConfig {
            template_id: "rust-dev",
            file_path: "/app/run.rs",
            run_cmd: "rustc /app/run.rs -o /app/run && /app/run",
        }),
        _ => Err(AppError::Validation(format!(
            "Unsupported language '{}'. Supported: python, javascript, bash, rust",
            language
        ))),
    }
}

/// Create a temporary sandbox container for code execution.
async fn create_temp_sandbox(
    docker: &Docker,
    template_id: &str,
    env: Option<Vec<String>>,
) -> Result<String, AppError> {
    let template = find_template(template_id)
        .ok_or_else(|| AppError::Validation(format!("Unknown template '{}'", template_id)))?;

    let sandbox_name = format!(
        "cratebay-{}-{}",
        template_id,
        &uuid::Uuid::new_v4().to_string()[..8]
    );

    let mut labels = HashMap::new();
    labels.insert(
        "com.cratebay.sandbox.managed".to_string(),
        "true".to_string(),
    );
    labels.insert(
        "com.cratebay.sandbox.template_id".to_string(),
        template_id.to_string(),
    );
    labels.insert(
        "com.cratebay.sandbox.owner".to_string(),
        "gui_run_code".to_string(),
    );

    let host_config = bollard::models::HostConfig {
        memory: Some((template.default_memory_mb * 1024 * 1024) as i64),
        nano_cpus: Some((template.default_cpu_cores as i64) * 1_000_000_000),
        ..Default::default()
    };

    let config = Config {
        image: Some(template.image.clone()),
        cmd: Some(vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            template.default_command.clone(),
        ]),
        env,
        host_config: Some(host_config),
        labels: Some(labels),
        ..Default::default()
    };

    let options = CreateContainerOptions {
        name: sandbox_name.as_str(),
        platform: None,
    };

    let response = docker.create_container(Some(options), config).await?;
    docker.start_container::<String>(&response.id, None).await?;

    Ok(response.id)
}

/// Delete a sandbox container.
async fn delete_temp_sandbox(docker: &Docker, sandbox_id: &str) {
    let options = Some(RemoveContainerOptions {
        force: true,
        ..Default::default()
    });
    let _ = docker.remove_container(sandbox_id, options).await;
}

/// Create a sandbox, write code, execute it, and return the result.
///
/// This is the primary high-level function for AI agents to run code.
pub async fn run_code(docker: &Docker, params: RunCodeParams) -> Result<RunCodeResult, AppError> {
    let start = std::time::Instant::now();
    let config = lang_config(&params.language)?;
    let timeout_secs = params.timeout_seconds.unwrap_or(60);
    let should_cleanup = params.cleanup.unwrap_or(true);

    let env: Option<Vec<String>> = params.environment.map(|map| {
        map.into_iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect()
    });

    // Use existing sandbox or create a new one
    let (sandbox_id, created_new) = if let Some(ref id) = params.sandbox_id {
        (id.clone(), false)
    } else {
        let id = create_temp_sandbox(docker, config.template_id, env).await?;
        (id, true)
    };

    // Write code to file
    if let Err(e) =
        crate::container::exec_put_text(docker, &sandbox_id, config.file_path, &params.code).await
    {
        if should_cleanup && created_new {
            delete_temp_sandbox(docker, &sandbox_id).await;
        }
        return Err(AppError::Runtime(format!(
            "Failed to write code file: {}",
            e
        )));
    }

    // Execute code
    let exec_result = crate::container::exec_with_timeout(
        docker,
        &sandbox_id,
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            config.run_cmd.to_string(),
        ],
        Some("/app".to_string()),
        timeout_secs,
    )
    .await;

    let exec_result = match exec_result {
        Ok(r) => r,
        Err(e) => {
            if should_cleanup && created_new {
                delete_temp_sandbox(docker, &sandbox_id).await;
            }
            return Err(AppError::Runtime(format!("Code execution failed: {}", e)));
        }
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    // Cleanup if requested
    if should_cleanup && created_new {
        delete_temp_sandbox(docker, &sandbox_id).await;
    }

    Ok(RunCodeResult {
        sandbox_id: if should_cleanup && created_new {
            sandbox_id.chars().take(12).collect()
        } else {
            sandbox_id
        },
        exit_code: exec_result.exit_code,
        stdout: exec_result.stdout,
        stderr: exec_result.stderr,
        duration_ms,
        language: params.language,
    })
}

// ---------------------------------------------------------------------------
// install_packages
// ---------------------------------------------------------------------------

/// Parameters for the install operation.
#[derive(Debug)]
pub struct InstallParams {
    pub sandbox_id: String,
    pub package_manager: String,
    pub packages: Vec<String>,
}

/// Result of an install operation.
#[derive(Debug, Serialize)]
pub struct InstallResult {
    pub sandbox_id: String,
    pub package_manager: String,
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

/// Install packages in an existing sandbox.
pub async fn install_packages(
    docker: &Docker,
    params: InstallParams,
) -> Result<InstallResult, AppError> {
    let start = std::time::Instant::now();

    // Validate package names
    for pkg in &params.packages {
        if pkg.contains(';') || pkg.contains('&') || pkg.contains('|') || pkg.contains('`') {
            return Err(AppError::Validation(format!(
                "Invalid package name '{}': contains shell metacharacters",
                pkg
            )));
        }
    }

    let packages_str = params.packages.join(" ");

    let cmd = match params.package_manager.as_str() {
        "pip" => format!("pip install --no-cache-dir {}", packages_str),
        "npm" => format!("npm install --no-fund --no-audit {}", packages_str),
        "cargo" => format!("cargo add {}", packages_str),
        "apt" => format!(
            "apt-get update -qq && apt-get install -y -qq {}",
            packages_str
        ),
        other => {
            return Err(AppError::Validation(format!(
                "Unsupported package manager '{}'. Supported: pip, npm, cargo, apt",
                other
            )));
        }
    };

    let result = crate::container::exec_with_timeout(
        docker,
        &params.sandbox_id,
        vec!["/bin/sh".to_string(), "-c".to_string(), cmd],
        None,
        300,
    )
    .await?;

    let duration_ms = start.elapsed().as_millis() as u64;

    Ok(InstallResult {
        sandbox_id: params.sandbox_id,
        package_manager: params.package_manager,
        exit_code: result.exit_code,
        stdout: result.stdout,
        stderr: result.stderr,
        duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_templates_count() {
        assert_eq!(builtin_templates().len(), 4);
    }

    #[test]
    fn test_find_template() {
        assert!(find_template("python-dev").is_some());
        assert!(find_template("node-dev").is_some());
        assert!(find_template("rust-dev").is_some());
        assert!(find_template("ubuntu-base").is_some());
        assert!(find_template("nonexistent").is_none());
    }

    #[test]
    fn test_lang_config() {
        assert!(lang_config("python").is_ok());
        assert!(lang_config("javascript").is_ok());
        assert!(lang_config("bash").is_ok());
        assert!(lang_config("rust").is_ok());
        assert!(lang_config("go").is_err());
    }

    #[test]
    fn test_lang_config_templates_exist() {
        for lang in &["python", "javascript", "bash", "rust"] {
            let cfg = lang_config(lang).unwrap();
            assert!(
                find_template(cfg.template_id).is_some(),
                "Template '{}' for language '{}' not found",
                cfg.template_id,
                lang
            );
        }
    }
}

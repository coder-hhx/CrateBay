//! MCP Tool definitions and dispatch.
//!
//! Defines the 11 sandbox tools per mcp-spec.md §2.2 and dispatches
//! tools/call requests to the appropriate handler.

use bollard::Docker;
use std::time::Instant;

use crate::audit::{self, AuditResult};
use crate::error::McpError;
use crate::protocol::{McpToolDefinition, ToolCallResult};
use crate::sandbox;
use crate::security;
use crate::templates;

/// Build the complete tool catalog (13 sandbox tools).
pub fn tool_catalog() -> Vec<McpToolDefinition> {
    vec![
        // --- High-level AI sandbox tools ---
        McpToolDefinition {
            name: "cratebay_sandbox_run_code".to_string(),
            description: "Create a sandbox, write code, execute it, and return stdout/stderr/exit_code in one call. This is the primary tool for AI agents to run code safely.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "enum": ["python", "javascript", "bash", "rust"],
                        "description": "Programming language to use"
                    },
                    "code": {
                        "type": "string",
                        "description": "Source code to execute"
                    },
                    "timeout_seconds": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 3600,
                        "description": "Execution timeout in seconds (default: 60)"
                    },
                    "environment": {
                        "type": "object",
                        "additionalProperties": { "type": "string" },
                        "description": "Optional environment variables"
                    },
                    "cleanup": {
                        "type": "boolean",
                        "description": "Remove sandbox after execution (default: true). Set false to keep sandbox for follow-up operations."
                    }
                },
                "required": ["language", "code"]
            }),
            confirmation_required: None,
        },
        McpToolDefinition {
            name: "cratebay_sandbox_install".to_string(),
            description: "Install packages in an existing sandbox using pip, npm, cargo, or apt.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "sandbox_id": {
                        "type": "string",
                        "description": "Target sandbox container ID"
                    },
                    "package_manager": {
                        "type": "string",
                        "enum": ["pip", "npm", "cargo", "apt"],
                        "description": "Package manager to use"
                    },
                    "packages": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Package names to install (e.g., ['numpy', 'pandas'])"
                    }
                },
                "required": ["sandbox_id", "package_manager", "packages"]
            }),
            confirmation_required: None,
        },
        // --- Low-level sandbox management tools ---
        McpToolDefinition {
            name: "cratebay_sandbox_templates".to_string(),
            description: "List available sandbox templates".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            confirmation_required: None,
        },
        McpToolDefinition {
            name: "cratebay_sandbox_list".to_string(),
            description: "List all managed sandboxes".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "description": "Filter by lifecycle state: running, stopped, expired, error",
                        "enum": ["running", "stopped", "expired", "error"]
                    }
                },
                "required": []
            }),
            confirmation_required: None,
        },
        McpToolDefinition {
            name: "cratebay_sandbox_inspect".to_string(),
            description: "Get detailed sandbox information".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "sandbox_id": {
                        "type": "string",
                        "description": "Sandbox container ID or short ID"
                    }
                },
                "required": ["sandbox_id"]
            }),
            confirmation_required: None,
        },
        McpToolDefinition {
            name: "cratebay_sandbox_create".to_string(),
            description: "Create a new sandbox from template".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "template_id": {
                        "type": "string",
                        "description": "Template ID: node-dev, python-dev, rust-dev, ubuntu-base"
                    },
                    "name": {
                        "type": "string",
                        "description": "Optional custom sandbox name"
                    },
                    "image": {
                        "type": "string",
                        "description": "Optional override for the template's default image"
                    },
                    "command": {
                        "type": "string",
                        "description": "Optional startup command override"
                    },
                    "env": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Environment variables in KEY=VALUE format"
                    },
                    "cpu_cores": {
                        "type": "integer",
                        "description": "CPU cores (1-16)",
                        "minimum": 1,
                        "maximum": 16
                    },
                    "memory_mb": {
                        "type": "integer",
                        "description": "Memory in MB (256-65536)",
                        "minimum": 256,
                        "maximum": 65536
                    },
                    "ttl_hours": {
                        "type": "integer",
                        "description": "Time-to-live in hours (1-168)",
                        "minimum": 1,
                        "maximum": 168
                    },
                    "owner": {
                        "type": "string",
                        "description": "Optional owner identifier"
                    }
                },
                "required": ["template_id"]
            }),
            confirmation_required: None,
        },
        McpToolDefinition {
            name: "cratebay_sandbox_start".to_string(),
            description: "Start a stopped sandbox".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "sandbox_id": {
                        "type": "string",
                        "description": "Sandbox container ID or short ID"
                    }
                },
                "required": ["sandbox_id"]
            }),
            confirmation_required: None,
        },
        McpToolDefinition {
            name: "cratebay_sandbox_stop".to_string(),
            description: "Stop a running sandbox".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "sandbox_id": {
                        "type": "string",
                        "description": "Sandbox container ID or short ID"
                    }
                },
                "required": ["sandbox_id"]
            }),
            confirmation_required: Some(true),
        },
        McpToolDefinition {
            name: "cratebay_sandbox_delete".to_string(),
            description: "Delete a sandbox permanently".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "sandbox_id": {
                        "type": "string",
                        "description": "Sandbox container ID or short ID"
                    }
                },
                "required": ["sandbox_id"]
            }),
            confirmation_required: Some(true),
        },
        McpToolDefinition {
            name: "cratebay_sandbox_exec".to_string(),
            description: "Execute a command in a sandbox".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "sandbox_id": {
                        "type": "string",
                        "description": "Sandbox container ID or short ID"
                    },
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Execution timeout in seconds"
                    }
                },
                "required": ["sandbox_id", "command"]
            }),
            confirmation_required: None,
        },
        McpToolDefinition {
            name: "cratebay_sandbox_cleanup_expired".to_string(),
            description: "Remove all expired sandboxes".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            confirmation_required: Some(true),
        },
        McpToolDefinition {
            name: "cratebay_sandbox_put_path".to_string(),
            description: "Copy a file into a sandbox".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "sandbox_id": {
                        "type": "string",
                        "description": "Sandbox container ID or short ID"
                    },
                    "container_path": {
                        "type": "string",
                        "description": "Destination path inside the container"
                    },
                    "content": {
                        "type": "string",
                        "description": "File content encoded as base64"
                    }
                },
                "required": ["sandbox_id", "container_path", "content"]
            }),
            confirmation_required: None,
        },
        McpToolDefinition {
            name: "cratebay_sandbox_get_path".to_string(),
            description: "Copy a file from a sandbox".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "sandbox_id": {
                        "type": "string",
                        "description": "Sandbox container ID or short ID"
                    },
                    "container_path": {
                        "type": "string",
                        "description": "Path of the file inside the container"
                    }
                },
                "required": ["sandbox_id", "container_path"]
            }),
            confirmation_required: None,
        },
    ]
}

/// Dispatch a tool call to the appropriate handler.
///
/// Records an audit entry for every call.
pub async fn dispatch_tool_call(
    docker: &Docker,
    tool_name: &str,
    arguments: &serde_json::Value,
    caller: &str,
) -> ToolCallResult {
    let start = Instant::now();

    let result = match tool_name {
        "cratebay_sandbox_run_code" => handle_run_code(docker, arguments).await,
        "cratebay_sandbox_install" => handle_install(docker, arguments).await,
        "cratebay_sandbox_templates" => handle_templates().await,
        "cratebay_sandbox_list" => handle_list(docker, arguments).await,
        "cratebay_sandbox_inspect" => handle_inspect(docker, arguments).await,
        "cratebay_sandbox_create" => handle_create(docker, arguments).await,
        "cratebay_sandbox_start" => handle_start(docker, arguments).await,
        "cratebay_sandbox_stop" => handle_stop(docker, arguments).await,
        "cratebay_sandbox_delete" => handle_delete(docker, arguments).await,
        "cratebay_sandbox_exec" => handle_exec(docker, arguments).await,
        "cratebay_sandbox_cleanup_expired" => handle_cleanup_expired(docker).await,
        "cratebay_sandbox_put_path" => handle_put_path(docker, arguments).await,
        "cratebay_sandbox_get_path" => handle_get_path(docker, arguments).await,
        _ => Err(McpError::InvalidParams(format!(
            "Unknown tool: {}",
            tool_name
        ))),
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(text) => {
            let entry = audit::create_entry(
                tool_name,
                arguments,
                AuditResult::Success(truncate_for_audit(&text)),
                caller,
                duration_ms,
            );
            audit::write_audit_entry(&entry);
            ToolCallResult::success(text)
        }
        Err(e) => {
            let error_msg = e.to_string();
            let entry = audit::create_entry(
                tool_name,
                arguments,
                AuditResult::Error(error_msg.clone()),
                caller,
                duration_ms,
            );
            audit::write_audit_entry(&entry);
            ToolCallResult::error(error_msg)
        }
    }
}

/// Truncate result text for audit logging.
fn truncate_for_audit(text: &str) -> String {
    if text.len() > 200 {
        format!("{}...[truncated]", &text[..200])
    } else {
        text.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tool handlers
// ---------------------------------------------------------------------------

async fn handle_run_code(
    docker: &Docker,
    arguments: &serde_json::Value,
) -> Result<String, McpError> {
    let language = arguments
        .get("language")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("language is required".to_string()))?;

    let code = arguments
        .get("code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("code is required".to_string()))?;

    let timeout_seconds = arguments.get("timeout_seconds").and_then(|v| v.as_u64());

    let environment = arguments.get("environment").and_then(|v| {
        v.as_object().map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
    });

    let cleanup = arguments.get("cleanup").and_then(|v| v.as_bool());

    let params = sandbox::RunCodeParams {
        language: language.to_string(),
        code: code.to_string(),
        timeout_seconds,
        environment,
        cleanup,
        sandbox_id: None,
    };

    let result = sandbox::run_code(docker, params).await?;
    serde_json::to_string_pretty(&result).map_err(McpError::Serialization)
}

async fn handle_install(
    docker: &Docker,
    arguments: &serde_json::Value,
) -> Result<String, McpError> {
    let sandbox_id = arguments
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("sandbox_id is required".to_string()))?;

    let package_manager = arguments
        .get("package_manager")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("package_manager is required".to_string()))?;

    let packages = arguments
        .get("packages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| McpError::InvalidParams("packages array is required".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect::<Vec<_>>();

    if packages.is_empty() {
        return Err(McpError::InvalidParams(
            "packages array must not be empty".to_string(),
        ));
    }

    let params = sandbox::InstallParams {
        sandbox_id: sandbox_id.to_string(),
        package_manager: package_manager.to_string(),
        packages,
    };

    let result = sandbox::install_packages(docker, params).await?;
    serde_json::to_string_pretty(&result).map_err(McpError::Serialization)
}

async fn handle_templates() -> Result<String, McpError> {
    let templates = templates::builtin_templates();
    serde_json::to_string_pretty(&templates).map_err(McpError::Serialization)
}

async fn handle_list(docker: &Docker, arguments: &serde_json::Value) -> Result<String, McpError> {
    let status = arguments
        .get("status")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let sandboxes = sandbox::list_sandboxes(docker, status).await?;
    serde_json::to_string_pretty(&sandboxes).map_err(McpError::Serialization)
}

async fn handle_inspect(
    docker: &Docker,
    arguments: &serde_json::Value,
) -> Result<String, McpError> {
    let sandbox_id = arguments
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("sandbox_id is required".to_string()))?;

    let info = sandbox::inspect_sandbox(docker, sandbox_id).await?;
    serde_json::to_string_pretty(&info).map_err(McpError::Serialization)
}

async fn handle_create(docker: &Docker, arguments: &serde_json::Value) -> Result<String, McpError> {
    let template_id = arguments
        .get("template_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("template_id is required".to_string()))?;

    let params = sandbox::CreateSandboxParams {
        template_id: template_id.to_string(),
        name: arguments
            .get("name")
            .and_then(|v| v.as_str())
            .map(String::from),
        image: arguments
            .get("image")
            .and_then(|v| v.as_str())
            .map(String::from),
        command: arguments
            .get("command")
            .and_then(|v| v.as_str())
            .map(String::from),
        env: arguments.get("env").and_then(|v| {
            v.as_array().map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_str().map(String::from))
                    .collect()
            })
        }),
        cpu_cores: arguments
            .get("cpu_cores")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        memory_mb: arguments.get("memory_mb").and_then(|v| v.as_u64()),
        ttl_hours: arguments
            .get("ttl_hours")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        owner: arguments
            .get("owner")
            .and_then(|v| v.as_str())
            .map(String::from),
    };

    let info = sandbox::create_sandbox(docker, params).await?;
    serde_json::to_string_pretty(&info).map_err(McpError::Serialization)
}

async fn handle_start(docker: &Docker, arguments: &serde_json::Value) -> Result<String, McpError> {
    let sandbox_id = arguments
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("sandbox_id is required".to_string()))?;

    sandbox::start_sandbox(docker, sandbox_id).await?;
    Ok(format!("Sandbox {} started successfully", sandbox_id))
}

async fn handle_stop(docker: &Docker, arguments: &serde_json::Value) -> Result<String, McpError> {
    let sandbox_id = arguments
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("sandbox_id is required".to_string()))?;

    sandbox::stop_sandbox(docker, sandbox_id).await?;
    Ok(format!("Sandbox {} stopped successfully", sandbox_id))
}

async fn handle_delete(docker: &Docker, arguments: &serde_json::Value) -> Result<String, McpError> {
    let sandbox_id = arguments
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("sandbox_id is required".to_string()))?;

    sandbox::delete_sandbox(docker, sandbox_id).await?;
    Ok(format!("Sandbox {} deleted successfully", sandbox_id))
}

async fn handle_exec(docker: &Docker, arguments: &serde_json::Value) -> Result<String, McpError> {
    let sandbox_id = arguments
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("sandbox_id is required".to_string()))?;

    let command = arguments
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("command is required".to_string()))?;

    let timeout = arguments
        .get("timeout")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);

    let result = sandbox::exec_in_sandbox(docker, sandbox_id, command, timeout).await?;
    serde_json::to_string_pretty(&result).map_err(McpError::Serialization)
}

async fn handle_cleanup_expired(docker: &Docker) -> Result<String, McpError> {
    let removed = sandbox::cleanup_expired(docker).await?;

    let result = serde_json::json!({
        "removed_count": removed.len(),
        "removed_ids": removed,
    });
    serde_json::to_string_pretty(&result).map_err(McpError::Serialization)
}

async fn handle_put_path(
    docker: &Docker,
    arguments: &serde_json::Value,
) -> Result<String, McpError> {
    use base64::Engine;

    let sandbox_id = arguments
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("sandbox_id is required".to_string()))?;

    let container_path = arguments
        .get("container_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("container_path is required".to_string()))?;

    let content_b64 = arguments
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("content (base64) is required".to_string()))?;

    // Validate path if workspace root is set
    if let Some(root) = security::workspace_root() {
        security::validate_path(container_path, &root)?;
    }

    let content = base64::engine::general_purpose::STANDARD
        .decode(content_b64)
        .map_err(|e| McpError::InvalidParams(format!("Invalid base64 content: {}", e)))?;

    sandbox::put_path(docker, sandbox_id, container_path, &content).await?;

    Ok(format!(
        "File written to {} ({} bytes)",
        container_path,
        content.len()
    ))
}

async fn handle_get_path(
    docker: &Docker,
    arguments: &serde_json::Value,
) -> Result<String, McpError> {
    use base64::Engine;

    let sandbox_id = arguments
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("sandbox_id is required".to_string()))?;

    let container_path = arguments
        .get("container_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::InvalidParams("container_path is required".to_string()))?;

    let content = sandbox::get_path(docker, sandbox_id, container_path).await?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&content);

    let result = serde_json::json!({
        "path": container_path,
        "content": encoded,
        "size_bytes": content.len(),
    });
    serde_json::to_string_pretty(&result).map_err(McpError::Serialization)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_catalog_has_13_tools() {
        let catalog = tool_catalog();
        assert_eq!(
            catalog.len(),
            13,
            "Expected 13 tools (2 high-level + 11 low-level)"
        );
    }

    #[test]
    fn test_tool_catalog_names() {
        let catalog = tool_catalog();
        let names: Vec<&str> = catalog.iter().map(|t| t.name.as_str()).collect();
        let expected_names = [
            "cratebay_sandbox_run_code",
            "cratebay_sandbox_install",
            "cratebay_sandbox_templates",
            "cratebay_sandbox_list",
            "cratebay_sandbox_inspect",
            "cratebay_sandbox_create",
            "cratebay_sandbox_start",
            "cratebay_sandbox_stop",
            "cratebay_sandbox_delete",
            "cratebay_sandbox_exec",
            "cratebay_sandbox_cleanup_expired",
            "cratebay_sandbox_put_path",
            "cratebay_sandbox_get_path",
        ];
        for expected in &expected_names {
            assert!(names.contains(expected), "Missing tool: {}", expected);
        }
    }

    #[test]
    fn test_tool_catalog_all_have_descriptions() {
        let catalog = tool_catalog();
        for tool in &catalog {
            assert!(
                !tool.description.is_empty(),
                "Tool '{}' has empty description",
                tool.name
            );
        }
    }

    #[test]
    fn test_tool_catalog_all_have_input_schemas() {
        let catalog = tool_catalog();
        for tool in &catalog {
            assert!(
                tool.input_schema.is_object(),
                "Tool '{}' has non-object input_schema",
                tool.name
            );
            assert_eq!(
                tool.input_schema["type"], "object",
                "Tool '{}' input_schema type should be 'object'",
                tool.name
            );
        }
    }

    #[test]
    fn test_destructive_tools_require_confirmation() {
        let catalog = tool_catalog();
        let destructive_tools = [
            "cratebay_sandbox_stop",
            "cratebay_sandbox_delete",
            "cratebay_sandbox_cleanup_expired",
        ];
        for tool in &catalog {
            if destructive_tools.contains(&tool.name.as_str()) {
                assert_eq!(
                    tool.confirmation_required,
                    Some(true),
                    "Destructive tool '{}' should require confirmation",
                    tool.name
                );
            }
        }
    }

    #[test]
    fn test_non_destructive_tools_no_confirmation() {
        let catalog = tool_catalog();
        let non_destructive = [
            "cratebay_sandbox_run_code",
            "cratebay_sandbox_install",
            "cratebay_sandbox_templates",
            "cratebay_sandbox_list",
            "cratebay_sandbox_inspect",
            "cratebay_sandbox_create",
            "cratebay_sandbox_start",
            "cratebay_sandbox_exec",
            "cratebay_sandbox_put_path",
            "cratebay_sandbox_get_path",
        ];
        for tool in &catalog {
            if non_destructive.contains(&tool.name.as_str()) {
                assert_eq!(
                    tool.confirmation_required, None,
                    "Non-destructive tool '{}' should not require confirmation",
                    tool.name
                );
            }
        }
    }

    #[test]
    fn test_create_tool_requires_template_id() {
        let catalog = tool_catalog();
        let create_tool = catalog
            .iter()
            .find(|t| t.name == "cratebay_sandbox_create")
            .unwrap();
        let required = create_tool.input_schema["required"]
            .as_array()
            .expect("required should be array");
        assert!(
            required.contains(&serde_json::json!("template_id")),
            "create tool should require template_id"
        );
    }

    #[test]
    fn test_exec_tool_requires_sandbox_id_and_command() {
        let catalog = tool_catalog();
        let exec_tool = catalog
            .iter()
            .find(|t| t.name == "cratebay_sandbox_exec")
            .unwrap();
        let required = exec_tool.input_schema["required"]
            .as_array()
            .expect("required should be array");
        assert!(required.contains(&serde_json::json!("sandbox_id")));
        assert!(required.contains(&serde_json::json!("command")));
    }

    #[test]
    fn test_tool_catalog_serializable() {
        let catalog = tool_catalog();
        let json = serde_json::to_string(&catalog);
        assert!(json.is_ok(), "Tool catalog should be serializable");
    }

    #[test]
    fn test_truncate_for_audit_short() {
        let text = "short text";
        assert_eq!(truncate_for_audit(text), "short text");
    }

    #[test]
    fn test_truncate_for_audit_long() {
        let text = "x".repeat(300);
        let truncated = truncate_for_audit(&text);
        assert!(truncated.len() < 300);
        assert!(truncated.ends_with("...[truncated]"));
    }

    #[test]
    fn test_truncate_for_audit_exactly_200() {
        let text = "a".repeat(200);
        assert_eq!(truncate_for_audit(&text), text);
    }
}

//! Sandbox execution Tauri commands.
//!
//! Exposes `cratebay_core::sandbox` operations (run_code, install_packages)
//! to the frontend via Tauri invoke.

use tauri::State;

use crate::state::AppState;
use cratebay_core::error::AppError;
use cratebay_core::sandbox;

/// Run code in a temporary sandbox container.
///
/// Creates a container, writes the code, executes it, and returns the result.
/// By default the container is cleaned up after execution.
#[tauri::command]
pub async fn sandbox_run_code(
    state: State<'_, AppState>,
    language: String,
    code: String,
    sandbox_id: Option<String>,
    timeout_seconds: Option<u64>,
) -> Result<sandbox::RunCodeResult, AppError> {
    let docker = state.ensure_docker_once().await?;

    let params = sandbox::RunCodeParams {
        language,
        code,
        timeout_seconds,
        environment: None,
        cleanup: if sandbox_id.is_some() {
            Some(false)
        } else {
            Some(true)
        },
        sandbox_id,
    };

    sandbox::run_code(&docker, params).await
}

/// Install packages in an existing sandbox container.
#[tauri::command]
pub async fn sandbox_install(
    state: State<'_, AppState>,
    sandbox_id: String,
    package_manager: String,
    packages: Vec<String>,
) -> Result<sandbox::InstallResult, AppError> {
    let docker = state.ensure_docker_once().await?;

    let params = sandbox::InstallParams {
        sandbox_id,
        package_manager,
        packages,
    };

    sandbox::install_packages(&docker, params).await
}

//! Pod management Tauri commands.

use tauri::State;

use crate::state::AppState;
use cratebay_core::error::AppError;
use cratebay_core::models::PodInfo;
use cratebay_core::pod;

/// List CrateBay-managed pods.
#[tauri::command]
pub async fn pod_list(state: State<'_, AppState>) -> Result<Vec<PodInfo>, AppError> {
    let docker = state.ensure_docker_once().await?;
    pod::list(&docker).await
}

/// Create a new pod.
#[tauri::command]
pub async fn pod_create(state: State<'_, AppState>, name: String) -> Result<PodInfo, AppError> {
    let docker = state.ensure_docker_once().await?;
    pod::create(&docker, &name).await
}

/// Inspect a pod by name or id.
#[tauri::command]
pub async fn pod_inspect(state: State<'_, AppState>, name: String) -> Result<PodInfo, AppError> {
    let docker = state.ensure_docker_once().await?;
    pod::inspect(&docker, &name).await
}

/// Delete a pod.
#[tauri::command]
pub async fn pod_delete(
    state: State<'_, AppState>,
    name: String,
    force: Option<bool>,
) -> Result<(), AppError> {
    let docker = state.ensure_docker_once().await?;
    pod::delete(&docker, &name, force.unwrap_or(false)).await
}

/// Add an existing container to a pod.
#[tauri::command]
pub async fn pod_add_container(
    state: State<'_, AppState>,
    name: String,
    container: String,
) -> Result<(), AppError> {
    let docker = state.ensure_docker_once().await?;
    pod::add_container(&docker, &name, &container).await
}

/// Remove a container from a pod.
#[tauri::command]
pub async fn pod_remove_container(
    state: State<'_, AppState>,
    name: String,
    container: String,
    force: Option<bool>,
) -> Result<(), AppError> {
    let docker = state.ensure_docker_once().await?;
    pod::remove_container(&docker, &name, &container, force.unwrap_or(false)).await
}

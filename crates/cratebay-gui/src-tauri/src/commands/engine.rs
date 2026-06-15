//! Native CrateBay Engine maintenance commands.

use serde_json::Value;
use tauri::State;

use crate::state::AppState;
use cratebay_core::error::AppError;
use cratebay_core::runtime;

#[tauri::command]
pub async fn engine_contract(state: State<'_, AppState>) -> Result<Value, AppError> {
    runtime::query_built_in_engine_contract(state.runtime.as_ref())
}

#[tauri::command]
pub async fn engine_substrate(state: State<'_, AppState>) -> Result<Value, AppError> {
    runtime::query_built_in_native_substrate(state.runtime.as_ref())
}

#[tauri::command]
pub async fn engine_storage_gc(
    state: State<'_, AppState>,
    apply: bool,
    prune_exited_containers: bool,
) -> Result<Value, AppError> {
    runtime::query_built_in_native_storage_gc(
        state.runtime.as_ref(),
        apply,
        prune_exited_containers,
    )
}

#[tauri::command]
pub async fn engine_shim_tasks(state: State<'_, AppState>) -> Result<Value, AppError> {
    runtime::query_built_in_native_shim_tasks(state.runtime.as_ref())
}

#[tauri::command]
pub async fn engine_shim_reap_task(
    state: State<'_, AppState>,
    id: String,
    apply: bool,
) -> Result<Value, AppError> {
    runtime::query_built_in_native_shim_reap_task(state.runtime.as_ref(), &id, apply)
}

//! Storage-related Tauri commands (settings only).

use tauri::State;

use crate::state::AppState;
use cratebay_core::audit;
use cratebay_core::error::AppError;
use cratebay_core::storage;
use cratebay_core::AuditAction;
use cratebay_core::MutexExt;

/// Get a setting value by key.
#[tauri::command]
pub async fn settings_get(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, AppError> {
    let db = state.db.lock_or_recover()?;
    storage::get_setting(&db, &key)
}

/// Update a setting value.
#[tauri::command]
pub async fn settings_update(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), AppError> {
    let db = state.db.lock_or_recover()?;
    storage::set_setting(&db, &key, &value)?;
    audit::log_action(
        &db,
        &AuditAction::SettingsUpdate,
        &key,
        Some(&value),
        "user",
    )?;
    Ok(())
}

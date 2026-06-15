//! Volume management Tauri commands.

use serde::Serialize;
use serde_json::{json, Value};
use tauri::State;

use crate::state::AppState;
use cratebay_core::error::AppError;
use cratebay_core::runtime;
use cratebay_core::runtime::NativeVolumeSummary;
use cratebay_core::validation;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeInfo {
    pub name: String,
    pub driver: String,
    pub mountpoint: String,
    pub created_at: Option<String>,
    pub scope: String,
    pub labels: Value,
    pub options: Value,
    pub managed_by: String,
}

#[tauri::command]
pub async fn volume_list(state: State<'_, AppState>) -> Result<Vec<VolumeInfo>, AppError> {
    ensure_native_engine(&state).await?;
    let payload = runtime::query_built_in_native_volumes(state.runtime.as_ref())?;
    let mut volumes = payload
        .items
        .into_iter()
        .map(volume_info_from_native_summary)
        .collect::<Vec<_>>();
    volumes.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(volumes)
}

#[tauri::command]
pub async fn volume_create(
    state: State<'_, AppState>,
    name: String,
    driver: Option<String>,
) -> Result<VolumeInfo, AppError> {
    let name = required_name(&name, "Volume name")?;
    let driver = volume_driver(driver.as_deref());
    validation::validate_container_name(name)?;
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_volume_create(
        state.runtime.as_ref(),
        &json!({
            "name": name,
            "driver": driver,
            "labels": {
                "com.cratebay.managed": "true",
                "com.cratebay.volume": "true",
            },
        }),
    )?;
    inspect_native_volume(&state, name)
}

#[tauri::command]
pub async fn volume_inspect(
    state: State<'_, AppState>,
    name: String,
) -> Result<VolumeInfo, AppError> {
    let name = required_name(&name, "Volume name")?;
    ensure_native_engine(&state).await?;
    inspect_native_volume(&state, name)
}

#[tauri::command]
pub async fn volume_delete(
    state: State<'_, AppState>,
    name: String,
    force: Option<bool>,
) -> Result<(), AppError> {
    let name = required_name(&name, "Volume name")?;
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_volume_remove(
        state.runtime.as_ref(),
        name,
        force.unwrap_or(false),
    )?;
    Ok(())
}

async fn ensure_native_engine(state: &AppState) -> Result<(), AppError> {
    state.ensure_native_engine_once().await
}

fn inspect_native_volume(state: &AppState, name: &str) -> Result<VolumeInfo, AppError> {
    let payload = runtime::query_built_in_native_volume_inspect(state.runtime.as_ref(), name)?;
    volume_info_from_value(payload.get("item").unwrap_or(&payload))
}

fn volume_info_from_native_summary(summary: NativeVolumeSummary) -> VolumeInfo {
    VolumeInfo {
        name: summary.name,
        driver: summary.driver,
        mountpoint: summary.mountpoint,
        created_at: optional_non_empty(summary.created_at),
        scope: summary.scope,
        labels: summary.labels,
        options: summary.options,
        managed_by: summary.managed_by,
    }
}

fn volume_info_from_value(value: &Value) -> Result<VolumeInfo, AppError> {
    Ok(VolumeInfo {
        name: string_field(value, &["name", "Name"]).unwrap_or_default(),
        driver: string_field(value, &["driver", "Driver"]).unwrap_or_else(|| "local".to_string()),
        mountpoint: string_field(value, &["mountpoint", "Mountpoint"]).unwrap_or_default(),
        created_at: string_field(value, &["createdAt", "CreatedAt"]).and_then(optional_non_empty),
        scope: string_field(value, &["scope", "Scope"]).unwrap_or_else(|| "local".to_string()),
        labels: value
            .get("labels")
            .or_else(|| value.get("Labels"))
            .cloned()
            .unwrap_or_else(|| json!({})),
        options: value
            .get("options")
            .or_else(|| value.get("Options"))
            .cloned()
            .unwrap_or_else(|| json!({})),
        managed_by: string_field(value, &["managedBy", "managed_by", "ManagedBy"])
            .unwrap_or_else(|| "cratebay".to_string()),
    })
}

fn required_name<'a>(value: &'a str, label: &str) -> Result<&'a str, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(format!("{label} is required")));
    }
    Ok(trimmed)
}

fn volume_driver(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("local")
        .to_string()
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToString::to_string)
}

fn optional_non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_volume_payload_maps_to_gui_info() {
        let info = volume_info_from_value(&json!({
            "name": "workspace-cache",
            "driver": "local",
            "mountpoint": "/var/lib/cratebay/volumes/workspace-cache/_data",
            "createdAt": "2026-06-14T00:00:00Z",
            "scope": "local",
            "labels": { "com.cratebay.volume": "true" },
            "options": {},
            "managedBy": "cratebay-engine"
        }))
        .expect("volume should map");

        assert_eq!(info.name, "workspace-cache");
        assert_eq!(info.driver, "local");
        assert_eq!(info.created_at.as_deref(), Some("2026-06-14T00:00:00Z"));
        assert_eq!(info.managed_by, "cratebay-engine");
    }

    #[test]
    fn required_name_rejects_blank_values() {
        assert!(required_name("  ", "Volume name").is_err());
        assert_eq!(required_name(" cache ", "Volume name").unwrap(), "cache");
    }

    #[test]
    fn volume_driver_defaults_to_local() {
        assert_eq!(volume_driver(None), "local");
        assert_eq!(volume_driver(Some("  ")), "local");
        assert_eq!(volume_driver(Some("nfs")), "nfs");
    }
}

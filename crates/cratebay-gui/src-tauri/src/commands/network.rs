//! Network management Tauri commands.

use serde::Serialize;
use serde_json::{json, Value};
use tauri::State;

use crate::state::AppState;
use cratebay_core::error::AppError;
use cratebay_core::runtime;
use cratebay_core::runtime::NativeNetworkSummary;
use cratebay_core::validation;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInfo {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub scope: String,
    pub internal: bool,
    pub attachable: bool,
    pub labels: Value,
    pub containers: Value,
    pub managed_by: String,
}

#[tauri::command]
pub async fn network_list(state: State<'_, AppState>) -> Result<Vec<NetworkInfo>, AppError> {
    ensure_native_engine(&state).await?;
    let payload = runtime::query_built_in_native_networks(state.runtime.as_ref())?;
    let mut networks = payload
        .items
        .into_iter()
        .map(network_info_from_native_summary)
        .collect::<Vec<_>>();
    networks.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(networks)
}

#[tauri::command]
pub async fn network_create(
    state: State<'_, AppState>,
    name: String,
    driver: Option<String>,
    internal: Option<bool>,
    enable_ipv6: Option<bool>,
) -> Result<NetworkInfo, AppError> {
    let name = required_name(&name, "Network name")?;
    let driver = network_driver(driver.as_deref());
    validation::validate_container_name(name)?;
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_network_create(
        state.runtime.as_ref(),
        &json!({
            "name": name,
            "driver": driver,
            "internal": internal.unwrap_or(false),
            "enableIPv6": enable_ipv6.unwrap_or(false),
            "labels": {
                "com.cratebay.managed": "true",
                "com.cratebay.network": "true",
            },
        }),
    )?;
    inspect_native_network(&state, name)
}

#[tauri::command]
pub async fn network_inspect(
    state: State<'_, AppState>,
    id: String,
) -> Result<NetworkInfo, AppError> {
    let id = required_name(&id, "Network name or id")?;
    ensure_native_engine(&state).await?;
    inspect_native_network(&state, id)
}

#[tauri::command]
pub async fn network_delete(
    state: State<'_, AppState>,
    id: String,
    force: Option<bool>,
) -> Result<(), AppError> {
    let id = required_name(&id, "Network name or id")?;
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_network_remove(
        state.runtime.as_ref(),
        id,
        force.unwrap_or(false),
    )?;
    Ok(())
}

async fn ensure_native_engine(state: &AppState) -> Result<(), AppError> {
    state.ensure_native_engine_once().await
}

fn inspect_native_network(state: &AppState, id: &str) -> Result<NetworkInfo, AppError> {
    let payload = runtime::query_built_in_native_network_inspect(state.runtime.as_ref(), id)?;
    network_info_from_value(payload.get("item").unwrap_or(&payload))
}

fn network_info_from_native_summary(summary: NativeNetworkSummary) -> NetworkInfo {
    NetworkInfo {
        id: summary.id,
        name: summary.name,
        driver: summary.driver,
        scope: summary.scope,
        internal: summary.internal,
        attachable: summary.attachable,
        labels: summary.labels,
        containers: summary.containers,
        managed_by: summary.managed_by,
    }
}

fn network_info_from_value(value: &Value) -> Result<NetworkInfo, AppError> {
    Ok(NetworkInfo {
        id: string_field(value, &["id", "Id"]).unwrap_or_default(),
        name: string_field(value, &["name", "Name"]).unwrap_or_default(),
        driver: string_field(value, &["driver", "Driver"]).unwrap_or_else(|| "bridge".to_string()),
        scope: string_field(value, &["scope", "Scope"]).unwrap_or_else(|| "local".to_string()),
        internal: bool_field(value, &["internal", "Internal"]).unwrap_or(false),
        attachable: bool_field(value, &["attachable", "Attachable"]).unwrap_or(true),
        labels: value
            .get("labels")
            .or_else(|| value.get("Labels"))
            .cloned()
            .unwrap_or_else(|| json!({})),
        containers: value
            .get("containers")
            .or_else(|| value.get("Containers"))
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

fn network_driver(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("bridge")
        .to_string()
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToString::to_string)
}

fn bool_field(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_bool))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_network_payload_maps_to_gui_info() {
        let info = network_info_from_value(&json!({
            "id": "net123",
            "name": "sandbox-net",
            "driver": "bridge",
            "scope": "local",
            "internal": false,
            "attachable": true,
            "labels": { "com.cratebay.network": "true" },
            "containers": { "container-1": { "Name": "worker" } },
            "managedBy": "cratebay-engine"
        }))
        .expect("network should map");

        assert_eq!(info.id, "net123");
        assert_eq!(info.name, "sandbox-net");
        assert_eq!(info.driver, "bridge");
        assert!(info.attachable);
        assert_eq!(info.managed_by, "cratebay-engine");
    }

    #[test]
    fn required_name_rejects_blank_values() {
        assert!(required_name("  ", "Network name").is_err());
        assert_eq!(required_name(" net ", "Network name").unwrap(), "net");
    }

    #[test]
    fn network_driver_defaults_to_bridge() {
        assert_eq!(network_driver(None), "bridge");
        assert_eq!(network_driver(Some("  ")), "bridge");
        assert_eq!(network_driver(Some("macvlan")), "macvlan");
    }
}

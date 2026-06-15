//! Pod management Tauri commands.

use tauri::State;

use crate::state::AppState;
use cratebay_core::error::AppError;
use cratebay_core::models::PodInfo;
use cratebay_core::{runtime, validation};
use serde_json::{json, Value};

/// List CrateBay-managed pods.
#[tauri::command]
pub async fn pod_list(state: State<'_, AppState>) -> Result<Vec<PodInfo>, AppError> {
    ensure_native_engine(&state).await?;
    let payload = runtime::query_built_in_native_pods(state.runtime.as_ref())?;
    pods_from_native_payload(payload)
}

/// Create a new pod.
#[tauri::command]
pub async fn pod_create(
    state: State<'_, AppState>,
    name: String,
    driver: Option<String>,
    internal: Option<bool>,
    enable_ipv6: Option<bool>,
) -> Result<PodInfo, AppError> {
    let name = required_name(&name, "Pod name")?;
    let driver = pod_driver(driver.as_deref());
    validation::validate_container_name(name)?;
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_pod_create(
        state.runtime.as_ref(),
        &json!({
            "name": name,
            "driver": driver,
            "internal": internal.unwrap_or(false),
            "enableIPv6": enable_ipv6.unwrap_or(false),
        }),
    )?;
    inspect_native_pod(&state, name)
}

/// Inspect a pod by name or id.
#[tauri::command]
pub async fn pod_inspect(state: State<'_, AppState>, name: String) -> Result<PodInfo, AppError> {
    ensure_native_engine(&state).await?;
    inspect_native_pod(&state, required_name(&name, "Pod name or id")?)
}

/// Delete a pod.
#[tauri::command]
pub async fn pod_delete(
    state: State<'_, AppState>,
    name: String,
    force: Option<bool>,
) -> Result<(), AppError> {
    let name = required_name(&name, "Pod name or id")?;
    ensure_native_engine(&state).await?;
    let force = force.unwrap_or(false);
    runtime::query_built_in_native_pod_remove(state.runtime.as_ref(), name, force)?;
    Ok(())
}

/// Add an existing container to a pod.
#[tauri::command]
pub async fn pod_add_container(
    state: State<'_, AppState>,
    name: String,
    container: String,
) -> Result<(), AppError> {
    let name = required_name(&name, "Pod name")?;
    let container = required_name(&container, "Container id or name")?;
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_pod_attach(state.runtime.as_ref(), name, container)?;
    Ok(())
}

/// Remove a container from a pod.
#[tauri::command]
pub async fn pod_remove_container(
    state: State<'_, AppState>,
    name: String,
    container: String,
    force: Option<bool>,
) -> Result<(), AppError> {
    let name = required_name(&name, "Pod name")?;
    let container = required_name(&container, "Container id or name")?;
    ensure_native_engine(&state).await?;
    runtime::query_built_in_native_pod_detach(
        state.runtime.as_ref(),
        name,
        container,
        force.unwrap_or(false),
    )?;
    Ok(())
}

async fn ensure_native_engine(state: &AppState) -> Result<(), AppError> {
    state.ensure_native_engine_once().await
}

fn inspect_native_pod(state: &AppState, name: &str) -> Result<PodInfo, AppError> {
    let payload = runtime::query_built_in_native_pod_inspect(state.runtime.as_ref(), name)?;
    pod_from_native_payload(payload)
}

fn pods_from_native_payload(payload: Value) -> Result<Vec<PodInfo>, AppError> {
    let mut pods = payload
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(pod_from_native_item)
        .collect::<Result<Vec<_>, _>>()?;
    pods.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(pods)
}

fn pod_from_native_payload(payload: Value) -> Result<PodInfo, AppError> {
    let item = payload
        .get("item")
        .cloned()
        .unwrap_or_else(|| payload.clone());
    pod_from_native_item(item)
}

fn pod_from_native_item(item: Value) -> Result<PodInfo, AppError> {
    serde_json::from_value(item).map_err(AppError::from)
}

fn required_name<'a>(value: &'a str, label: &str) -> Result<&'a str, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(format!("{} is required", label)));
    }
    Ok(trimmed)
}

fn pod_driver(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("bridge")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_pod_list_payload_maps_to_gui_pod_info() {
        let pods = pods_from_native_payload(json!({
            "api": "cratebay.pods.v1",
            "items": [
                {
                    "id": "pod-b",
                    "name": "pod-b",
                    "driver": "bridge",
                    "createdAt": "2026-06-03T00:00:00Z",
                    "labels": { "com.cratebay.pod": "true" },
                    "containers": [
                        {
                            "id": "container-1",
                            "name": "sandbox",
                            "ipv4Address": "10.88.0.2/24",
                            "ipv6Address": null
                        }
                    ]
                },
                {
                    "id": "pod-a",
                    "name": "pod-a",
                    "driver": "bridge",
                    "labels": {},
                    "containers": []
                }
            ]
        }))
        .expect("native pods should map");

        assert_eq!(pods[0].name, "pod-a");
        assert_eq!(pods[1].containers[0].id, "container-1");
        assert_eq!(
            pods[1].containers[0].ipv4_address.as_deref(),
            Some("10.88.0.2/24")
        );
    }

    #[test]
    fn native_pod_inspect_payload_uses_item_field() {
        let pod = pod_from_native_payload(json!({
            "api": "cratebay.pod.inspect.v1",
            "item": {
                "id": "pod-1",
                "name": "pod-1",
                "driver": "bridge",
                "labels": { "com.cratebay.managed": "true" },
                "containers": []
            }
        }))
        .expect("native pod should map");

        assert_eq!(pod.id, "pod-1");
        assert_eq!(
            pod.labels.get("com.cratebay.managed").map(String::as_str),
            Some("true")
        );
    }

    #[test]
    fn required_name_rejects_blank_values() {
        assert!(required_name("  ", "Pod name").is_err());
        assert_eq!(required_name(" pod-a ", "Pod name").unwrap(), "pod-a");
    }

    #[test]
    fn pod_driver_defaults_to_bridge() {
        assert_eq!(pod_driver(None), "bridge");
        assert_eq!(pod_driver(Some("  ")), "bridge");
        assert_eq!(pod_driver(Some("macvlan")), "macvlan");
    }
}

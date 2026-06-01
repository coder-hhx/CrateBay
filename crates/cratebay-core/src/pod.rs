//! Pod/group operations backed by Docker networks.
//!
//! Docker does not have a native Pod abstraction like Kubernetes. CrateBay
//! models a pod as a managed, attachable bridge network with CrateBay labels,
//! which gives the CLI and GUI a stable grouping primitive without adding a
//! second runtime concept.

use bollard::models::Network;
use bollard::network::{
    ConnectNetworkOptions, CreateNetworkOptions, DisconnectNetworkOptions, InspectNetworkOptions,
    ListNetworksOptions,
};
use bollard::Docker;
use std::collections::HashMap;
use std::time::Duration;

use crate::error::AppError;
use crate::models::{PodContainerInfo, PodInfo};
use crate::validation;

pub const POD_LABEL: &str = "com.cratebay.pod";
pub const MANAGED_LABEL: &str = "com.cratebay.managed";

const POD_LABEL_VALUE: &str = "true";
const DOCKER_POD_LIST_TIMEOUT: Duration = Duration::from_secs(8);
const DOCKER_POD_CREATE_TIMEOUT: Duration = Duration::from_secs(20);
const DOCKER_POD_INSPECT_TIMEOUT: Duration = Duration::from_secs(8);
const DOCKER_POD_DELETE_TIMEOUT: Duration = Duration::from_secs(20);
const DOCKER_POD_CONNECT_TIMEOUT: Duration = Duration::from_secs(12);

/// List all CrateBay-managed pods.
pub async fn list(docker: &Docker) -> Result<Vec<PodInfo>, AppError> {
    let mut filters = HashMap::new();
    filters.insert(
        "label".to_string(),
        vec![format!("{}={}", POD_LABEL, POD_LABEL_VALUE)],
    );

    let options = Some(ListNetworksOptions::<String> { filters });
    let networks = tokio::time::timeout(DOCKER_POD_LIST_TIMEOUT, docker.list_networks(options))
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Docker pod list timed out after {:?}",
                DOCKER_POD_LIST_TIMEOUT
            ))
        })??;

    let mut pods = Vec::new();
    for network in networks {
        if let Some(name) = network.name.clone() {
            match inspect(docker, &name).await {
                Ok(pod) => pods.push(pod),
                Err(e) => {
                    tracing::warn!("Failed to inspect pod network '{}': {}", name, e);
                    pods.push(map_network_to_pod(network));
                }
            }
        } else {
            pods.push(map_network_to_pod(network));
        }
    }

    pods.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(pods)
}

/// Create a new pod.
pub async fn create(docker: &Docker, name: &str) -> Result<PodInfo, AppError> {
    let name = required_arg(name, "Pod name")?;
    validation::validate_container_name(name)?;

    let mut labels = HashMap::new();
    labels.insert(MANAGED_LABEL.to_string(), POD_LABEL_VALUE.to_string());
    labels.insert(POD_LABEL.to_string(), POD_LABEL_VALUE.to_string());

    let options = CreateNetworkOptions::<String> {
        name: name.to_string(),
        check_duplicate: true,
        driver: "bridge".to_string(),
        attachable: true,
        labels,
        ..Default::default()
    };

    tokio::time::timeout(DOCKER_POD_CREATE_TIMEOUT, docker.create_network(options))
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Docker pod create timed out after {:?}",
                DOCKER_POD_CREATE_TIMEOUT
            ))
        })??;

    inspect(docker, name).await
}

/// Inspect a pod by name or id.
pub async fn inspect(docker: &Docker, name: &str) -> Result<PodInfo, AppError> {
    let name = required_arg(name, "Pod name or id")?;
    let options = Some(InspectNetworkOptions::<String> {
        verbose: true,
        ..Default::default()
    });

    let network = tokio::time::timeout(
        DOCKER_POD_INSPECT_TIMEOUT,
        docker.inspect_network(name, options),
    )
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker pod inspect timed out after {:?}",
            DOCKER_POD_INSPECT_TIMEOUT
        ))
    })??;

    if !is_cratebay_pod(&network) {
        return Err(AppError::NotFound {
            entity: "pod".to_string(),
            id: name.to_string(),
        });
    }

    Ok(map_network_to_pod(network))
}

/// Delete a pod. Non-empty pods require `force=true`.
pub async fn delete(docker: &Docker, name: &str, force: bool) -> Result<(), AppError> {
    let name = required_arg(name, "Pod name or id")?;
    let pod = inspect(docker, name).await?;

    if !force && !pod.containers.is_empty() {
        return Err(AppError::Validation(format!(
            "Pod '{}' still has {} container(s); use --force to disconnect them",
            pod.name,
            pod.containers.len()
        )));
    }

    if force {
        for container in &pod.containers {
            let options = DisconnectNetworkOptions::<String> {
                container: container.id.clone(),
                force: true,
            };
            match tokio::time::timeout(
                DOCKER_POD_CONNECT_TIMEOUT,
                docker.disconnect_network(name, options),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::warn!(
                    "Failed to disconnect container '{}' from pod '{}': {}",
                    container.id,
                    name,
                    e
                ),
                Err(_) => tracing::warn!(
                    "Timed out disconnecting container '{}' from pod '{}'",
                    container.id,
                    name
                ),
            }
        }
    }

    tokio::time::timeout(DOCKER_POD_DELETE_TIMEOUT, docker.remove_network(name))
        .await
        .map_err(|_| {
            AppError::Runtime(format!(
                "Docker pod delete timed out after {:?}",
                DOCKER_POD_DELETE_TIMEOUT
            ))
        })??;
    Ok(())
}

/// Attach an existing container to a pod.
pub async fn add_container(
    docker: &Docker,
    pod_name: &str,
    container: &str,
) -> Result<(), AppError> {
    let pod_name = required_arg(pod_name, "Pod name or id")?;
    let container = required_arg(container, "Container name or id")?;
    inspect(docker, pod_name).await?;

    let options = ConnectNetworkOptions::<String> {
        container: container.to_string(),
        endpoint_config: Default::default(),
    };

    tokio::time::timeout(
        DOCKER_POD_CONNECT_TIMEOUT,
        docker.connect_network(pod_name, options),
    )
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker pod connect timed out after {:?}",
            DOCKER_POD_CONNECT_TIMEOUT
        ))
    })??;
    Ok(())
}

/// Detach a container from a pod.
pub async fn remove_container(
    docker: &Docker,
    pod_name: &str,
    container: &str,
    force: bool,
) -> Result<(), AppError> {
    let pod_name = required_arg(pod_name, "Pod name or id")?;
    let container = required_arg(container, "Container name or id")?;
    inspect(docker, pod_name).await?;

    let options = DisconnectNetworkOptions::<String> {
        container: container.to_string(),
        force,
    };

    tokio::time::timeout(
        DOCKER_POD_CONNECT_TIMEOUT,
        docker.disconnect_network(pod_name, options),
    )
    .await
    .map_err(|_| {
        AppError::Runtime(format!(
            "Docker pod disconnect timed out after {:?}",
            DOCKER_POD_CONNECT_TIMEOUT
        ))
    })??;
    Ok(())
}

fn is_cratebay_pod(network: &Network) -> bool {
    network
        .labels
        .as_ref()
        .and_then(|labels| labels.get(POD_LABEL))
        .map(|value| value == POD_LABEL_VALUE)
        .unwrap_or(false)
}

fn required_arg<'a>(value: &'a str, label: &str) -> Result<&'a str, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(format!("{} must not be empty", label)));
    }
    Ok(value)
}

fn map_network_to_pod(network: Network) -> PodInfo {
    let mut containers: Vec<PodContainerInfo> = network
        .containers
        .unwrap_or_default()
        .into_iter()
        .map(|(id, container)| PodContainerInfo {
            id,
            name: container.name.unwrap_or_default(),
            ipv4_address: clean_address(container.ipv4_address),
            ipv6_address: clean_address(container.ipv6_address),
        })
        .collect();
    containers.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));

    PodInfo {
        id: network.id.unwrap_or_default(),
        name: network.name.unwrap_or_default(),
        driver: network.driver.unwrap_or_default(),
        created_at: network.created.map(|created| created.to_string()),
        labels: network.labels.unwrap_or_default(),
        containers,
    }
}

fn clean_address(value: Option<String>) -> Option<String> {
    value
        .map(|address| address.trim().to_string())
        .filter(|address| !address.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_address_filters_empty_values() {
        assert_eq!(clean_address(Some("  ".to_string())), None);
        assert_eq!(
            clean_address(Some("172.18.0.2/16".to_string())),
            Some("172.18.0.2/16".to_string())
        );
    }

    #[test]
    fn required_arg_trims_values() {
        assert_eq!(
            required_arg("  demo-pod  ", "Pod name").unwrap(),
            "demo-pod"
        );
    }

    #[test]
    fn required_arg_rejects_blank_values() {
        let err = required_arg("  ", "Pod name").unwrap_err();
        assert!(err.to_string().contains("Pod name must not be empty"));
    }

    #[test]
    fn recognizes_cratebay_pod_label() {
        let mut labels = HashMap::new();
        labels.insert(POD_LABEL.to_string(), POD_LABEL_VALUE.to_string());
        let network = Network {
            labels: Some(labels),
            ..Default::default()
        };

        assert!(is_cratebay_pod(&network));
    }
}

use std::collections::HashMap;

use anyhow::Result;
use bollard::network::{CreateNetworkOptions, InspectNetworkOptions, ListNetworksOptions};
use bollard::Docker;

use super::{print_structured, OutputFormat};

pub async fn create(
    docker: &Docker,
    name: &str,
    driver: Option<String>,
    internal: bool,
    enable_ipv6: bool,
) -> Result<()> {
    let response = docker
        .create_network(CreateNetworkOptions::<String> {
            name: name.to_string(),
            driver: driver.unwrap_or_else(|| "bridge".to_string()),
            internal,
            enable_ipv6,
            attachable: true,
            ingress: false,
            ipam: Default::default(),
            options: HashMap::new(),
            labels: HashMap::new(),
            check_duplicate: true,
        })
        .await?;
    println!("Created network {} ({})", name, response.id);
    Ok(())
}

pub async fn list(docker: &Docker, format: &OutputFormat) -> Result<()> {
    let networks = docker
        .list_networks::<String>(None::<ListNetworksOptions<String>>)
        .await?;

    match format {
        OutputFormat::Table => {
            println!("{:<16} {:<24} {:<12} SCOPE", "ID", "NAME", "DRIVER");
            for network in networks {
                let id = network.id.unwrap_or_default();
                let short_id: String = id.chars().take(12).collect();
                println!(
                    "{:<16} {:<24} {:<12} {}",
                    short_id,
                    network.name.unwrap_or_default(),
                    network.driver.unwrap_or_default(),
                    network.scope.unwrap_or_default(),
                );
            }
            Ok(())
        }
        _ => print_structured(&networks, format),
    }
}

pub async fn inspect(docker: &Docker, id: &str, format: &OutputFormat) -> Result<()> {
    let network = docker
        .inspect_network::<String>(id, None::<InspectNetworkOptions<String>>)
        .await?;
    match format {
        OutputFormat::Table => {
            println!(
                "Network: {}",
                network.name.unwrap_or_else(|| id.to_string())
            );
            println!("Driver: {}", network.driver.unwrap_or_default());
            println!("Scope: {}", network.scope.unwrap_or_default());
            Ok(())
        }
        _ => print_structured(&network, format),
    }
}

pub async fn remove(docker: &Docker, id: &str) -> Result<()> {
    docker.remove_network(id).await?;
    println!("Removed network {}", id);
    Ok(())
}

use std::collections::HashMap;

use anyhow::Result;
use bollard::volume::{CreateVolumeOptions, ListVolumesOptions, RemoveVolumeOptions};
use bollard::Docker;

use super::{print_structured, OutputFormat};

pub async fn create(docker: &Docker, name: &str, driver: Option<String>) -> Result<()> {
    let volume = docker
        .create_volume(CreateVolumeOptions {
            name: name.to_string(),
            driver: driver.unwrap_or_default(),
            driver_opts: HashMap::new(),
            labels: HashMap::new(),
        })
        .await?;
    println!("Created volume {}", volume.name);
    Ok(())
}

pub async fn list(docker: &Docker, format: &OutputFormat) -> Result<()> {
    let response = docker
        .list_volumes::<String>(None::<ListVolumesOptions<String>>)
        .await?;

    match format {
        OutputFormat::Table => {
            println!("{:<32} {:<12} MOUNTPOINT", "NAME", "DRIVER");
            for volume in response.volumes.unwrap_or_default() {
                println!(
                    "{:<32} {:<12} {}",
                    volume.name, volume.driver, volume.mountpoint,
                );
            }
            Ok(())
        }
        _ => print_structured(&response, format),
    }
}

pub async fn inspect(docker: &Docker, name: &str, format: &OutputFormat) -> Result<()> {
    let volume = docker.inspect_volume(name).await?;
    match format {
        OutputFormat::Table => {
            println!("\"Name\": \"{}\"", volume.name);
            println!("\"Driver\": \"{}\"", volume.driver);
            println!("\"Mountpoint\": \"{}\"", volume.mountpoint);
            Ok(())
        }
        _ => print_structured(&volume, format),
    }
}

pub async fn remove(docker: &Docker, name: &str, force: bool) -> Result<()> {
    docker
        .remove_volume(name, Some(RemoveVolumeOptions { force }))
        .await?;
    println!("Removed volume {}", name);
    Ok(())
}

use anyhow::Result;
use bollard::Docker;

use cratebay_core::pod;

use super::{print_structured, OutputFormat};

pub async fn list(docker: &Docker, format: &OutputFormat) -> Result<()> {
    let pods = pod::list(docker).await?;

    match format {
        OutputFormat::Table => {
            println!("{:<20} {:<12} {:<10} CONTAINERS", "NAME", "ID", "DRIVER");
            for p in pods {
                let id = short_id(&p.id);
                println!(
                    "{:<20} {:<12} {:<10} {}",
                    p.name,
                    id,
                    p.driver,
                    p.containers.len()
                );
            }
            Ok(())
        }
        _ => print_structured(&pods, format),
    }
}

pub async fn create(docker: &Docker, name: &str, format: &OutputFormat) -> Result<()> {
    let created = pod::create(docker, name).await?;

    match format {
        OutputFormat::Table => {
            println!("Created pod {} ({})", created.name, short_id(&created.id));
            Ok(())
        }
        _ => print_structured(&created, format),
    }
}

pub async fn inspect(docker: &Docker, name: &str, format: &OutputFormat) -> Result<()> {
    let detail = pod::inspect(docker, name).await?;

    match format {
        OutputFormat::Table => {
            println!("ID: {}", detail.id);
            println!("Name: {}", detail.name);
            println!("Driver: {}", detail.driver);
            println!("Containers: {}", detail.containers.len());
            for container in detail.containers {
                let address = container
                    .ipv4_address
                    .or(container.ipv6_address)
                    .unwrap_or_else(|| "-".to_string());
                println!(
                    "  {} ({}) {}",
                    container.name,
                    short_id(&container.id),
                    address
                );
            }
            Ok(())
        }
        _ => print_structured(&detail, format),
    }
}

pub async fn delete(docker: &Docker, name: &str, force: bool) -> Result<()> {
    pod::delete(docker, name, force).await?;
    println!("Deleted pod {}", name);
    Ok(())
}

pub async fn add(docker: &Docker, name: &str, container: &str) -> Result<()> {
    pod::add_container(docker, name, container).await?;
    println!("Added container {} to pod {}", container, name);
    Ok(())
}

pub async fn remove(docker: &Docker, name: &str, container: &str, force: bool) -> Result<()> {
    pod::remove_container(docker, name, container, force).await?;
    println!("Removed container {} from pod {}", container, name);
    Ok(())
}

fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

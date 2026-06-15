use anyhow::Result;
use bollard::Docker;
use serde_json::{json, Value};

use cratebay_core::{pod, runtime};

use super::{print_structured, OutputFormat};

pub async fn list(format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_pods(runtime.as_ref())?;

    match format {
        OutputFormat::Table => print_native_pod_list(&payload),
        _ => print_structured(&payload, format),
    }
}

pub async fn create(
    name: &str,
    driver: Option<String>,
    internal: bool,
    enable_ipv6: bool,
    format: &OutputFormat,
) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let created = runtime::query_built_in_native_pod_create(
        runtime.as_ref(),
        &json!({
            "name": name,
            "driver": driver,
            "internal": internal,
            "enableIPv6": enable_ipv6,
        }),
    )?;

    match format {
        OutputFormat::Table => {
            println!(
                "Created pod {} ({})",
                created["name"].as_str().unwrap_or(name),
                short_id(created["id"].as_str().unwrap_or(name))
            );
            Ok(())
        }
        _ => print_structured(&created, format),
    }
}

pub async fn inspect(name: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let detail = runtime::query_built_in_native_pod_inspect(runtime.as_ref(), name)?;

    match format {
        OutputFormat::Table => print_native_pod_detail(detail.get("item").unwrap_or(&detail)),
        _ => print_structured(&detail, format),
    }
}

pub async fn delete(name: &str, force: bool, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();

    let deleted = runtime::query_built_in_native_pod_remove(runtime.as_ref(), name, force)?;

    match format {
        OutputFormat::Table => {
            println!("Deleted pod {}", deleted["name"].as_str().unwrap_or(name));
            Ok(())
        }
        _ => print_structured(&deleted, format),
    }
}

pub async fn add(name: &str, container: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let attached = runtime::query_built_in_native_pod_attach(runtime.as_ref(), name, container)?;

    match format {
        OutputFormat::Table => {
            println!(
                "Added container {} to pod {}",
                attached["container"].as_str().unwrap_or(container),
                attached["pod"].as_str().unwrap_or(name)
            );
            Ok(())
        }
        _ => print_structured(&attached, format),
    }
}

pub async fn remove(name: &str, container: &str, force: bool, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let detached =
        runtime::query_built_in_native_pod_detach(runtime.as_ref(), name, container, force)?;

    match format {
        OutputFormat::Table => {
            println!(
                "Removed container {} from pod {}",
                detached["container"].as_str().unwrap_or(container),
                detached["pod"].as_str().unwrap_or(name)
            );
            Ok(())
        }
        _ => print_structured(&detached, format),
    }
}

pub async fn list_compat(docker: &Docker, format: &OutputFormat) -> Result<()> {
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

pub async fn create_compat(docker: &Docker, name: &str, format: &OutputFormat) -> Result<()> {
    let created = pod::create(docker, name).await?;

    match format {
        OutputFormat::Table => {
            println!("Created pod {} ({})", created.name, short_id(&created.id));
            Ok(())
        }
        _ => print_structured(&created, format),
    }
}

pub async fn inspect_compat(docker: &Docker, name: &str, format: &OutputFormat) -> Result<()> {
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

pub async fn delete_compat(docker: &Docker, name: &str, force: bool) -> Result<()> {
    pod::delete(docker, name, force).await?;
    println!("Deleted pod {}", name);
    Ok(())
}

pub async fn add_compat(docker: &Docker, name: &str, container: &str) -> Result<()> {
    pod::add_container(docker, name, container).await?;
    println!("Added container {} to pod {}", container, name);
    Ok(())
}

pub async fn remove_compat(
    docker: &Docker,
    name: &str,
    container: &str,
    force: bool,
) -> Result<()> {
    pod::remove_container(docker, name, container, force).await?;
    println!("Removed container {} from pod {}", container, name);
    Ok(())
}

fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

fn print_native_pod_list(payload: &Value) -> Result<()> {
    let items = payload["items"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if items.is_empty() {
        println!("No CrateBay pods found.");
        return Ok(());
    }

    println!("{:<20} {:<12} {:<10} CONTAINERS", "NAME", "ID", "DRIVER");
    for pod in items {
        println!(
            "{:<20} {:<12} {:<10} {}",
            pod["name"].as_str().unwrap_or("-"),
            short_id(pod["id"].as_str().unwrap_or_default()),
            pod["driver"].as_str().unwrap_or("bridge"),
            pod["containerCount"].as_u64().unwrap_or_default()
        );
    }
    Ok(())
}

#[cfg(test)]
fn native_pod_container_ids(detail: &Value) -> Vec<String> {
    detail["containers"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter_map(|container| {
            let value = container["id"]
                .as_str()
                .or_else(|| container["name"].as_str())
                .unwrap_or_default()
                .trim();
            (!value.is_empty()).then(|| value.to_string())
        })
        .collect()
}

fn print_native_pod_detail(detail: &Value) -> Result<()> {
    println!("ID: {}", detail["id"].as_str().unwrap_or("-"));
    println!("Name: {}", detail["name"].as_str().unwrap_or("-"));
    println!("Driver: {}", detail["driver"].as_str().unwrap_or("bridge"));
    let containers = detail["containers"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    println!("Containers: {}", containers.len());
    for container in containers {
        let address = container["ipv4Address"]
            .as_str()
            .or_else(|| container["ipv6Address"].as_str())
            .unwrap_or("-");
        println!(
            "  {} ({}) {}",
            container["name"].as_str().unwrap_or("-"),
            short_id(container["id"].as_str().unwrap_or_default()),
            address
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::native_pod_container_ids;
    use serde_json::json;

    #[test]
    fn native_force_delete_uses_container_ids_for_detach() {
        let detail = json!({
            "containers": [
                { "id": "abc123", "name": "sandbox-a" },
                { "name": "sandbox-b" },
                { "id": "" }
            ]
        });

        assert_eq!(
            native_pod_container_ids(&detail),
            vec!["abc123".to_string(), "sandbox-b".to_string()]
        );
    }
}

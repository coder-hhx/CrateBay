//! Native CrateBay Engine API commands.

use anyhow::{bail, Result};
use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cratebay_core::runtime;

use super::{print_structured, OutputFormat};

pub async fn status(format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_engine_contract(runtime.as_ref())?;

    match format {
        OutputFormat::Table => {
            println!(
                "Engine: {}",
                payload["name"].as_str().unwrap_or("CrateBay Engine")
            );
            println!(
                "Kind: {}",
                payload["kind"].as_str().unwrap_or("cratebay-containerd")
            );
            println!(
                "API: {}",
                payload["adapter"]["api"]
                    .as_str()
                    .unwrap_or("cratebay.engine.v1")
            );
            if let Some(runtime) = payload["backend"]["runtime"].as_str() {
                println!("Backend runtime: {runtime}");
            }
            if let Some(oci_runtime) = payload["backend"]["ociRuntime"].as_str() {
                println!("OCI runtime: {oci_runtime}");
            }
            if let Some(network_stack) = payload["network"]["stack"].as_str() {
                println!("Network stack: {network_stack}");
            }
            if let Some(namespace) = payload["backend"]["namespace"].as_str() {
                println!("Namespace: {namespace}");
            }
            if let Some(compatible) = payload["compatibility"]["dockerCompatible"].as_bool() {
                println!(
                    "Compatibility API: {}",
                    if compatible { "enabled" } else { "disabled" }
                );
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn substrate(format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_substrate(runtime.as_ref())?;

    match format {
        OutputFormat::Table => {
            println!(
                "Engine: {}",
                payload["engine"].as_str().unwrap_or("CrateBay Engine")
            );
            println!(
                "VM: {}",
                payload["vm"]["runtime"]
                    .as_str()
                    .unwrap_or("cratebay-managed-vm")
            );
            println!(
                "Shim: {} ({})",
                payload["shim"]["manager"]
                    .as_str()
                    .unwrap_or("cratebay-containerd-shim"),
                payload["shim"]["backend"]
                    .as_str()
                    .unwrap_or("containerd task service")
            );
            println!(
                "Network: {} ({})",
                payload["network"]["manager"]
                    .as_str()
                    .unwrap_or("cratebay-cni"),
                payload["network"]["stack"].as_str().unwrap_or("CNI")
            );
            println!(
                "Storage: {} ({} volumes, {} bytes)",
                payload["storage"]["manager"]
                    .as_str()
                    .unwrap_or("cratebay-storage"),
                payload["storage"]["volumeCount"]
                    .as_u64()
                    .unwrap_or_default(),
                payload["storage"]["volumeBytes"]
                    .as_u64()
                    .unwrap_or_default()
            );
            let endpoint = payload["daemon"]["compatibilityEndpoint"]
                .as_str()
                .unwrap_or("CrateBay Engine API shim");
            let daemon_state = if payload["compatibility"]["dockerDaemon"]
                .as_bool()
                .unwrap_or(false)
            {
                "external daemon present"
            } else {
                "API shim only; no Docker daemon"
            };
            println!("Compatibility endpoint: {endpoint} ({daemon_state})");
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn storage_gc(
    apply: bool,
    prune_exited_containers: bool,
    format: &OutputFormat,
) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_storage_gc(
        runtime.as_ref(),
        apply,
        prune_exited_containers,
    )?;

    match format {
        OutputFormat::Table => {
            println!(
                "Storage GC: {}",
                if payload["applied"].as_bool().unwrap_or(false) {
                    "applied"
                } else {
                    "dry run"
                }
            );
            println!(
                "Candidates: {}",
                payload["candidateCount"].as_u64().unwrap_or_default()
            );
            println!(
                "Reclaimable: {} bytes",
                payload["reclaimableBytes"].as_u64().unwrap_or_default()
            );
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn shim_tasks(format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_shim_tasks(runtime.as_ref())?;

    match format {
        OutputFormat::Table => {
            let items = payload["items"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            if items.is_empty() {
                println!("No CrateBay shim tasks found.");
                return Ok(());
            }
            println!("{:<16} {:<24} {:<10} IMAGE", "ID", "NAME", "STATE");
            for item in items {
                let short_id: String = item["id"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(12)
                    .collect();
                println!(
                    "{:<16} {:<24} {:<10} {}",
                    short_id,
                    item["name"].as_str().unwrap_or("-"),
                    item["state"].as_str().unwrap_or("-"),
                    item["image"].as_str().unwrap_or("-")
                );
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn reap_shim_task(id: &str, apply: bool, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_shim_reap_task(runtime.as_ref(), id, apply)?;

    match format {
        OutputFormat::Table => {
            println!(
                "Shim reap: {}",
                if payload["applied"].as_bool().unwrap_or(false) {
                    "applied"
                } else {
                    "dry run"
                }
            );
            println!("Task: {}", payload["name"].as_str().unwrap_or(id));
            println!(
                "Reclaimable: {} bytes",
                payload["reclaimableBytes"].as_u64().unwrap_or_default()
            );
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn containers(format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_containers(runtime.as_ref())?;

    match format {
        OutputFormat::Table => {
            if payload.items.is_empty() {
                println!("No CrateBay containers found.");
                return Ok(());
            }

            println!("{:<16} {:<24} {:<12} IMAGE", "ID", "NAME", "STATE");
            for container in &payload.items {
                let short_id: String = container.id.chars().take(12).collect();
                println!(
                    "{:<16} {:<24} {:<12} {}",
                    short_id, container.name, container.state, container.image
                );
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn images(format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_images(runtime.as_ref())?;

    match format {
        OutputFormat::Table => {
            if payload.items.is_empty() {
                println!("No CrateBay images found.");
                return Ok(());
            }

            println!("{:<16} {:<34} {:>12}", "ID", "IMAGE", "SIZE");
            for image in &payload.items {
                let short_id: String = image
                    .id
                    .trim_start_matches("sha256:")
                    .chars()
                    .take(12)
                    .collect();
                let tag = image
                    .tags
                    .first()
                    .cloned()
                    .unwrap_or_else(|| format!("{}:{}", image.repository, image.tag));
                println!("{:<16} {:<34} {:>12}", short_id, tag, image.size_bytes);
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn pull_image(
    image: String,
    tag: Option<String>,
    mirrors: Vec<String>,
    format: &OutputFormat,
) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let requested = image_reference_with_optional_tag(&image, tag.as_deref());
    let mirrors = normalize_native_registry_mirrors(mirrors);
    let payload = pull_image_native_with_mirrors(runtime.as_ref(), &requested, &mirrors)?;

    match format {
        OutputFormat::Table => {
            println!("Pulled {}", payload["image"].as_str().unwrap_or(&requested));
            if let Some(backend) = payload["backend"].as_str() {
                println!("Backend: {backend}");
            }
            if let Some(image_ref) = payload["imageRef"].as_str() {
                println!("Image ref: {image_ref}");
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

fn pull_image_native_with_mirrors(
    runtime: &dyn runtime::RuntimeManager,
    image: &str,
    mirrors: &[String],
) -> Result<serde_json::Value> {
    if mirrors.is_empty() {
        return Ok(runtime::query_built_in_native_image_pull(
            runtime, image, None,
        )?);
    }

    for (index, mirror) in mirrors.iter().enumerate() {
        let mirror_ref = rewrite_image_for_native_mirror(image, mirror);
        eprintln!("Trying mirror {}/{}: {}", index + 1, mirrors.len(), mirror);
        match runtime::query_built_in_native_image_pull(runtime, &mirror_ref, None) {
            Ok(payload) => {
                if mirror_ref != image {
                    let tag_payload =
                        runtime::query_built_in_native_image_tag(runtime, &mirror_ref, image)?;
                    if let Err(error) =
                        runtime::query_built_in_native_image_remove(runtime, &mirror_ref, true)
                    {
                        tracing::warn!(
                            "Failed to remove native mirror image tag '{}': {}",
                            mirror_ref,
                            error
                        );
                    }
                    return Ok(json!({
                        "api": "cratebay.image.pull.v1",
                        "image": image,
                        "imageRef": tag_payload["targetRef"].clone(),
                        "pulled": true,
                        "backend": payload["backend"].clone(),
                        "mirror": mirror,
                        "mirrorRef": mirror_ref,
                        "sourceImageRef": payload["imageRef"].clone(),
                        "tagged": tag_payload["tagged"].clone(),
                    }));
                }
                return Ok(payload);
            }
            Err(error) => {
                tracing::warn!(
                    "Mirror '{}' failed for native image '{}': {}",
                    mirror,
                    image,
                    error
                );
            }
        }
    }

    eprintln!("All mirrors failed; trying direct pull");
    Ok(runtime::query_built_in_native_image_pull(
        runtime, image, None,
    )?)
}

fn image_reference_with_optional_tag(image: &str, tag: Option<&str>) -> String {
    let image = image.trim();
    match tag.map(str::trim).filter(|tag| !tag.is_empty()) {
        Some(tag) if !image_reference_has_tag(image) => format!("{image}:{tag}"),
        _ => image.to_string(),
    }
}

fn image_reference_has_tag(image: &str) -> bool {
    image
        .rsplit('/')
        .next()
        .map(|tail| tail.contains(':'))
        .unwrap_or(false)
}

fn normalize_native_registry_mirrors(mirrors: Vec<String>) -> Vec<String> {
    mirrors
        .into_iter()
        .map(|mirror| normalize_native_registry_mirror(&mirror))
        .filter(|mirror| !mirror.is_empty())
        .collect()
}

fn normalize_native_registry_mirror(mirror: &str) -> String {
    mirror
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string()
}

fn rewrite_image_for_native_mirror(image: &str, mirror: &str) -> String {
    let image = image.trim();
    let mirror = normalize_native_registry_mirror(mirror);

    if image.is_empty() || mirror.is_empty() {
        return image.to_string();
    }

    if let Some(first_slash_pos) = image.find('/') {
        let before_slash = &image[..first_slash_pos];
        if before_slash.contains('.') || before_slash.contains(':') {
            return image.to_string();
        }
        return format!("{}/{}", mirror, image);
    }

    format!("{}/library/{}", mirror, image)
}

pub async fn inspect_image(id: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_image_inspect(runtime.as_ref(), id)?;

    match format {
        OutputFormat::Table => {
            println!("ID: {}", payload["id"].as_str().unwrap_or(id));
            println!("Image ref: {}", payload["imageRef"].as_str().unwrap_or("-"));
            println!(
                "Backend: {}",
                payload["backend"].as_str().unwrap_or("containerd")
            );
            println!(
                "Size: {}",
                payload["sizeBytes"].as_u64().unwrap_or_default()
            );
            println!("Layers: {}", payload["layers"].as_u64().unwrap_or_default());
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn remove_image(id: &str, force: bool, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_image_remove(runtime.as_ref(), id, force)?;

    match format {
        OutputFormat::Table => {
            println!(
                "Removed image {}",
                payload["imageRef"].as_str().unwrap_or(id)
            );
            if let Some(backend) = payload["backend"].as_str() {
                println!("Backend: {backend}");
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn tag_image(source: &str, target: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_image_tag(runtime.as_ref(), source, target)?;

    match format {
        OutputFormat::Table => {
            println!(
                "Tagged {} as {}",
                payload["source"].as_str().unwrap_or(source),
                payload["target"].as_str().unwrap_or(target)
            );
            if let Some(backend) = payload["backend"].as_str() {
                println!("Backend: {backend}");
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn pack_image(container: &str, image: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload =
        runtime::query_built_in_native_image_pack_container(runtime.as_ref(), container, image)?;

    match format {
        OutputFormat::Table => {
            println!(
                "Packed container {} into {}",
                payload["container"].as_str().unwrap_or(container),
                payload["image"].as_str().unwrap_or(image)
            );
            if let Some(backend) = payload["backend"].as_str() {
                println!("Backend: {backend}");
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn export_images(images: Vec<String>, output: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let archive = runtime::query_built_in_native_image_export(runtime.as_ref(), &images)?;
    fs::write(output, &archive)?;

    match format {
        OutputFormat::Table => {
            println!(
                "Exported {} image(s) to {} ({} bytes)",
                images.len(),
                output,
                archive.len()
            );
            println!("Backend: containerd");
            Ok(())
        }
        _ => print_structured(
            &json!({
                "api": "cratebay.image.export.v1",
                "backend": "containerd",
                "managedBy": "cratebay",
                "images": images,
                "output": output,
                "bytes": archive.len(),
            }),
            format,
        ),
    }
}

pub async fn import_image(input: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload =
        runtime::query_built_in_native_image_import_file(runtime.as_ref(), Path::new(input))?;

    match format {
        OutputFormat::Table => {
            println!("Imported image archive from {}", input);
            if let Some(images) = payload["images"].as_array() {
                for image in images {
                    if let Some(image) = image.as_str() {
                        println!("  {image}");
                    }
                }
            }
            if let Some(backend) = payload["backend"].as_str() {
                println!("Backend: {backend}");
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn networks(format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_networks(runtime.as_ref())?;

    match format {
        OutputFormat::Table => {
            if payload.items.is_empty() {
                println!("No CrateBay networks found.");
                return Ok(());
            }

            println!("{:<16} {:<24} {:<12} SCOPE", "ID", "NAME", "DRIVER");
            for network in &payload.items {
                let short_id: String = network.id.chars().take(12).collect();
                println!(
                    "{:<16} {:<24} {:<12} {}",
                    short_id, network.name, network.driver, network.scope
                );
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn create_network(
    name: String,
    driver: Option<String>,
    internal: bool,
    enable_ipv6: bool,
    format: &OutputFormat,
) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_network_create(
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
                "Created network {} ({})",
                payload["name"].as_str().unwrap_or("-"),
                payload["id"].as_str().unwrap_or("-")
            );
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn inspect_network(id: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_network_inspect(runtime.as_ref(), id)?;

    match format {
        OutputFormat::Table => {
            println!("Network: {}", payload["name"].as_str().unwrap_or(id));
            println!(
                "Backend: {}",
                payload["backend"].as_str().unwrap_or("cratebay-cni")
            );
            if let Some(inspect) = payload["inspect"].as_object() {
                if let Some(ipam) = inspect.get("IPAM") {
                    println!("IPAM: {}", serde_json::to_string(ipam)?);
                }
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn remove_network(id: &str, force: bool, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_network_remove(runtime.as_ref(), id, force)?;

    match format {
        OutputFormat::Table => {
            println!("Removed network {}", payload["id"].as_str().unwrap_or(id));
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn volumes(format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_volumes(runtime.as_ref())?;

    match format {
        OutputFormat::Table => {
            if payload.items.is_empty() {
                println!("No CrateBay volumes found.");
                return Ok(());
            }

            println!("{:<28} {:<10} MOUNTPOINT", "NAME", "DRIVER");
            for volume in &payload.items {
                println!(
                    "{:<28} {:<10} {}",
                    volume.name, volume.driver, volume.mountpoint
                );
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn inspect_volume(name: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_volume_inspect(runtime.as_ref(), name)?;

    match format {
        OutputFormat::Table => {
            println!("Volume: {}", payload["name"].as_str().unwrap_or(name));
            println!(
                "Backend: {}",
                payload["backend"].as_str().unwrap_or("cratebay-storage")
            );
            println!(
                "Size: {} bytes",
                payload["item"]["sizeBytes"].as_u64().unwrap_or_default()
            );
            if let Some(path) = payload["item"]["dataPath"].as_str() {
                println!("Path: {path}");
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn create_volume(
    name: String,
    driver: Option<String>,
    format: &OutputFormat,
) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_volume_create(
        runtime.as_ref(),
        &json!({
            "name": name,
            "driver": driver,
        }),
    )?;

    match format {
        OutputFormat::Table => {
            println!("Created volume {}", payload["name"].as_str().unwrap_or("-"));
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn remove_volume(name: &str, force: bool, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_volume_remove(runtime.as_ref(), name, force)?;

    match format {
        OutputFormat::Table => {
            println!(
                "Removed volume {}",
                payload["name"].as_str().unwrap_or(name)
            );
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn pods(format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_pods(runtime.as_ref())?;

    match format {
        OutputFormat::Table => {
            let items = payload["items"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            if items.is_empty() {
                println!("No CrateBay pods found.");
                return Ok(());
            }

            println!("{:<16} {:<24} {:<12} CONTAINERS", "ID", "NAME", "DRIVER");
            for pod in items {
                let short_id: String = pod["id"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(12)
                    .collect();
                println!(
                    "{:<16} {:<24} {:<12} {}",
                    short_id,
                    pod["name"].as_str().unwrap_or("-"),
                    pod["driver"].as_str().unwrap_or("bridge"),
                    pod["containerCount"].as_u64().unwrap_or_default()
                );
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn create_pod(
    name: String,
    driver: Option<String>,
    internal: bool,
    enable_ipv6: bool,
    format: &OutputFormat,
) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_pod_create(
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
                payload["name"].as_str().unwrap_or("-"),
                payload["id"].as_str().unwrap_or("-")
            );
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn remove_pod(name: &str, force: bool, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_pod_remove(runtime.as_ref(), name, force)?;

    match format {
        OutputFormat::Table => {
            println!("Removed pod {}", payload["name"].as_str().unwrap_or(name));
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    name: String,
    image: String,
    command: Option<String>,
    entrypoint: Option<String>,
    working_dir: Option<String>,
    env: Vec<String>,
    publish: Vec<String>,
    volume: Vec<String>,
    pod: Option<String>,
    network: Option<String>,
    user: Option<String>,
    read_only: bool,
    no_start: bool,
    cpu: Option<f64>,
    memory: Option<u64>,
    registry_mirrors: Vec<String>,
    format: &OutputFormat,
) -> Result<()> {
    ensure_pod_network_exclusive(pod.as_deref(), network.as_deref())?;
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_container_create(
        runtime.as_ref(),
        &json!({
            "name": name,
            "image": image,
            "command": command.clone(),
            "entrypoint": entrypoint,
            "workingDir": working_dir,
            "env": env,
            "publish": publish,
            "volume": volume,
            "pod": pod,
            "network": network,
            "user": user,
            "readOnly": read_only,
            "noStart": no_start,
            "cpu": cpu,
            "memory": memory,
            "registryMirrors": normalize_native_registry_mirrors(registry_mirrors),
        }),
    )?;

    match format {
        OutputFormat::Table => {
            println!(
                "Created {} ({})",
                payload["name"].as_str().unwrap_or("-"),
                payload["id"].as_str().unwrap_or("-")
            );
            if let Some(backend) = payload["backend"].as_str() {
                println!("Backend: {backend}");
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_once(
    name: Option<String>,
    image: String,
    command: Vec<String>,
    env: Vec<String>,
    volume: Vec<String>,
    publish: Vec<String>,
    cpu: Option<u32>,
    memory: Option<u64>,
    working_dir: Option<String>,
    entrypoint: Option<String>,
    pod: Option<String>,
    network: Option<String>,
    user: Option<String>,
    read_only: bool,
    no_pull: bool,
    keep: bool,
    timeout: u64,
    max_output_bytes: u64,
    no_propagate_exit_code: bool,
    registry_mirrors: Vec<String>,
    format: &OutputFormat,
) -> Result<()> {
    ensure_pod_network_exclusive(pod.as_deref(), network.as_deref())?;
    let runtime = runtime::create_runtime_manager();
    let generated_name = format!("cratebay-run-{}", now_millis());
    let name = name.unwrap_or(generated_name);
    let create_payload = runtime::query_built_in_native_container_create(
        runtime.as_ref(),
        &json!({
            "name": name,
            "image": image,
            "command": command,
            "entrypoint": entrypoint,
            "workingDir": working_dir,
            "env": env,
            "publish": publish,
            "volume": volume,
            "pod": pod,
            "network": network,
            "user": user,
            "readOnly": read_only,
            "noPull": no_pull,
            "autoStart": true,
            "cpu": cpu,
            "memory": memory,
            "registryMirrors": normalize_native_registry_mirrors(registry_mirrors),
        }),
    )?;
    let id = create_payload["id"]
        .as_str()
        .unwrap_or_else(|| create_payload["name"].as_str().unwrap_or("cratebay-run"))
        .to_string();
    let wait_timeout = (timeout > 0).then_some(timeout);
    let wait_payload =
        runtime::query_built_in_native_container_wait(runtime.as_ref(), &id, wait_timeout)?;
    let timed_out = wait_payload["timedOut"].as_bool().unwrap_or(false);
    if timed_out {
        let _ = runtime::query_built_in_native_container_stop(runtime.as_ref(), &id, Some(1));
    }
    let logs_payload =
        runtime::query_built_in_native_container_logs(runtime.as_ref(), &id, None, false)?;
    let (stdout, stdout_truncated) = truncate_text(
        logs_payload["stdout"].as_str().unwrap_or_default(),
        max_output_bytes,
    );
    let (stderr, stderr_truncated) = truncate_text(
        logs_payload["stderr"].as_str().unwrap_or_default(),
        max_output_bytes,
    );

    let mut removed = false;
    if !keep {
        removed = runtime::query_built_in_native_container_remove(runtime.as_ref(), &id, true)
            .map(|payload| payload["removed"].as_bool().unwrap_or(true))
            .unwrap_or(false);
    }

    let exit_code = wait_payload["exitCode"].as_i64().unwrap_or(124);
    let result = json!({
        "api": "cratebay.container.run.v1",
        "id": id,
        "name": create_payload["name"],
        "image": create_payload["image"],
        "command": command,
        "backend": "containerd",
        "exitCode": exit_code,
        "stdout": stdout,
        "stderr": stderr,
        "stdoutTruncated": stdout_truncated,
        "stderrTruncated": stderr_truncated,
        "timedOut": timed_out,
        "removed": removed,
    });

    match format {
        OutputFormat::Table => {
            if !stdout.is_empty() {
                print!("{stdout}");
            }
            if !stderr.is_empty() {
                eprint!("{stderr}");
            }
        }
        _ => print_structured(&result, format)?,
    }

    if !no_propagate_exit_code && (timed_out || exit_code != 0) {
        std::process::exit(if timed_out { 124 } else { exit_code as i32 });
    }
    Ok(())
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn ensure_pod_network_exclusive(pod: Option<&str>, network: Option<&str>) -> Result<()> {
    let pod = pod.map(str::trim).filter(|value| !value.is_empty());
    let network = network.map(str::trim).filter(|value| !value.is_empty());
    if pod.is_some() && network.is_some() {
        bail!("Pod and network cannot both be set for one container");
    }
    Ok(())
}

fn truncate_text(value: &str, max_bytes: u64) -> (String, bool) {
    if max_bytes == 0 || value.len() <= max_bytes as usize {
        return (value.to_string(), false);
    }
    let mut end = max_bytes as usize;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_string(), true)
}

pub async fn start(id: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_container_start(runtime.as_ref(), id)?;

    match format {
        OutputFormat::Table => {
            println!("Started {}", payload["id"].as_str().unwrap_or(id));
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn stop(id: &str, timeout: Option<u64>, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_container_stop(runtime.as_ref(), id, timeout)?;

    match format {
        OutputFormat::Table => {
            println!("Stopped {}", payload["id"].as_str().unwrap_or(id));
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn remove(id: &str, force: bool, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_container_remove(runtime.as_ref(), id, force)?;

    match format {
        OutputFormat::Table => {
            println!("Removed {}", payload["id"].as_str().unwrap_or(id));
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn inspect(id: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_container_inspect(runtime.as_ref(), id)?;

    match format {
        OutputFormat::Table => {
            let item = &payload["item"];
            println!("ID: {}", item["id"].as_str().unwrap_or(id));
            println!("Name: {}", item["name"].as_str().unwrap_or("-"));
            println!("Image: {}", item["image"].as_str().unwrap_or("-"));
            println!("State: {}", item["state"]["Status"].as_str().unwrap_or("-"));
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn stats(id: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_container_stats(runtime.as_ref(), id)?;

    match format {
        OutputFormat::Table => {
            println!("ID: {}", payload["id"].as_str().unwrap_or(id));
            println!("Name: {}", payload["name"].as_str().unwrap_or("-"));
            println!("Backend: {}", payload["backend"].as_str().unwrap_or("-"));
            println!(
                "CPU: {:.2}% ({:.3} cores)",
                payload["cpu"]["percent"].as_f64().unwrap_or_default(),
                payload["cpu"]["coresUsed"].as_f64().unwrap_or_default()
            );
            println!(
                "Memory: {:.1} / {:.1} MB ({:.1}%)",
                payload["memory"]["usedMb"].as_f64().unwrap_or_default(),
                payload["memory"]["limitMb"].as_f64().unwrap_or_default(),
                payload["memory"]["percent"].as_f64().unwrap_or_default()
            );
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn logs(
    id: &str,
    tail: Option<u64>,
    timestamps: bool,
    follow: bool,
    format: &OutputFormat,
) -> Result<()> {
    if follow && !matches!(format, OutputFormat::Table) {
        bail!("--follow is only supported with table output");
    }

    let runtime = runtime::create_runtime_manager();
    let follow_tail = if follow { tail.or(Some(100)) } else { tail };
    let payload = runtime::query_built_in_native_container_logs(
        runtime.as_ref(),
        id,
        follow_tail,
        timestamps,
    )?;

    match format {
        OutputFormat::Table => {
            let mut stdout_seen = payload["stdout"].as_str().unwrap_or_default().to_string();
            let mut stderr_seen = payload["stderr"].as_str().unwrap_or_default().to_string();
            print!("{stdout_seen}");
            eprint!("{stderr_seen}");

            if follow {
                loop {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let payload = runtime::query_built_in_native_container_logs(
                        runtime.as_ref(),
                        id,
                        follow_tail,
                        timestamps,
                    )?;
                    let stdout = payload["stdout"].as_str().unwrap_or_default();
                    let stderr = payload["stderr"].as_str().unwrap_or_default();
                    print!("{}", appended_log_delta(&stdout_seen, stdout));
                    eprint!("{}", appended_log_delta(&stderr_seen, stderr));
                    stdout_seen.clear();
                    stdout_seen.push_str(stdout);
                    stderr_seen.clear();
                    stderr_seen.push_str(stderr);
                }
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

fn appended_log_delta<'a>(previous: &str, current: &'a str) -> &'a str {
    if let Some(stripped) = current.strip_prefix(previous) {
        return stripped;
    }

    let mut overlap = previous.len().min(current.len()).min(64 * 1024);
    while overlap > 0 {
        while overlap > 0 && !current.is_char_boundary(overlap) {
            overlap -= 1;
        }
        if previous.ends_with(&current[..overlap]) {
            return &current[overlap..];
        }
        overlap -= 1;
    }

    current
}

pub async fn exec(
    id: &str,
    command: Vec<String>,
    working_dir: Option<String>,
    timeout: Option<u64>,
    max_output_bytes: Option<u64>,
    no_propagate_exit_code: bool,
    format: &OutputFormat,
) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_container_exec(
        runtime.as_ref(),
        id,
        command,
        working_dir,
        timeout,
        max_output_bytes,
    )?;

    match format {
        OutputFormat::Table => {
            if let Some(stdout) = payload["stdout"].as_str() {
                print!("{stdout}");
            }
            if let Some(stderr) = payload["stderr"].as_str() {
                eprint!("{stderr}");
            }
            if payload["exitCode"].as_i64().unwrap_or_default() != 0 {
                eprintln!(
                    "CrateBay Engine exec exited with code {}",
                    payload["exitCode"].as_i64().unwrap_or_default()
                );
            }
            Ok(())
        }
        _ => print_structured(
            &json!({
                "api": payload["api"],
                "id": payload["id"],
                "command": payload["command"],
                "workingDir": payload["workingDir"],
                "backend": payload["backend"],
                "args": payload["args"],
                "exitCode": payload["exitCode"],
                "timedOut": payload["timedOut"],
                "stdoutTruncated": payload["stdoutTruncated"],
                "stderrTruncated": payload["stderrTruncated"],
                "stdout": payload["stdout"],
                "stderr": payload["stderr"],
            }),
            format,
        ),
    }?;

    let exit_code = payload["exitCode"].as_i64().unwrap_or_default();
    let timed_out = payload["timedOut"].as_bool().unwrap_or(false);
    if !no_propagate_exit_code && (timed_out || exit_code != 0) {
        std::process::exit(cli_process_exit_code(exit_code, timed_out));
    }
    Ok(())
}

fn cli_process_exit_code(exit_code: i64, timed_out: bool) -> i32 {
    if timed_out {
        return 124;
    }
    if (0..=255).contains(&exit_code) {
        exit_code as i32
    } else {
        1
    }
}

pub async fn terminal_open(
    id: &str,
    session_id: Option<String>,
    working_dir: Option<String>,
    cols: u16,
    rows: u16,
    command: Vec<String>,
    format: &OutputFormat,
) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let session_id = session_id.unwrap_or_else(|| format!("cratebay-cli-tty-{}", now_millis()));
    let command = (!command.is_empty()).then_some(command);
    let payload = runtime::query_built_in_native_container_terminal_open(
        runtime.as_ref(),
        id,
        &session_id,
        Some(cols),
        Some(rows),
        command,
        working_dir,
    )?;

    match format {
        OutputFormat::Table => {
            println!(
                "Opened terminal session {}",
                payload["sessionId"].as_str().unwrap_or(&session_id)
            );
            println!(
                "Transport: {}",
                payload["transport"].as_str().unwrap_or("-")
            );
            println!(
                "Size: {}x{}",
                payload["cols"].as_u64().unwrap_or(cols as u64),
                payload["rows"].as_u64().unwrap_or(rows as u64)
            );
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn terminal_input(
    id: &str,
    session_id: &str,
    data: &str,
    format: &OutputFormat,
) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_container_terminal_input(
        runtime.as_ref(),
        id,
        session_id,
        data,
    )?;

    match format {
        OutputFormat::Table => {
            println!(
                "Wrote {} bytes",
                payload["bytes"].as_u64().unwrap_or(data.len() as u64)
            );
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn terminal_read(id: &str, session_id: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload =
        runtime::query_built_in_native_container_terminal_read(runtime.as_ref(), id, session_id)?;

    match format {
        OutputFormat::Table => {
            if let Some(chunks) = payload["chunks"].as_array() {
                for chunk in chunks {
                    if let Some(data) = chunk["data"].as_str() {
                        print!("{data}");
                    }
                }
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn terminal_resize(
    id: &str,
    session_id: &str,
    cols: u16,
    rows: u16,
    format: &OutputFormat,
) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload = runtime::query_built_in_native_container_terminal_resize(
        runtime.as_ref(),
        id,
        session_id,
        cols,
        rows,
    )?;

    match format {
        OutputFormat::Table => {
            if payload["resized"].as_bool().unwrap_or(false) {
                println!("Resized terminal to {}x{}", cols, rows);
            } else {
                println!(
                    "Terminal resize was not applied: {}",
                    payload["message"].as_str().unwrap_or("unknown reason")
                );
            }
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

pub async fn terminal_close(id: &str, session_id: &str, format: &OutputFormat) -> Result<()> {
    let runtime = runtime::create_runtime_manager();
    let payload =
        runtime::query_built_in_native_container_terminal_close(runtime.as_ref(), id, session_id)?;

    match format {
        OutputFormat::Table => {
            println!(
                "Closed terminal session {}",
                payload["sessionId"].as_str().unwrap_or(session_id)
            );
            Ok(())
        }
        _ => print_structured(&payload, format),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        appended_log_delta, cli_process_exit_code, ensure_pod_network_exclusive,
        image_reference_with_optional_tag, normalize_native_registry_mirrors,
        rewrite_image_for_native_mirror,
    };

    #[test]
    fn appended_log_delta_prints_only_new_suffix() {
        assert_eq!(appended_log_delta("one\n", "one\ntwo\n"), "two\n");
    }

    #[test]
    fn appended_log_delta_handles_tail_window_overlap() {
        assert_eq!(appended_log_delta("a\nb\nc\n", "c\nd\n"), "d\n");
    }

    #[test]
    fn appended_log_delta_reprints_when_no_overlap_is_available() {
        assert_eq!(appended_log_delta("old\n", "new\n"), "new\n");
    }

    #[test]
    fn native_exec_process_exit_code_matches_run_contract() {
        assert_eq!(cli_process_exit_code(0, false), 0);
        assert_eq!(cli_process_exit_code(42, false), 42);
        assert_eq!(cli_process_exit_code(300, false), 1);
        assert_eq!(cli_process_exit_code(0, true), 124);
    }

    #[test]
    fn image_reference_with_optional_tag_appends_only_when_missing() {
        assert_eq!(
            image_reference_with_optional_tag("alpine", Some("3.20")),
            "alpine:3.20"
        );
        assert_eq!(
            image_reference_with_optional_tag("alpine:latest", Some("3.20")),
            "alpine:latest"
        );
        assert_eq!(
            image_reference_with_optional_tag("registry.example.com:5000/team/app", Some("v1")),
            "registry.example.com:5000/team/app:v1"
        );
    }

    #[test]
    fn pod_and_network_are_mutually_exclusive_for_native_container_payloads() {
        assert!(ensure_pod_network_exclusive(Some("demo-pod"), None).is_ok());
        assert!(ensure_pod_network_exclusive(None, Some("workspace-net")).is_ok());
        assert!(ensure_pod_network_exclusive(Some(" "), Some("workspace-net")).is_ok());
        assert!(ensure_pod_network_exclusive(Some("demo-pod"), Some("workspace-net")).is_err());
    }

    #[test]
    fn native_registry_mirrors_are_normalized_for_containerd_refs() {
        assert_eq!(
            normalize_native_registry_mirrors(vec![
                " https://mirror.example.com/ ".to_string(),
                "http://mirror-2.example.com".to_string(),
                " ".to_string(),
            ]),
            vec![
                "mirror.example.com".to_string(),
                "mirror-2.example.com".to_string()
            ]
        );
    }

    #[test]
    fn native_mirror_rewrite_keeps_docker_hub_rules() {
        assert_eq!(
            rewrite_image_for_native_mirror("node:20-alpine", "https://mirror.example.com/"),
            "mirror.example.com/library/node:20-alpine"
        );
        assert_eq!(
            rewrite_image_for_native_mirror("library/node:20-alpine", "mirror.example.com"),
            "mirror.example.com/library/node:20-alpine"
        );
        assert_eq!(
            rewrite_image_for_native_mirror("myuser/myapp:latest", "mirror.example.com"),
            "mirror.example.com/myuser/myapp:latest"
        );
    }

    #[test]
    fn native_mirror_rewrite_leaves_explicit_registries_unchanged() {
        assert_eq!(
            rewrite_image_for_native_mirror("gcr.io/project/image:tag", "mirror.local"),
            "gcr.io/project/image:tag"
        );
        assert_eq!(
            rewrite_image_for_native_mirror(
                "registry.example.com:5000/team/image:tag",
                "mirror.local"
            ),
            "registry.example.com:5000/team/image:tag"
        );
    }
}

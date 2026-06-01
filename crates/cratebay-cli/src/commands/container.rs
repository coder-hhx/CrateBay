use anyhow::{bail, Result};
use bollard::container::{InspectContainerOptions, LogOutput, LogsOptions};
use bollard::errors::Error as BollardError;
use bollard::Docker;
use futures_util::StreamExt;

use cratebay_core::container;
use cratebay_core::models::{
    ContainerCreateRequest, ContainerRunRequest, LogOptions, PortMapping, VolumeMount,
};
use cratebay_core::{validation, AppError};

use super::{print_structured, OutputFormat};

pub async fn list(docker: &Docker, all: bool, format: &OutputFormat) -> Result<()> {
    let containers = container::list(docker, all, None).await?;

    match format {
        OutputFormat::Table => {
            println!("{:<12} {:<30} {:<12} IMAGE", "ID", "NAME", "STATUS");
            for c in containers {
                let id = c.id.chars().take(12).collect::<String>();
                println!("{:<12} {:<30} {:<12} {}", id, c.name, c.state, c.image);
            }
            Ok(())
        }
        _ => print_structured(&containers, format),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    docker: &Docker,
    name: String,
    image: String,
    cpu_cores: Option<u32>,
    memory_mb: Option<u64>,
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
    format: &OutputFormat,
) -> Result<()> {
    validation::validate_container_name(&name)?;
    if let (Some(cpu), Some(mem)) = (cpu_cores, memory_mb) {
        validation::validate_resource_limits(cpu, mem)?;
    }
    let ports = parse_publish_specs(&publish)?;
    let volumes = parse_volume_specs(&volume)?;

    let request = ContainerCreateRequest {
        name,
        image: image.clone(),
        entrypoint,
        command,
        env: if env.is_empty() { None } else { Some(env) },
        ports: if ports.is_empty() { None } else { Some(ports) },
        volumes: if volumes.is_empty() {
            None
        } else {
            Some(volumes)
        },
        cpu_cores,
        memory_mb,
        working_dir,
        pod,
        network,
        user,
        read_only_rootfs: Some(read_only).filter(|value| *value),
        auto_start: Some(!no_start),
        labels: None,
        template_id: None,
    };

    let created = match container::create(docker, request.clone()).await {
        Ok(info) => info,
        Err(e) if is_missing_image_error(&e) => {
            // Mimic `docker run` behavior: auto-pull missing image then retry.
            eprintln!("Image '{}' not found locally, pulling...", image);
            container::image_pull(docker, &image, None, None).await?;
            container::create(docker, request).await?
        }
        Err(e) => return Err(e.into()),
    };

    match format {
        OutputFormat::Table => {
            println!(
                "Created {} ({})",
                created.name,
                created.id.chars().take(12).collect::<String>()
            );
            Ok(())
        }
        _ => print_structured(&created, format),
    }
}

fn parse_publish_specs(specs: &[String]) -> Result<Vec<PortMapping>> {
    specs.iter().map(|spec| parse_publish_spec(spec)).collect()
}

fn parse_publish_spec(spec: &str) -> Result<PortMapping> {
    let spec = spec.trim();
    if spec.is_empty() {
        bail!("Port mapping cannot be empty");
    }

    let (port_part, protocol) = match spec.rsplit_once('/') {
        Some((ports, protocol)) => (ports, protocol.trim().to_ascii_lowercase()),
        None => (spec, "tcp".to_string()),
    };
    if !matches!(protocol.as_str(), "tcp" | "udp" | "sctp") {
        bail!(
            "Unsupported port protocol '{}'; expected tcp, udp, or sctp",
            protocol
        );
    }

    let parts = port_part.split(':').collect::<Vec<_>>();
    let (host_port, container_port) = match parts.as_slice() {
        [container] => {
            let port = parse_port(container, "container")?;
            (port, port)
        }
        [host, container] => (
            parse_port(host, "host")?,
            parse_port(container, "container")?,
        ),
        _ => bail!(
            "Invalid port mapping '{}'; expected CONTAINER[/proto] or HOST:CONTAINER[/proto]",
            spec
        ),
    };

    Ok(PortMapping {
        host_port,
        container_port,
        protocol,
    })
}

fn parse_port(value: &str, label: &str) -> Result<u16> {
    let port = value
        .trim()
        .parse::<u16>()
        .map_err(|_| anyhow::anyhow!("Invalid {} port '{}'", label, value))?;
    if port == 0 {
        bail!("Invalid {} port '{}'; expected 1-65535", label, value);
    }
    Ok(port)
}

fn parse_volume_specs(specs: &[String]) -> Result<Vec<VolumeMount>> {
    specs.iter().map(|spec| parse_volume_spec(spec)).collect()
}

fn parse_volume_spec(spec: &str) -> Result<VolumeMount> {
    let spec = spec.trim();
    if spec.is_empty() {
        bail!("Volume mount cannot be empty");
    }

    let parts = spec.split(':').collect::<Vec<_>>();
    let (host_path, container_path, read_only) = match parts.as_slice() {
        [host, container] => (*host, *container, None),
        [host, container, mode] => match mode.trim() {
            "ro" => (*host, *container, Some(true)),
            "rw" => (*host, *container, Some(false)),
            other => bail!(
                "Invalid volume mode '{}'; expected ro or rw in host:container[:ro|rw]",
                other
            ),
        },
        _ => bail!(
            "Invalid volume mount '{}'; expected host:container[:ro|rw]",
            spec
        ),
    };

    let host_path = host_path.trim();
    let container_path = container_path.trim();
    if host_path.is_empty() || container_path.is_empty() {
        bail!("Volume mount must include both host and container paths");
    }
    if !container_path.starts_with('/') {
        bail!("Container mount path '{}' must be absolute", container_path);
    }

    Ok(VolumeMount {
        host_path: host_path.to_string(),
        container_path: container_path.to_string(),
        read_only,
    })
}

pub async fn start(docker: &Docker, id: &str) -> Result<()> {
    container::start(docker, id).await?;
    println!("Started {}", id);
    Ok(())
}

pub async fn stop(docker: &Docker, id: &str, timeout: Option<u32>) -> Result<()> {
    container::stop(docker, id, timeout).await?;
    println!("Stopped {}", id);
    Ok(())
}

pub async fn delete(docker: &Docker, id: &str, force: bool) -> Result<()> {
    container::delete(docker, id, force).await?;
    println!("Deleted {}", id);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn exec(
    docker: &Docker,
    id: &str,
    cmd: Vec<String>,
    working_dir: Option<String>,
    timeout: Option<u64>,
    max_output_bytes: u64,
    no_propagate_exit_code: bool,
    format: &OutputFormat,
) -> Result<()> {
    let max_output_bytes = (max_output_bytes > 0).then_some(max_output_bytes);
    let result = container::exec_with_output_limit(
        docker,
        id,
        cmd,
        working_dir,
        timeout.filter(|secs| *secs > 0),
        max_output_bytes,
    )
    .await?;
    match format {
        OutputFormat::Table => {
            if !result.stdout.is_empty() {
                print!("{}", result.stdout);
            }
            if !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }
            if result.stdout_truncated || result.stderr_truncated {
                eprintln!("Output truncated; use --max-output-bytes 0 to disable");
            }
        }
        _ => {
            print_structured(&result, format)?;
        }
    }

    std::process::exit(cli_process_exit_code(
        result.exit_code,
        result.timed_out,
        no_propagate_exit_code,
    ));
}

pub async fn logs(
    docker: &Docker,
    id: &str,
    follow: bool,
    tail: Option<u32>,
    timestamps: bool,
) -> Result<()> {
    if follow {
        let log_options = LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            tail: tail.unwrap_or(100).to_string(),
            timestamps,
            ..Default::default()
        };

        let mut stream = docker.logs(id, Some(log_options));
        while let Some(chunk) = stream.next().await {
            match chunk? {
                LogOutput::StdOut { message } => {
                    print!("{}", String::from_utf8_lossy(&message));
                }
                LogOutput::StdErr { message } => {
                    eprint!("{}", String::from_utf8_lossy(&message));
                }
                _ => {}
            }
        }

        return Ok(());
    }

    let options = LogOptions {
        tail,
        timestamps: Some(timestamps),
        ..Default::default()
    };
    let entries = container::logs(docker, id, Some(options)).await?;
    for entry in entries {
        match entry.stream.as_str() {
            "stderr" => eprint!("{}", entry.message),
            _ => print!("{}", entry.message),
        }
    }
    Ok(())
}

pub async fn inspect(docker: &Docker, id: &str, format: &OutputFormat) -> Result<()> {
    let detail = container::inspect(docker, id).await?;
    match format {
        OutputFormat::Table => {
            println!("ID: {}", detail.info.id);
            println!("Name: {}", detail.info.name);
            println!("Image: {}", detail.info.image);
            println!("State: {}", detail.info.state);
            println!("Status: {:?}", detail.info.status);
            Ok(())
        }
        _ => print_structured(&detail, format),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_once(
    docker: &Docker,
    name: Option<String>,
    image: String,
    command: Vec<String>,
    env: Vec<String>,
    volume: Vec<String>,
    cpu_cores: Option<u32>,
    memory_mb: Option<u64>,
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
    format: &OutputFormat,
) -> Result<()> {
    let volumes = parse_volume_specs(&volume)?;
    let max_output_bytes = (max_output_bytes > 0).then_some(max_output_bytes);
    let request = ContainerRunRequest {
        name,
        image,
        entrypoint,
        command,
        env: if env.is_empty() { None } else { Some(env) },
        volumes: if volumes.is_empty() {
            None
        } else {
            Some(volumes)
        },
        cpu_cores,
        memory_mb,
        working_dir,
        pod,
        network,
        user,
        read_only_rootfs: Some(read_only).filter(|value| *value),
        pull: !no_pull,
        remove: !keep,
        timeout_secs: Some(timeout),
        max_output_bytes,
    };

    let result = match container::run_once(docker, request.clone()).await {
        Ok(result) => result,
        Err(e) if !request.pull && is_missing_image_error(&e) => {
            return Err(anyhow::anyhow!(
                "Image '{}' was not found locally. Re-run without --no-pull to pull it automatically.",
                request.image
            ));
        }
        Err(e) => return Err(e.into()),
    };

    match format {
        OutputFormat::Table => {
            if !result.stdout.is_empty() {
                print!("{}", result.stdout);
            }
            if !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }
        }
        _ => print_structured(&result, format)?,
    }

    std::process::exit(cli_process_exit_code(
        result.exit_code,
        result.timed_out,
        no_propagate_exit_code,
    ));
}

pub async fn run_compat(
    docker: &Docker,
    name: String,
    image: String,
    env: Vec<String>,
) -> Result<()> {
    create(
        docker,
        name,
        image,
        None,
        None,
        None,
        None,
        None,
        env,
        Vec::new(),
        Vec::new(),
        None,
        None,
        None,
        false,
        false,
        &OutputFormat::Table,
    )
    .await
}

pub async fn ps_compat(docker: &Docker) -> Result<()> {
    list(docker, false, &OutputFormat::Table).await
}

pub async fn print_env(docker: &Docker, id: &str) -> Result<()> {
    let detail = docker
        .inspect_container(id, Some(InspectContainerOptions { size: false }))
        .await?;
    for env_entry in detail
        .config
        .and_then(|config| config.env)
        .unwrap_or_default()
    {
        println!("{}", env_entry);
    }
    Ok(())
}

pub fn print_login_cmd(id: &str) {
    println!("docker exec -it {} /bin/sh", id);
}

pub async fn start_compat(docker: &Docker, id: &str) -> Result<()> {
    container::start(docker, id).await?;
    println!("Started container {}", id);
    Ok(())
}

pub async fn stop_compat(docker: &Docker, id: &str) -> Result<()> {
    container::stop(docker, id, None).await?;
    println!("Stopped container {}", id);
    Ok(())
}

pub async fn rm_compat(docker: &Docker, id: &str, _force: bool) -> Result<()> {
    container::delete(docker, id, true).await?;
    println!("Removed container {}", id);
    Ok(())
}

fn process_exit_code(exit_code: i64, timed_out: bool) -> i32 {
    if timed_out {
        return 124;
    }
    if (0..=255).contains(&exit_code) {
        exit_code as i32
    } else {
        1
    }
}

fn cli_process_exit_code(exit_code: i64, timed_out: bool, no_propagate_exit_code: bool) -> i32 {
    if no_propagate_exit_code {
        0
    } else {
        process_exit_code(exit_code, timed_out)
    }
}

fn is_missing_image_error(err: &AppError) -> bool {
    match err {
        AppError::Docker(BollardError::DockerResponseServerError {
            status_code,
            message,
        }) if *status_code == 404
            || message.contains("No such image")
            || message.to_ascii_lowercase().contains("not found") =>
        {
            true
        }
        _ => err.to_string().contains("No such image"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_volume_spec_accepts_read_only_mount() {
        let volume = parse_volume_spec("/host/path:/workspace:ro").unwrap();

        assert_eq!(volume.host_path, "/host/path");
        assert_eq!(volume.container_path, "/workspace");
        assert_eq!(volume.read_only, Some(true));
    }

    #[test]
    fn parse_volume_spec_rejects_relative_container_path() {
        let err = parse_volume_spec("/host/path:workspace").unwrap_err();

        assert!(
            err.to_string().contains("must be absolute"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_publish_spec_defaults_to_tcp() {
        let port = parse_publish_spec("8080:80").unwrap();

        assert_eq!(port.host_port, 8080);
        assert_eq!(port.container_port, 80);
        assert_eq!(port.protocol, "tcp");
    }

    #[test]
    fn process_exit_code_uses_timeout_code() {
        assert_eq!(process_exit_code(0, true), 124);
    }

    #[test]
    fn process_exit_code_preserves_container_exit_status() {
        assert_eq!(process_exit_code(0, false), 0);
        assert_eq!(process_exit_code(42, false), 42);
        assert_eq!(process_exit_code(255, false), 255);
    }

    #[test]
    fn process_exit_code_clamps_invalid_container_exit_status() {
        assert_eq!(process_exit_code(-1, false), 1);
        assert_eq!(process_exit_code(256, false), 1);
    }

    #[test]
    fn cli_process_exit_code_can_leave_completed_command_successful_for_callers() {
        assert_eq!(cli_process_exit_code(66, false, true), 0);
        assert_eq!(cli_process_exit_code(124, true, true), 0);
        assert_eq!(cli_process_exit_code(66, false, false), 66);
        assert_eq!(cli_process_exit_code(124, true, false), 124);
    }
}

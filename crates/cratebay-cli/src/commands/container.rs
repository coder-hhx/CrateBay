use anyhow::Result;
use bollard::container::{InspectContainerOptions, LogOutput, LogsOptions};
use bollard::errors::Error as BollardError;
use bollard::Docker;
use futures_util::StreamExt;

use cratebay_core::container;
use cratebay_core::models::{ContainerCreateRequest, LogOptions};
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
    working_dir: Option<String>,
    env: Vec<String>,
    no_start: bool,
    format: &OutputFormat,
) -> Result<()> {
    validation::validate_container_name(&name)?;
    if let (Some(cpu), Some(mem)) = (cpu_cores, memory_mb) {
        validation::validate_resource_limits(cpu, mem)?;
    }

    let request = ContainerCreateRequest {
        name,
        image: image.clone(),
        command,
        env: if env.is_empty() { None } else { Some(env) },
        ports: None,
        volumes: None,
        cpu_cores,
        memory_mb,
        working_dir,
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

pub async fn exec(
    docker: &Docker,
    id: &str,
    cmd: Vec<String>,
    working_dir: Option<String>,
    format: &OutputFormat,
) -> Result<()> {
    let result = container::exec(docker, id, cmd, working_dir).await?;
    match format {
        OutputFormat::Table => {
            if !result.stdout.is_empty() {
                print!("{}", result.stdout);
            }
            if !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }
            std::process::exit(result.exit_code as i32);
        }
        _ => {
            print_structured(&result, format)?;
            Ok(())
        }
    }
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
        env,
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

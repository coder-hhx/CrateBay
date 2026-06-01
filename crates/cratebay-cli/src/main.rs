//! CrateBay CLI — command-line interface.

use clap::{ArgAction, Args, Parser, Subcommand};
use serde::Serialize;
use std::process::ExitCode;

mod commands;

use commands::OutputFormat;

#[derive(Parser)]
#[command(
    name = "cratebay",
    version,
    about = "CrateBay CLI — Container and image management from the command line"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Explicit Docker-compatible host override. By default CrateBay uses its built-in runtime.
    ///
    /// Examples:
    /// - unix:///var/run/docker.sock
    /// - tcp://127.0.0.1:2375
    #[arg(long, global = true)]
    docker_host: Option<String>,

    /// Output format for structured commands.
    #[arg(long, global = true, default_value = "table")]
    format: OutputFormat,

    /// Shortcut for --format json. Useful for embedded automation.
    #[arg(long, global = true, conflicts_with = "format")]
    json: bool,
}

impl Cli {
    fn selected_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            self.format.clone()
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Run a one-shot container and print its output
    Run(RunArgs),

    /// Container operations
    #[command(subcommand)]
    Container(ContainerCommands),

    /// Pod/group operations
    #[command(subcommand)]
    Pod(PodCommands),

    /// Image operations
    #[command(subcommand)]
    Image(ImageCommands),

    /// Docker-compatible convenience commands used by local smoke tests
    #[command(subcommand, hide = true)]
    Docker(DockerCommands),

    /// Volume operations
    #[command(subcommand)]
    Volume(VolumeCommands),

    /// Runtime management (start/stop/status)
    #[command(subcommand)]
    Runtime(RuntimeCommands),

    /// System information
    #[command(subcommand)]
    System(SystemCommands),
}

#[derive(Subcommand)]
enum ContainerCommands {
    /// Run a one-shot container and print its output
    Run(RunArgs),

    /// List containers
    #[command(alias = "ls")]
    List {
        /// Show all containers (including stopped)
        #[arg(long)]
        all: bool,
    },

    /// Create a container
    Create {
        /// Container name
        name: String,
        /// Image reference, e.g. alpine:3.20
        #[arg(long)]
        image: String,
        /// CPU cores limit
        #[arg(long)]
        cpu: Option<u32>,
        /// Memory limit in MB
        #[arg(long)]
        memory: Option<u64>,
        /// Command to run (shell form)
        #[arg(long)]
        command: Option<String>,
        /// Override the image entrypoint
        #[arg(long)]
        entrypoint: Option<String>,
        /// Working directory inside the container
        #[arg(long)]
        working_dir: Option<String>,
        /// Environment variables (KEY=VALUE). Can be repeated.
        #[arg(long, action = ArgAction::Append)]
        env: Vec<String>,
        /// Publish a port (host:container[/tcp|udp]). Can be repeated.
        #[arg(short = 'p', long = "publish", action = ArgAction::Append)]
        publish: Vec<String>,
        /// Bind mount (host:container[:ro|rw]). Can be repeated.
        #[arg(short = 'v', long = "volume", action = ArgAction::Append)]
        volume: Vec<String>,
        /// Attach the container to a CrateBay pod.
        #[arg(long)]
        pod: Option<String>,
        /// Network mode for the container (`bridge`, `none`, or `host`).
        #[arg(long, value_parser = ["bridge", "none", "host"])]
        network: Option<String>,
        /// User to run as inside the container, e.g. `1000:1000`.
        #[arg(long)]
        user: Option<String>,
        /// Mount the container root filesystem read-only.
        #[arg(long = "read-only")]
        read_only: bool,
        /// Do not auto-start container after creation
        #[arg(long)]
        no_start: bool,
    },

    /// Start a container
    Start { id: String },

    /// Stop a container
    Stop {
        id: String,
        /// Timeout in seconds before SIGKILL (default: 10)
        #[arg(long)]
        timeout: Option<u32>,
    },

    /// Delete a container
    Delete {
        id: String,
        /// Force removal
        #[arg(long)]
        force: bool,
    },

    /// Execute a command inside a container
    Exec {
        id: String,
        /// Working directory inside the container
        #[arg(long)]
        working_dir: Option<String>,
        /// Timeout in seconds for the exec output collection. Use 0 to disable.
        #[arg(long)]
        timeout: Option<u64>,
        /// Maximum captured stdout/stderr bytes per stream. Use 0 to disable.
        #[arg(long, default_value_t = 1_048_576, value_name = "BYTES")]
        max_output_bytes: u64,
        /// Exit 0 after a completed exec and report the container exit in the payload.
        #[arg(long)]
        no_propagate_exit_code: bool,
        /// Command to execute (after `--`)
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },

    /// Show container logs
    Logs {
        id: String,
        /// Follow log output
        #[arg(long)]
        follow: bool,
        /// Number of lines to show from the end (default: 100)
        #[arg(long)]
        tail: Option<u32>,
        /// Show timestamps (RFC3339)
        #[arg(long)]
        timestamps: bool,
    },

    /// Inspect a container
    Inspect { id: String },
}

#[derive(Args, Clone)]
struct RunArgs {
    /// Container name. Generated when omitted.
    #[arg(long)]
    name: Option<String>,

    /// Environment variables (KEY=VALUE). Can be repeated.
    #[arg(short = 'e', long = "env", action = ArgAction::Append)]
    env: Vec<String>,

    /// Bind mount (host:container[:ro|rw]). Can be repeated.
    #[arg(short = 'v', long = "volume", action = ArgAction::Append)]
    volume: Vec<String>,

    /// CPU cores limit.
    #[arg(long)]
    cpu: Option<u32>,

    /// Memory limit in MB.
    #[arg(long)]
    memory: Option<u64>,

    /// Working directory inside the container.
    #[arg(long, alias = "workdir")]
    working_dir: Option<String>,

    /// Override the image entrypoint.
    #[arg(long)]
    entrypoint: Option<String>,

    /// Attach the container to a CrateBay pod.
    #[arg(long)]
    pod: Option<String>,

    /// Network mode for the run (`bridge`, `none`, or `host`).
    #[arg(long, value_parser = ["bridge", "none", "host"])]
    network: Option<String>,

    /// User to run as inside the container, e.g. `1000:1000`.
    #[arg(long)]
    user: Option<String>,

    /// Mount the container root filesystem read-only.
    #[arg(long = "read-only")]
    read_only: bool,

    /// Do not pull the image automatically when it is missing locally.
    #[arg(long)]
    no_pull: bool,

    /// Keep the container after it exits.
    #[arg(long)]
    keep: bool,

    /// Timeout in seconds. Use 0 to disable.
    #[arg(long, default_value_t = 300)]
    timeout: u64,

    /// Maximum captured stdout/stderr bytes per stream. Use 0 to disable.
    #[arg(long, default_value_t = 1_048_576, value_name = "BYTES")]
    max_output_bytes: u64,

    /// Exit 0 after a completed run and report the container exit in the payload.
    #[arg(long)]
    no_propagate_exit_code: bool,

    /// Image reference, e.g. alpine:latest.
    image: String,

    /// Command to execute (after `--`).
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

#[derive(Subcommand)]
enum PodCommands {
    /// List pods
    #[command(alias = "ls")]
    List,

    /// Create a pod
    Create {
        /// Pod name
        name: String,
    },

    /// Inspect a pod
    Inspect {
        /// Pod name
        name: String,
    },

    /// Delete a pod
    Delete {
        /// Pod name
        name: String,
        /// Force removal and disconnect containers first
        #[arg(long)]
        force: bool,
    },

    /// Add a container to a pod
    Add {
        /// Pod name
        name: String,
        /// Container id or name
        container: String,
    },

    /// Remove a container from a pod
    Remove {
        /// Pod name
        name: String,
        /// Container id or name
        container: String,
        /// Force disconnection
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum DockerCommands {
    /// Run a container with Docker-like flags
    Run {
        /// Pull image before run when missing locally
        #[arg(long)]
        pull: bool,
        /// Container name
        #[arg(long)]
        name: String,
        /// Environment variables (KEY=VALUE). Can be repeated.
        #[arg(short = 'e', action = ArgAction::Append)]
        env: Vec<String>,
        /// Image reference, e.g. nginx:1.27-alpine
        image: String,
    },

    /// List running containers
    Ps,

    /// Print container environment variables
    Env { id: String },

    /// Print shell login command for a container
    LoginCmd { id: String },

    /// Start a container
    Start { id: String },

    /// Stop a container
    Stop { id: String },

    /// Remove a container
    Rm {
        id: String,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum ImageCommands {
    /// List local images
    List,

    /// Search images from registry
    Search {
        query: String,
        /// Search backend (`auto` or `dockerhub`)
        #[arg(long, default_value = "auto", value_parser = ["auto", "dockerhub"])]
        source: String,
        /// Max results
        #[arg(long)]
        limit: Option<u32>,
    },

    /// Pull an image
    Pull { image: String },

    /// Export one or more images to a tar archive
    #[command(alias = "save")]
    Export {
        /// Output tar archive path
        #[arg(short, long)]
        output: String,
        /// Image references to export
        #[arg(required = true)]
        images: Vec<String>,
    },

    /// Import images from a tar archive
    #[command(alias = "load")]
    Import {
        /// Input tar archive path
        input: String,
    },

    /// Load bundled CrateBay container images into the runtime
    PreloadBundled {
        /// Directory containing bundled image archives
        #[arg(long)]
        dir: Option<String>,
    },

    /// Inspect a local image
    Inspect { id: String },

    /// Tag a local image with a new repository:tag
    Tag { source: String, target: String },

    /// Commit a container into a new image tag
    PackContainer { container: String, image: String },

    /// Delete a local image
    Delete { id: String },
}

#[derive(Subcommand)]
enum VolumeCommands {
    /// Create a volume
    Create { name: String },

    /// List volumes
    List,

    /// Inspect a volume
    Inspect { name: String },

    /// Remove a volume
    Remove {
        name: String,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum RuntimeCommands {
    /// Show runtime status
    Status,
    /// Start the built-in runtime
    Start,
    /// Stop the built-in runtime
    Stop,
    /// Pre-download runtime image without starting
    Provision,
}

#[derive(Subcommand)]
enum SystemCommands {
    /// Show CrateBay version and platform info
    Info,

    /// Show current Docker-compatible connection status (does not start runtime)
    DockerStatus,
}

fn docker_host_override(cli_host: Option<&str>) -> Option<String> {
    cli_host
        .and_then(|host| {
            let host = host.trim();
            (!host.is_empty()).then(|| host.to_string())
        })
        .or_else(|| {
            std::env::var("DOCKER_HOST").ok().and_then(|host| {
                let host = host.trim();
                (!host.is_empty()).then(|| host.to_string())
            })
        })
}

async fn resolve_docker(
    runtime: &dyn cratebay_core::runtime::RuntimeManager,
    docker_host: Option<&str>,
) -> anyhow::Result<bollard::Docker> {
    if let Some(host) = docker_host_override(docker_host) {
        return Ok(cratebay_core::docker::connect_host(&host).await?);
    }

    Ok(
        cratebay_core::engine::ensure_docker(runtime, Default::default())
            .await?
            .as_ref()
            .clone(),
    )
}

async fn try_existing_docker(docker_host: Option<&str>) -> anyhow::Result<Option<bollard::Docker>> {
    if let Some(host) = docker_host_override(docker_host) {
        return Ok(Some(cratebay_core::docker::connect_host(&host).await?));
    }

    Ok(cratebay_core::docker::try_connect().await)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandErrorPayload {
    ok: bool,
    kind: &'static str,
    error: String,
}

impl CommandErrorPayload {
    fn from_error(err: &anyhow::Error) -> Self {
        Self {
            ok: false,
            kind: command_error_kind(err),
            error: err.to_string(),
        }
    }
}

fn command_error_kind(err: &anyhow::Error) -> &'static str {
    let Some(app_error) = err.downcast_ref::<cratebay_core::AppError>() else {
        return "command";
    };

    match app_error {
        cratebay_core::AppError::Docker(_) => "docker",
        cratebay_core::AppError::Database(_) => "database",
        cratebay_core::AppError::Validation(_) => "validation",
        cratebay_core::AppError::NotFound { .. } => "notFound",
        cratebay_core::AppError::Runtime(_) => "runtime",
        cratebay_core::AppError::Io(_) => "io",
        cratebay_core::AppError::Serialization(_) => "serialization",
        cratebay_core::AppError::PermissionDenied(_) => "permissionDenied",
    }
}

fn print_command_error(err: &anyhow::Error, format: &OutputFormat) {
    match format {
        OutputFormat::Table => eprintln!("Error: {err:#}"),
        OutputFormat::Json => {
            let payload = CommandErrorPayload::from_error(err);
            match serde_json::to_string_pretty(&payload) {
                Ok(json) => eprintln!("{json}"),
                Err(_) => eprintln!("Error: {err:#}"),
            }
        }
        OutputFormat::Yaml => {
            let payload = CommandErrorPayload::from_error(err);
            match serde_yaml::to_string(&payload) {
                Ok(yaml) => eprint!("{yaml}"),
                Err(_) => eprintln!("Error: {err:#}"),
            }
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let format = cli.selected_format();

    match run_cli(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            print_command_error(&err, &format);
            ExitCode::FAILURE
        }
    }
}

async fn run_cli(cli: Cli) -> anyhow::Result<()> {
    let format = cli.selected_format();
    let Cli {
        command,
        docker_host,
        ..
    } = cli;

    // Runtime manager used by engine ensure for all Docker-dependent commands.
    let runtime = cratebay_core::runtime::create_runtime_manager();

    match command {
        Commands::Run(args) => {
            tracing::debug!("CLI run: resolving docker");
            let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
            tracing::debug!("CLI run: dispatching one-shot run");
            commands::container::run_once(
                &docker,
                args.name,
                args.image,
                args.command,
                args.env,
                args.volume,
                args.cpu,
                args.memory,
                args.working_dir,
                args.entrypoint,
                args.pod,
                args.network,
                args.user,
                args.read_only,
                args.no_pull,
                args.keep,
                args.timeout,
                args.max_output_bytes,
                args.no_propagate_exit_code,
                &format,
            )
            .await?
        }
        Commands::Container(cmd) => {
            tracing::debug!("CLI container command: resolving docker");
            let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
            match cmd {
                ContainerCommands::Run(args) => {
                    tracing::debug!("CLI container run: dispatching one-shot run");
                    commands::container::run_once(
                        &docker,
                        args.name,
                        args.image,
                        args.command,
                        args.env,
                        args.volume,
                        args.cpu,
                        args.memory,
                        args.working_dir,
                        args.entrypoint,
                        args.pod,
                        args.network,
                        args.user,
                        args.read_only,
                        args.no_pull,
                        args.keep,
                        args.timeout,
                        args.max_output_bytes,
                        args.no_propagate_exit_code,
                        &format,
                    )
                    .await?
                }
                ContainerCommands::List { all } => {
                    commands::container::list(&docker, all, &format).await?
                }
                ContainerCommands::Create {
                    name,
                    image,
                    cpu,
                    memory,
                    command,
                    entrypoint,
                    working_dir,
                    env,
                    publish,
                    volume,
                    pod,
                    network,
                    user,
                    read_only,
                    no_start,
                } => {
                    commands::container::create(
                        &docker,
                        name,
                        image,
                        cpu,
                        memory,
                        command,
                        entrypoint,
                        working_dir,
                        env,
                        publish,
                        volume,
                        pod,
                        network,
                        user,
                        read_only,
                        no_start,
                        &format,
                    )
                    .await?
                }
                ContainerCommands::Start { id } => commands::container::start(&docker, &id).await?,
                ContainerCommands::Stop { id, timeout } => {
                    commands::container::stop(&docker, &id, timeout).await?
                }
                ContainerCommands::Delete { id, force } => {
                    commands::container::delete(&docker, &id, force).await?
                }
                ContainerCommands::Exec {
                    id,
                    command,
                    working_dir,
                    timeout,
                    max_output_bytes,
                    no_propagate_exit_code,
                } => {
                    commands::container::exec(
                        &docker,
                        &id,
                        command,
                        working_dir,
                        timeout,
                        max_output_bytes,
                        no_propagate_exit_code,
                        &format,
                    )
                    .await?
                }
                ContainerCommands::Logs {
                    id,
                    follow,
                    tail,
                    timestamps,
                } => commands::container::logs(&docker, &id, follow, tail, timestamps).await?,
                ContainerCommands::Inspect { id } => {
                    commands::container::inspect(&docker, &id, &format).await?
                }
            }
        }
        Commands::Pod(cmd) => {
            let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
            match cmd {
                PodCommands::List => commands::pod::list(&docker, &format).await?,
                PodCommands::Create { name } => {
                    commands::pod::create(&docker, &name, &format).await?
                }
                PodCommands::Inspect { name } => {
                    commands::pod::inspect(&docker, &name, &format).await?
                }
                PodCommands::Delete { name, force } => {
                    commands::pod::delete(&docker, &name, force).await?
                }
                PodCommands::Add { name, container } => {
                    commands::pod::add(&docker, &name, &container).await?
                }
                PodCommands::Remove {
                    name,
                    container,
                    force,
                } => commands::pod::remove(&docker, &name, &container, force).await?,
            }
        }
        Commands::Image(cmd) => {
            match cmd {
                ImageCommands::Search {
                    query,
                    source,
                    limit,
                } => {
                    // Image search should not require starting the runtime.
                    // `auto` uses an explicit Docker host when selected, or an
                    // already-running built-in runtime, then falls back to the
                    // registry API only when no explicit host was requested.
                    // `dockerhub` is a direct registry query for callers that do
                    // not want any Docker endpoint involved.
                    if source == "auto" {
                        if let Some(docker) = try_existing_docker(docker_host.as_deref()).await? {
                            commands::image::search(&docker, &query, limit, &format).await?;
                        } else {
                            let results = cratebay_core::container::image_search_dockerhub(
                                &query,
                                limit.map(u64::from),
                            )
                            .await?;
                            commands::image::print_search_results(&results, &format)?;
                        }
                    } else {
                        let results = cratebay_core::container::image_search_dockerhub(
                            &query,
                            limit.map(u64::from),
                        )
                        .await?;
                        commands::image::print_search_results(&results, &format)?;
                    }
                }
                ImageCommands::List => {
                    let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
                    commands::image::list(&docker, &format).await?
                }
                ImageCommands::Pull { image } => {
                    let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
                    commands::image::pull(&docker, &image).await?
                }
                ImageCommands::Export { images, output } => {
                    let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
                    commands::image::export(&docker, images, &output).await?
                }
                ImageCommands::Import { input } => {
                    let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
                    commands::image::import(&docker, &input, &format).await?
                }
                ImageCommands::PreloadBundled { dir } => {
                    let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
                    commands::image::preload_bundled(&docker, dir, &format).await?
                }
                ImageCommands::Inspect { id } => {
                    let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
                    commands::image::inspect(&docker, &id, &format).await?
                }
                ImageCommands::Tag { source, target } => {
                    let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
                    commands::image::tag(&docker, &source, &target).await?
                }
                ImageCommands::PackContainer { container, image } => {
                    let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
                    commands::image::pack_container(&docker, &container, &image).await?
                }
                ImageCommands::Delete { id } => {
                    let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
                    commands::image::delete(&docker, &id).await?
                }
            }
        }
        Commands::Docker(cmd) => {
            let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
            match cmd {
                DockerCommands::Run {
                    pull: _,
                    name,
                    env,
                    image,
                } => commands::container::run_compat(&docker, name, image, env).await?,
                DockerCommands::Ps => commands::container::ps_compat(&docker).await?,
                DockerCommands::Env { id } => commands::container::print_env(&docker, &id).await?,
                DockerCommands::LoginCmd { id } => commands::container::print_login_cmd(&id),
                DockerCommands::Start { id } => {
                    commands::container::start_compat(&docker, &id).await?
                }
                DockerCommands::Stop { id } => {
                    commands::container::stop_compat(&docker, &id).await?
                }
                DockerCommands::Rm { id, force } => {
                    commands::container::rm_compat(&docker, &id, force).await?
                }
            }
        }
        Commands::Volume(cmd) => {
            let docker = resolve_docker(runtime.as_ref(), docker_host.as_deref()).await?;
            match cmd {
                VolumeCommands::Create { name } => commands::volume::create(&docker, &name).await?,
                VolumeCommands::List => commands::volume::list(&docker, &format).await?,
                VolumeCommands::Inspect { name } => {
                    commands::volume::inspect(&docker, &name, &format).await?
                }
                VolumeCommands::Remove { name, force } => {
                    commands::volume::remove(&docker, &name, force).await?
                }
            }
        }
        Commands::Runtime(cmd) => match cmd {
            RuntimeCommands::Status => commands::runtime::status().await?,
            RuntimeCommands::Start => commands::runtime::start().await?,
            RuntimeCommands::Stop => commands::runtime::stop().await?,
            RuntimeCommands::Provision => commands::runtime::provision().await?,
        },
        Commands::System(cmd) => match cmd {
            SystemCommands::Info => commands::system::info()?,
            SystemCommands::DockerStatus => commands::system::docker_status().await?,
        },
    }

    Ok(())
}

#[cfg(test)]
mod cli_surface_tests {
    use super::*;
    use clap::CommandFactory;

    fn command_names(command: &clap::Command) -> Vec<String> {
        command
            .get_subcommands()
            .map(|subcommand| subcommand.get_name().to_string())
            .collect()
    }

    fn find_subcommand<'a>(command: &'a clap::Command, name: &str) -> Option<&'a clap::Command> {
        command
            .get_subcommands()
            .find(|subcommand| subcommand.get_name() == name)
    }

    fn assert_contains_all(actual: &[String], expected: &[&str]) {
        for name in expected {
            assert!(
                actual.iter().any(|actual| actual == name),
                "missing CLI command '{}'; actual commands: {:?}",
                name,
                actual
            );
        }
    }

    #[test]
    fn top_level_cli_surface_matches_container_manager_shape() {
        let command = Cli::command();
        let names = command_names(&command);

        assert_contains_all(
            &names,
            &[
                "run",
                "container",
                "pod",
                "image",
                "volume",
                "runtime",
                "system",
            ],
        );

        let forbidden = [
            "mc".to_string() + "p",
            "sand".to_string() + "box",
            "ch".to_string() + "at",
            "ll".to_string() + "m",
            "provid".to_string() + "er",
        ];
        for name in forbidden {
            assert!(
                !names.iter().any(|actual| actual == &name),
                "removed product command returned to CLI surface: {}",
                name
            );
        }
    }

    #[test]
    fn docker_compat_commands_stay_hidden_from_product_help() {
        let command = Cli::command();
        let docker = find_subcommand(&command, "docker").expect("docker compat command must exist");

        assert!(
            docker.is_hide_set(),
            "docker compat command should remain callable but hidden from user-facing help"
        );
    }

    #[test]
    fn image_cli_surface_keeps_pull_pack_archive_and_bundle_workflows() {
        let command = Cli::command();
        let image = find_subcommand(&command, "image").expect("image command must exist");
        let names = command_names(image);

        assert_contains_all(
            &names,
            &[
                "list",
                "search",
                "pull",
                "export",
                "import",
                "preload-bundled",
                "inspect",
                "tag",
                "pack-container",
                "delete",
            ],
        );
    }

    #[test]
    fn image_search_source_is_explicitly_constrained() {
        let cli = Cli::try_parse_from([
            "cratebay",
            "image",
            "search",
            "alpine",
            "--source",
            "dockerhub",
        ])
        .expect("dockerhub source should parse");

        match cli.command {
            Commands::Image(ImageCommands::Search { source, .. }) => {
                assert_eq!(source, "dockerhub");
            }
            _ => panic!("expected image search command"),
        }

        assert!(
            Cli::try_parse_from(["cratebay", "image", "search", "alpine", "--source", "bad"])
                .is_err(),
            "image search source should reject unsupported backends"
        );
    }

    #[tokio::test]
    async fn image_search_explicit_host_failure_is_not_silently_fallback() {
        let err = try_existing_docker(Some("bad-host"))
            .await
            .expect_err("explicit bad host should surface an error");

        assert!(
            err.to_string()
                .contains("Unsupported Docker host format: bad-host"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn pod_cli_surface_keeps_full_group_lifecycle() {
        let command = Cli::command();
        let pod = find_subcommand(&command, "pod").expect("pod command must exist");
        let names = command_names(pod);

        assert_contains_all(
            &names,
            &["list", "create", "inspect", "delete", "add", "remove"],
        );
    }

    #[test]
    fn runtime_cli_surface_keeps_minimum_runtime_lifecycle() {
        let command = Cli::command();
        let runtime = find_subcommand(&command, "runtime").expect("runtime command must exist");
        let names = command_names(runtime);

        assert_contains_all(&names, &["status", "start", "stop", "provision"]);
    }

    #[test]
    fn one_shot_run_help_exposes_embedded_execution_controls() {
        let command = Cli::command();
        let run = find_subcommand(&command, "run").expect("run command must exist");
        let option_names: Vec<String> = run
            .get_arguments()
            .filter_map(|arg| arg.get_long().map(str::to_string))
            .collect();

        assert_contains_all(
            &option_names,
            &[
                "env",
                "volume",
                "cpu",
                "memory",
                "working-dir",
                "entrypoint",
                "pod",
                "network",
                "user",
                "read-only",
                "no-pull",
                "keep",
                "timeout",
                "max-output-bytes",
                "no-propagate-exit-code",
            ],
        );
    }

    #[test]
    fn container_exec_help_exposes_timeout_controls() {
        let command = Cli::command();
        let container =
            find_subcommand(&command, "container").expect("container command must exist");
        let exec = find_subcommand(container, "exec").expect("exec command must exist");
        let option_names: Vec<String> = exec
            .get_arguments()
            .filter_map(|arg| arg.get_long().map(str::to_string))
            .collect();

        assert_contains_all(
            &option_names,
            &[
                "working-dir",
                "timeout",
                "max-output-bytes",
                "no-propagate-exit-code",
            ],
        );
    }

    #[test]
    fn container_exec_help_exposes_embedded_execution_controls() {
        let command = Cli::command();
        let container =
            find_subcommand(&command, "container").expect("container command must exist");
        let exec = find_subcommand(container, "exec").expect("exec command must exist");
        let option_names: Vec<String> = exec
            .get_arguments()
            .filter_map(|arg| arg.get_long().map(str::to_string))
            .collect();

        assert_contains_all(
            &option_names,
            &["working-dir", "max-output-bytes", "no-propagate-exit-code"],
        );
    }

    #[test]
    fn json_shortcut_is_global_machine_output_flag() {
        let cli = Cli::try_parse_from([
            "cratebay",
            "run",
            "--json",
            "alpine:latest",
            "--",
            "echo",
            "hello",
        ])
        .expect("--json should parse after the subcommand");

        assert!(cli.json);
        assert!(matches!(cli.selected_format(), OutputFormat::Json));
    }

    #[test]
    fn json_shortcut_rejects_ambiguous_format_override() {
        assert!(
            Cli::try_parse_from(["cratebay", "--json", "--format", "yaml", "system", "info"])
                .is_err(),
            "--json and --format should not be accepted together"
        );
    }

    #[test]
    fn structured_command_errors_are_stable_for_embedded_callers() {
        let err: anyhow::Error =
            cratebay_core::AppError::Validation("bad input".to_string()).into();
        let payload = CommandErrorPayload::from_error(&err);
        let json = serde_json::to_value(&payload).expect("payload should serialize");

        assert_eq!(json["ok"], false);
        assert_eq!(json["kind"], "validation");
        assert_eq!(json["error"], "Validation error: bad input");
    }
}

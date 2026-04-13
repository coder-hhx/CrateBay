//! CrateBay CLI — command-line interface.

use clap::{ArgAction, Parser, Subcommand};

mod commands;

use commands::OutputFormat;

#[derive(Parser)]
#[command(
    name = "cratebay",
    version,
    about = "CrateBay CLI — Container management from the command line"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Docker host (overrides auto-detection). Examples:
    /// - unix:///var/run/docker.sock
    /// - tcp://127.0.0.1:2375
    #[arg(long, global = true)]
    docker_host: Option<String>,

    /// Output format for structured commands.
    #[arg(long, global = true, default_value = "table")]
    format: OutputFormat,
}

#[derive(Subcommand)]
enum Commands {
    /// Container operations
    #[command(subcommand)]
    Container(ContainerCommands),

    /// Image operations
    #[command(subcommand)]
    Image(ImageCommands),

    /// Docker-compatible convenience commands used by local smoke tests
    #[command(subcommand)]
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

    /// MCP server operations
    #[command(subcommand)]
    Mcp(McpCommands),
}

#[derive(Subcommand)]
enum ContainerCommands {
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
        /// Working directory inside the container
        #[arg(long)]
        working_dir: Option<String>,
        /// Environment variables (KEY=VALUE). Can be repeated.
        #[arg(long, action = ArgAction::Append)]
        env: Vec<String>,
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

    /// Inspect a local image
    Inspect { id: String },

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
enum McpCommands {
    /// Export MCP config for Claude Desktop, Cursor, or other MCP clients
    Export {
        /// Target client (claude, cursor, generic)
        #[arg(default_value = "claude")]
        target: String,
    },
}

#[derive(Subcommand)]
enum SystemCommands {
    /// Show CrateBay version and platform info
    Info,

    /// Show Docker connection status (does not start runtime)
    DockerStatus,
}

fn current_docker_context_host() -> Option<String> {
    let context_output = std::process::Command::new("docker")
        .args(["context", "show"])
        .output()
        .ok()?;
    if !context_output.status.success() {
        return None;
    }

    let context = String::from_utf8(context_output.stdout).ok()?;
    let context = context.trim();
    if context.is_empty() {
        return None;
    }

    let host_output = std::process::Command::new("docker")
        .args([
            "context",
            "inspect",
            context,
            "--format",
            "{{.Endpoints.docker.Host}}",
        ])
        .output()
        .ok()?;
    if !host_output.status.success() {
        return None;
    }

    let host = String::from_utf8(host_output.stdout).ok()?;
    let host = host.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

async fn try_connect_host_docker() -> Option<bollard::Docker> {
    if let Some(host) = current_docker_context_host() {
        let previous = std::env::var("DOCKER_HOST").ok();
        std::env::set_var("DOCKER_HOST", &host);
        let docker = cratebay_core::docker::connect().await.ok();
        if let Some(previous) = previous {
            std::env::set_var("DOCKER_HOST", previous);
        } else {
            std::env::remove_var("DOCKER_HOST");
        }
        if docker.is_some() {
            return docker;
        }
    }

    let docker = bollard::Docker::connect_with_local_defaults().ok()?;
    if cratebay_core::docker::is_available(&docker).await {
        Some(docker)
    } else {
        None
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    if let Some(host) = cli.docker_host.as_ref() {
        std::env::set_var("DOCKER_HOST", host);
    }

    // Runtime manager used by engine ensure for all Docker-dependent commands.
    let runtime = cratebay_core::runtime::create_runtime_manager();

    match cli.command {
        Commands::Container(cmd) => {
            let docker = cratebay_core::engine::ensure_docker(runtime.as_ref(), Default::default())
                .await?
                .as_ref()
                .clone();
            match cmd {
                ContainerCommands::List { all } => {
                    commands::container::list(&docker, all, &cli.format).await?
                }
                ContainerCommands::Create {
                    name,
                    image,
                    cpu,
                    memory,
                    command,
                    working_dir,
                    env,
                    no_start,
                } => {
                    commands::container::create(
                        &docker,
                        name,
                        image,
                        cpu,
                        memory,
                        command,
                        working_dir,
                        env,
                        no_start,
                        &cli.format,
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
                } => {
                    commands::container::exec(&docker, &id, command, working_dir, &cli.format)
                        .await?
                }
                ContainerCommands::Logs {
                    id,
                    follow,
                    tail,
                    timestamps,
                } => commands::container::logs(&docker, &id, follow, tail, timestamps).await?,
                ContainerCommands::Inspect { id } => {
                    commands::container::inspect(&docker, &id, &cli.format).await?
                }
            }
        }
        Commands::Image(cmd) => {
            match cmd {
                ImageCommands::Search {
                    query,
                    source: _,
                    limit,
                } => {
                    // Image search should not require starting the runtime. Both
                    // `auto` and `dockerhub` prefer any already-available Docker first
                    // because the engine queries the Docker Hub index and is more
                    // reliable in CI, then fall back to the Docker Hub HTTP API.
                    if let Some(docker) = cratebay_core::docker::try_connect().await {
                        commands::image::search(&docker, &query, limit, &cli.format).await?;
                    } else {
                        let results = cratebay_core::container::image_search_dockerhub(
                            &query,
                            limit.map(u64::from),
                        )
                        .await?;
                        commands::image::print_search_results(&results, &cli.format)?;
                    }
                }
                ImageCommands::List => {
                    let docker = if let Some(docker) = try_connect_host_docker().await {
                        docker
                    } else {
                        cratebay_core::engine::ensure_docker(runtime.as_ref(), Default::default())
                            .await?
                            .as_ref()
                            .clone()
                    };
                    commands::image::list(&docker, &cli.format).await?
                }
                ImageCommands::Pull { image } => {
                    let docker = if let Some(docker) = try_connect_host_docker().await {
                        docker
                    } else {
                        cratebay_core::engine::ensure_docker(runtime.as_ref(), Default::default())
                            .await?
                            .as_ref()
                            .clone()
                    };
                    commands::image::pull(&docker, &image).await?
                }
                ImageCommands::Inspect { id } => {
                    let docker = if let Some(docker) = try_connect_host_docker().await {
                        docker
                    } else {
                        cratebay_core::engine::ensure_docker(runtime.as_ref(), Default::default())
                            .await?
                            .as_ref()
                            .clone()
                    };
                    commands::image::inspect(&docker, &id, &cli.format).await?
                }
                ImageCommands::PackContainer { container, image } => {
                    let docker = if let Some(docker) = try_connect_host_docker().await {
                        docker
                    } else {
                        cratebay_core::engine::ensure_docker(runtime.as_ref(), Default::default())
                            .await?
                            .as_ref()
                            .clone()
                    };
                    commands::image::pack_container(&docker, &container, &image).await?
                }
                ImageCommands::Delete { id } => {
                    let docker = if let Some(docker) = try_connect_host_docker().await {
                        docker
                    } else {
                        cratebay_core::engine::ensure_docker(runtime.as_ref(), Default::default())
                            .await?
                            .as_ref()
                            .clone()
                    };
                    commands::image::delete(&docker, &id).await?
                }
            }
        }
        Commands::Docker(cmd) => {
            let docker = if let Some(docker) = try_connect_host_docker().await {
                docker
            } else {
                cratebay_core::engine::ensure_docker(runtime.as_ref(), Default::default())
                    .await?
                    .as_ref()
                    .clone()
            };
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
            let docker = if let Some(docker) = try_connect_host_docker().await {
                docker
            } else {
                cratebay_core::engine::ensure_docker(runtime.as_ref(), Default::default())
                    .await?
                    .as_ref()
                    .clone()
            };
            match cmd {
                VolumeCommands::Create { name } => commands::volume::create(&docker, &name).await?,
                VolumeCommands::List => commands::volume::list(&docker, &cli.format).await?,
                VolumeCommands::Inspect { name } => {
                    commands::volume::inspect(&docker, &name, &cli.format).await?
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
        Commands::Mcp(cmd) => match cmd {
            McpCommands::Export { target } => commands::mcp::export_config(&target)?,
        },
    }

    Ok(())
}

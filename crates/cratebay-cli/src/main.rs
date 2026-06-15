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

    /// Explicit Engine-compatible host for diagnostics/smoke tests. By default CrateBay uses its built-in runtime.
    ///
    /// Examples:
    /// - unix:///var/run/docker.sock
    /// - tcp://127.0.0.1:2375
    #[arg(long = "engine-host", alias = "docker-host", global = true)]
    engine_host: Option<String>,

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

    /// Network operations
    #[command(subcommand)]
    Network(NetworkCommands),

    /// Runtime management (start/stop/status)
    #[command(subcommand)]
    Runtime(RuntimeCommands),

    /// Persisted app settings shared with the desktop UI
    #[command(subcommand)]
    Settings(SettingsCommands),

    /// App update checks
    #[command(subcommand)]
    Update(UpdateCommands),

    /// Native CrateBay Engine API
    #[command(subcommand)]
    Engine(EngineCommands),

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
        /// Publish a port (host:container[/tcp|udp|sctp]). Can be repeated.
        #[arg(short = 'p', long = "publish", action = ArgAction::Append)]
        publish: Vec<String>,
        /// Bind mount (host:container[:ro|rw]). Can be repeated.
        #[arg(short = 'v', long = "volume", action = ArgAction::Append)]
        volume: Vec<String>,
        /// Attach the container to a CrateBay pod.
        #[arg(long, conflicts_with = "network")]
        pod: Option<String>,
        /// Network mode (`bridge`, `none`, `host`) or CrateBay network name.
        #[arg(long)]
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
    #[command(alias = "remove", alias = "rm")]
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

    /// Read container resource stats
    Stats { id: String },

    /// Open a native PTY terminal session
    TerminalOpen {
        id: String,
        /// Stable terminal session id. Generated when omitted.
        #[arg(long)]
        session_id: Option<String>,
        /// Working directory inside the container.
        #[arg(long)]
        working_dir: Option<String>,
        /// Initial terminal columns.
        #[arg(long, default_value_t = 80)]
        cols: u16,
        /// Initial terminal rows.
        #[arg(long, default_value_t = 24)]
        rows: u16,
        /// Command to run in the PTY (after `--`). Defaults to `sh -i`.
        #[arg(last = true)]
        command: Vec<String>,
    },

    /// Send raw input to a native PTY terminal session
    TerminalInput {
        id: String,
        #[arg(long)]
        session_id: String,
        #[arg(long)]
        data: String,
    },

    /// Read pending output from a native PTY terminal session
    TerminalRead {
        id: String,
        #[arg(long)]
        session_id: String,
    },

    /// Resize a native PTY terminal session
    TerminalResize {
        id: String,
        #[arg(long)]
        session_id: String,
        #[arg(long)]
        cols: u16,
        #[arg(long)]
        rows: u16,
    },

    /// Close a native PTY terminal session
    TerminalClose {
        id: String,
        #[arg(long)]
        session_id: String,
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

    /// Publish a port (host:container[/tcp|udp|sctp]). Can be repeated.
    #[arg(short = 'p', long = "publish", action = ArgAction::Append)]
    publish: Vec<String>,

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
    #[arg(long, conflicts_with = "network")]
    pod: Option<String>,

    /// Network mode (`bridge`, `none`, `host`) or CrateBay network name.
    #[arg(long)]
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
        /// Network driver
        #[arg(long)]
        driver: Option<String>,
        /// Create an internal pod network
        #[arg(long)]
        internal: bool,
        /// Enable IPv6
        #[arg(long = "ipv6")]
        enable_ipv6: bool,
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
    Pull {
        image: String,
        /// Docker Hub mirror registry to try before direct pull; can be repeated
        #[arg(long = "mirror")]
        mirrors: Vec<String>,
    },

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
    #[command(alias = "remove", alias = "rmi")]
    Delete {
        id: String,
        /// Force removal when CrateBay-managed containers still reference it
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum VolumeCommands {
    /// Create a volume
    Create {
        name: String,
        /// Volume driver
        #[arg(long)]
        driver: Option<String>,
    },

    /// List volumes
    List,

    /// Inspect a volume
    Inspect { name: String },

    /// Remove a volume
    #[command(alias = "delete")]
    Remove {
        name: String,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum NetworkCommands {
    /// Create a network
    Create {
        name: String,
        /// Network driver
        #[arg(long)]
        driver: Option<String>,
        /// Create an internal network
        #[arg(long)]
        internal: bool,
        /// Enable IPv6
        #[arg(long = "ipv6")]
        enable_ipv6: bool,
    },

    /// List networks
    #[command(alias = "ls")]
    List,

    /// Inspect a network
    Inspect { id: String },

    /// Remove a network
    #[command(alias = "delete")]
    Remove {
        id: String,
        /// Force removal when CrateBay-managed containers still reference it
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum RuntimeCommands {
    /// Show runtime status
    Status,
    /// Show runtime, Engine contract, substrate, storage, and shim diagnostics
    Diagnostics {
        /// Include exited CrateBay-managed container metadata/logs in the GC dry-run snapshot
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        prune_exited_containers: bool,
    },
    /// Start the built-in runtime
    Start,
    /// Stop the built-in runtime
    Stop,
    /// Restart the built-in runtime
    Restart,
    /// Configure runtime HTTP proxy settings
    #[command(subcommand)]
    Proxy(RuntimeProxyCommands),
    /// Pre-download runtime image without starting
    Provision,
}

#[derive(Subcommand)]
enum RuntimeProxyCommands {
    /// Show persisted runtime HTTP proxy settings
    Show,
    /// Persist runtime HTTP proxy settings used by runtime start/restart
    Set {
        /// HTTP proxy endpoint, for example http://127.0.0.1:7890
        proxy: Option<String>,
        /// Bridge a host proxy into the runtime VM
        #[arg(long, conflicts_with = "no_bridge")]
        bridge: bool,
        /// Disable host proxy bridging
        #[arg(long = "no-bridge")]
        no_bridge: bool,
        /// Host address used when bridging a local proxy into the runtime VM
        #[arg(long)]
        bind_host: Option<String>,
        /// Host port used when bridging a local proxy into the runtime VM
        #[arg(long)]
        bind_port: Option<u16>,
        /// Hostname/IP that the runtime VM should use for the bridged proxy
        #[arg(long)]
        guest_host: Option<String>,
    },
    /// Clear the runtime HTTP proxy and restore bridge defaults
    Clear,
}

#[derive(Subcommand)]
enum SettingsCommands {
    /// List supported persisted settings
    List,
    /// Show one persisted setting
    Get {
        /// Settings key from the desktop Settings page
        key: String,
    },
    /// Persist one setting
    Set {
        /// Settings key from the desktop Settings page
        key: String,
        /// Value to store. registryMirrors accepts JSON, comma-separated, or newline-separated input.
        value: String,
    },
    /// Reset one setting to the desktop default
    Reset {
        /// Settings key from the desktop Settings page
        key: String,
    },
}

#[derive(Subcommand)]
enum UpdateCommands {
    /// Check GitHub Releases for the desktop updater manifest
    Check {
        /// Include prerelease releases, overriding the desktop includePrereleases setting
        #[arg(long, conflicts_with = "stable")]
        include_prerelease: bool,
        /// Only check stable releases, overriding the desktop includePrereleases setting
        #[arg(long)]
        stable: bool,
        /// Override release repository, for example nicepkg/CrateBay
        #[arg(long)]
        repository: Option<String>,
    },
}

#[derive(Subcommand)]
enum EngineCommands {
    /// Show native CrateBay Engine contract
    Status,

    /// Show the CrateBay-owned VM/shim/network/storage substrate
    Substrate,

    /// Run CrateBay storage garbage collection. Defaults to a dry run.
    StorageGc {
        /// Apply the GC plan. Without this flag, the command only reports candidates.
        #[arg(long)]
        apply: bool,
        /// Include exited CrateBay-managed container metadata/logs as GC candidates.
        #[arg(long, default_value_t = true, action = ArgAction::Set)]
        prune_exited_containers: bool,
    },

    /// List CrateBay-managed containerd shim tasks
    ShimTasks,

    /// Reap exited CrateBay shim task metadata/logs. Defaults to a dry run.
    ReapShimTask {
        id: String,
        /// Apply the reap plan. Without this flag, the command only reports the candidate.
        #[arg(long)]
        apply: bool,
    },

    /// List containers through the native CrateBay Engine API
    #[command(alias = "list", alias = "ps")]
    Containers,

    /// List images through the native CrateBay Engine API
    Images,

    /// Pull an image through the native CrateBay Engine API
    PullImage {
        image: String,
        /// Optional tag to append when the image has no tag
        #[arg(long)]
        tag: Option<String>,
        /// Docker Hub mirror registry to try before direct pull; can be repeated
        #[arg(long = "mirror")]
        mirrors: Vec<String>,
    },

    /// Inspect an image through the native CrateBay Engine API
    InspectImage { id: String },

    /// Remove an image through the native CrateBay Engine API
    RemoveImage {
        id: String,
        /// Force removal when the backend supports it
        #[arg(long)]
        force: bool,
    },

    /// Tag an image through the native CrateBay Engine API
    TagImage { source: String, target: String },

    /// Pack a running CrateBay container rootfs into an image
    PackImage { container: String, image: String },

    /// Export images through the native CrateBay Engine API
    ExportImages {
        /// Output tar archive path
        #[arg(short, long)]
        output: String,
        /// Image references to export
        #[arg(required = true)]
        images: Vec<String>,
    },

    /// Import an image archive through the native CrateBay Engine API
    ImportImage { input: String },

    /// List networks through the native CrateBay Engine API
    Networks,

    /// Inspect a network through the native CrateBay Engine API
    InspectNetwork { id: String },

    /// Create a network through the native CrateBay Engine API
    CreateNetwork {
        name: String,
        /// Network driver
        #[arg(long)]
        driver: Option<String>,
        /// Create an internal network
        #[arg(long)]
        internal: bool,
        /// Enable IPv6
        #[arg(long = "ipv6")]
        enable_ipv6: bool,
    },

    /// Remove a network through the native CrateBay Engine API
    RemoveNetwork {
        id: String,
        /// Force removal when CrateBay-managed containers still reference it
        #[arg(long)]
        force: bool,
    },

    /// List volumes through the native CrateBay Engine API
    Volumes,

    /// Inspect a volume through the native CrateBay Engine API
    InspectVolume { name: String },

    /// Create a volume through the native CrateBay Engine API
    CreateVolume {
        name: String,
        /// Volume driver
        #[arg(long)]
        driver: Option<String>,
    },

    /// Remove a volume through the native CrateBay Engine API
    RemoveVolume {
        name: String,
        /// Force removal when the backend supports it
        #[arg(long)]
        force: bool,
    },

    /// List pods through the native CrateBay Engine API
    Pods,

    /// Create a pod through the native CrateBay Engine API
    CreatePod {
        name: String,
        /// Network driver
        #[arg(long)]
        driver: Option<String>,
        /// Create an internal pod network
        #[arg(long)]
        internal: bool,
        /// Enable IPv6
        #[arg(long = "ipv6")]
        enable_ipv6: bool,
    },

    /// Remove a pod through the native CrateBay Engine API
    RemovePod {
        name: String,
        /// Force removal when CrateBay-managed containers still reference it
        #[arg(long)]
        force: bool,
    },

    /// Create a container through the native CrateBay Engine API
    Create {
        name: String,
        /// Container image to use
        #[arg(long)]
        image: String,
        /// Shell-form command to run as `/bin/sh -c <command>`
        #[arg(long)]
        command: Option<String>,
        /// Entrypoint override
        #[arg(long)]
        entrypoint: Option<String>,
        /// Working directory inside the container
        #[arg(long)]
        working_dir: Option<String>,
        /// Environment variable, e.g. KEY=value
        #[arg(long = "env")]
        env: Vec<String>,
        /// Publish a port, e.g. 8080:80/tcp, 5353:53/udp, or 5000:5000/sctp
        #[arg(long = "publish")]
        publish: Vec<String>,
        /// Bind mount, e.g. /host:/container[:ro]
        #[arg(long = "volume")]
        volume: Vec<String>,
        /// Attach the container to a CrateBay pod.
        #[arg(long, conflicts_with = "network")]
        pod: Option<String>,
        /// Network mode or CrateBay network name
        #[arg(long)]
        network: Option<String>,
        /// User to run as inside the container
        #[arg(long)]
        user: Option<String>,
        /// Mount the container root filesystem read-only
        #[arg(long = "read-only")]
        read_only: bool,
        /// Do not auto-start after create
        #[arg(long)]
        no_start: bool,
        /// CPU cores limit
        #[arg(long)]
        cpu: Option<f64>,
        /// Memory limit in MB
        #[arg(long)]
        memory: Option<u64>,
    },

    /// Run a one-shot container through the native CrateBay Engine API
    Run(RunArgs),

    /// Start a container through the native CrateBay Engine API
    Start { id: String },

    /// Stop a container through the native CrateBay Engine API
    Stop {
        id: String,
        /// Timeout in seconds before forceful termination
        #[arg(long)]
        timeout: Option<u64>,
    },

    /// Remove a container through the native CrateBay Engine API
    Remove {
        id: String,
        /// Force removal
        #[arg(long)]
        force: bool,
    },

    /// Inspect a container through the native CrateBay Engine API
    Inspect { id: String },

    /// Read container resource stats through the native CrateBay Engine API
    Stats { id: String },

    /// Read container logs through the native CrateBay Engine API
    Logs {
        id: String,
        /// Number of trailing log lines to return
        #[arg(long)]
        tail: Option<u64>,
        /// Include timestamps when supported by the runtime
        #[arg(long)]
        timestamps: bool,
    },

    /// Execute a command through the native CrateBay Engine API
    Exec {
        id: String,
        /// Working directory inside the container
        #[arg(long)]
        working_dir: Option<String>,
        /// Timeout in seconds. Use 0 or omit to disable.
        #[arg(long)]
        timeout: Option<u64>,
        /// Maximum captured stdout/stderr bytes per stream. Use 0 to disable.
        #[arg(
            long = "max-output-bytes",
            default_value_t = 1_048_576,
            value_name = "BYTES"
        )]
        max_output_bytes: u64,
        /// Exit 0 after a completed exec and report the container exit in the payload.
        #[arg(long)]
        no_propagate_exit_code: bool,
        /// Command to execute (after `--`)
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },

    /// Open a native PTY terminal session through the CrateBay Engine API
    TerminalOpen {
        id: String,
        /// Stable terminal session id. Generated when omitted.
        #[arg(long)]
        session_id: Option<String>,
        /// Working directory inside the container
        #[arg(long)]
        working_dir: Option<String>,
        /// Initial terminal columns
        #[arg(long, default_value_t = 80)]
        cols: u16,
        /// Initial terminal rows
        #[arg(long, default_value_t = 24)]
        rows: u16,
        /// Command to run in the PTY (after `--`). Defaults to `sh -i`.
        #[arg(last = true)]
        command: Vec<String>,
    },

    /// Send raw input to a native PTY terminal session
    TerminalInput {
        id: String,
        #[arg(long)]
        session_id: String,
        #[arg(long)]
        data: String,
    },

    /// Read pending output from a native PTY terminal session
    TerminalRead {
        id: String,
        #[arg(long)]
        session_id: String,
    },

    /// Resize a native PTY terminal session
    TerminalResize {
        id: String,
        #[arg(long)]
        session_id: String,
        #[arg(long)]
        cols: u16,
        #[arg(long)]
        rows: u16,
    },

    /// Close a native PTY terminal session
    TerminalClose {
        id: String,
        #[arg(long)]
        session_id: String,
    },
}

#[derive(Subcommand)]
enum SystemCommands {
    /// Show CrateBay version and platform info
    Info,

    /// Show current native CrateBay Engine status (does not start runtime)
    #[command(alias = "docker-status")]
    EngineStatus,
}

fn explicit_engine_host(cli_host: Option<&str>) -> Option<String> {
    cli_host.and_then(|host| {
        let host = host.trim();
        (!host.is_empty()).then(|| host.to_string())
    })
}

async fn resolve_docker(
    runtime: &dyn cratebay_core::runtime::RuntimeManager,
    engine_host: Option<&str>,
) -> anyhow::Result<bollard::Docker> {
    if let Some(host) = explicit_engine_host(engine_host) {
        return Ok(cratebay_core::docker::connect_host(&host).await?);
    }

    Ok(
        cratebay_core::engine::ensure_engine_compatibility(runtime, Default::default())
            .await?
            .as_ref()
            .clone(),
    )
}

async fn try_existing_docker(engine_host: Option<&str>) -> anyhow::Result<Option<bollard::Docker>> {
    if let Some(host) = explicit_engine_host(engine_host) {
        return Ok(Some(cratebay_core::docker::connect_host(&host).await?));
    }

    Ok(cratebay_core::docker::try_connect().await)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeDispatchMode {
    NativeEngine,
    ExplicitCompatibilityHost,
}

fn native_dispatch_mode(engine_host: Option<&str>) -> NativeDispatchMode {
    if explicit_engine_host(engine_host).is_some() {
        NativeDispatchMode::ExplicitCompatibilityHost
    } else {
        NativeDispatchMode::NativeEngine
    }
}

async fn ensure_native_engine_ready(
    runtime: &dyn cratebay_core::runtime::RuntimeManager,
) -> anyhow::Result<()> {
    commands::runtime::apply_persisted_runtime_proxy_env()?;
    cratebay_core::engine::ensure_engine_contract(runtime, Default::default()).await?;
    Ok(())
}

fn should_ensure_native_engine(command: &Commands, engine_host: Option<&str>) -> bool {
    if matches!(
        native_dispatch_mode(engine_host),
        NativeDispatchMode::ExplicitCompatibilityHost
    ) {
        return false;
    }

    match command {
        Commands::Run(_) => true,
        Commands::Container(_) => true,
        Commands::Pod(_) => true,
        Commands::Volume(_) => true,
        Commands::Network(_) => true,
        Commands::Engine(cmd) => engine_command_requires_native_ensure(cmd),
        Commands::Image(cmd) => !matches!(cmd, ImageCommands::Search { .. }),
        Commands::Docker(_)
        | Commands::Runtime(_)
        | Commands::Settings(_)
        | Commands::Update(_)
        | Commands::System(_) => false,
    }
}

fn engine_command_requires_native_ensure(command: &EngineCommands) -> bool {
    !matches!(
        command,
        EngineCommands::Status
            | EngineCommands::Substrate
            | EngineCommands::StorageGc { apply: false, .. }
            | EngineCommands::ShimTasks
            | EngineCommands::ReapShimTask { apply: false, .. }
    )
}

fn ensure_native_only_terminal(engine_host: Option<&str>) -> anyhow::Result<()> {
    if explicit_engine_host(engine_host).is_some() {
        anyhow::bail!(
            "container terminal commands require the native CrateBay Engine; use `cratebay engine terminal-*` or omit --engine-host"
        );
    }
    Ok(())
}

async fn run_container_once(
    args: RunArgs,
    engine_host: Option<&str>,
    format: &OutputFormat,
) -> anyhow::Result<()> {
    match native_dispatch_mode(engine_host) {
        NativeDispatchMode::ExplicitCompatibilityHost => {
            tracing::debug!(
                "CLI run: dispatching one-shot run through explicit compatibility host"
            );
            let runtime = cratebay_core::runtime::create_runtime_manager();
            let docker = resolve_docker(runtime.as_ref(), engine_host).await?;
            commands::container::run_once(
                &docker,
                args.name,
                args.image,
                args.command,
                args.env,
                args.volume,
                args.publish,
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
                format,
            )
            .await
        }
        NativeDispatchMode::NativeEngine => {
            tracing::debug!("CLI run: dispatching one-shot run through native CrateBay Engine API");
            commands::engine::run_once(
                args.name,
                args.image,
                args.command,
                args.env,
                args.volume,
                args.publish,
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
                image_pull_mirrors_or_settings(Vec::new()),
                format,
            )
            .await
        }
    }
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
        cratebay_core::AppError::Docker(_) => "engineCompatibility",
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

fn image_pull_mirrors_or_settings(explicit_mirrors: Vec<String>) -> Vec<String> {
    if !explicit_mirrors.is_empty() {
        return explicit_mirrors;
    }

    persisted_registry_mirrors().unwrap_or_else(default_registry_mirrors)
}

fn persisted_registry_mirrors() -> Option<Vec<String>> {
    let db_path = cratebay_core::storage::default_db_path().ok()?;
    let conn = cratebay_core::storage::init(&db_path).ok()?;
    let value = cratebay_core::storage::get_setting(
        &conn,
        cratebay_core::settings::SETTINGS_KEY_REGISTRY_MIRRORS,
    )
    .ok()
    .flatten()?;
    Some(cratebay_core::settings::parse_registry_mirrors_setting(
        &value,
    ))
}

fn default_registry_mirrors() -> Vec<String> {
    cratebay_core::settings::default_registry_mirrors()
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
        engine_host,
        ..
    } = cli;

    // Runtime manager used by engine ensure for all compatibility-API commands.
    let runtime = cratebay_core::runtime::create_runtime_manager();

    if should_ensure_native_engine(&command, engine_host.as_deref()) {
        ensure_native_engine_ready(runtime.as_ref()).await?;
    }

    match command {
        Commands::Run(args) => run_container_once(args, engine_host.as_deref(), &format).await?,
        Commands::Container(cmd) => {
            tracing::debug!("CLI container command: resolving CrateBay Engine API client");
            match cmd {
                ContainerCommands::Run(args) => {
                    run_container_once(args, engine_host.as_deref(), &format).await?
                }
                ContainerCommands::List { all } => {
                    match native_dispatch_mode(engine_host.as_deref()) {
                        NativeDispatchMode::ExplicitCompatibilityHost => {
                            let docker =
                                resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                            commands::container::list(&docker, all, &format).await?
                        }
                        NativeDispatchMode::NativeEngine => {
                            let _ = all;
                            commands::engine::containers(&format).await?
                        }
                    }
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
                } => match native_dispatch_mode(engine_host.as_deref()) {
                    NativeDispatchMode::ExplicitCompatibilityHost => {
                        let docker =
                            resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
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
                    NativeDispatchMode::NativeEngine => {
                        commands::engine::create(
                            name,
                            image,
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
                            cpu.map(f64::from),
                            memory,
                            image_pull_mirrors_or_settings(Vec::new()),
                            &format,
                        )
                        .await?
                    }
                },
                ContainerCommands::Start { id } => {
                    match native_dispatch_mode(engine_host.as_deref()) {
                        NativeDispatchMode::ExplicitCompatibilityHost => {
                            let docker =
                                resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                            commands::container::start(&docker, &id).await?
                        }
                        NativeDispatchMode::NativeEngine => {
                            commands::engine::start(&id, &format).await?
                        }
                    }
                }
                ContainerCommands::Stop { id, timeout } => {
                    match native_dispatch_mode(engine_host.as_deref()) {
                        NativeDispatchMode::ExplicitCompatibilityHost => {
                            let docker =
                                resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                            commands::container::stop(&docker, &id, timeout).await?
                        }
                        NativeDispatchMode::NativeEngine => {
                            commands::engine::stop(&id, timeout.map(u64::from), &format).await?
                        }
                    }
                }
                ContainerCommands::Delete { id, force } => {
                    match native_dispatch_mode(engine_host.as_deref()) {
                        NativeDispatchMode::ExplicitCompatibilityHost => {
                            let docker =
                                resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                            commands::container::delete(&docker, &id, force).await?
                        }
                        NativeDispatchMode::NativeEngine => {
                            commands::engine::remove(&id, force, &format).await?
                        }
                    }
                }
                ContainerCommands::Exec {
                    id,
                    command,
                    working_dir,
                    timeout,
                    max_output_bytes,
                    no_propagate_exit_code,
                } => match native_dispatch_mode(engine_host.as_deref()) {
                    NativeDispatchMode::ExplicitCompatibilityHost => {
                        let docker =
                            resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
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
                    NativeDispatchMode::NativeEngine => {
                        commands::engine::exec(
                            &id,
                            command,
                            working_dir,
                            timeout,
                            (max_output_bytes > 0).then_some(max_output_bytes),
                            no_propagate_exit_code,
                            &format,
                        )
                        .await?
                    }
                },
                ContainerCommands::Logs {
                    id,
                    follow,
                    tail,
                    timestamps,
                } => match native_dispatch_mode(engine_host.as_deref()) {
                    NativeDispatchMode::ExplicitCompatibilityHost => {
                        let docker =
                            resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                        commands::container::logs(&docker, &id, follow, tail, timestamps, &format)
                            .await?
                    }
                    NativeDispatchMode::NativeEngine => {
                        commands::engine::logs(
                            &id,
                            tail.map(u64::from),
                            timestamps,
                            follow,
                            &format,
                        )
                        .await?
                    }
                },
                ContainerCommands::Stats { id } => {
                    match native_dispatch_mode(engine_host.as_deref()) {
                        NativeDispatchMode::ExplicitCompatibilityHost => {
                            let docker =
                                resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                            commands::container::stats(&docker, &id, &format).await?
                        }
                        NativeDispatchMode::NativeEngine => {
                            commands::engine::stats(&id, &format).await?
                        }
                    }
                }
                ContainerCommands::TerminalOpen {
                    id,
                    session_id,
                    working_dir,
                    cols,
                    rows,
                    command,
                } => {
                    ensure_native_only_terminal(engine_host.as_deref())?;
                    commands::engine::terminal_open(
                        &id,
                        session_id,
                        working_dir,
                        cols,
                        rows,
                        command,
                        &format,
                    )
                    .await?
                }
                ContainerCommands::TerminalInput {
                    id,
                    session_id,
                    data,
                } => {
                    ensure_native_only_terminal(engine_host.as_deref())?;
                    commands::engine::terminal_input(&id, &session_id, &data, &format).await?
                }
                ContainerCommands::TerminalRead { id, session_id } => {
                    ensure_native_only_terminal(engine_host.as_deref())?;
                    commands::engine::terminal_read(&id, &session_id, &format).await?
                }
                ContainerCommands::TerminalResize {
                    id,
                    session_id,
                    cols,
                    rows,
                } => {
                    ensure_native_only_terminal(engine_host.as_deref())?;
                    commands::engine::terminal_resize(&id, &session_id, cols, rows, &format).await?
                }
                ContainerCommands::TerminalClose { id, session_id } => {
                    ensure_native_only_terminal(engine_host.as_deref())?;
                    commands::engine::terminal_close(&id, &session_id, &format).await?
                }
                ContainerCommands::Inspect { id } => {
                    match native_dispatch_mode(engine_host.as_deref()) {
                        NativeDispatchMode::ExplicitCompatibilityHost => {
                            let docker =
                                resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                            commands::container::inspect(&docker, &id, &format).await?
                        }
                        NativeDispatchMode::NativeEngine => {
                            commands::engine::inspect(&id, &format).await?
                        }
                    }
                }
            }
        }
        Commands::Pod(cmd) => {
            let explicit_host = explicit_engine_host(engine_host.as_deref()).is_some();
            match cmd {
                PodCommands::List if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::pod::list_compat(&docker, &format).await?
                }
                PodCommands::List => commands::pod::list(&format).await?,
                PodCommands::Create {
                    name,
                    driver: _,
                    internal: _,
                    enable_ipv6: _,
                } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::pod::create_compat(&docker, &name, &format).await?
                }
                PodCommands::Create {
                    name,
                    driver,
                    internal,
                    enable_ipv6,
                } => commands::pod::create(&name, driver, internal, enable_ipv6, &format).await?,
                PodCommands::Inspect { name } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::pod::inspect_compat(&docker, &name, &format).await?
                }
                PodCommands::Inspect { name } => commands::pod::inspect(&name, &format).await?,
                PodCommands::Delete { name, force } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::pod::delete_compat(&docker, &name, force).await?
                }
                PodCommands::Delete { name, force } => {
                    commands::pod::delete(&name, force, &format).await?
                }
                PodCommands::Add { name, container } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::pod::add_compat(&docker, &name, &container).await?
                }
                PodCommands::Add { name, container } => {
                    commands::pod::add(&name, &container, &format).await?
                }
                PodCommands::Remove {
                    name,
                    container,
                    force,
                } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::pod::remove_compat(&docker, &name, &container, force).await?
                }
                PodCommands::Remove {
                    name,
                    container,
                    force,
                } => commands::pod::remove(&name, &container, force, &format).await?,
            }
        }
        Commands::Image(cmd) => {
            let explicit_host = explicit_engine_host(engine_host.as_deref()).is_some();
            match cmd {
                ImageCommands::Search {
                    query,
                    source,
                    limit,
                } => {
                    // Image search should not require starting the runtime.
                    // `auto` uses an explicit compatibility host when selected, or an
                    // already-running built-in runtime, then falls back to the
                    // registry API only when no explicit host was requested.
                    // `dockerhub` is a direct registry query for callers that do
                    // not want any Engine endpoint involved.
                    if source == "auto" {
                        if let Some(docker) = try_existing_docker(engine_host.as_deref()).await? {
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
                ImageCommands::List if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::image::list(&docker, &format).await?
                }
                ImageCommands::List => commands::engine::images(&format).await?,
                ImageCommands::Pull { image, mirrors } if explicit_host => {
                    if !mirrors.is_empty() {
                        eprintln!(
                            "warning: --mirror is ignored when an explicit Engine host is used"
                        );
                    }
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::image::pull(&docker, &image).await?
                }
                ImageCommands::Pull { image, mirrors } => {
                    let mirrors = image_pull_mirrors_or_settings(mirrors);
                    commands::engine::pull_image(image, None, mirrors, &format).await?
                }
                ImageCommands::Export { images, output } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::image::export(&docker, images, &output).await?
                }
                ImageCommands::Export { images, output } => {
                    commands::engine::export_images(images, &output, &format).await?
                }
                ImageCommands::Import { input } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::image::import(&docker, &input, &format).await?
                }
                ImageCommands::Import { input } => {
                    commands::engine::import_image(&input, &format).await?
                }
                ImageCommands::PreloadBundled { dir } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::image::preload_bundled(&docker, dir, &format).await?
                }
                ImageCommands::PreloadBundled { dir } => {
                    commands::image::preload_bundled_native(runtime.as_ref(), dir, &format).await?
                }
                ImageCommands::Inspect { id } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::image::inspect(&docker, &id, &format).await?
                }
                ImageCommands::Inspect { id } => {
                    commands::engine::inspect_image(&id, &format).await?
                }
                ImageCommands::Tag { source, target } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::image::tag(&docker, &source, &target).await?
                }
                ImageCommands::Tag { source, target } => {
                    commands::engine::tag_image(&source, &target, &format).await?
                }
                ImageCommands::PackContainer { container, image } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::image::pack_container(&docker, &container, &image).await?
                }
                ImageCommands::PackContainer { container, image } => {
                    commands::engine::pack_image(&container, &image, &format).await?
                }
                ImageCommands::Delete { id, force } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::image::delete(&docker, &id, force).await?
                }
                ImageCommands::Delete { id, force } => {
                    commands::engine::remove_image(&id, force, &format).await?
                }
            }
        }
        Commands::Docker(cmd) => {
            let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
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
            let explicit_host = explicit_engine_host(engine_host.as_deref()).is_some();
            match cmd {
                VolumeCommands::Create { name, driver } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::volume::create(&docker, &name, driver).await?
                }
                VolumeCommands::Create { name, driver } => {
                    commands::engine::create_volume(name, driver, &format).await?
                }
                VolumeCommands::List if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::volume::list(&docker, &format).await?
                }
                VolumeCommands::List => commands::engine::volumes(&format).await?,
                VolumeCommands::Inspect { name } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::volume::inspect(&docker, &name, &format).await?
                }
                VolumeCommands::Inspect { name } => {
                    commands::engine::inspect_volume(&name, &format).await?
                }
                VolumeCommands::Remove { name, force } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::volume::remove(&docker, &name, force).await?
                }
                VolumeCommands::Remove { name, force } => {
                    commands::engine::remove_volume(&name, force, &format).await?
                }
            }
        }
        Commands::Network(cmd) => {
            let explicit_host = explicit_engine_host(engine_host.as_deref()).is_some();
            match cmd {
                NetworkCommands::Create {
                    name,
                    driver,
                    internal,
                    enable_ipv6,
                } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::network::create(&docker, &name, driver, internal, enable_ipv6).await?
                }
                NetworkCommands::Create {
                    name,
                    driver,
                    internal,
                    enable_ipv6,
                } => {
                    commands::engine::create_network(name, driver, internal, enable_ipv6, &format)
                        .await?
                }
                NetworkCommands::List if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::network::list(&docker, &format).await?
                }
                NetworkCommands::List => commands::engine::networks(&format).await?,
                NetworkCommands::Inspect { id } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::network::inspect(&docker, &id, &format).await?
                }
                NetworkCommands::Inspect { id } => {
                    commands::engine::inspect_network(&id, &format).await?
                }
                NetworkCommands::Remove { id, force: _ } if explicit_host => {
                    let docker = resolve_docker(runtime.as_ref(), engine_host.as_deref()).await?;
                    commands::network::remove(&docker, &id).await?
                }
                NetworkCommands::Remove { id, force } => {
                    commands::engine::remove_network(&id, force, &format).await?
                }
            }
        }
        Commands::Runtime(cmd) => match cmd {
            RuntimeCommands::Status => commands::runtime::status(&format).await?,
            RuntimeCommands::Diagnostics {
                prune_exited_containers,
            } => commands::runtime::diagnostics(prune_exited_containers, &format).await?,
            RuntimeCommands::Start => commands::runtime::start(&format).await?,
            RuntimeCommands::Stop => commands::runtime::stop(&format).await?,
            RuntimeCommands::Restart => commands::runtime::restart(&format).await?,
            RuntimeCommands::Proxy(cmd) => match cmd {
                RuntimeProxyCommands::Show => commands::runtime::proxy_show(&format).await?,
                RuntimeProxyCommands::Set {
                    proxy,
                    bridge,
                    no_bridge,
                    bind_host,
                    bind_port,
                    guest_host,
                } => {
                    commands::runtime::proxy_set(
                        proxy, bridge, no_bridge, bind_host, bind_port, guest_host, &format,
                    )
                    .await?
                }
                RuntimeProxyCommands::Clear => commands::runtime::proxy_clear(&format).await?,
            },
            RuntimeCommands::Provision => commands::runtime::provision(&format).await?,
        },
        Commands::Settings(cmd) => match cmd {
            SettingsCommands::List => commands::settings::list(&format)?,
            SettingsCommands::Get { key } => commands::settings::get(&key, &format)?,
            SettingsCommands::Set { key, value } => commands::settings::set(&key, &value, &format)?,
            SettingsCommands::Reset { key } => commands::settings::reset(&key, &format)?,
        },
        Commands::Update(cmd) => match cmd {
            UpdateCommands::Check {
                include_prerelease,
                stable,
                repository,
            } => {
                let include_prerelease = if include_prerelease {
                    Some(true)
                } else if stable {
                    Some(false)
                } else {
                    None
                };
                commands::update::check(include_prerelease, repository, &format).await?
            }
        },
        Commands::Engine(cmd) => match cmd {
            EngineCommands::Status => commands::engine::status(&format).await?,
            EngineCommands::Substrate => commands::engine::substrate(&format).await?,
            EngineCommands::StorageGc {
                apply,
                prune_exited_containers,
            } => commands::engine::storage_gc(apply, prune_exited_containers, &format).await?,
            EngineCommands::ShimTasks => commands::engine::shim_tasks(&format).await?,
            EngineCommands::ReapShimTask { id, apply } => {
                commands::engine::reap_shim_task(&id, apply, &format).await?
            }
            EngineCommands::Containers => commands::engine::containers(&format).await?,
            EngineCommands::Images => commands::engine::images(&format).await?,
            EngineCommands::PullImage {
                image,
                tag,
                mirrors,
            } => {
                let mirrors = image_pull_mirrors_or_settings(mirrors);
                commands::engine::pull_image(image, tag, mirrors, &format).await?
            }
            EngineCommands::InspectImage { id } => {
                commands::engine::inspect_image(&id, &format).await?
            }
            EngineCommands::RemoveImage { id, force } => {
                commands::engine::remove_image(&id, force, &format).await?
            }
            EngineCommands::TagImage { source, target } => {
                commands::engine::tag_image(&source, &target, &format).await?
            }
            EngineCommands::PackImage { container, image } => {
                commands::engine::pack_image(&container, &image, &format).await?
            }
            EngineCommands::ExportImages { images, output } => {
                commands::engine::export_images(images, &output, &format).await?
            }
            EngineCommands::ImportImage { input } => {
                commands::engine::import_image(&input, &format).await?
            }
            EngineCommands::Networks => commands::engine::networks(&format).await?,
            EngineCommands::InspectNetwork { id } => {
                commands::engine::inspect_network(&id, &format).await?
            }
            EngineCommands::CreateNetwork {
                name,
                driver,
                internal,
                enable_ipv6,
            } => {
                commands::engine::create_network(name, driver, internal, enable_ipv6, &format)
                    .await?
            }
            EngineCommands::RemoveNetwork { id, force } => {
                commands::engine::remove_network(&id, force, &format).await?
            }
            EngineCommands::Volumes => commands::engine::volumes(&format).await?,
            EngineCommands::InspectVolume { name } => {
                commands::engine::inspect_volume(&name, &format).await?
            }
            EngineCommands::CreateVolume { name, driver } => {
                commands::engine::create_volume(name, driver, &format).await?
            }
            EngineCommands::RemoveVolume { name, force } => {
                commands::engine::remove_volume(&name, force, &format).await?
            }
            EngineCommands::Pods => commands::engine::pods(&format).await?,
            EngineCommands::CreatePod {
                name,
                driver,
                internal,
                enable_ipv6,
            } => commands::engine::create_pod(name, driver, internal, enable_ipv6, &format).await?,
            EngineCommands::RemovePod { name, force } => {
                commands::engine::remove_pod(&name, force, &format).await?
            }
            EngineCommands::Create {
                name,
                image,
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
                cpu,
                memory,
            } => {
                commands::engine::create(
                    name,
                    image,
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
                    cpu,
                    memory,
                    image_pull_mirrors_or_settings(Vec::new()),
                    &format,
                )
                .await?
            }
            EngineCommands::Run(args) => {
                commands::engine::run_once(
                    args.name,
                    args.image,
                    args.command,
                    args.env,
                    args.volume,
                    args.publish,
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
                    image_pull_mirrors_or_settings(Vec::new()),
                    &format,
                )
                .await?
            }
            EngineCommands::Start { id } => commands::engine::start(&id, &format).await?,
            EngineCommands::Stop { id, timeout } => {
                commands::engine::stop(&id, timeout, &format).await?
            }
            EngineCommands::Remove { id, force } => {
                commands::engine::remove(&id, force, &format).await?
            }
            EngineCommands::Inspect { id } => commands::engine::inspect(&id, &format).await?,
            EngineCommands::Stats { id } => commands::engine::stats(&id, &format).await?,
            EngineCommands::Logs {
                id,
                tail,
                timestamps,
            } => commands::engine::logs(&id, tail, timestamps, false, &format).await?,
            EngineCommands::Exec {
                id,
                command,
                working_dir,
                timeout,
                max_output_bytes,
                no_propagate_exit_code,
            } => {
                commands::engine::exec(
                    &id,
                    command,
                    working_dir,
                    timeout,
                    (max_output_bytes > 0).then_some(max_output_bytes),
                    no_propagate_exit_code,
                    &format,
                )
                .await?
            }
            EngineCommands::TerminalOpen {
                id,
                session_id,
                working_dir,
                cols,
                rows,
                command,
            } => {
                commands::engine::terminal_open(
                    &id,
                    session_id,
                    working_dir,
                    cols,
                    rows,
                    command,
                    &format,
                )
                .await?
            }
            EngineCommands::TerminalInput {
                id,
                session_id,
                data,
            } => commands::engine::terminal_input(&id, &session_id, &data, &format).await?,
            EngineCommands::TerminalRead { id, session_id } => {
                commands::engine::terminal_read(&id, &session_id, &format).await?
            }
            EngineCommands::TerminalResize {
                id,
                session_id,
                cols,
                rows,
            } => commands::engine::terminal_resize(&id, &session_id, cols, rows, &format).await?,
            EngineCommands::TerminalClose { id, session_id } => {
                commands::engine::terminal_close(&id, &session_id, &format).await?
            }
        },
        Commands::System(cmd) => match cmd {
            SystemCommands::Info => commands::system::info(&format)?,
            SystemCommands::EngineStatus => commands::system::engine_status(&format).await?,
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
                "network",
                "runtime",
                "settings",
                "update",
                "engine",
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
    fn product_management_commands_auto_ensure_native_engine_by_default() {
        let container = Cli::try_parse_from(["cratebay", "container", "list"])
            .expect("container list should parse");
        assert!(should_ensure_native_engine(
            &container.command,
            container.engine_host.as_deref()
        ));

        let image = Cli::try_parse_from(["cratebay", "image", "pull", "alpine:latest"])
            .expect("image pull should parse");
        assert!(should_ensure_native_engine(
            &image.command,
            image.engine_host.as_deref()
        ));

        let engine = Cli::try_parse_from(["cratebay", "engine", "containers"])
            .expect("engine containers should parse");
        assert!(should_ensure_native_engine(
            &engine.command,
            engine.engine_host.as_deref()
        ));
    }

    #[test]
    fn diagnostics_and_explicit_hosts_do_not_auto_start_native_engine() {
        let search = Cli::try_parse_from(["cratebay", "image", "search", "alpine"])
            .expect("image search should parse");
        assert!(!should_ensure_native_engine(
            &search.command,
            search.engine_host.as_deref()
        ));

        let runtime_status =
            Cli::try_parse_from(["cratebay", "runtime", "status"]).expect("runtime status parses");
        assert!(!should_ensure_native_engine(
            &runtime_status.command,
            runtime_status.engine_host.as_deref()
        ));

        let system_status = Cli::try_parse_from(["cratebay", "system", "engine-status"])
            .expect("system engine-status parses");
        assert!(!should_ensure_native_engine(
            &system_status.command,
            system_status.engine_host.as_deref()
        ));

        let engine_status =
            Cli::try_parse_from(["cratebay", "engine", "status"]).expect("engine status parses");
        assert!(!should_ensure_native_engine(
            &engine_status.command,
            engine_status.engine_host.as_deref()
        ));

        let engine_substrate = Cli::try_parse_from(["cratebay", "engine", "substrate"])
            .expect("engine substrate parses");
        assert!(!should_ensure_native_engine(
            &engine_substrate.command,
            engine_substrate.engine_host.as_deref()
        ));

        let engine_storage_gc = Cli::try_parse_from(["cratebay", "engine", "storage-gc"])
            .expect("engine storage-gc dry run parses");
        assert!(!should_ensure_native_engine(
            &engine_storage_gc.command,
            engine_storage_gc.engine_host.as_deref()
        ));

        let engine_shim_tasks = Cli::try_parse_from(["cratebay", "engine", "shim-tasks"])
            .expect("engine shim-tasks parses");
        assert!(!should_ensure_native_engine(
            &engine_shim_tasks.command,
            engine_shim_tasks.engine_host.as_deref()
        ));

        let engine_reap_dry_run =
            Cli::try_parse_from(["cratebay", "engine", "reap-shim-task", "demo"])
                .expect("engine reap dry-run parses");
        assert!(!should_ensure_native_engine(
            &engine_reap_dry_run.command,
            engine_reap_dry_run.engine_host.as_deref()
        ));

        let explicit = Cli::try_parse_from([
            "cratebay",
            "--engine-host",
            "tcp://127.0.0.1:2375",
            "container",
            "list",
        ])
        .expect("explicit host container list should parse");
        assert!(!should_ensure_native_engine(
            &explicit.command,
            explicit.engine_host.as_deref()
        ));
    }

    #[test]
    fn mutating_engine_maintenance_commands_auto_ensure_native_engine() {
        let storage_gc_apply = Cli::try_parse_from(["cratebay", "engine", "storage-gc", "--apply"])
            .expect("engine storage-gc --apply parses");
        assert!(should_ensure_native_engine(
            &storage_gc_apply.command,
            storage_gc_apply.engine_host.as_deref()
        ));

        let reap_apply =
            Cli::try_parse_from(["cratebay", "engine", "reap-shim-task", "demo", "--apply"])
                .expect("engine reap --apply parses");
        assert!(should_ensure_native_engine(
            &reap_apply.command,
            reap_apply.engine_host.as_deref()
        ));
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
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "image", "delete", "alpine:latest", "--force"])
                .expect("image delete --force should parse")
                .command,
            Commands::Image(ImageCommands::Delete { force: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "image", "remove", "alpine:latest", "--force"])
                .expect("image remove alias should parse")
                .command,
            Commands::Image(ImageCommands::Delete { force: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "image", "rmi", "alpine:latest", "--force"])
                .expect("image rmi alias should parse")
                .command,
            Commands::Image(ImageCommands::Delete { force: true, .. })
        ));
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
                .contains("Unsupported Engine host format: bad-host"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn container_management_defaults_to_native_engine_dispatch() {
        let command = Cli::command();
        let container =
            find_subcommand(&command, "container").expect("container command must exist");
        let names = command_names(container);

        assert_contains_all(
            &names,
            &[
                "list",
                "create",
                "run",
                "start",
                "stop",
                "delete",
                "exec",
                "logs",
                "stats",
                "terminal-open",
                "terminal-input",
                "terminal-read",
                "terminal-resize",
                "terminal-close",
                "inspect",
            ],
        );
        assert_eq!(native_dispatch_mode(None), NativeDispatchMode::NativeEngine);
        assert_eq!(
            native_dispatch_mode(Some("   ")),
            NativeDispatchMode::NativeEngine
        );
        assert_eq!(
            native_dispatch_mode(Some("unix:///tmp/docker.sock")),
            NativeDispatchMode::ExplicitCompatibilityHost
        );

        assert!(matches!(
            Cli::try_parse_from(["cratebay", "container", "list"])
                .expect("container list should parse")
                .command,
            Commands::Container(ContainerCommands::List { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "container", "start", "sandbox-demo"])
                .expect("container start should parse")
                .command,
            Commands::Container(ContainerCommands::Start { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "container",
                "logs",
                "sandbox-demo",
                "--tail",
                "20"
            ])
            .expect("container logs should parse")
            .command,
            Commands::Container(ContainerCommands::Logs { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "container", "delete", "sandbox-demo", "--force"])
                .expect("container delete should parse")
                .command,
            Commands::Container(ContainerCommands::Delete { force: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "container", "remove", "sandbox-demo", "--force"])
                .expect("container remove alias should parse")
                .command,
            Commands::Container(ContainerCommands::Delete { force: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "container", "rm", "sandbox-demo", "--force"])
                .expect("container rm alias should parse")
                .command,
            Commands::Container(ContainerCommands::Delete { force: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "container", "stats", "sandbox-demo"])
                .expect("container stats should parse")
                .command,
            Commands::Container(ContainerCommands::Stats { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "container",
                "terminal-open",
                "sandbox-demo",
                "--session-id",
                "tty-1",
                "--cols",
                "120",
                "--rows",
                "33",
                "--",
                "sh",
                "-i",
            ])
            .expect("container terminal-open should parse")
            .command,
            Commands::Container(ContainerCommands::TerminalOpen { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "container",
                "terminal-input",
                "sandbox-demo",
                "--session-id",
                "tty-1",
                "--data",
                "echo ok\n",
            ])
            .expect("container terminal-input should parse")
            .command,
            Commands::Container(ContainerCommands::TerminalInput { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "container",
                "terminal-read",
                "sandbox-demo",
                "--session-id",
                "tty-1",
            ])
            .expect("container terminal-read should parse")
            .command,
            Commands::Container(ContainerCommands::TerminalRead { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "container",
                "terminal-resize",
                "sandbox-demo",
                "--session-id",
                "tty-1",
                "--cols",
                "100",
                "--rows",
                "40",
            ])
            .expect("container terminal-resize should parse")
            .command,
            Commands::Container(ContainerCommands::TerminalResize { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "container",
                "terminal-close",
                "sandbox-demo",
                "--session-id",
                "tty-1",
            ])
            .expect("container terminal-close should parse")
            .command,
            Commands::Container(ContainerCommands::TerminalClose { .. })
        ));
    }

    #[test]
    fn container_terminal_commands_require_native_engine_dispatch() {
        assert!(ensure_native_only_terminal(None).is_ok());
        assert!(ensure_native_only_terminal(Some("   ")).is_ok());

        let err = ensure_native_only_terminal(Some("unix:///tmp/docker.sock"))
            .expect_err("explicit compatibility hosts should not claim native terminal support");
        assert!(
            err.to_string().contains("native CrateBay Engine"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn pod_cli_defaults_to_native_engine_dispatch() {
        let command = Cli::command();
        let pod = find_subcommand(&command, "pod").expect("pod command must exist");
        let names = command_names(pod);

        assert_contains_all(
            &names,
            &["list", "create", "inspect", "delete", "add", "remove"],
        );
        assert_eq!(native_dispatch_mode(None), NativeDispatchMode::NativeEngine);
        assert_eq!(
            native_dispatch_mode(Some("unix:///tmp/docker.sock")),
            NativeDispatchMode::ExplicitCompatibilityHost
        );
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "pod",
                "create",
                "demo-pod",
                "--driver",
                "macvlan",
                "--internal",
                "--ipv6",
            ])
            .expect("pod create options should parse")
            .command,
            Commands::Pod(PodCommands::Create {
                driver: Some(driver),
                internal: true,
                enable_ipv6: true,
                ..
            }) if driver == "macvlan"
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "pod", "delete", "demo-pod", "--force"])
                .expect("pod delete --force should parse")
                .command,
            Commands::Pod(PodCommands::Delete { force: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "pod", "add", "demo-pod", "sandbox-demo"])
                .expect("pod add should parse")
                .command,
            Commands::Pod(PodCommands::Add { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "pod",
                "remove",
                "demo-pod",
                "sandbox-demo",
                "--force"
            ])
            .expect("pod remove --force should parse")
            .command,
            Commands::Pod(PodCommands::Remove { force: true, .. })
        ));
    }

    #[test]
    fn runtime_cli_surface_keeps_minimum_runtime_lifecycle() {
        let command = Cli::command();
        let runtime = find_subcommand(&command, "runtime").expect("runtime command must exist");
        let names = command_names(runtime);

        assert_contains_all(
            &names,
            &[
                "status",
                "diagnostics",
                "start",
                "stop",
                "restart",
                "proxy",
                "provision",
            ],
        );

        assert!(matches!(
            Cli::try_parse_from(["cratebay", "runtime", "restart"])
                .expect("runtime restart should parse")
                .command,
            Commands::Runtime(RuntimeCommands::Restart)
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "runtime",
                "diagnostics",
                "--prune-exited-containers",
                "false",
            ])
            .expect("runtime diagnostics should parse")
            .command,
            Commands::Runtime(RuntimeCommands::Diagnostics {
                prune_exited_containers: false
            })
        ));

        let runtime_proxy = find_subcommand(runtime, "proxy").expect("runtime proxy must exist");
        let proxy_names = command_names(runtime_proxy);
        assert_contains_all(&proxy_names, &["show", "set", "clear"]);

        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "runtime",
                "proxy",
                "set",
                "http://127.0.0.1:7890",
                "--bridge",
                "--bind-host",
                "0.0.0.0",
                "--bind-port",
                "3128",
                "--guest-host",
                "192.168.64.1",
            ])
            .expect("runtime proxy set should parse")
            .command,
            Commands::Runtime(RuntimeCommands::Proxy(RuntimeProxyCommands::Set {
                bridge: true,
                bind_port: Some(3128),
                ..
            }))
        ));
    }

    #[test]
    fn settings_cli_surface_matches_desktop_settings_store() {
        let command = Cli::command();
        let settings = find_subcommand(&command, "settings").expect("settings command must exist");
        let names = command_names(settings);

        assert_contains_all(&names, &["list", "get", "set", "reset"]);

        assert!(matches!(
            Cli::try_parse_from(["cratebay", "settings", "get", "registryMirrors"])
                .expect("settings get should parse")
                .command,
            Commands::Settings(SettingsCommands::Get { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "settings",
                "set",
                "registryMirrors",
                "docker.1ms.run,mirror.local"
            ])
            .expect("settings set should parse")
            .command,
            Commands::Settings(SettingsCommands::Set { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "settings", "reset", "registryMirrors"])
                .expect("settings reset should parse")
                .command,
            Commands::Settings(SettingsCommands::Reset { .. })
        ));
    }

    #[test]
    fn update_cli_surface_matches_desktop_update_check() {
        let command = Cli::command();
        let update = find_subcommand(&command, "update").expect("update command must exist");
        let names = command_names(update);

        assert_contains_all(&names, &["check"]);

        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "update",
                "check",
                "--include-prerelease",
                "--repository",
                "nicepkg/CrateBay",
            ])
            .expect("update check options should parse")
            .command,
            Commands::Update(UpdateCommands::Check {
                include_prerelease: true,
                stable: false,
                repository: Some(_),
            })
        ));

        assert!(matches!(
            Cli::try_parse_from(["cratebay", "update", "check", "--stable"])
                .expect("update check --stable should parse")
                .command,
            Commands::Update(UpdateCommands::Check {
                include_prerelease: false,
                stable: true,
                ..
            })
        ));

        assert!(
            Cli::try_parse_from([
                "cratebay",
                "update",
                "check",
                "--include-prerelease",
                "--stable",
            ])
            .is_err(),
            "update channel overrides should be mutually exclusive"
        );
    }

    #[test]
    fn volume_cli_defaults_to_native_engine_dispatch() {
        let command = Cli::command();
        let volume = find_subcommand(&command, "volume").expect("volume command must exist");
        let names = command_names(volume);

        assert_contains_all(&names, &["create", "list", "inspect", "remove"]);
        assert_eq!(native_dispatch_mode(None), NativeDispatchMode::NativeEngine);
        assert_eq!(
            native_dispatch_mode(Some("tcp://127.0.0.1:2375")),
            NativeDispatchMode::ExplicitCompatibilityHost
        );

        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "volume",
                "create",
                "workspace-cache",
                "--driver",
                "local",
            ])
                .expect("volume create should parse")
                .command,
            Commands::Volume(VolumeCommands::Create {
                driver: Some(driver),
                ..
            }) if driver == "local"
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "volume", "list"])
                .expect("volume list should parse")
                .command,
            Commands::Volume(VolumeCommands::List)
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "volume", "inspect", "workspace-cache"])
                .expect("volume inspect should parse")
                .command,
            Commands::Volume(VolumeCommands::Inspect { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "volume", "remove", "workspace-cache", "--force"])
                .expect("volume remove should parse")
                .command,
            Commands::Volume(VolumeCommands::Remove { force: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "volume", "delete", "workspace-cache", "--force"])
                .expect("volume delete alias should parse")
                .command,
            Commands::Volume(VolumeCommands::Remove { force: true, .. })
        ));
    }

    #[test]
    fn network_cli_defaults_to_native_engine_dispatch() {
        let command = Cli::command();
        let network = find_subcommand(&command, "network").expect("network command must exist");
        let names = command_names(network);

        assert_contains_all(&names, &["create", "list", "inspect", "remove"]);
        assert_eq!(native_dispatch_mode(None), NativeDispatchMode::NativeEngine);
        assert_eq!(
            native_dispatch_mode(Some("tcp://127.0.0.1:2375")),
            NativeDispatchMode::ExplicitCompatibilityHost
        );

        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "network",
                "create",
                "sandbox-net",
                "--driver",
                "bridge",
                "--internal",
                "--ipv6"
            ])
            .expect("network create should parse")
            .command,
            Commands::Network(NetworkCommands::Create { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "network", "list"])
                .expect("network list should parse")
                .command,
            Commands::Network(NetworkCommands::List)
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "network", "inspect", "sandbox-net"])
                .expect("network inspect should parse")
                .command,
            Commands::Network(NetworkCommands::Inspect { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "network", "remove", "sandbox-net", "--force"])
                .expect("network remove should parse")
                .command,
            Commands::Network(NetworkCommands::Remove { force: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "network", "delete", "sandbox-net", "--force"])
                .expect("network delete alias should parse")
                .command,
            Commands::Network(NetworkCommands::Remove { force: true, .. })
        ));
    }

    #[test]
    fn engine_cli_surface_exposes_native_engine_api() {
        let command = Cli::command();
        let engine = find_subcommand(&command, "engine").expect("engine command must exist");
        let names = command_names(engine);

        assert_contains_all(
            &names,
            &[
                "status",
                "substrate",
                "storage-gc",
                "shim-tasks",
                "reap-shim-task",
                "containers",
                "images",
                "pull-image",
                "inspect-image",
                "remove-image",
                "tag-image",
                "pack-image",
                "export-images",
                "import-image",
                "networks",
                "inspect-network",
                "create-network",
                "remove-network",
                "volumes",
                "inspect-volume",
                "create-volume",
                "remove-volume",
                "pods",
                "create-pod",
                "remove-pod",
                "create",
                "run",
                "start",
                "stop",
                "remove",
                "inspect",
                "logs",
                "exec",
                "terminal-open",
                "terminal-input",
                "terminal-read",
                "terminal-resize",
                "terminal-close",
            ],
        );
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "status"])
                .expect("engine status should parse")
                .command,
            Commands::Engine(EngineCommands::Status)
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "substrate"])
                .expect("engine substrate should parse")
                .command,
            Commands::Engine(EngineCommands::Substrate)
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "storage-gc", "--apply"])
                .expect("engine storage-gc should parse")
                .command,
            Commands::Engine(EngineCommands::StorageGc { apply: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "shim-tasks"])
                .expect("engine shim-tasks should parse")
                .command,
            Commands::Engine(EngineCommands::ShimTasks)
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "reap-shim-task",
                "sandbox-demo",
                "--apply"
            ])
            .expect("engine reap-shim-task should parse")
            .command,
            Commands::Engine(EngineCommands::ReapShimTask { apply: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "list"])
                .expect("engine list alias should parse")
                .command,
            Commands::Engine(EngineCommands::Containers)
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "images"])
                .expect("engine images should parse")
                .command,
            Commands::Engine(EngineCommands::Images)
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "pull-image",
                "alpine",
                "--tag",
                "latest"
            ])
            .expect("engine pull-image should parse")
            .command,
            Commands::Engine(EngineCommands::PullImage { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "pull-image",
                "alpine",
                "--mirror",
                "mirror.local"
            ])
            .expect("engine pull-image --mirror should parse")
            .command,
            Commands::Engine(EngineCommands::PullImage { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "inspect-image", "alpine:latest"])
                .expect("engine inspect-image should parse")
                .command,
            Commands::Engine(EngineCommands::InspectImage { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "remove-image",
                "alpine:latest",
                "--force"
            ])
            .expect("engine remove-image should parse")
            .command,
            Commands::Engine(EngineCommands::RemoveImage { force: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "tag-image",
                "alpine:latest",
                "sandbox:latest"
            ])
            .expect("engine tag-image should parse")
            .command,
            Commands::Engine(EngineCommands::TagImage { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "pack-image",
                "sandbox-demo",
                "sandbox-pack:latest"
            ])
            .expect("engine pack-image should parse")
            .command,
            Commands::Engine(EngineCommands::PackImage { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "export-images",
                "--output",
                "/tmp/cratebay-images.tar",
                "alpine:latest"
            ])
            .expect("engine export-images should parse")
            .command,
            Commands::Engine(EngineCommands::ExportImages { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "import-image",
                "/tmp/cratebay-images.tar"
            ])
            .expect("engine import-image should parse")
            .command,
            Commands::Engine(EngineCommands::ImportImage { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "networks"])
                .expect("engine networks should parse")
                .command,
            Commands::Engine(EngineCommands::Networks)
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "inspect-network", "pod-demo"])
                .expect("engine inspect-network should parse")
                .command,
            Commands::Engine(EngineCommands::InspectNetwork { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "create-network",
                "pod-demo",
                "--driver",
                "bridge"
            ])
            .expect("engine create-network should parse")
            .command,
            Commands::Engine(EngineCommands::CreateNetwork { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "remove-network",
                "pod-demo",
                "--force"
            ])
            .expect("engine remove-network should parse")
            .command,
            Commands::Engine(EngineCommands::RemoveNetwork { force: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "volumes"])
                .expect("engine volumes should parse")
                .command,
            Commands::Engine(EngineCommands::Volumes)
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "inspect-volume", "workspace-cache"])
                .expect("engine inspect-volume should parse")
                .command,
            Commands::Engine(EngineCommands::InspectVolume { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "create-volume", "workspace-cache"])
                .expect("engine create-volume should parse")
                .command,
            Commands::Engine(EngineCommands::CreateVolume { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "remove-volume",
                "workspace-cache",
                "--force"
            ])
            .expect("engine remove-volume should parse")
            .command,
            Commands::Engine(EngineCommands::RemoveVolume { force: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "pods"])
                .expect("engine pods should parse")
                .command,
            Commands::Engine(EngineCommands::Pods)
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "create-pod",
                "demo-pod",
                "--driver",
                "bridge"
            ])
            .expect("engine create-pod should parse")
            .command,
            Commands::Engine(EngineCommands::CreatePod { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "remove-pod", "demo-pod", "--force"])
                .expect("engine remove-pod should parse")
                .command,
            Commands::Engine(EngineCommands::RemovePod { force: true, .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "create",
                "sandbox-demo",
                "--image",
                "alpine:latest",
                "--command",
                "sleep 60",
                "--pod",
                "demo-pod",
                "--no-start"
            ])
            .expect("engine create should parse")
            .command,
            Commands::Engine(EngineCommands::Create { .. })
        ));
        assert!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "create",
                "sandbox-demo",
                "--image",
                "alpine:latest",
                "--pod",
                "demo-pod",
                "--network",
                "none",
            ])
            .is_err(),
            "engine create should keep --pod and --network mutually exclusive"
        );
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "run",
                "--name",
                "native-run",
                "--env",
                "A=1",
                "--volume",
                "/tmp:/tmp:ro",
                "--publish",
                "5000:5000/sctp",
                "--network",
                "none",
                "--no-pull",
                "--keep",
                "--timeout",
                "30",
                "--max-output-bytes",
                "4096",
                "alpine:latest",
                "--",
                "sh",
                "-lc",
                "printf ok",
            ])
            .expect("engine run should parse")
            .command,
            Commands::Engine(EngineCommands::Run(RunArgs {
                publish,
                ..
            })) if publish == vec!["5000:5000/sctp".to_string()]
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "start", "sandbox-demo"])
                .expect("engine start should parse")
                .command,
            Commands::Engine(EngineCommands::Start { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "stop",
                "sandbox-demo",
                "--timeout",
                "5"
            ])
            .expect("engine stop should parse")
            .command,
            Commands::Engine(EngineCommands::Stop { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "remove", "sandbox-demo", "--force"])
                .expect("engine remove should parse")
                .command,
            Commands::Engine(EngineCommands::Remove { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "inspect", "sandbox-demo"])
                .expect("engine inspect should parse")
                .command,
            Commands::Engine(EngineCommands::Inspect { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "stats", "sandbox-demo"])
                .expect("engine stats should parse")
                .command,
            Commands::Engine(EngineCommands::Stats { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "engine", "logs", "sandbox-demo", "--tail", "20"])
                .expect("engine logs should parse")
                .command,
            Commands::Engine(EngineCommands::Logs { .. })
        ));
        let parsed_engine_exec = Cli::try_parse_from([
            "cratebay",
            "engine",
            "exec",
            "sandbox-demo",
            "--timeout",
            "2",
            "--max-output-bytes",
            "1024",
            "--no-propagate-exit-code",
            "--",
            "echo",
            "ok",
        ])
        .expect("engine exec should parse")
        .command;
        assert!(matches!(
            parsed_engine_exec,
            Commands::Engine(EngineCommands::Exec {
                ref id,
                ref command,
                timeout: Some(2),
                max_output_bytes: 1024,
                no_propagate_exit_code: true,
                ..
            }) if id == "sandbox-demo" && command == &vec!["echo".to_string(), "ok".to_string()]
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "exec",
                "sandbox-demo",
                "--",
                "echo",
                "ok",
            ])
            .expect("engine exec should parse with default output limit")
            .command,
            Commands::Engine(EngineCommands::Exec {
                max_output_bytes: 1_048_576,
                ..
            })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "terminal-open",
                "sandbox-demo",
                "--session-id",
                "tty-1",
                "--cols",
                "120",
                "--rows",
                "33",
                "--",
                "/usr/local/bin/cratebay-smoke",
                "pty",
            ])
            .expect("engine terminal-open should parse")
            .command,
            Commands::Engine(EngineCommands::TerminalOpen { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "terminal-input",
                "sandbox-demo",
                "--session-id",
                "tty-1",
                "--data",
                "echo ok\n",
            ])
            .expect("engine terminal-input should parse")
            .command,
            Commands::Engine(EngineCommands::TerminalInput { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "terminal-read",
                "sandbox-demo",
                "--session-id",
                "tty-1",
            ])
            .expect("engine terminal-read should parse")
            .command,
            Commands::Engine(EngineCommands::TerminalRead { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "terminal-resize",
                "sandbox-demo",
                "--session-id",
                "tty-1",
                "--cols",
                "100",
                "--rows",
                "40",
            ])
            .expect("engine terminal-resize should parse")
            .command,
            Commands::Engine(EngineCommands::TerminalResize { .. })
        ));
        assert!(matches!(
            Cli::try_parse_from([
                "cratebay",
                "engine",
                "terminal-close",
                "sandbox-demo",
                "--session-id",
                "tty-1",
            ])
            .expect("engine terminal-close should parse")
            .command,
            Commands::Engine(EngineCommands::TerminalClose { .. })
        ));
    }

    #[test]
    fn system_cli_uses_engine_status_as_primary_command() {
        let command = Cli::command();
        let system = find_subcommand(&command, "system").expect("system command must exist");
        let names = command_names(system);

        assert_contains_all(&names, &["info", "engine-status"]);
        assert!(
            !names.iter().any(|name| name == "docker-status"),
            "docker-status should remain a compatibility alias, not the primary system command"
        );

        assert!(matches!(
            Cli::try_parse_from(["cratebay", "system", "engine-status"])
                .expect("engine-status should parse")
                .command,
            Commands::System(SystemCommands::EngineStatus)
        ));
        assert!(matches!(
            Cli::try_parse_from(["cratebay", "system", "docker-status"])
                .expect("docker-status compatibility alias should parse")
                .command,
            Commands::System(SystemCommands::EngineStatus)
        ));
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
                "publish",
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
    fn container_run_and_create_accept_cratebay_network_names() {
        let top_level_run = Cli::try_parse_from([
            "cratebay",
            "run",
            "--publish",
            "5000:5000/sctp",
            "--network",
            "workspace-net",
            "alpine:latest",
            "--",
            "true",
        ])
        .expect("top-level run should accept a CrateBay network name");
        assert!(matches!(
            top_level_run.command,
            Commands::Run(RunArgs { publish, .. }) if publish == vec!["5000:5000/sctp".to_string()]
        ));

        let container_run = Cli::try_parse_from([
            "cratebay",
            "container",
            "run",
            "-p",
            "5353:53/udp",
            "--network",
            "workspace-net",
            "alpine:latest",
            "--",
            "true",
        ])
        .expect("container run should accept a CrateBay network name");
        assert!(matches!(
            container_run.command,
            Commands::Container(ContainerCommands::Run(RunArgs { publish, .. }))
                if publish == vec!["5353:53/udp".to_string()]
        ));

        let container_create = Cli::try_parse_from([
            "cratebay",
            "container",
            "create",
            "workspace-shell",
            "--image",
            "alpine:latest",
            "--network",
            "workspace-net",
            "--no-start",
        ])
        .expect("container create should accept a CrateBay network name");
        assert!(matches!(
            container_create.command,
            Commands::Container(ContainerCommands::Create { .. })
        ));
    }

    #[test]
    fn container_run_and_create_keep_pod_and_network_mutually_exclusive() {
        assert!(
            Cli::try_parse_from([
                "cratebay",
                "run",
                "--pod",
                "demo-pod",
                "--network",
                "workspace-net",
                "alpine:latest",
                "--",
                "true",
            ])
            .is_err(),
            "top-level run should reject both --pod and --network"
        );

        assert!(
            Cli::try_parse_from([
                "cratebay",
                "container",
                "run",
                "--pod",
                "demo-pod",
                "--network",
                "workspace-net",
                "alpine:latest",
                "--",
                "true",
            ])
            .is_err(),
            "container run should reject both --pod and --network"
        );

        assert!(
            Cli::try_parse_from([
                "cratebay",
                "container",
                "create",
                "workspace-shell",
                "--image",
                "alpine:latest",
                "--pod",
                "demo-pod",
                "--network",
                "workspace-net",
            ])
            .is_err(),
            "container create should reject both --pod and --network"
        );
    }

    #[test]
    fn registry_mirror_settings_parse_gui_storage_formats() {
        assert_eq!(
            cratebay_core::settings::parse_registry_mirrors_setting(
                r#"["docker.1ms.run"," https://mirror.local/ "]"#
            ),
            vec!["docker.1ms.run", "https://mirror.local/"]
        );
        assert_eq!(
            cratebay_core::settings::parse_registry_mirrors_setting(
                "docker.1ms.run,\nmirror.local\n "
            ),
            vec!["docker.1ms.run", "mirror.local"]
        );
        assert!(cratebay_core::settings::parse_registry_mirrors_setting("[]").is_empty());
    }

    #[test]
    fn default_registry_mirrors_match_gui_fresh_install_defaults() {
        assert_eq!(
            default_registry_mirrors(),
            vec!["docker.1ms.run", "docker.xuanyuan.me", "dockerhub.icu"]
        );

        let gui_settings_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cratebay-gui/src/types/settings.ts");
        let gui_settings = std::fs::read_to_string(gui_settings_path)
            .expect("GUI settings defaults should be readable");
        for mirror in default_registry_mirrors() {
            assert!(
                gui_settings.contains(&format!("\"{mirror}\"")),
                "GUI DEFAULT_REGISTRY_MIRRORS should include {mirror}"
            );
        }
    }

    #[test]
    fn explicit_pull_mirrors_override_persisted_settings() {
        assert_eq!(
            image_pull_mirrors_or_settings(vec!["explicit.mirror".to_string()]),
            vec!["explicit.mirror"]
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
    fn engine_exec_help_exposes_embedded_execution_controls() {
        let command = Cli::command();
        let engine = find_subcommand(&command, "engine").expect("engine command must exist");
        let exec = find_subcommand(engine, "exec").expect("engine exec command must exist");
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

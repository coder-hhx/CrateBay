//! cratebay-vz — VM runner process using Apple Virtualization.framework.
//!
//! This binary is spawned by `MacOSHypervisor` (in cratebay-core) to run a
//! single Linux VM via the Virtualization.framework Swift bridge.
//!
//! On non-macOS platforms, it prints an error and exits.

#[cfg(target_os = "macos")]
mod ffi;

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("cratebay-vz is only supported on macOS");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing_subscriber::filter::LevelFilter::INFO.into()),
        )
        .init();

    let args = match Args::parse() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", e);
            eprintln!();
            eprintln!("{}", Args::usage());
            std::process::exit(2);
        }
    };

    if let Err(e) = run(args) {
        tracing::error!("{}", e);
        std::process::exit(1);
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct Args {
    kernel: std::path::PathBuf,
    initrd: Option<std::path::PathBuf>,
    disk: std::path::PathBuf,
    cpus: u32,
    memory_mb: u64,
    cmdline: String,
    ready_file: Option<std::path::PathBuf>,
    console_log: Option<std::path::PathBuf>,
    rosetta: bool,
    /// Shared directories in "tag:host_path[:ro]" format.
    shared_dirs: Vec<String>,
    /// Vsock forwards in "guest_port:unix_socket_path" format.
    vsock_forwards: Vec<String>,
    /// TCP forwards in "guest_port:unix_socket_path" format (guest connects back to host).
    tcp_forwards: Vec<String>,
    /// Reverse TCP forwards in "bind_host:bind_port=unix_socket_path" format.
    reverse_tcp_forwards: Vec<String>,
    /// Host TCP listeners forwarded to target host:port pairs.
    host_tcp_forwards: Vec<String>,
    /// Internal HTTP CONNECT proxies bound on the host bridge.
    http_connect_proxies: Vec<String>,
}

#[cfg(target_os = "macos")]
impl Args {
    fn usage() -> &'static str {
        "Usage:\n  cratebay-vz --kernel <path> --disk <path> --cpus <n> --memory-mb <n> \
         [--initrd <path>] [--cmdline <str>] [--ready-file <path>] \
         [--console-log <path>] [--rosetta] [--share tag:host_path[:ro]] \
         [--vsock-forward guest_port:unix_socket_path] \
         [--tcp-forward guest_port:unix_socket_path] \
         [--reverse-tcp-forward bind_host:bind_port=unix_socket_path] \
         [--host-tcp-forward bind_host:bind_port=target_host:target_port] \
         [--http-connect-proxy bind_host:bind_port]\n"
    }

    fn parse() -> Result<Self, String> {
        let mut kernel: Option<std::path::PathBuf> = None;
        let mut initrd: Option<std::path::PathBuf> = None;
        let mut disk: Option<std::path::PathBuf> = None;
        let mut cpus: Option<u32> = None;
        let mut memory_mb: Option<u64> = None;
        let mut cmdline: Option<String> = None;
        let mut ready_file: Option<std::path::PathBuf> = None;
        let mut console_log: Option<std::path::PathBuf> = None;
        let mut rosetta = false;
        let mut shared_dirs: Vec<String> = Vec::new();
        let mut vsock_forwards: Vec<String> = Vec::new();
        let mut tcp_forwards: Vec<String> = Vec::new();
        let mut reverse_tcp_forwards: Vec<String> = Vec::new();
        let mut host_tcp_forwards: Vec<String> = Vec::new();
        let mut http_connect_proxies: Vec<String> = Vec::new();

        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--help" | "-h" => {
                    return Err(Self::usage().to_string());
                }
                "--kernel" => {
                    kernel = Some(
                        it.next()
                            .ok_or_else(|| "--kernel requires a value".to_string())?
                            .into(),
                    );
                }
                "--initrd" => {
                    initrd = Some(
                        it.next()
                            .ok_or_else(|| "--initrd requires a value".to_string())?
                            .into(),
                    );
                }
                "--disk" => {
                    disk = Some(
                        it.next()
                            .ok_or_else(|| "--disk requires a value".to_string())?
                            .into(),
                    );
                }
                "--cpus" => {
                    let raw = it
                        .next()
                        .ok_or_else(|| "--cpus requires a value".to_string())?;
                    cpus = Some(
                        raw.parse::<u32>()
                            .map_err(|_| "Invalid --cpus".to_string())?,
                    );
                }
                "--memory-mb" => {
                    let raw = it
                        .next()
                        .ok_or_else(|| "--memory-mb requires a value".to_string())?;
                    memory_mb = Some(
                        raw.parse::<u64>()
                            .map_err(|_| "Invalid --memory-mb".to_string())?,
                    );
                }
                "--cmdline" => {
                    cmdline = Some(
                        it.next()
                            .ok_or_else(|| "--cmdline requires a value".to_string())?,
                    );
                }
                "--ready-file" => {
                    ready_file = Some(
                        it.next()
                            .ok_or_else(|| "--ready-file requires a value".to_string())?
                            .into(),
                    );
                }
                "--console-log" => {
                    console_log = Some(
                        it.next()
                            .ok_or_else(|| "--console-log requires a value".to_string())?
                            .into(),
                    );
                }
                "--rosetta" => {
                    rosetta = true;
                }
                "--share" => {
                    shared_dirs.push(
                        it.next()
                            .ok_or_else(|| "--share requires a value".to_string())?,
                    );
                }
                "--vsock-forward" => {
                    vsock_forwards.push(
                        it.next()
                            .ok_or_else(|| "--vsock-forward requires a value".to_string())?,
                    );
                }
                "--tcp-forward" => {
                    tcp_forwards.push(
                        it.next()
                            .ok_or_else(|| "--tcp-forward requires a value".to_string())?,
                    );
                }
                "--reverse-tcp-forward" => {
                    reverse_tcp_forwards.push(
                        it.next()
                            .ok_or_else(|| "--reverse-tcp-forward requires a value".to_string())?,
                    );
                }
                "--host-tcp-forward" => {
                    host_tcp_forwards.push(
                        it.next()
                            .ok_or_else(|| "--host-tcp-forward requires a value".to_string())?,
                    );
                }
                "--http-connect-proxy" => {
                    http_connect_proxies.push(
                        it.next()
                            .ok_or_else(|| "--http-connect-proxy requires a value".to_string())?,
                    );
                }
                other => return Err(format!("Unknown argument: {}", other)),
            }
        }

        let kernel = kernel.ok_or_else(|| "Missing --kernel".to_string())?;
        let disk = disk.ok_or_else(|| "Missing --disk".to_string())?;
        let cpus = cpus.ok_or_else(|| "Missing --cpus".to_string())?;
        let memory_mb = memory_mb.ok_or_else(|| "Missing --memory-mb".to_string())?;
        let cmdline = cmdline.unwrap_or_else(|| "console=hvc0".to_string());

        Ok(Self {
            kernel,
            initrd,
            disk,
            cpus,
            memory_mb,
            cmdline,
            ready_file,
            console_log,
            rosetta,
            shared_dirs,
            vsock_forwards,
            tcp_forwards,
            reverse_tcp_forwards,
            host_tcp_forwards,
            http_connect_proxies,
        })
    }
}

#[cfg(target_os = "macos")]
fn parse_shared_dir(spec: &str) -> Result<ffi::SharedDirFFI, String> {
    // Format: "tag:host_path" or "tag:host_path:ro"
    // Tag is guaranteed to not contain colons (validated by mount_virtiofs).
    // We split on the first colon to get the tag, then check if the remainder
    // ends with ":ro" to determine read-only mode.
    let first_colon = spec.find(':').ok_or_else(|| {
        format!(
            "Invalid --share format '{}', expected 'tag:host_path[:ro]'",
            spec
        )
    })?;
    let tag = &spec[..first_colon];
    let rest = &spec[first_colon + 1..];

    let (host_path, read_only) = if let Some(stripped) = rest.strip_suffix(":ro") {
        (stripped, true)
    } else {
        (rest, false)
    };

    if tag.is_empty() || host_path.is_empty() {
        return Err(format!(
            "Invalid --share format '{}', expected 'tag:host_path[:ro]'",
            spec
        ));
    }

    let tag = std::ffi::CString::new(tag).map_err(|e| format!("invalid tag: {}", e))?;
    let host_path =
        std::ffi::CString::new(host_path).map_err(|e| format!("invalid host_path: {}", e))?;

    Ok(ffi::SharedDirFFI {
        tag,
        host_path,
        read_only,
    })
}

#[cfg(target_os = "macos")]
fn run(args: Args) -> Result<(), String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let kernel_path = args
        .kernel
        .to_str()
        .ok_or_else(|| "Kernel path is not valid UTF-8".to_string())?
        .to_string();
    let disk_path = args
        .disk
        .to_str()
        .ok_or_else(|| "Disk path is not valid UTF-8".to_string())?
        .to_string();
    let initrd_path = args
        .initrd
        .as_ref()
        .map(|p| {
            p.to_str()
                .ok_or_else(|| "Initrd path is not valid UTF-8".to_string())
                .map(|s| s.to_string())
        })
        .transpose()?;
    let console_log_path = args
        .console_log
        .as_ref()
        .map(|p| {
            p.to_str()
                .ok_or_else(|| "Console log path is not valid UTF-8".to_string())
                .map(|s| s.to_string())
        })
        .transpose()?;

    // Parse shared directory specs.
    let shared_dirs: Vec<ffi::SharedDirFFI> = args
        .shared_dirs
        .iter()
        .map(|s| parse_shared_dir(s))
        .collect::<Result<Vec<_>, _>>()?;

    let config = ffi::VmCreateConfig {
        kernel_path,
        initrd_path,
        cmdline: args.cmdline.clone(),
        disk_path,
        console_log_path,
        cpus: args.cpus,
        memory_mb: args.memory_mb,
        rosetta: args.rosetta,
        enable_vsock: !args.vsock_forwards.is_empty(),
        shared_dirs,
    };

    let handle = Arc::new(ffi::create_and_start_vm(&config)?);

    tracing::info!(
        "VZ VM started (pid {}, state {:?})",
        std::process::id(),
        handle.state()
    );

    // Set up SIGTERM handler for graceful ACPI shutdown.
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    {
        let flag = shutdown_requested.clone();
        // SAFETY: We only set an atomic bool inside the signal handler, which is
        // async-signal-safe in practice.
        unsafe {
            libc::signal(
                libc::SIGTERM,
                sigterm_handler as *const () as libc::sighandler_t,
            );
        }
        // Store the flag in a global so the C signal handler can access it.
        SHUTDOWN_FLAG.store(
            flag.as_ref() as *const AtomicBool as *mut AtomicBool,
            std::sync::atomic::Ordering::SeqCst,
        );
    }

    // Set up vsock forwards (host unix socket -> guest vsock port).
    let mut forward_threads = Vec::new();
    for spec in &args.vsock_forwards {
        let (guest_port, sock_path) = parse_vsock_forward(spec)?;
        let thread = start_vsock_forward(
            handle.clone(),
            guest_port,
            sock_path,
            shutdown_requested.clone(),
        )?;
        forward_threads.push(thread);
    }

    for spec in &args.http_connect_proxies {
        let (bind_host, bind_port) = parse_host_port(spec, "http connect proxy bind")?;
        let thread = start_http_connect_proxy(bind_host, bind_port, shutdown_requested.clone())?;
        forward_threads.push(thread);
    }

    for spec in &args.host_tcp_forwards {
        let ((bind_host, bind_port), (target_host, target_port)) = parse_host_tcp_forward(spec)?;
        let thread = start_host_tcp_forward(
            bind_host,
            bind_port,
            target_host,
            target_port,
            shutdown_requested.clone(),
        )?;
        forward_threads.push(thread);
    }

    for spec in &args.reverse_tcp_forwards {
        let ((bind_host, bind_port), sock_path) = parse_reverse_tcp_forward(spec)?;
        let thread =
            start_reverse_tcp_forward(bind_host, bind_port, sock_path, shutdown_requested.clone())?;
        forward_threads.push(thread);
    }

    // Set up TCP forwards (host unix socket -> guest docker over a guest-
    // initiated TCP tunnel). This avoids the less reliable direct host->guest
    // TCP path on Apple Virtualization shared networking.
    if !args.tcp_forwards.is_empty() {
        for spec in &args.tcp_forwards {
            let (guest_port, sock_path) = parse_vsock_forward(spec)?;
            let thread = start_tcp_forward(guest_port, sock_path, shutdown_requested.clone())?;
            forward_threads.push(thread);
        }
    }

    // Signal readiness after forwards are bound.
    if let Some(path) = args.ready_file.as_ref() {
        let _ = std::fs::create_dir_all(path.parent().unwrap_or_else(|| std::path::Path::new(".")));
        std::fs::write(path, b"ready\n")
            .map_err(|e| format!("Failed to write ready file: {}", e))?;
    }

    // Wait for SIGTERM or VM to stop on its own.
    loop {
        if shutdown_requested.load(Ordering::SeqCst) {
            tracing::info!("SIGTERM received, initiating graceful ACPI shutdown...");
            match handle.stop(15.0) {
                Ok(()) => tracing::info!("VM stopped gracefully"),
                Err(e) => tracing::warn!("VM stop error: {}", e),
            }
            break;
        }

        // Check if the VM has stopped on its own (e.g., guest shutdown).
        let state = handle.state();
        if state == ffi::VzState::Stopped || state == ffi::VzState::Error {
            tracing::info!("VM entered state {:?}, exiting.", state);
            shutdown_requested.store(true, Ordering::SeqCst);
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    for thread in forward_threads {
        let _ = thread.join();
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn parse_vsock_forward(spec: &str) -> Result<(u32, std::path::PathBuf), String> {
    let first_colon = spec.find(':').ok_or_else(|| {
        format!(
            "Invalid --vsock-forward '{}', expected 'guest_port:unix_socket_path'",
            spec
        )
    })?;
    let port_str = &spec[..first_colon];
    let path_str = &spec[first_colon + 1..];

    let port = port_str
        .parse::<u32>()
        .map_err(|_| format!("Invalid vsock guest_port '{}'", port_str))?;
    if port == 0 {
        return Err("vsock guest_port must be > 0".to_string());
    }

    let path = std::path::PathBuf::from(path_str);

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        // macOS sockaddr_un.sun_path limit.
        const MAX_SUN_PATH: usize = 103;
        let bytes = path.as_os_str().as_bytes();
        if bytes.len() > MAX_SUN_PATH {
            return Err(format!(
                "Unix socket path too long ({} > {}): {}",
                bytes.len(),
                MAX_SUN_PATH,
                path.display()
            ));
        }
    }

    Ok((port, path))
}

#[cfg(target_os = "macos")]
fn parse_host_port(spec: &str, label: &str) -> Result<(String, u16), String> {
    if spec.starts_with('[') {
        let end = spec
            .find(']')
            .ok_or_else(|| format!("Invalid {} '{}': missing closing ']'", label, spec))?;
        let host = spec[1..end].to_string();
        let port = spec[end + 1..]
            .strip_prefix(':')
            .ok_or_else(|| format!("Invalid {} '{}': missing port", label, spec))?
            .parse::<u16>()
            .map_err(|_| format!("Invalid {} '{}': bad port", label, spec))?;
        return Ok((host, port));
    }

    let colon = spec
        .rfind(':')
        .ok_or_else(|| format!("Invalid {} '{}': expected host:port", label, spec))?;
    let host = spec[..colon].to_string();
    let port = spec[colon + 1..]
        .parse::<u16>()
        .map_err(|_| format!("Invalid {} '{}': bad port", label, spec))?;
    if host.trim().is_empty() {
        return Err(format!("Invalid {} '{}': empty host", label, spec));
    }
    Ok((host, port))
}

#[cfg(target_os = "macos")]
fn connect_target_tcp(target_host: &str, target_port: u16) -> Result<std::net::TcpStream, String> {
    use std::net::ToSocketAddrs;
    use std::time::Duration;

    let mut addresses = (target_host, target_port)
        .to_socket_addrs()
        .map_err(|error| format!("resolve {}:{}: {}", target_host, target_port, error))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(format!(
            "resolve {}:{}: no addresses returned",
            target_host, target_port
        ));
    }

    addresses.sort_by_key(|address| if address.is_ipv4() { 0 } else { 1 });

    let mut last_error = None;
    for address in addresses {
        match std::net::TcpStream::connect_timeout(&address, Duration::from_secs(5)) {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = Some(format!("{}: {}", address, error)),
        }
    }

    Err(format!(
        "connect to {}:{} failed: {}",
        target_host,
        target_port,
        last_error.unwrap_or_else(|| "unknown error".to_string())
    ))
}

#[cfg(target_os = "macos")]
type HostTcpForwardEndpoint = (String, u16);

#[cfg(target_os = "macos")]
fn parse_host_tcp_forward(
    spec: &str,
) -> Result<(HostTcpForwardEndpoint, HostTcpForwardEndpoint), String> {
    let (bind_spec, target_spec) = spec.split_once('=').ok_or_else(|| {
        format!(
            "Invalid --host-tcp-forward '{}': expected bind_host:bind_port=target_host:target_port",
            spec
        )
    })?;

    Ok((
        parse_host_port(bind_spec, "host tcp forward bind")?,
        parse_host_port(target_spec, "host tcp forward target")?,
    ))
}

#[cfg(target_os = "macos")]
fn parse_reverse_tcp_forward(
    spec: &str,
) -> Result<(HostTcpForwardEndpoint, std::path::PathBuf), String> {
    let (bind_spec, socket_spec) = spec.split_once('=').ok_or_else(|| {
        format!(
            "Invalid --reverse-tcp-forward '{}': expected bind_host:bind_port=unix_socket_path",
            spec
        )
    })?;

    let socket_spec = socket_spec.trim();
    if socket_spec.is_empty() {
        return Err(format!(
            "Invalid --reverse-tcp-forward '{}': unix_socket_path must not be empty",
            spec
        ));
    }

    Ok((
        parse_host_port(bind_spec, "reverse tcp forward bind")?,
        socket_spec.into(),
    ))
}

fn start_vsock_forward(
    handle: std::sync::Arc<ffi::VmHandle>,
    guest_port: u32,
    sock_path: std::path::PathBuf,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, String> {
    use std::io;
    use std::os::unix::net::UnixListener;
    use std::time::Duration;

    if let Some(parent) = sock_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create socket dir {}: {}", parent.display(), e))?;
    }

    // Remove any stale socket from previous runs.
    let _ = std::fs::remove_file(&sock_path);

    let listener = UnixListener::bind(&sock_path)
        .map_err(|e| format!("Failed to bind unix socket {}: {}", sock_path.display(), e))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set socket nonblocking: {}", e))?;

    tracing::info!(
        "vsock forward enabled: {} -> guest vsock:{}",
        sock_path.display(),
        guest_port
    );

    Ok(std::thread::spawn(move || {
        while !shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    let handle = handle.clone();
                    std::thread::spawn(move || {
                        if let Err(error) = stream.set_nonblocking(false) {
                            tracing::warn!(
                                "failed to switch accepted unix stream to blocking: {}",
                                error
                            );
                            return;
                        }
                        let connection = match handle.vsock_connect(guest_port) {
                            Ok(connection) => connection,
                            Err(e) => {
                                tracing::warn!("vsock connect failed: {}", e);
                                return;
                            }
                        };

                        let vsock = match connection.try_clone_file() {
                            Ok(file) => file,
                            Err(e) => {
                                tracing::warn!("vsock clone failed: {}", e);
                                return;
                            }
                        };
                        if let Err(e) = proxy_bidirectional(stream, vsock) {
                            tracing::debug!("vsock proxy ended: {}", e);
                        }
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    tracing::warn!(
                        "unix socket accept failed on {}: {}",
                        sock_path.display(),
                        e
                    );
                    break;
                }
            }
        }

        let _ = std::fs::remove_file(&sock_path);
    }))
}

#[cfg(target_os = "macos")]
fn start_tcp_forward(
    guest_port: u32,
    sock_path: std::path::PathBuf,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, String> {
    use std::io::{self, Write as _};
    use std::net::TcpListener;
    use std::os::unix::net::UnixListener;
    use std::time::Duration;
    use std::time::Instant;

    let port = u16::try_from(guest_port)
        .map_err(|_| format!("Invalid tcp guest_port '{}': must be 1-65535", guest_port))?;
    if port == 0 {
        return Err("tcp guest_port must be > 0".to_string());
    }

    if let Some(parent) = sock_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create socket dir {}: {}", parent.display(), e))?;
    }

    // Remove any stale socket from previous runs.
    let _ = std::fs::remove_file(&sock_path);

    let listener = UnixListener::bind(&sock_path)
        .map_err(|e| format!("Failed to bind unix socket {}: {}", sock_path.display(), e))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set socket nonblocking: {}", e))?;

    let guest_bind_addr = format!("0.0.0.0:{}", port);
    let guest_listener = TcpListener::bind(&guest_bind_addr).map_err(|e| {
        format!(
            "Failed to bind guest reverse listener {}: {}",
            guest_bind_addr, e
        )
    })?;
    guest_listener
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set guest reverse listener nonblocking: {}", e))?;

    tracing::info!(
        "tcp forward enabled: {} <- guest tcp:{}",
        sock_path.display(),
        port
    );

    Ok(std::thread::spawn(move || {
        while !shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, addr)) => {
                    tracing::debug!(
                        "tcp forward accepted unix client on {} from {:?}",
                        sock_path.display(),
                        addr
                    );
                    let accepted = {
                        // Increased timeout from 5s to 30s to handle concurrent connections
                        // when guest agent is processing multiple requests
                        let deadline = Instant::now() + Duration::from_secs(30);
                        loop {
                            if shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) {
                                break None;
                            }
                            if Instant::now() >= deadline {
                                break None;
                            }
                            match guest_listener.accept() {
                                Ok((tcp, guest_addr)) => {
                                    tracing::debug!(
                                        "tcp forward accepted guest reverse tcp {} for {}",
                                        guest_addr,
                                        sock_path.display()
                                    );
                                    break Some((tcp, guest_addr));
                                }
                                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                                    std::thread::sleep(Duration::from_millis(50));
                                }
                                Err(error) => {
                                    tracing::warn!(
                                        "guest reverse tcp accept failed on {}: {}",
                                        guest_bind_addr,
                                        error
                                    );
                                    break None;
                                }
                            }
                        }
                    };

                    let Some((mut tcp, guest_addr)) = accepted else {
                        tracing::warn!(
                            "tcp forward timed out waiting for guest connection on {} (closing client {})",
                            guest_bind_addr,
                            sock_path.display()
                        );
                        // Drop the unix client stream so the docker client fails fast instead
                        // of hanging forever. Keep the forward thread alive for future requests.
                        continue;
                    };

                    let sock_path = sock_path.clone();
                    std::thread::spawn(move || {
                        if let Err(error) = stream.set_nonblocking(false) {
                            tracing::warn!(
                                "failed to switch accepted unix client to blocking on {}: {}",
                                sock_path.display(),
                                error
                            );
                            return;
                        }
                        if let Err(error) = tcp.set_nonblocking(false) {
                            tracing::warn!(
                                "failed to switch guest reverse tcp to blocking on {}: {}",
                                sock_path.display(),
                                error
                            );
                            return;
                        }
                        let _ = tcp.set_nodelay(true);
                        let preface = match capture_http_request_preface(&mut stream) {
                            Ok(preface) => preface,
                            Err(error) => {
                                tracing::warn!(
                                    "failed to capture unix client request preface on {}: {}",
                                    sock_path.display(),
                                    error
                                );
                                Vec::new()
                            }
                        };
                        if !preface.is_empty() {
                            tracing::debug!(
                                "tcp forward rewrote request preface ({} bytes) on {}",
                                preface.len(),
                                sock_path.display()
                            );
                            if let Err(error) = tcp.write_all(&preface) {
                                tracing::warn!(
                                    "failed to write request preface to guest tcp on {}: {}",
                                    sock_path.display(),
                                    error
                                );
                                return;
                            }
                        }
                        tracing::debug!(
                            "tcp forward starting proxy: {} <-> guest {}",
                            sock_path.display(),
                            guest_addr
                        );
                        if let Err(error) = proxy_bidirectional_tcp(stream, tcp) {
                            tracing::debug!(
                                "tcp proxy ended with error on {}: {}",
                                sock_path.display(),
                                error
                            );
                        } else {
                            tracing::debug!("tcp proxy completed on {}", sock_path.display());
                        }
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    tracing::warn!(
                        "unix socket accept failed on {}: {}",
                        sock_path.display(),
                        e
                    );
                    break;
                }
            }
        }

        let _ = std::fs::remove_file(&sock_path);
    }))
}

#[cfg(target_os = "macos")]
fn capture_http_request_preface(
    stream: &mut std::os::unix::net::UnixStream,
) -> Result<Vec<u8>, String> {
    use std::io::{ErrorKind, Read};
    use std::time::Duration;

    // Try to set read timeout, but don't fail if not supported on this platform
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));

    let mut request = Vec::with_capacity(2048);
    let mut scratch = [0u8; 1024];
    let mut header_complete = false;

    while request.len() < 64 * 1024 {
        match stream.read(&mut scratch) {
            Ok(0) => break,
            Ok(read) => {
                request.extend_from_slice(&scratch[..read]);
                if find_http_header_end(&request).is_some() {
                    header_complete = true;
                    break;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                ) =>
            {
                break;
            }
            Err(error) => {
                let _ = stream.set_read_timeout(None);
                return Err(format!("read unix client preface: {}", error));
            }
        }
    }

    let _ = stream.set_read_timeout(None);

    if !header_complete {
        return Ok(request);
    }

    Ok(rewrite_http_request_preface(&request).unwrap_or(request))
}

#[cfg(target_os = "macos")]
fn find_http_header_end(buf: &[u8]) -> Option<usize> {
    if let Some(index) = buf.windows(4).position(|chunk| chunk == b"\r\n\r\n") {
        return Some(index + 4);
    }
    buf.windows(2)
        .position(|chunk| chunk == b"\n\n")
        .map(|index| index + 2)
}

#[cfg(target_os = "macos")]
fn rewrite_http_request_preface(request: &[u8]) -> Option<Vec<u8>> {
    let header_end = find_http_header_end(request)?;
    let request_head = std::str::from_utf8(&request[..header_end]).ok()?;
    let mut lines = request_head.lines();
    let request_line = lines.next()?.trim_end_matches('\r');
    let mut request_line_parts = request_line.split_whitespace();
    let _method = request_line_parts.next()?;
    let _target = request_line_parts.next()?;
    let version = request_line_parts.next()?;
    if !version.starts_with("HTTP/") {
        return None;
    }

    let mut header_lines = Vec::new();
    let mut has_content_length = false;
    let mut has_transfer_encoding = false;
    let mut is_upgrade = false;

    for line in lines {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }

        let (name, value) = line.split_once(':')?;
        let name = name.trim();
        let value = value.trim();

        if name.eq_ignore_ascii_case("connection") {
            if value
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
            {
                is_upgrade = true;
            }
            continue;
        }

        if name.eq_ignore_ascii_case("upgrade") {
            is_upgrade = true;
        } else if name.eq_ignore_ascii_case("content-length") {
            has_content_length = true;
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            has_transfer_encoding = true;
        }

        header_lines.push(format!("{}: {}", name, value));
    }

    if is_upgrade {
        return None;
    }

    let mut rewritten = Vec::with_capacity(request.len() + 64);
    rewritten.extend_from_slice(request_line.as_bytes());
    rewritten.extend_from_slice(b"\r\n");
    for line in header_lines {
        rewritten.extend_from_slice(line.as_bytes());
        rewritten.extend_from_slice(b"\r\n");
    }
    if !has_content_length && !has_transfer_encoding {
        rewritten.extend_from_slice(b"Content-Length: 0\r\n");
    }
    // Reverse TCP mode still uses one tunnel per accepted Unix client.
    // Force connection close so the upstream engine releases the TCP connection
    // and guest-agent workers can return to the pool for the next request.
    rewritten.extend_from_slice(b"Connection: close\r\n");
    rewritten.extend_from_slice(b"\r\n");
    rewritten.extend_from_slice(&request[header_end..]);
    Some(rewritten)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::{parse_http_proxy_url, rewrite_http_proxy_request, rewrite_http_request_preface};

    #[test]
    fn rewrites_plain_http_request_for_connection_close_proxying() {
        let request = b"GET /version HTTP/1.1\r\nHost: docker\r\nUser-Agent: test\r\n\r\n";
        let rewritten = rewrite_http_request_preface(request).expect("request should rewrite");
        let text = String::from_utf8(rewritten).expect("valid utf-8");

        assert!(text.starts_with("GET /version HTTP/1.1\r\n"));
        assert!(text.contains("Host: docker\r\n"));
        assert!(text.contains("User-Agent: test\r\n"));
        assert!(text.contains("Content-Length: 0\r\n"));
        assert!(text.contains("Connection: close\r\n"));
    }

    #[test]
    fn keeps_existing_content_length() {
        let request =
            b"POST /containers/create HTTP/1.1\r\nHost: docker\r\nContent-Length: 17\r\n\r\n{\"Image\":\"nginx\"}";
        let rewritten = rewrite_http_request_preface(request).expect("request should rewrite");
        let text = String::from_utf8(rewritten).expect("valid utf-8");

        assert!(text.contains("Content-Length: 17\r\n"));
        assert!(!text.contains("Content-Length: 0\r\n"));
        assert!(text.contains("Connection: close\r\n"));
        assert!(text.ends_with("{\"Image\":\"nginx\"}"));
    }

    #[test]
    fn skips_upgrade_requests() {
        let request = b"POST /exec/1/start HTTP/1.1\r\nHost: docker\r\nConnection: Upgrade\r\nUpgrade: tcp\r\n\r\n";
        assert!(rewrite_http_request_preface(request).is_none());
    }

    #[test]
    fn rewrites_absolute_http_proxy_request_to_origin_form() {
        let request = "GET http://oci-gz.example.test/docker/blob?q=1 HTTP/1.1\r\nHost: oci-gz.example.test\r\nProxy-Connection: Keep-Alive\r\nUser-Agent: buildkit\r\n\r\n";
        let (host, port, rewritten) =
            rewrite_http_proxy_request(request).expect("absolute http proxy request");

        assert_eq!(host, "oci-gz.example.test");
        assert_eq!(port, 80);
        assert!(rewritten.starts_with("GET /docker/blob?q=1 HTTP/1.1\r\n"));
        assert!(rewritten.contains("Host: oci-gz.example.test\r\n"));
        assert!(rewritten.contains("User-Agent: buildkit\r\n"));
        assert!(!rewritten.contains("Proxy-Connection"));
    }

    #[test]
    fn rewrites_curl_absolute_http_proxy_request() {
        let request = "GET http://oci-gz-1258344702.cos-internal.ap-guangzhou.tencentcos.cn/ HTTP/1.1\r\nHost: oci-gz-1258344702.cos-internal.ap-guangzhou.tencentcos.cn\r\nUser-Agent: curl/8.7.1\r\nAccept: */*\r\nProxy-Connection: Keep-Alive\r\n\r\n";
        let (host, port, rewritten) =
            rewrite_http_proxy_request(request).expect("curl absolute http proxy request");

        assert_eq!(
            host,
            "oci-gz-1258344702.cos-internal.ap-guangzhou.tencentcos.cn"
        );
        assert_eq!(port, 80);
        assert!(rewritten.starts_with("GET / HTTP/1.1\r\n"));
        assert!(rewritten
            .contains("Host: oci-gz-1258344702.cos-internal.ap-guangzhou.tencentcos.cn\r\n"));
        assert!(rewritten.contains("Accept: */*\r\n"));
        assert!(!rewritten.contains("Proxy-Connection"));
    }

    #[test]
    fn forwards_origin_form_http_proxy_request_using_host_header() {
        let request = "GET /docker/blob?q=1 HTTP/1.1\r\nHost: oci-gz.example.test:8080\r\nProxy-Connection: Keep-Alive\r\nUser-Agent: buildkit\r\n\r\n";
        let (host, port, rewritten) =
            rewrite_http_proxy_request(request).expect("origin-form http proxy request");

        assert_eq!(host, "oci-gz.example.test");
        assert_eq!(port, 8080);
        assert!(rewritten.starts_with("GET /docker/blob?q=1 HTTP/1.1\r\n"));
        assert!(rewritten.contains("Host: oci-gz.example.test:8080\r\n"));
        assert!(rewritten.contains("User-Agent: buildkit\r\n"));
        assert!(!rewritten.contains("Proxy-Connection"));
    }

    #[test]
    fn parses_http_proxy_url_with_explicit_port() {
        let (host, port, path) = parse_http_proxy_url("http://cache.example.test:8080/layers/data")
            .expect("http proxy url");

        assert_eq!(host, "cache.example.test");
        assert_eq!(port, 8080);
        assert_eq!(path, "/layers/data");
    }
}

#[cfg(target_os = "macos")]
fn start_host_tcp_forward(
    bind_host: String,
    bind_port: u16,
    target_host: String,
    target_port: u16,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, String> {
    use std::io;
    use std::net::TcpListener;
    use std::time::Duration;

    let bind_addr = format!("{}:{}", bind_host, bind_port);
    let listener = TcpListener::bind(&bind_addr)
        .map_err(|error| format!("Failed to bind host TCP forward {}: {}", bind_addr, error))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to set host TCP forward nonblocking: {}", error))?;

    tracing::info!(
        "host tcp forward enabled on {} -> {}:{}",
        bind_addr,
        target_host,
        target_port
    );

    Ok(std::thread::spawn(move || {
        while !shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) {
            match listener.accept() {
                Ok((client, _addr)) => {
                    let target_host = target_host.clone();
                    std::thread::spawn(move || {
                        match connect_target_tcp(target_host.as_str(), target_port) {
                            Ok(server) => {
                                let _ = server.set_nodelay(true);
                                let _ = client.set_nodelay(true);
                                if let Err(error) = proxy_bidirectional_host_tcp(client, server) {
                                    tracing::debug!("host tcp forward ended: {}", error);
                                }
                            }
                            Err(error) => {
                                tracing::warn!(
                                    "host tcp forward connect failed ({}:{}): {}",
                                    target_host,
                                    target_port,
                                    error
                                );
                            }
                        }
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    tracing::warn!("host tcp forward accept failed on {}: {}", bind_addr, error);
                    break;
                }
            }
        }
    }))
}

#[cfg(target_os = "macos")]
fn start_reverse_tcp_forward(
    bind_host: String,
    bind_port: u16,
    sock_path: std::path::PathBuf,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, String> {
    use std::io;
    use std::net::TcpListener;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let bind_addr = format!("{}:{}", bind_host, bind_port);
    let listener = TcpListener::bind(&bind_addr).map_err(|error| {
        format!(
            "Failed to bind reverse TCP forward {} -> {}: {}",
            bind_addr,
            sock_path.display(),
            error
        )
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to set reverse TCP forward nonblocking: {}", error))?;

    tracing::info!(
        "reverse tcp forward enabled on {} -> {}",
        bind_addr,
        sock_path.display()
    );

    Ok(std::thread::spawn(move || {
        while !shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) {
            match listener.accept() {
                Ok((client, _addr)) => {
                    let sock_path = sock_path.clone();
                    std::thread::spawn(move || match UnixStream::connect(&sock_path) {
                        Ok(unix) => {
                            if let Err(error) = client.set_nonblocking(false) {
                                tracing::warn!(
                                    "failed to switch reverse tcp client to blocking on {}: {}",
                                    sock_path.display(),
                                    error
                                );
                                return;
                            }
                            let _ = client.set_nodelay(true);
                            if let Err(error) = proxy_bidirectional_tcp(unix, client) {
                                tracing::debug!("reverse tcp forward ended: {}", error);
                            }
                        }
                        Err(error) => {
                            tracing::warn!(
                                "reverse tcp forward connect failed ({}): {}",
                                sock_path.display(),
                                error
                            );
                        }
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    tracing::warn!(
                        "reverse tcp forward accept failed on {}: {}",
                        bind_addr,
                        error
                    );
                    break;
                }
            }
        }
    }))
}

#[cfg(target_os = "macos")]
fn start_http_connect_proxy(
    bind_host: String,
    bind_port: u16,
    shutdown_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<std::thread::JoinHandle<()>, String> {
    use std::io;
    use std::net::TcpListener;
    use std::time::Duration;

    let bind_addr = format!("{}:{}", bind_host, bind_port);
    let listener = TcpListener::bind(&bind_addr)
        .map_err(|error| format!("Failed to bind HTTP CONNECT proxy {}: {}", bind_addr, error))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("Failed to set HTTP CONNECT proxy nonblocking: {}", error))?;

    tracing::info!("http connect proxy enabled on {}", bind_addr);

    Ok(std::thread::spawn(move || {
        while !shutdown_requested.load(std::sync::atomic::Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _addr)) => {
                    std::thread::spawn(move || {
                        if let Err(error) = handle_http_connect_client(stream) {
                            tracing::debug!("http connect proxy client ended: {}", error);
                        }
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => {
                    tracing::warn!(
                        "http connect proxy accept failed on {}: {}",
                        bind_addr,
                        error
                    );
                    break;
                }
            }
        }
    }))
}

#[cfg(target_os = "macos")]
fn handle_http_connect_client(mut client: std::net::TcpStream) -> Result<(), String> {
    use std::io::{Read, Write};

    client
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|error| format!("client timeout: {}", error))?;

    let mut request = Vec::with_capacity(1024);
    let mut scratch = [0u8; 1024];
    let header_end = loop {
        let read = client
            .read(&mut scratch)
            .map_err(|error| format!("read request: {}", error))?;
        if read == 0 {
            return Err("client closed before proxy request completed".to_string());
        }
        request.extend_from_slice(&scratch[..read]);
        if request.len() > 16 * 1024 {
            return Err("proxy request headers too large".to_string());
        }

        if let Some(index) = request.windows(4).position(|chunk| chunk == b"\r\n\r\n") {
            break index + 4;
        }
        if let Some(index) = request.windows(2).position(|chunk| chunk == b"\n\n") {
            break index + 2;
        }
    };

    let request_head = &request[..header_end];
    let buffered_body = &request[header_end..];
    let request_text = String::from_utf8_lossy(request_head);
    let first_line = request_text
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .ok_or_else(|| "empty proxy request".to_string())?;
    let mut first_parts = first_line.split_whitespace();
    let method = first_parts
        .next()
        .ok_or_else(|| "empty proxy request method".to_string())?;
    if method.eq_ignore_ascii_case("CONNECT") {
        let target = first_parts
            .next()
            .ok_or_else(|| format!("invalid CONNECT request '{}'", first_line))?;
        let (target_host, target_port) = parse_host_port(target, "CONNECT target")?;
        let server = connect_target_tcp(target_host.as_str(), target_port)?;
        let _ = server.set_nodelay(true);
        let _ = client.set_nodelay(true);
        let _ = client.set_read_timeout(None);

        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .map_err(|error| format!("write proxy response: {}", error))?;
        client
            .flush()
            .map_err(|error| format!("flush proxy response: {}", error))?;

        let mut server = server;
        if !buffered_body.is_empty() {
            server
                .write_all(buffered_body)
                .map_err(|error| format!("forward buffered client bytes: {}", error))?;
            server
                .flush()
                .map_err(|error| format!("flush buffered client bytes: {}", error))?;
        }

        return proxy_bidirectional_host_tcp(client, server);
    }

    let Some((target_host, target_port, rewritten_head)) =
        rewrite_http_proxy_request(request_text.as_ref())
    else {
        let preview = request_text.chars().take(512).collect::<String>();
        tracing::warn!(
            "unsupported http proxy request first_line={:?} head={:?}",
            first_line,
            preview
        );
        client
            .write_all(b"HTTP/1.1 501 Not Implemented\r\nConnection: close\r\n\r\n")
            .map_err(|error| format!("write 501: {}", error))?;
        return Err(format!("unsupported proxy request '{}'", first_line));
    };
    let mut server = connect_target_tcp(target_host.as_str(), target_port)?;
    let _ = server.set_nodelay(true);
    let _ = client.set_nodelay(true);
    let _ = client.set_read_timeout(None);
    server
        .write_all(rewritten_head.as_bytes())
        .map_err(|error| format!("write forwarded proxy request: {}", error))?;
    if !buffered_body.is_empty() {
        server
            .write_all(buffered_body)
            .map_err(|error| format!("forward buffered proxy body: {}", error))?;
    }
    server
        .flush()
        .map_err(|error| format!("flush forwarded proxy request: {}", error))?;

    proxy_bidirectional_host_tcp(client, server)
}

#[cfg(target_os = "macos")]
fn rewrite_http_proxy_request(request_head: &str) -> Option<(String, u16, String)> {
    let lines = request_head
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>();
    let (first_line_index, first_line) = lines
        .iter()
        .enumerate()
        .find(|(_, line)| !line.trim().is_empty())?;
    let first_line = first_line.trim();
    let header_lines = &lines[first_line_index + 1..];
    let mut first_parts = first_line.split_whitespace();
    let method = first_parts.next()?;
    let target = first_parts.next()?;
    let version = first_parts.next().unwrap_or("HTTP/1.1");
    let (host, port, path) = if has_http_proxy_scheme(target) {
        parse_http_proxy_url(target)?
    } else if target.starts_with('/') {
        let (host, port) = http_host_header(header_lines)?;
        (host, port, target.to_string())
    } else {
        return None;
    };

    let mut rewritten = String::new();
    rewritten.push_str(method);
    rewritten.push(' ');
    rewritten.push_str(&path);
    rewritten.push(' ');
    rewritten.push_str(version);
    rewritten.push_str("\r\n");

    let mut has_host = false;
    for line in header_lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, _)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("proxy-connection") {
            continue;
        }
        if name.eq_ignore_ascii_case("host") {
            has_host = true;
        }
        rewritten.push_str(line);
        rewritten.push_str("\r\n");
    }
    if !has_host {
        rewritten.push_str("Host: ");
        rewritten.push_str(&host);
        if port != 80 {
            rewritten.push(':');
            rewritten.push_str(&port.to_string());
        }
        rewritten.push_str("\r\n");
    }
    rewritten.push_str("\r\n");

    Some((host, port, rewritten))
}

#[cfg(target_os = "macos")]
fn http_host_header(lines: &[&str]) -> Option<(String, u16)> {
    let host = lines.iter().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.eq_ignore_ascii_case("host") {
            Some(value.trim())
        } else {
            None
        }
    })?;
    if host.starts_with('[') {
        parse_host_port(host, "HTTP host header").ok()
    } else if let Some((name, port)) = host.rsplit_once(':') {
        Some((name.to_string(), port.parse::<u16>().ok()?))
    } else if !host.is_empty() {
        Some((host.to_string(), 80))
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn parse_http_proxy_url(target: &str) -> Option<(String, u16, String)> {
    let rest = strip_http_proxy_scheme(target)?;
    let (authority, path) = rest
        .split_once('/')
        .map(|(authority, path)| (authority, format!("/{path}")))
        .unwrap_or_else(|| (rest, "/".to_string()));
    if authority.trim().is_empty() {
        return None;
    }
    let (host, port) = if authority.starts_with('[') {
        parse_host_port(authority, "HTTP proxy target").ok()?
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        (host.to_string(), port.parse::<u16>().ok()?)
    } else {
        (authority.to_string(), 80)
    };
    if host.trim().is_empty() {
        return None;
    }
    Some((host, port, path))
}

#[cfg(target_os = "macos")]
fn has_http_proxy_scheme(target: &str) -> bool {
    target
        .as_bytes()
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"http://"))
}

#[cfg(target_os = "macos")]
fn strip_http_proxy_scheme(target: &str) -> Option<&str> {
    if has_http_proxy_scheme(target) {
        Some(&target[7..])
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn proxy_bidirectional(
    unix: std::os::unix::net::UnixStream,
    vsock: std::fs::File,
) -> Result<(), String> {
    use std::net::Shutdown;
    use std::os::fd::AsRawFd;

    let mut unix_r = unix.try_clone().map_err(|e| format!("unix clone: {}", e))?;
    let mut unix_w = unix;

    let mut vsock_r = vsock
        .try_clone()
        .map_err(|e| format!("vsock clone: {}", e))?;
    let mut vsock_w = vsock;

    let t1 = std::thread::spawn(move || -> Result<(), String> {
        std::io::copy(&mut unix_r, &mut vsock_w).map_err(|e| format!("unix->vsock copy: {}", e))?;
        let _ = unsafe { libc::shutdown(vsock_w.as_raw_fd(), libc::SHUT_WR) };
        Ok(())
    });

    let t2 = std::thread::spawn(move || -> Result<(), String> {
        std::io::copy(&mut vsock_r, &mut unix_w).map_err(|e| format!("vsock->unix copy: {}", e))?;
        let _ = unix_w.shutdown(Shutdown::Write);
        Ok(())
    });

    t1.join()
        .map_err(|_| "unix->vsock proxy thread panicked".to_string())??;
    t2.join()
        .map_err(|_| "vsock->unix proxy thread panicked".to_string())??;
    Ok(())
}

#[cfg(target_os = "macos")]
fn proxy_bidirectional_tcp(
    unix: std::os::unix::net::UnixStream,
    tcp: std::net::TcpStream,
) -> Result<(), String> {
    use std::net::Shutdown;

    let mut unix_r = unix.try_clone().map_err(|e| format!("unix clone: {}", e))?;
    let mut unix_w = unix;

    let mut tcp_r = tcp.try_clone().map_err(|e| format!("tcp clone: {}", e))?;
    let mut tcp_w = tcp;

    let t1 = std::thread::spawn(move || -> Result<(), String> {
        std::io::copy(&mut unix_r, &mut tcp_w).map_err(|e| format!("unix->tcp copy: {}", e))?;
        let _ = tcp_w.shutdown(Shutdown::Write);
        Ok(())
    });

    let t2 = std::thread::spawn(move || -> Result<(), String> {
        std::io::copy(&mut tcp_r, &mut unix_w).map_err(|e| format!("tcp->unix copy: {}", e))?;
        let _ = unix_w.shutdown(Shutdown::Write);
        Ok(())
    });

    t1.join()
        .map_err(|_| "unix->tcp proxy thread panicked".to_string())??;
    t2.join()
        .map_err(|_| "tcp->unix proxy thread panicked".to_string())??;
    Ok(())
}

#[cfg(target_os = "macos")]
fn proxy_bidirectional_host_tcp(
    inbound: std::net::TcpStream,
    outbound: std::net::TcpStream,
) -> Result<(), String> {
    inbound
        .set_nonblocking(true)
        .map_err(|error| format!("set inbound nonblocking: {}", error))?;
    outbound
        .set_nonblocking(true)
        .map_err(|error| format!("set outbound nonblocking: {}", error))?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .map_err(|error| format!("build proxy runtime: {}", error))?;

    runtime.block_on(async move {
        let mut inbound = tokio::net::TcpStream::from_std(inbound)
            .map_err(|error| format!("wrap inbound tcp stream: {}", error))?;
        let mut outbound = tokio::net::TcpStream::from_std(outbound)
            .map_err(|error| format!("wrap outbound tcp stream: {}", error))?;

        tokio::io::copy_bidirectional(&mut inbound, &mut outbound)
            .await
            .map_err(|error| format!("copy bidirectional host tcp: {}", error))?;

        Ok(())
    })
}

#[cfg(target_os = "macos")]
static SHUTDOWN_FLAG: std::sync::atomic::AtomicPtr<std::sync::atomic::AtomicBool> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

#[cfg(target_os = "macos")]
extern "C" fn sigterm_handler(_sig: libc::c_int) {
    let ptr = SHUTDOWN_FLAG.load(std::sync::atomic::Ordering::SeqCst);
    if !ptr.is_null() {
        unsafe {
            (*ptr).store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

//! Shared runtime infrastructure — platform-agnostic helpers.
//!
//! Functions in this module are used by all three platform backends
//! (macOS VZ.framework, Linux KVM/QEMU, Windows WSL2). They cover:
//!
//! - Global runtime configuration (`runtime_vm_name`, `docker_proxy_port`, ...)
//! - Host Docker socket path management
//! - Bundled runtime asset discovery and installation
//! - Docker HTTP health ping (TCP-based, for Linux/Windows)
//! - Runtime image readiness verification
//!
//! Ported from `master:crates/cratebay-core/src/runtime.rs` and adapted for
//! the v2 multi-file architecture with `AppError` error model.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::error::AppError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default VM name for the CrateBay built-in runtime.
pub const DEFAULT_RUNTIME_VM_NAME: &str = "cratebay-runtime";

/// Default vsock / proxy port for Docker API inside the runtime VM.
pub const DEFAULT_DOCKER_PROXY_PORT: u32 = 6237;

/// Subdirectory name for bundled runtime image assets.
pub const DEFAULT_RUNTIME_ASSETS_SUBDIR: &str = "runtime-images";

/// Linux-specific bundled runtime assets subdirectory.
#[cfg(target_os = "linux")]
pub const DEFAULT_LINUX_RUNTIME_ASSETS_SUBDIR: &str = "runtime-linux";

/// Windows WSL2-specific bundled runtime assets subdirectory.
#[cfg(target_os = "windows")]
pub const DEFAULT_WSL_ASSETS_SUBDIR: &str = "runtime-wsl";

/// Default Docker TCP port for the Windows WSL2 runtime.
#[cfg(target_os = "windows")]
pub const DEFAULT_WSL_DOCKER_PORT: u16 = 2375;

/// Default Docker TCP port for the Linux KVM/QEMU runtime.
#[cfg(target_os = "linux")]
pub const DEFAULT_LINUX_DOCKER_PORT: u16 = 2475;

// ---------------------------------------------------------------------------
// Global singletons (OnceLock for lazy init)
// ---------------------------------------------------------------------------

static RUNTIME_VM_NAME: OnceLock<String> = OnceLock::new();
static DOCKER_PROXY_PORT: OnceLock<u32> = OnceLock::new();
static DOCKER_SOCKET_PATH: OnceLock<PathBuf> = OnceLock::new();
static RUNTIME_OS_IMAGE_ID: OnceLock<String> = OnceLock::new();

// ---------------------------------------------------------------------------
// Environment helpers
// ---------------------------------------------------------------------------

/// Check if an environment variable is set to a truthy value.
pub fn env_flag_truthy(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Check if an environment variable is set and truthy.
pub fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|raw| env_flag_truthy(&raw))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Global configuration accessors
// ---------------------------------------------------------------------------

/// The VM name CrateBay uses for its built-in container runtime.
///
/// Override via `CRATEBAY_RUNTIME_VM_NAME`. Defaults to `"cratebay-runtime"`.
pub fn runtime_vm_name() -> &'static str {
    RUNTIME_VM_NAME
        .get_or_init(|| {
            std::env::var("CRATEBAY_RUNTIME_VM_NAME")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_RUNTIME_VM_NAME.to_string())
        })
        .as_str()
}

/// The guest port for the Docker API proxy inside the runtime VM.
///
/// Override via `CRATEBAY_DOCKER_PROXY_PORT` or the legacy
/// `CRATEBAY_DOCKER_VSOCK_PORT`. When `CRATEBAY_DATA_DIR` is set and no port
/// override is provided, derive a deterministic high port from the data dir so
/// isolated runtimes do not all collide on the global default port.
pub fn docker_proxy_port() -> u32 {
    *DOCKER_PROXY_PORT.get_or_init(|| {
        std::env::var("CRATEBAY_DOCKER_PROXY_PORT")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .or_else(|| {
                std::env::var("CRATEBAY_DOCKER_VSOCK_PORT")
                    .ok()
                    .and_then(|v| v.parse::<u32>().ok())
                    .filter(|v| *v > 0)
            })
            .or_else(|| {
                std::env::var("CRATEBAY_DATA_DIR").ok().and_then(|dir| {
                    let dir = dir.trim();
                    if dir.is_empty() {
                        return None;
                    }
                    let hash = dir.bytes().fold(0_u32, |acc, byte| {
                        acc.wrapping_mul(131).wrapping_add(byte as u32)
                    });
                    Some(42000 + (hash % 10000))
                })
            })
            .unwrap_or(DEFAULT_DOCKER_PROXY_PORT)
    })
}

/// The host-side Docker-compatible Unix socket path exposed by CrateBay.
///
/// Defaults to `$HOME/.cratebay/runtime/docker.sock`. When `CRATEBAY_DATA_DIR`
/// is explicitly set, derive a short, isolated socket path under the system
/// temp directory so isolated runtimes do not share the global socket path and
/// do not hit macOS Unix socket path length limits.
///
/// Override via `CRATEBAY_DOCKER_SOCKET_PATH`.
pub fn host_docker_socket_path() -> &'static Path {
    let path = DOCKER_SOCKET_PATH
        .get_or_init(|| {
            if let Ok(p) = std::env::var("CRATEBAY_DOCKER_SOCKET_PATH") {
                if !p.trim().is_empty() {
                    return PathBuf::from(p);
                }
            }

            if let Ok(dir) = std::env::var("CRATEBAY_DATA_DIR") {
                let dir = dir.trim();
                if !dir.is_empty() {
                    let hash = dir.bytes().fold(0_u32, |acc, byte| {
                        acc.wrapping_mul(131).wrapping_add(byte as u32)
                    });
                    return std::env::temp_dir()
                        .join(format!("cratebay-runtime-{}", hash))
                        .join("docker.sock");
                }
            }

            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home)
                    .join(".cratebay")
                    .join("runtime")
                    .join("docker.sock");
            }

            crate::storage::data_dir()
                .join("runtime")
                .join("docker.sock")
        })
        .as_path();
    tracing::debug!("Host Docker socket path: {}", path.display());
    path
}

/// Per-VM Docker socket path on the host.
///
/// Located alongside `host_docker_socket_path()` and includes an additional
/// suffix when `CRATEBAY_DATA_DIR` is explicitly set, so isolated runtimes do
/// not collide on the same `/tmp/docker-<vm>.sock` path.
pub fn runtime_host_docker_socket_path(vm_id: &str) -> PathBuf {
    let base = host_docker_socket_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| crate::storage::data_dir().join("runtime"));

    let suffix = std::env::var("CRATEBAY_DATA_DIR")
        .ok()
        .map(|dir| dir.trim().to_string())
        .filter(|dir| !dir.is_empty())
        .map(|dir| {
            dir.bytes().fold(0_u32, |acc, byte| {
                acc.wrapping_mul(131).wrapping_add(byte as u32)
            })
        });

    match suffix {
        Some(hash) => base.join(format!("docker-{}-{}.sock", vm_id, hash)),
        None => base.join(format!("docker-{}.sock", vm_id)),
    }
}

/// Create a symlink from the canonical `host_docker_socket_path()` to the
/// actual per-VM socket.
#[cfg(unix)]
pub fn link_runtime_host_docker_socket(vm_id: &str) -> Result<(), AppError> {
    use std::os::unix::fs::symlink;

    let alias = host_docker_socket_path();
    let actual = runtime_host_docker_socket_path(vm_id);
    if let Some(parent) = actual.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match std::fs::symlink_metadata(alias) {
        Ok(_) => {
            let _ = std::fs::remove_file(alias);
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(AppError::Io(err)),
    }

    symlink(&actual, alias)?;
    Ok(())
}

/// Remove the canonical socket symlink if it points to this VM.
#[cfg(unix)]
pub fn unlink_runtime_host_docker_socket(vm_id: &str) {
    let alias = host_docker_socket_path();
    let actual = runtime_host_docker_socket_path(vm_id);

    if let Ok(target) = std::fs::read_link(alias) {
        if target == actual {
            let _ = std::fs::remove_file(alias);
        }
    }
    let _ = std::fs::remove_file(&actual);
}

// ---------------------------------------------------------------------------
// Runtime OS image selection
// ---------------------------------------------------------------------------

/// OS image id used for the built-in runtime VM.
///
/// Can be overridden via `CRATEBAY_RUNTIME_OS_IMAGE_ID`.
/// Defaults to `cratebay-runtime-aarch64` or `cratebay-runtime-x86_64`
/// depending on the host architecture.
pub fn runtime_os_image_id() -> &'static str {
    RUNTIME_OS_IMAGE_ID
        .get_or_init(|| {
            if let Ok(id) = std::env::var("CRATEBAY_RUNTIME_OS_IMAGE_ID") {
                if !id.trim().is_empty() {
                    return id;
                }
            }

            // Runtime detection: supports Universal Binary (aarch64 + x86_64 in single app)
            let arch = std::env::consts::ARCH;
            match arch {
                "aarch64" => "cratebay-runtime-aarch64".to_string(),
                "x86_64" => "cratebay-runtime-x86_64".to_string(),
                _ => "cratebay-runtime-aarch64".to_string(), // fallback
            }
        })
        .as_str()
}

/// Check if the runtime OS image is downloaded and ready.
pub fn runtime_image_ready() -> bool {
    crate::images::is_image_ready(runtime_os_image_id())
}

// ---------------------------------------------------------------------------
// Bundled asset discovery
// ---------------------------------------------------------------------------

/// Determine the subdirectory containing runtime image files within a root
/// directory. Returns `Some(dir)` if the directory exists.
fn runtime_images_dir_from_root(root: &Path) -> Option<PathBuf> {
    if root
        .file_name()
        .is_some_and(|n| n == DEFAULT_RUNTIME_ASSETS_SUBDIR)
        && root.is_dir()
    {
        return Some(root.to_path_buf());
    }

    let dir = root.join(DEFAULT_RUNTIME_ASSETS_SUBDIR);
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// macOS app bundle: `<App>.app/Contents/MacOS/<exe>` → `Contents/Resources/`.
fn bundled_runtime_assets_root_from_exe_dir(exe_dir: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        if let Some(macos_dir) = exe_dir.file_name().and_then(|s| s.to_str()) {
            if macos_dir == "MacOS" {
                if let Some(contents_dir) = exe_dir.parent() {
                    if contents_dir
                        .file_name()
                        .and_then(|s| s.to_str())
                        .is_some_and(|n| n == "Contents")
                    {
                        let resources = contents_dir.join("Resources");
                        if resources.is_dir() {
                            return Some(resources);
                        }
                    }
                }
            }
        }
    }

    // Tauri Windows/Linux layout: sibling `resources/` directory.
    let direct_resources = exe_dir.join("resources");
    if direct_resources.is_dir() {
        return Some(direct_resources);
    }
    if let Some(parent) = exe_dir.parent() {
        let parent_resources = parent.join("resources");
        if parent_resources.is_dir() {
            return Some(parent_resources);
        }
    }

    None
}

/// Walk up from exe dir to find the workspace root containing `Cargo.toml`
/// and look for runtime images under `crates/cratebay-gui/src-tauri/`.
fn workspace_runtime_assets_root_from_exe_dir(exe_dir: &Path) -> Option<PathBuf> {
    for ancestor in exe_dir.ancestors() {
        if !ancestor.join("Cargo.toml").is_file() {
            continue;
        }

        let src_tauri_dir = ancestor
            .join("crates")
            .join("cratebay-gui")
            .join("src-tauri");
        if runtime_images_dir_from_root(&src_tauri_dir).is_some() {
            return Some(src_tauri_dir);
        }
    }

    None
}

/// Collect all candidate root directories where runtime assets might live,
/// in priority order.
pub fn runtime_assets_root_candidates() -> Vec<PathBuf> {
    fn push_unique(roots: &mut Vec<PathBuf>, path: PathBuf) {
        if !roots.iter().any(|existing| existing == &path) {
            roots.push(path);
        }
    }

    let mut roots: Vec<PathBuf> = Vec::new();

    // 1. Explicit environment override
    if let Ok(dir) = std::env::var("CRATEBAY_RUNTIME_ASSETS_DIR") {
        if !dir.trim().is_empty() {
            push_unique(&mut roots, PathBuf::from(dir));
        }
    }

    // 2. Bundled assets next to the executable (app bundle / installer layout)
    if let Ok(exe) = std::env::current_exe() {
        tracing::debug!("current_exe: {:?}", exe);
        if let Some(exe_dir) = exe.parent() {
            if let Some(root) = bundled_runtime_assets_root_from_exe_dir(exe_dir) {
                tracing::debug!("bundled_runtime_assets_root: {:?}", root);
                push_unique(&mut roots, root);
            } else {
                tracing::debug!(
                    "bundled_runtime_assets_root_from_exe_dir returned None for {:?}",
                    exe_dir
                );
            }
        }
    }

    // 3. Platform-specific common install locations
    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            push_unique(
                &mut roots,
                PathBuf::from(local_app_data)
                    .join("Programs")
                    .join("CrateBay")
                    .join("resources"),
            );
        }
        if let Some(program_files) = std::env::var_os("ProgramFiles") {
            push_unique(
                &mut roots,
                PathBuf::from(program_files)
                    .join("CrateBay")
                    .join("resources"),
            );
        }
        if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
            push_unique(
                &mut roots,
                PathBuf::from(program_files_x86)
                    .join("CrateBay")
                    .join("resources"),
            );
        }
    }

    #[cfg(target_os = "linux")]
    {
        push_unique(&mut roots, PathBuf::from("/opt/CrateBay").join("resources"));
        push_unique(
            &mut roots,
            PathBuf::from("/usr/lib/CrateBay").join("resources"),
        );
        push_unique(
            &mut roots,
            PathBuf::from("/usr/lib/cratebay").join("resources"),
        );
    }

    // 4. Workspace root (development builds)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            if let Some(root) = workspace_runtime_assets_root_from_exe_dir(exe_dir) {
                push_unique(&mut roots, root);
            }
            push_unique(&mut roots, exe_dir.to_path_buf());
        }
    }

    // 5. macOS default app bundle locations (CLI fallback)
    #[cfg(target_os = "macos")]
    {
        push_unique(
            &mut roots,
            PathBuf::from("/Applications/CrateBay.app/Contents/Resources"),
        );
        if let Some(home) = std::env::var_os("HOME") {
            push_unique(
                &mut roots,
                PathBuf::from(home)
                    .join("Applications")
                    .join("CrateBay.app")
                    .join("Contents")
                    .join("Resources"),
            );
        }
    }

    tracing::debug!("runtime_assets_root_candidates: {:?}", roots);
    roots
}

/// All candidate directories that may contain runtime image directories.
fn runtime_images_dir_candidates() -> Vec<PathBuf> {
    runtime_assets_root_candidates()
        .into_iter()
        .filter_map(|root| runtime_images_dir_from_root(&root))
        .collect()
}

/// First available bundled runtime assets directory.
pub fn bundled_runtime_assets_dir() -> Option<PathBuf> {
    runtime_images_dir_candidates().into_iter().next()
}

// ---------------------------------------------------------------------------
// Placeholder detection
// ---------------------------------------------------------------------------

/// Check if a file is a placeholder (too small and contains `PLACEHOLDER` or
/// a Git LFS pointer).
pub(crate) fn file_contains_placeholder_marker(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() >= 1024 {
        return false;
    }

    std::fs::read_to_string(path)
        .map(|txt| {
            txt.contains("PLACEHOLDER")
                || txt.contains("version https://git-lfs.github.com/spec/v1")
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Image files verification
// ---------------------------------------------------------------------------

/// Required image files for a given OS image.
pub fn required_image_files(image_id: &str) -> Vec<&'static str> {
    let rootfs_required = crate::images::find_image(image_id)
        .map(|e| !e.rootfs_url.trim().is_empty())
        .unwrap_or(true);

    let mut files = vec!["vmlinuz", "initramfs"];
    if rootfs_required {
        files.push("rootfs.img");
    }
    files
}

// ---------------------------------------------------------------------------
// Platform helper asset discovery
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn runtime_linux_dir_from_root(root: &Path) -> Option<PathBuf> {
    if root
        .file_name()
        .is_some_and(|n| n == DEFAULT_LINUX_RUNTIME_ASSETS_SUBDIR)
        && root.is_dir()
    {
        return Some(root.to_path_buf());
    }

    let dir = root.join(DEFAULT_LINUX_RUNTIME_ASSETS_SUBDIR);
    if dir.is_dir() {
        Some(dir)
    } else {
        None
    }
}

/// First available bundled Linux runtime helper directory (`runtime-linux/`).
#[cfg(target_os = "linux")]
pub fn bundled_linux_runtime_assets_dir() -> Option<PathBuf> {
    runtime_assets_root_candidates()
        .into_iter()
        .find_map(|root| runtime_linux_dir_from_root(&root))
}

/// Locate bundled assets for a specific image.
pub fn runtime_image_assets_dir(image_id: &str) -> Option<PathBuf> {
    let mut placeholder_dir: Option<PathBuf> = None;

    for images_dir in runtime_images_dir_candidates() {
        let dir = images_dir.join(image_id);
        if !dir.is_dir() {
            continue;
        }

        match bundled_assets_ready(image_id, &dir) {
            Some(true) => return Some(dir),
            Some(false) => {
                if placeholder_dir.is_none() {
                    placeholder_dir = Some(dir);
                }
            }
            None => {}
        }
    }

    placeholder_dir
}

/// Check if all required image files are present in a directory and are
/// not placeholders.
///
/// - Returns `Some(true)` if all required files exist and are non-placeholder.
/// - Returns `Some(false)` if files exist but at least one is a placeholder.
/// - Returns `None` if any required file is missing entirely.
fn bundled_assets_ready(image_id: &str, image_dir: &Path) -> Option<bool> {
    let mut has_placeholder = false;
    for name in required_image_files(image_id) {
        let path = image_dir.join(name);
        if !path.is_file() {
            return None;
        }
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.len() < 1024 && file_contains_placeholder_marker(&path) {
                has_placeholder = true;
            }
        }
    }

    Some(!has_placeholder)
}

/// Check if all required image files are present in the installed
/// images directory.
fn image_files_present(image_id: &str) -> bool {
    let dest_dir = crate::images::image_dir(image_id);
    required_image_files(image_id)
        .into_iter()
        .all(|name| dest_dir.join(name).is_file())
}

// ---------------------------------------------------------------------------
// File comparison (for update detection)
// ---------------------------------------------------------------------------

/// Byte-for-byte file comparison.
fn files_equal(src: &Path, dest: &Path) -> Result<bool, AppError> {
    use std::io::Read;

    let mut src_file = std::fs::File::open(src)?;
    let mut dest_file = std::fs::File::open(dest)?;
    let mut src_buf = [0u8; 128 * 1024];
    let mut dest_buf = [0u8; 128 * 1024];

    loop {
        let src_read = src_file.read(&mut src_buf)?;
        let dest_read = dest_file.read(&mut dest_buf)?;

        if src_read != dest_read {
            return Ok(false);
        }
        if src_read == 0 {
            return Ok(true);
        }

        if src_buf[..src_read] != dest_buf[..dest_read] {
            return Ok(false);
        }
    }
}

/// Check if src and dest are identical (size + content).
fn file_matches(src: &Path, dest: &Path) -> Result<bool, AppError> {
    if !src.is_file() || !dest.is_file() {
        return Ok(false);
    }
    let src_meta = std::fs::metadata(src)?;
    let dest_meta = std::fs::metadata(dest)?;
    if src_meta.len() != dest_meta.len() {
        return Ok(false);
    }

    files_equal(src, dest)
}

// ---------------------------------------------------------------------------
// Runtime image installation
// ---------------------------------------------------------------------------

/// Check if the installed runtime image is present and up-to-date with
/// bundled assets (if available).
pub fn runtime_image_installed_up_to_date(image_id: &str) -> Result<bool, AppError> {
    if !crate::images::is_image_ready(image_id) {
        return Ok(false);
    }
    if !image_files_present(image_id) {
        return Ok(false);
    }

    // If we can't locate bundled assets (e.g. only the CLI is installed),
    // keep the already-installed runtime image usable.
    let Some(assets_dir) = runtime_image_assets_dir(image_id) else {
        return Ok(true);
    };

    let dest_dir = crate::images::image_dir(image_id);
    for name in required_image_files(image_id) {
        let src = assets_dir.join(name);
        let dest = dest_dir.join(name);

        // If bundled assets are missing, we can't compare;
        // don't fail an existing install.
        if !src.is_file() {
            continue;
        }

        if !file_matches(&src, &dest)? {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Write the `metadata.json` file that marks an image as ready.
fn write_ready_metadata(image_id: &str) -> Result<(), AppError> {
    let dir = crate::images::image_dir(image_id);
    std::fs::create_dir_all(&dir)?;

    crate::images::save_image_status(image_id, &crate::images::ImageStatus::Ready)
        .map_err(|e| AppError::Runtime(format!("Failed to write image metadata: {}", e)))
}

/// Install runtime image files from bundled assets into the images directory.
pub fn install_runtime_image_from_assets(image_id: &str) -> Result<(), AppError> {
    let assets_dir = runtime_image_assets_dir(image_id).ok_or_else(|| {
        AppError::Runtime(format!(
            "CrateBay Runtime assets not found for image '{}'. \
             Ensure the desktop app is installed correctly or set \
             CRATEBAY_RUNTIME_ASSETS_DIR.",
            image_id
        ))
    })?;

    let dest_dir = crate::images::image_dir(image_id);
    std::fs::create_dir_all(&dest_dir)?;

    let copy_required = |name: &str| -> Result<(), AppError> {
        let src = assets_dir.join(name);
        if !src.is_file() {
            return Err(AppError::Runtime(format!(
                "Missing runtime asset '{}': {}",
                name,
                src.display()
            )));
        }
        if file_contains_placeholder_marker(&src) {
            return Err(AppError::Runtime(format!(
                "Runtime asset '{}' is a placeholder or Git LFS pointer. \
                 Fetch real assets before using CrateBay Runtime.",
                src.display()
            )));
        }
        let dest = dest_dir.join(name);
        crate::fsutil::copy_file_fast(&src, &dest)?;
        Ok(())
    };

    copy_required("vmlinuz")?;
    copy_required("initramfs")?;

    // rootfs.img is only required if the catalog entry has a non-empty rootfs_url
    let rootfs_required = crate::images::find_image(image_id)
        .map(|e| !e.rootfs_url.trim().is_empty())
        .unwrap_or(true);
    if rootfs_required {
        copy_required("rootfs.img")?;
    }

    write_ready_metadata(image_id)?;
    Ok(())
}

/// Ensure the runtime image is installed and up to date.
///
/// If the image is outdated or missing, installs it from bundled assets.
pub fn ensure_runtime_image_ready(image_id: &str) -> Result<(), AppError> {
    if runtime_image_installed_up_to_date(image_id)? {
        return Ok(());
    }

    install_runtime_image_from_assets(image_id)?;
    if !crate::images::is_image_ready(image_id) {
        return Err(AppError::Runtime(format!(
            "Runtime OS image '{}' was installed but is still not marked ready",
            image_id
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Docker TCP endpoint parsing
// ---------------------------------------------------------------------------

/// Parse a `tcp://host:port` string into `(host, port)`.
///
/// Supports IPv4 (`tcp://127.0.0.1:2375`) and IPv6 (`tcp://[::1]:2375`).
pub fn docker_host_tcp_endpoint(host: &str) -> Option<(String, u16)> {
    let endpoint = host.strip_prefix("tcp://")?;

    // IPv6 bracket notation: [host]:port
    if endpoint.starts_with('[') {
        let end = endpoint.find(']')?;
        let host_part = endpoint.get(1..end)?.to_string();
        let port = endpoint.get(end + 1..)?.strip_prefix(':')?.parse().ok()?;
        return Some((host_part, port));
    }

    // IPv4 or hostname: host:port
    let (host_part, port_part) = endpoint.rsplit_once(':')?;
    let port = port_part.parse().ok()?;
    if host_part.trim().is_empty() {
        return None;
    }
    Some((host_part.to_string(), port))
}

/// Ping a Docker daemon over TCP HTTP (no TLS).
///
/// Sends `GET /_ping HTTP/1.1` and checks for a `200 OK` response.
/// This function uses raw TCP and is suitable for Linux/Windows runtimes
/// that expose Docker via TCP rather than Unix sockets.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn docker_http_ping_host(host: &str) -> Result<(), String> {
    use std::io::{Read, Write};
    use std::net::ToSocketAddrs;
    use std::time::Duration;

    let (tcp_host, port) =
        docker_host_tcp_endpoint(host).ok_or_else(|| format!("invalid Docker host '{}'", host))?;

    let mut addresses = (tcp_host.as_str(), port)
        .to_socket_addrs()
        .map_err(|error| format!("resolve {}:{}: {}", tcp_host, port, error))?
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        return Err(format!(
            "resolve {}:{}: no addresses returned",
            tcp_host, port
        ));
    }
    // Prefer IPv4 over IPv6 for compatibility
    addresses.sort_by_key(|address| if address.is_ipv4() { 0 } else { 1 });

    let mut last_error = None;
    for address in addresses {
        match std::net::TcpStream::connect_timeout(&address, Duration::from_millis(500)) {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

                stream
                    .write_all(b"GET /_ping HTTP/1.1\r\nHost: docker\r\nConnection: close\r\n\r\n")
                    .map_err(|error| format!("write {}: {}", address, error))?;

                let mut buf = [0_u8; 512];
                let n = stream
                    .read(&mut buf)
                    .map_err(|error| format!("read {}: {}", address, error))?;
                let response = String::from_utf8_lossy(&buf[..n]);
                if response.contains("200 OK") || response.ends_with("OK") {
                    return Ok(());
                }

                last_error = Some(format!("{} returned unexpected response", address));
            }
            Err(error) => {
                last_error = Some(format!("connect {}: {}", address, error));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "unknown error".to_string()))
}

/// Wait for a Docker TCP endpoint to become responsive.
///
/// Polls every 500ms until either Docker responds to a ping or
/// the timeout expires.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn wait_for_docker_tcp(host: &str, timeout: std::time::Duration) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    let mut last_error = "Docker host is still starting".to_string();

    while std::time::Instant::now() < deadline {
        match docker_http_ping_host(host) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    Err(last_error)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_flag_truthy_values() {
        assert!(env_flag_truthy("1"));
        assert!(env_flag_truthy("true"));
        assert!(env_flag_truthy("TRUE"));
        assert!(env_flag_truthy("True"));
        assert!(env_flag_truthy("yes"));
        assert!(env_flag_truthy("YES"));
        assert!(env_flag_truthy("on"));
        assert!(env_flag_truthy("ON"));
        assert!(env_flag_truthy(" true "));
    }

    #[test]
    fn env_flag_falsy_values() {
        assert!(!env_flag_truthy("0"));
        assert!(!env_flag_truthy("false"));
        assert!(!env_flag_truthy("no"));
        assert!(!env_flag_truthy("off"));
        assert!(!env_flag_truthy(""));
        assert!(!env_flag_truthy("random"));
    }

    #[test]
    fn runtime_vm_name_default() {
        // Should return a non-empty string
        let name = runtime_vm_name();
        assert!(!name.is_empty());
        // Default should be "cratebay-runtime" unless overridden by env
        // (can't assert exact value in CI where env may be set)
    }

    #[test]
    fn docker_proxy_port_default() {
        let port = docker_proxy_port();
        assert!(port > 0, "proxy port should be positive");
    }

    #[test]
    fn host_docker_socket_path_not_empty() {
        let path = host_docker_socket_path();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn host_docker_socket_path_contains_docker_sock() {
        let path = host_docker_socket_path();
        let s = path.to_string_lossy();
        assert!(
            s.contains("docker.sock"),
            "path should contain docker.sock: {}",
            s
        );
    }

    #[test]
    fn runtime_host_docker_socket_path_contains_vm_id() {
        let path = runtime_host_docker_socket_path("test-vm");
        let s = path.to_string_lossy();
        assert!(
            s.contains("docker-test-vm.sock"),
            "path should contain vm id: {}",
            s
        );
    }

    #[test]
    fn runtime_os_image_id_not_empty() {
        let id = runtime_os_image_id();
        assert!(!id.is_empty());
        assert!(
            id.starts_with("cratebay-runtime-"),
            "should start with cratebay-runtime-: {}",
            id
        );
    }

    #[test]
    fn docker_host_tcp_endpoint_parses_ipv4() {
        let result = docker_host_tcp_endpoint("tcp://127.0.0.1:2375");
        assert_eq!(result, Some(("127.0.0.1".to_string(), 2375)));
    }

    #[test]
    fn docker_host_tcp_endpoint_parses_ipv6() {
        let result = docker_host_tcp_endpoint("tcp://[::1]:2375");
        assert_eq!(result, Some(("::1".to_string(), 2375)));
    }

    #[test]
    fn docker_host_tcp_endpoint_rejects_invalid() {
        assert!(docker_host_tcp_endpoint("unix:///var/run/docker.sock").is_none());
        assert!(docker_host_tcp_endpoint("tcp://").is_none());
        assert!(docker_host_tcp_endpoint("tcp://:2375").is_none());
        assert!(docker_host_tcp_endpoint("").is_none());
        assert!(docker_host_tcp_endpoint("not-a-url").is_none());
    }

    #[test]
    fn docker_host_tcp_endpoint_parses_hostname() {
        let result = docker_host_tcp_endpoint("tcp://docker.local:2376");
        assert_eq!(result, Some(("docker.local".to_string(), 2376)));
    }

    #[test]
    fn runtime_assets_root_candidates_not_empty() {
        // There should always be at least one candidate (the exe dir)
        let candidates = runtime_assets_root_candidates();
        assert!(
            !candidates.is_empty(),
            "should have at least one asset root candidate"
        );
    }

    #[test]
    fn required_image_files_includes_kernel_and_initramfs() {
        let files = required_image_files("cratebay-runtime-aarch64");
        assert!(files.contains(&"vmlinuz"));
        assert!(files.contains(&"initramfs"));
    }

    #[test]
    fn runtime_image_ready_for_nonexistent_image() {
        // An image with a made-up id should never be ready
        assert!(!crate::images::is_image_ready(
            "nonexistent-runtime-test-xyz"
        ));
    }

    #[test]
    fn file_contains_placeholder_marker_returns_false_for_nonexistent() {
        assert!(!file_contains_placeholder_marker(Path::new(
            "/nonexistent/path"
        )));
    }

    #[test]
    fn file_contains_placeholder_marker_detects_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.txt");
        std::fs::write(&path, "PLACEHOLDER: this is not a real file").unwrap();
        assert!(file_contains_placeholder_marker(&path));
    }

    #[test]
    fn file_contains_placeholder_marker_detects_git_lfs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.bin");
        std::fs::write(
            &path,
            "version https://git-lfs.github.com/spec/v1\noid sha256:abc",
        )
        .unwrap();
        assert!(file_contains_placeholder_marker(&path));
    }

    #[test]
    fn file_contains_placeholder_marker_false_for_large_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("big.bin");
        let data = vec![0u8; 2048];
        std::fs::write(&path, &data).unwrap();
        assert!(!file_contains_placeholder_marker(&path));
    }

    #[test]
    fn files_equal_identical() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        std::fs::write(&a, b"hello world").unwrap();
        std::fs::write(&b, b"hello world").unwrap();
        assert!(files_equal(&a, &b).unwrap());
    }

    #[test]
    fn files_equal_different() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        std::fs::write(&a, b"hello world").unwrap();
        std::fs::write(&b, b"hello earth").unwrap();
        assert!(!files_equal(&a, &b).unwrap());
    }

    #[test]
    fn file_matches_nonexistent() {
        assert!(!file_matches(Path::new("/nonexistent/a"), Path::new("/nonexistent/b")).unwrap());
    }

    #[test]
    fn file_matches_different_sizes() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a.bin");
        let b = tmp.path().join("b.bin");
        std::fs::write(&a, b"short").unwrap();
        std::fs::write(&b, b"a longer string").unwrap();
        assert!(!file_matches(&a, &b).unwrap());
    }
}

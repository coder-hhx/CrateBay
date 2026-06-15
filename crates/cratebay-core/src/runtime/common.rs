//! Shared runtime infrastructure — platform-agnostic helpers.
//!
//! Functions in this module are used by all three platform backends
//! (macOS VZ.framework, Linux KVM/QEMU, Windows WSL2). They cover:
//!
//! - Global runtime configuration (`runtime_vm_name`, `engine_proxy_port`, ...)
//! - Host engine compatibility socket path management
//! - Bundled runtime asset discovery and installation
//! - Compatibility API health ping (TCP-based, for Linux/Windows)
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

/// Default vsock / proxy port for the engine compatibility API inside the runtime VM.
pub const DEFAULT_ENGINE_PROXY_PORT: u32 = 6237;

/// Compatibility alias for older callers.
pub const DEFAULT_DOCKER_PROXY_PORT: u32 = DEFAULT_ENGINE_PROXY_PORT;

/// Subdirectory name for bundled runtime image assets.
pub const DEFAULT_RUNTIME_ASSETS_SUBDIR: &str = "runtime-images";

/// Linux-specific bundled runtime assets subdirectory.
#[cfg(target_os = "linux")]
pub const DEFAULT_LINUX_RUNTIME_ASSETS_SUBDIR: &str = "runtime-linux";

/// Windows WSL2-specific bundled runtime assets subdirectory.
#[cfg(target_os = "windows")]
pub const DEFAULT_WSL_ASSETS_SUBDIR: &str = "runtime-wsl";

/// Default engine compatibility TCP port for the Windows WSL2 runtime.
#[cfg(target_os = "windows")]
pub const DEFAULT_WSL_DOCKER_PORT: u16 = 2375;

/// Default engine compatibility TCP port for the Linux KVM/QEMU runtime.
#[cfg(target_os = "linux")]
pub const DEFAULT_LINUX_DOCKER_PORT: u16 = 2475;

// ---------------------------------------------------------------------------
// Global singletons (OnceLock for lazy init)
// ---------------------------------------------------------------------------

static RUNTIME_VM_NAME: OnceLock<String> = OnceLock::new();
static ENGINE_PROXY_PORT: OnceLock<u32> = OnceLock::new();
static ENGINE_SOCKET_PATH: OnceLock<PathBuf> = OnceLock::new();
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

/// The guest port for the engine compatibility API proxy inside the runtime VM.
///
/// Override via `CRATEBAY_ENGINE_PROXY_PORT` or `CRATEBAY_ENGINE_VSOCK_PORT`.
/// Legacy `CRATEBAY_DOCKER_PROXY_PORT` and `CRATEBAY_DOCKER_VSOCK_PORT`
/// remain supported for compatibility. When `CRATEBAY_DATA_DIR` is set and no
/// port override is provided, derive a deterministic high port from the data
/// dir so isolated runtimes do not all collide on the global default port.
pub fn engine_proxy_port() -> u32 {
    *ENGINE_PROXY_PORT.get_or_init(|| {
        std::env::var("CRATEBAY_ENGINE_PROXY_PORT")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .or_else(|| {
                std::env::var("CRATEBAY_ENGINE_VSOCK_PORT")
                    .ok()
                    .and_then(|v| v.parse::<u32>().ok())
                    .filter(|v| *v > 0)
            })
            .or_else(|| {
                std::env::var("CRATEBAY_DOCKER_PROXY_PORT")
                    .ok()
                    .and_then(|v| v.parse::<u32>().ok())
                    .filter(|v| *v > 0)
            })
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
                    Some(42000 + (data_dir_hash(dir) % 10000))
                })
            })
            .unwrap_or(DEFAULT_ENGINE_PROXY_PORT)
    })
}

/// Compatibility alias for older call sites that still use Docker-shaped naming.
pub fn docker_proxy_port() -> u32 {
    engine_proxy_port()
}

fn data_dir_hash(dir: &str) -> u32 {
    dir.bytes().fold(0_u32, |acc, byte| {
        acc.wrapping_mul(131).wrapping_add(byte as u32)
    })
}

fn isolated_runtime_socket_dir(hash: u32) -> PathBuf {
    #[cfg(unix)]
    {
        PathBuf::from("/tmp").join(format!("cratebay-{}", hash))
    }

    #[cfg(not(unix))]
    {
        std::env::temp_dir().join(format!("cratebay-{}", hash))
    }
}

const ENGINE_SOCKET_FILE_NAME: &str = "engine.sock";
const LEGACY_DOCKER_SOCKET_FILE_NAME: &str = "docker.sock";

/// The host-side CrateBay Engine Unix socket path exposed by CrateBay.
///
/// Defaults to `$HOME/.cratebay/runtime/engine.sock`. When `CRATEBAY_DATA_DIR`
/// is explicitly set, derive a short, isolated socket path under `/tmp` on
/// Unix so isolated runtimes do not share the global socket path and do not hit
/// macOS Unix socket path length limits.
///
/// Override via `CRATEBAY_ENGINE_SOCKET_PATH`. The legacy
/// `CRATEBAY_DOCKER_SOCKET_PATH` override is still honored for compatibility.
pub fn host_engine_socket_path() -> &'static Path {
    let path = ENGINE_SOCKET_PATH
        .get_or_init(|| {
            if let Ok(p) = std::env::var("CRATEBAY_ENGINE_SOCKET_PATH") {
                if !p.trim().is_empty() {
                    return PathBuf::from(p);
                }
            }

            if let Ok(p) = std::env::var("CRATEBAY_DOCKER_SOCKET_PATH") {
                if !p.trim().is_empty() {
                    return PathBuf::from(p);
                }
            }

            if let Ok(dir) = std::env::var("CRATEBAY_DATA_DIR") {
                let dir = dir.trim();
                if !dir.is_empty() {
                    return isolated_runtime_socket_dir(data_dir_hash(dir))
                        .join(ENGINE_SOCKET_FILE_NAME);
                }
            }

            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home)
                    .join(".cratebay")
                    .join("runtime")
                    .join(ENGINE_SOCKET_FILE_NAME);
            }

            crate::storage::data_dir()
                .join("runtime")
                .join(ENGINE_SOCKET_FILE_NAME)
        })
        .as_path();
    tracing::debug!("Host CrateBay Engine socket path: {}", path.display());
    path
}

/// Compatibility alias for existing call sites.
///
/// The returned path is CrateBay's canonical Engine socket. A `docker.sock`
/// symlink is created alongside it for external Docker-compatible clients.
pub fn host_docker_socket_path() -> &'static Path {
    host_engine_socket_path()
}

/// The legacy Docker-compatible host socket alias path.
pub fn host_legacy_docker_socket_path() -> PathBuf {
    host_engine_socket_path()
        .parent()
        .map(|parent| parent.join(LEGACY_DOCKER_SOCKET_FILE_NAME))
        .unwrap_or_else(|| {
            crate::storage::data_dir()
                .join("runtime")
                .join(LEGACY_DOCKER_SOCKET_FILE_NAME)
        })
}

/// Per-VM CrateBay Engine socket path on the host.
///
/// Located alongside `host_engine_socket_path()` and includes an additional
/// suffix when `CRATEBAY_DATA_DIR` is explicitly set, so isolated runtimes do
/// not collide on the same `/tmp/engine-<vm>.sock` path.
pub fn runtime_host_engine_socket_path(vm_id: &str) -> PathBuf {
    let base = host_engine_socket_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| crate::storage::data_dir().join("runtime"));

    let suffix = std::env::var("CRATEBAY_DATA_DIR")
        .ok()
        .map(|dir| dir.trim().to_string())
        .filter(|dir| !dir.is_empty())
        .map(|dir| data_dir_hash(&dir));

    match suffix {
        Some(hash) => base.join(format!("engine-{}-{}.sock", vm_id, hash)),
        None => base.join(format!("engine-{}.sock", vm_id)),
    }
}

/// Compatibility alias for existing runtime backends.
pub fn runtime_host_docker_socket_path(vm_id: &str) -> PathBuf {
    runtime_host_engine_socket_path(vm_id)
}

#[cfg(unix)]
fn replace_socket_symlink(alias: &Path, actual: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::symlink;

    if let Some(parent) = alias.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match std::fs::symlink_metadata(alias) {
        Ok(_) => {
            let _ = std::fs::remove_file(alias);
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(AppError::Io(err)),
    }

    symlink(actual, alias)?;
    Ok(())
}

/// Create symlinks from the canonical Engine socket and legacy compatibility
/// socket alias to the actual per-VM socket.
#[cfg(unix)]
pub fn link_runtime_host_docker_socket(vm_id: &str) -> Result<(), AppError> {
    let alias = host_engine_socket_path();
    let legacy_alias = host_legacy_docker_socket_path();
    let actual = runtime_host_engine_socket_path(vm_id);
    if let Some(parent) = actual.parent() {
        std::fs::create_dir_all(parent)?;
    }

    replace_socket_symlink(alias, &actual)?;
    if legacy_alias != alias {
        replace_socket_symlink(&legacy_alias, &actual)?;
    }
    Ok(())
}

/// Remove the canonical and legacy socket symlinks if they point to this VM.
#[cfg(unix)]
pub fn unlink_runtime_host_docker_socket(vm_id: &str) {
    let alias = host_engine_socket_path();
    let legacy_alias = host_legacy_docker_socket_path();
    let actual = runtime_host_engine_socket_path(vm_id);

    for alias in [alias.to_path_buf(), legacy_alias] {
        if let Ok(target) = std::fs::read_link(&alias) {
            if target == actual {
                let _ = std::fs::remove_file(alias);
            }
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
    } else if root.join("cratebay-runtime-aarch64").is_dir()
        || root.join("cratebay-runtime-x86_64").is_dir()
    {
        // Allow explicit CRATEBAY_RUNTIME_ASSETS_DIR values to point directly
        // at the directory produced by scripts/build-runtime-assets-*.sh.
        Some(root.to_path_buf())
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
// Engine TCP endpoint parsing
// ---------------------------------------------------------------------------

/// Parse a `tcp://host:port` string into `(host, port)`.
///
/// Supports IPv4 (`tcp://127.0.0.1:2375`) and IPv6 (`tcp://[::1]:2375`).
pub fn engine_host_tcp_endpoint(host: &str) -> Option<(String, u16)> {
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

/// Compatibility alias for Docker-compatible endpoint callers.
pub fn docker_host_tcp_endpoint(host: &str) -> Option<(String, u16)> {
    engine_host_tcp_endpoint(host)
}

/// Ping a CrateBay Engine compatibility endpoint over TCP HTTP (no TLS).
///
/// Sends `GET /_ping HTTP/1.1` and checks for a `200 OK` response.
/// This function uses raw TCP and is suitable for Linux/Windows runtimes
/// that expose Docker via TCP rather than Unix sockets.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn docker_http_ping_host(host: &str) -> Result<(), String> {
    engine_http_ping_host(host)
}

/// Ping a CrateBay Engine compatibility endpoint over TCP HTTP (no TLS).
///
/// Sends `GET /_ping HTTP/1.1` and checks for a `200 OK` response.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn engine_http_ping_host(host: &str) -> Result<(), String> {
    use std::io::{Read, Write};
    use std::net::ToSocketAddrs;
    use std::time::Duration;

    let (tcp_host, port) =
        engine_host_tcp_endpoint(host).ok_or_else(|| format!("invalid Engine host '{}'", host))?;

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

fn engine_http_request(path: &str) -> Vec<u8> {
    engine_http_json_request("GET", path, &[])
}

fn engine_http_json_request(method: &str, path: &str, body: &[u8]) -> Vec<u8> {
    engine_http_raw_request(method, path, "application/json", body)
}

fn normalized_engine_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn engine_http_raw_header(
    method: &str,
    path: &str,
    content_type: &str,
    content_length: u64,
) -> Vec<u8> {
    let normalized_path = normalized_engine_path(path);
    if content_length == 0 {
        return format!(
            "{method} {normalized_path} HTTP/1.1\r\nHost: cratebay\r\nConnection: close\r\n\r\n"
        )
        .into_bytes();
    }

    format!(
        "{method} {normalized_path} HTTP/1.1\r\nHost: cratebay\r\nConnection: close\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n",
        if content_type.trim().is_empty() { "application/octet-stream" } else { content_type },
        content_length,
    )
    .into_bytes()
}

fn engine_http_raw_request(method: &str, path: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    if body.is_empty() {
        return engine_http_raw_header(method, path, content_type, 0);
    }

    let mut request = engine_http_raw_header(method, path, content_type, body.len() as u64);
    request.extend_from_slice(body);
    request
}

fn parse_http_body_response(response: &[u8]) -> Result<Vec<u8>, String> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "response did not include HTTP headers".to_string())?;
    let headers = String::from_utf8_lossy(&response[..header_end]);
    let status = headers
        .lines()
        .next()
        .ok_or_else(|| "response did not include a status line".to_string())?;
    let status_code = status
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or_default();
    if !(200..300).contains(&status_code) {
        let body = String::from_utf8_lossy(&response[header_end + 4..]);
        let body = body.trim();
        if body.is_empty() {
            return Err(format!("Engine endpoint returned {status}"));
        }
        return Err(format!("Engine endpoint returned {status}: {body}"));
    }
    Ok(response[header_end + 4..].to_vec())
}

fn parse_http_json_response(response: &[u8]) -> Result<serde_json::Value, String> {
    let body = parse_http_body_response(response)?;
    serde_json::from_slice(&body).map_err(|error| format!("parse Engine JSON response: {error}"))
}

/// Query a CrateBay Engine JSON endpoint over a Unix socket.
#[cfg(unix)]
pub fn engine_http_get_json_unix_socket(
    socket: &std::path::Path,
    path: &str,
) -> Result<serde_json::Value, String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let mut stream = UnixStream::connect(socket)
        .map_err(|error| format!("connect {}: {error}", socket.display()))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    stream
        .write_all(&engine_http_request(path))
        .map_err(|error| format!("write {}: {error}", socket.display()))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| format!("read {}: {error}", socket.display()))?;
    parse_http_json_response(&response)
}

/// Send a JSON request to a CrateBay Engine endpoint over a Unix socket.
#[cfg(unix)]
pub fn engine_http_json_unix_socket(
    socket: &std::path::Path,
    method: &str,
    path: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let body = serde_json::to_vec(payload)
        .map_err(|error| format!("encode Engine JSON request: {error}"))?;
    let mut stream = UnixStream::connect(socket)
        .map_err(|error| format!("connect {}: {error}", socket.display()))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    stream
        .write_all(&engine_http_json_request(method, path, &body))
        .map_err(|error| format!("write {}: {error}", socket.display()))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| format!("read {}: {error}", socket.display()))?;
    parse_http_json_response(&response)
}

/// Send a raw request to a CrateBay Engine endpoint over a Unix socket.
#[cfg(unix)]
pub fn engine_http_raw_unix_socket(
    socket: &std::path::Path,
    method: &str,
    path: &str,
    content_type: &str,
    body: &[u8],
) -> Result<Vec<u8>, String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let mut stream = UnixStream::connect(socket)
        .map_err(|error| format!("connect {}: {error}", socket.display()))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(600)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
    stream
        .write_all(&engine_http_raw_request(method, path, content_type, body))
        .map_err(|error| format!("write {}: {error}", socket.display()))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| format!("read {}: {error}", socket.display()))?;
    parse_http_body_response(&response)
}

/// Send a raw request body from a file to a CrateBay Engine endpoint over a Unix socket.
#[cfg(unix)]
pub fn engine_http_raw_file_unix_socket(
    socket: &std::path::Path,
    method: &str,
    path: &str,
    content_type: &str,
    body_path: &std::path::Path,
) -> Result<Vec<u8>, String> {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let mut body = File::open(body_path)
        .map_err(|error| format!("open request body {}: {error}", body_path.display()))?;
    let body_len = body
        .metadata()
        .map_err(|error| format!("stat request body {}: {error}", body_path.display()))?
        .len();
    let mut stream = UnixStream::connect(socket)
        .map_err(|error| format!("connect {}: {error}", socket.display()))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
    stream
        .write_all(&engine_http_raw_header(
            method,
            path,
            content_type,
            body_len,
        ))
        .map_err(|error| format!("write headers {}: {error}", socket.display()))?;
    std::io::copy(&mut body, &mut stream)
        .map_err(|error| format!("write body {}: {error}", body_path.display()))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| format!("read {}: {error}", socket.display()))?;
    parse_http_body_response(&response)
}

/// Query a CrateBay Engine JSON endpoint over a TCP compatibility host.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn engine_http_get_json_tcp_host(host: &str, path: &str) -> Result<serde_json::Value, String> {
    use std::io::{Read, Write};
    use std::net::ToSocketAddrs;
    use std::time::Duration;

    let (tcp_host, port) =
        engine_host_tcp_endpoint(host).ok_or_else(|| format!("invalid Engine host '{}'", host))?;

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
    addresses.sort_by_key(|address| if address.is_ipv4() { 0 } else { 1 });

    let mut last_error = None;
    for address in addresses {
        match std::net::TcpStream::connect_timeout(&address, Duration::from_millis(500)) {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                if let Err(error) = stream.write_all(&engine_http_request(path)) {
                    last_error = Some(format!("write {}: {}", address, error));
                    continue;
                }
                let mut response = Vec::new();
                if let Err(error) = stream.read_to_end(&mut response) {
                    last_error = Some(format!("read {}: {}", address, error));
                    continue;
                }
                return parse_http_json_response(&response);
            }
            Err(error) => {
                last_error = Some(format!("connect {}: {}", address, error));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "unknown error".to_string()))
}

/// Send a JSON request to a CrateBay Engine endpoint over a TCP compatibility host.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn engine_http_json_tcp_host(
    host: &str,
    method: &str,
    path: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    use std::io::{Read, Write};
    use std::net::ToSocketAddrs;
    use std::time::Duration;

    let body = serde_json::to_vec(payload)
        .map_err(|error| format!("encode Engine JSON request: {error}"))?;
    let (tcp_host, port) =
        engine_host_tcp_endpoint(host).ok_or_else(|| format!("invalid Engine host '{}'", host))?;

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
    addresses.sort_by_key(|address| if address.is_ipv4() { 0 } else { 1 });

    let mut last_error = None;
    for address in addresses {
        match std::net::TcpStream::connect_timeout(&address, Duration::from_millis(500)) {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
                if let Err(error) = stream.write_all(&engine_http_json_request(method, path, &body))
                {
                    last_error = Some(format!("write {}: {}", address, error));
                    continue;
                }
                let mut response = Vec::new();
                if let Err(error) = stream.read_to_end(&mut response) {
                    last_error = Some(format!("read {}: {}", address, error));
                    continue;
                }
                return parse_http_json_response(&response);
            }
            Err(error) => {
                last_error = Some(format!("connect {}: {}", address, error));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "unknown error".to_string()))
}

/// Send a raw request to a CrateBay Engine endpoint over a TCP compatibility host.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn engine_http_raw_tcp_host(
    host: &str,
    method: &str,
    path: &str,
    content_type: &str,
    body: &[u8],
) -> Result<Vec<u8>, String> {
    use std::io::{Read, Write};
    use std::net::ToSocketAddrs;
    use std::time::Duration;

    let (tcp_host, port) =
        engine_host_tcp_endpoint(host).ok_or_else(|| format!("invalid Engine host '{}'", host))?;

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
    addresses.sort_by_key(|address| if address.is_ipv4() { 0 } else { 1 });

    let mut last_error = None;
    for address in addresses {
        match std::net::TcpStream::connect_timeout(&address, Duration::from_millis(500)) {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(600)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
                if let Err(error) =
                    stream.write_all(&engine_http_raw_request(method, path, content_type, body))
                {
                    last_error = Some(format!("write {}: {}", address, error));
                    continue;
                }
                let mut response = Vec::new();
                if let Err(error) = stream.read_to_end(&mut response) {
                    last_error = Some(format!("read {}: {}", address, error));
                    continue;
                }
                return parse_http_body_response(&response);
            }
            Err(error) => {
                last_error = Some(format!("connect {}: {}", address, error));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "unknown error".to_string()))
}

/// Send a raw request body from a file to a CrateBay Engine endpoint over a TCP compatibility host.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn engine_http_raw_file_tcp_host(
    host: &str,
    method: &str,
    path: &str,
    content_type: &str,
    body_path: &std::path::Path,
) -> Result<Vec<u8>, String> {
    use std::fs::File;
    use std::io::{Read, Write};
    use std::net::ToSocketAddrs;
    use std::time::Duration;

    let body_len = std::fs::metadata(body_path)
        .map_err(|error| format!("stat request body {}: {error}", body_path.display()))?
        .len();
    let (tcp_host, port) =
        engine_host_tcp_endpoint(host).ok_or_else(|| format!("invalid Engine host '{}'", host))?;

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
    addresses.sort_by_key(|address| if address.is_ipv4() { 0 } else { 1 });

    let mut last_error = None;
    for address in addresses {
        match std::net::TcpStream::connect_timeout(&address, Duration::from_millis(500)) {
            Ok(mut stream) => {
                let mut body = File::open(body_path).map_err(|error| {
                    format!("open request body {}: {error}", body_path.display())
                })?;
                let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(60)));
                if let Err(error) = stream.write_all(&engine_http_raw_header(
                    method,
                    path,
                    content_type,
                    body_len,
                )) {
                    last_error = Some(format!("write headers {}: {}", address, error));
                    continue;
                }
                if let Err(error) = std::io::copy(&mut body, &mut stream) {
                    last_error = Some(format!("write body {}: {}", body_path.display(), error));
                    continue;
                }
                let mut response = Vec::new();
                if let Err(error) = stream.read_to_end(&mut response) {
                    last_error = Some(format!("read {}: {}", address, error));
                    continue;
                }
                return parse_http_body_response(&response);
            }
            Err(error) => {
                last_error = Some(format!("connect {}: {}", address, error));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "unknown error".to_string()))
}

/// Wait for an Engine compatibility TCP endpoint to become responsive.
///
/// Polls every 500ms until either the compatibility endpoint responds to a ping or
/// the timeout expires.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn wait_for_docker_tcp(host: &str, timeout: std::time::Duration) -> Result<(), String> {
    wait_for_engine_tcp(host, timeout)
}

/// Wait for an Engine compatibility TCP endpoint to become responsive.
///
/// Polls every 500ms until either the compatibility endpoint responds to a ping or
/// the timeout expires.
#[cfg(any(target_os = "windows", target_os = "linux"))]
pub fn wait_for_engine_tcp(host: &str, timeout: std::time::Duration) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    let mut last_error = "Engine host is still starting".to_string();

    while std::time::Instant::now() < deadline {
        match engine_http_ping_host(host) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    Err(last_error)
}

/// CPU usage from two Linux `/proc/stat` samples.
///
/// The input can be raw `/proc/stat` snapshots or just the first `cpu ...`
/// aggregate line. Returns a percentage in the range 0..100.
pub fn linux_proc_stat_cpu_percent(previous: &str, current: &str) -> Option<f32> {
    let previous = parse_linux_proc_stat_cpu_line(previous)?;
    let current = parse_linux_proc_stat_cpu_line(current)?;
    let total_delta = current.total.saturating_sub(previous.total);
    if total_delta == 0 {
        return Some(0.0);
    }

    let idle_delta = current.idle.saturating_sub(previous.idle);
    let busy_delta = total_delta.saturating_sub(idle_delta);
    Some(((busy_delta as f64 / total_delta as f64) * 100.0) as f32)
}

/// Disk usage in GiB from `df -B1` output.
pub fn linux_df_used_gb(output: &str) -> Option<f32> {
    output.lines().skip(1).find_map(|line| {
        let columns = line.split_whitespace().collect::<Vec<_>>();
        let used_bytes = columns.get(2)?.parse::<u64>().ok()?;
        Some(bytes_to_gib(used_bytes))
    })
}

/// Host-allocated disk usage in GiB for a sparse runtime disk image.
///
/// On Unix this uses allocated filesystem blocks instead of logical length, so
/// sparse `disk.raw` images report real host usage. Non-Unix platforms fall
/// back to logical file length.
pub fn file_allocated_gb(path: &Path) -> f32 {
    let Ok(metadata) = std::fs::metadata(path) else {
        return 0.0;
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        return bytes_to_gib(metadata.blocks().saturating_mul(512));
    }

    #[allow(unreachable_code)]
    bytes_to_gib(metadata.len())
}

#[derive(Debug, Clone, Copy)]
struct LinuxProcStatCpu {
    idle: u64,
    total: u64,
}

fn parse_linux_proc_stat_cpu_line(input: &str) -> Option<LinuxProcStatCpu> {
    let line = input.lines().find(|line| {
        line.trim_start()
            .strip_prefix("cpu")
            .is_some_and(|rest| rest.starts_with(char::is_whitespace))
    })?;
    let values = line
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse::<u64>().ok())
        .collect::<Vec<_>>();
    if values.len() < 4 {
        return None;
    }

    let idle = values
        .get(3)
        .copied()
        .unwrap_or_default()
        .saturating_add(values.get(4).copied().unwrap_or_default());
    let total = values
        .iter()
        .copied()
        .fold(0_u64, |total, value| total.saturating_add(value));

    Some(LinuxProcStatCpu { idle, total })
}

pub fn bytes_to_gib(bytes: u64) -> f32 {
    bytes as f32 / 1024.0 / 1024.0 / 1024.0
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
    fn engine_proxy_port_default() {
        let port = engine_proxy_port();
        assert!(port > 0, "proxy port should be positive");
    }

    #[test]
    fn host_engine_socket_path_not_empty() {
        let path = host_engine_socket_path();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn host_engine_socket_path_contains_engine_sock() {
        let path = host_engine_socket_path();
        let s = path.to_string_lossy();
        assert!(
            s.contains("engine.sock"),
            "path should contain engine.sock: {}",
            s
        );
    }

    #[test]
    fn host_legacy_docker_socket_path_contains_docker_sock() {
        let path = host_legacy_docker_socket_path();
        let s = path.to_string_lossy();
        assert!(
            s.contains("docker.sock"),
            "legacy alias should contain docker.sock: {}",
            s
        );
    }

    #[test]
    fn runtime_host_engine_socket_path_contains_vm_id() {
        let path = runtime_host_engine_socket_path("test-vm");
        let s = path.to_string_lossy();
        assert!(
            s.contains("engine-test-vm.sock"),
            "path should contain vm id: {}",
            s
        );
    }

    #[test]
    fn isolated_runtime_socket_paths_stay_short_for_unix_sockets() {
        let hash = data_dir_hash(
            "/Users/example/Library/Application Support/com.xiaofei.liveagent/cratebay-sandbox/runtime/data",
        );
        let alias = isolated_runtime_socket_dir(hash).join("engine.sock");
        let actual =
            isolated_runtime_socket_dir(hash).join(format!("engine-cratebay-runtime-{hash}.sock"));

        assert!(
            alias.to_string_lossy().len() < 103,
            "alias socket path is too long: {}",
            alias.display()
        );
        assert!(
            actual.to_string_lossy().len() < 103,
            "runtime socket path is too long: {}",
            actual.display()
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
    fn engine_host_tcp_endpoint_parses_ipv4() {
        let result = engine_host_tcp_endpoint("tcp://127.0.0.1:2375");
        assert_eq!(result, Some(("127.0.0.1".to_string(), 2375)));
    }

    #[test]
    fn engine_host_tcp_endpoint_parses_ipv6() {
        let result = engine_host_tcp_endpoint("tcp://[::1]:2375");
        assert_eq!(result, Some(("::1".to_string(), 2375)));
    }

    #[test]
    fn engine_host_tcp_endpoint_rejects_invalid() {
        assert!(engine_host_tcp_endpoint("unix:///var/run/docker.sock").is_none());
        assert!(engine_host_tcp_endpoint("tcp://").is_none());
        assert!(engine_host_tcp_endpoint("tcp://:2375").is_none());
        assert!(engine_host_tcp_endpoint("").is_none());
        assert!(engine_host_tcp_endpoint("not-a-url").is_none());
    }

    #[test]
    fn engine_host_tcp_endpoint_parses_hostname() {
        let result = engine_host_tcp_endpoint("tcp://engine.local:2376");
        assert_eq!(result, Some(("engine.local".to_string(), 2376)));
    }

    #[test]
    fn docker_host_tcp_endpoint_alias_parses_hostname() {
        let result = docker_host_tcp_endpoint("tcp://docker.local:2376");
        assert_eq!(result, Some(("docker.local".to_string(), 2376)));
    }

    #[test]
    fn parses_engine_http_json_response() {
        let response =
            b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"name\":\"CrateBay Engine\"}";
        let json = parse_http_json_response(response).expect("json response");
        assert_eq!(json["name"], "CrateBay Engine");
    }

    #[test]
    fn parses_created_engine_http_json_response() {
        let response =
            b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\n\r\n{\"id\":\"abc123\"}";
        let json = parse_http_json_response(response).expect("json response");
        assert_eq!(json["id"], "abc123");
    }

    #[test]
    fn rejects_non_ok_engine_http_json_response() {
        let response = b"HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\n\r\n{}";
        let err = parse_http_json_response(response).expect_err("non-ok response");
        assert!(err.contains("404 Not Found"), "unexpected error: {err}");
    }

    #[test]
    fn parses_engine_http_raw_body_response() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Type: application/x-tar\r\n\r\nabc123";
        let body = parse_http_body_response(response).expect("raw response");
        assert_eq!(body, b"abc123");
    }

    #[test]
    fn engine_http_request_normalizes_path() {
        let request = String::from_utf8(engine_http_request("cratebay/engine")).unwrap();
        assert!(request.starts_with("GET /cratebay/engine HTTP/1.1"));
    }

    #[test]
    fn engine_http_json_request_includes_body_headers() {
        let request = String::from_utf8(engine_http_json_request(
            "POST",
            "/cratebay/containers/abc/exec",
            br#"{"command":["true"]}"#,
        ))
        .unwrap();
        assert!(request.starts_with("POST /cratebay/containers/abc/exec HTTP/1.1"));
        assert!(request.contains("Content-Type: application/json"));
        assert!(request.contains("Content-Length: 20"));
        assert!(request.ends_with(r#"{"command":["true"]}"#));
    }

    #[test]
    fn engine_http_raw_request_includes_content_type_and_body() {
        let request = String::from_utf8(engine_http_raw_request(
            "POST",
            "/cratebay/images/import",
            "application/x-tar",
            b"tar-bytes",
        ))
        .unwrap();
        assert!(request.starts_with("POST /cratebay/images/import HTTP/1.1"));
        assert!(request.contains("Content-Type: application/x-tar"));
        assert!(request.contains("Content-Length: 9"));
        assert!(request.ends_with("tar-bytes"));
    }

    #[test]
    fn engine_http_raw_header_includes_length_without_materializing_body() {
        let request = String::from_utf8(engine_http_raw_header(
            "POST",
            "/cratebay/images/import",
            "application/x-tar",
            1024 * 1024 * 1024,
        ))
        .unwrap();
        assert!(request.starts_with("POST /cratebay/images/import HTTP/1.1"));
        assert!(request.contains("Content-Type: application/x-tar"));
        assert!(request.contains("Content-Length: 1073741824"));
        assert!(request.ends_with("\r\n\r\n"));
    }

    #[test]
    fn linux_proc_stat_cpu_percent_uses_aggregate_busy_delta() {
        let previous = "cpu  100 0 100 800 0 0 0 0 0 0\ncpu0 1 0 1 8";
        let current = "cpu  150 0 150 900 0 0 0 0 0 0\ncpu0 1 0 1 9";

        let percent = linux_proc_stat_cpu_percent(previous, current).unwrap();

        assert!((percent - 50.0).abs() < 0.01, "percent={percent}");
    }

    #[test]
    fn linux_proc_stat_cpu_percent_handles_unchanged_snapshot() {
        let snapshot = "cpu  100 0 100 800 0 0 0 0 0 0";

        assert_eq!(linux_proc_stat_cpu_percent(snapshot, snapshot), Some(0.0));
    }

    #[test]
    fn linux_proc_stat_cpu_percent_rejects_missing_aggregate_line() {
        assert_eq!(linux_proc_stat_cpu_percent("intr 1", "intr 2"), None);
    }

    #[test]
    fn linux_df_used_gb_parses_byte_output() {
        let output = "Filesystem     1B-blocks       Used Available Use% Mounted on\n/dev/sdb       42949672960 1073741824 41875931136   3% /";

        let used = linux_df_used_gb(output).unwrap();

        assert!((used - 1.0).abs() < 0.01, "used={used}");
    }

    #[test]
    fn linux_df_used_gb_rejects_empty_output() {
        assert_eq!(
            linux_df_used_gb("Filesystem 1B-blocks Used Available Use% Mounted on"),
            None
        );
    }

    #[test]
    fn bytes_to_gib_converts_binary_gib() {
        assert!((bytes_to_gib(1024 * 1024 * 1024) - 1.0).abs() < 0.01);
    }

    #[test]
    fn file_allocated_gb_returns_zero_for_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing.raw");

        assert_eq!(file_allocated_gb(&missing), 0.0);
    }

    #[test]
    fn file_allocated_gb_reads_existing_file_without_error() {
        let tmp = tempfile::tempdir().unwrap();
        let disk = tmp.path().join("disk.raw");
        std::fs::write(&disk, vec![0_u8; 4096]).unwrap();

        assert!(file_allocated_gb(&disk) >= 0.0);
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
    fn runtime_images_dir_accepts_direct_generated_assets_root() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("cratebay-runtime-x86_64")).unwrap();

        let dir = runtime_images_dir_from_root(tmp.path());

        assert_eq!(dir.as_deref(), Some(tmp.path()));
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

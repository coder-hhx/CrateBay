//! Pre-packaged container image archives shipped with CrateBay.

use std::path::{Path, PathBuf};

use bollard::Docker;
use serde::{Deserialize, Serialize};

use crate::{container, AppError};

/// Definition of a bundle image that can be preloaded.
#[derive(Debug, Clone, Copy)]
pub struct BundleImageDef {
    pub tar_filename: &'static str,
    pub image_name: &'static str,
}

/// Result for a single bundled image archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleImageLoadResult {
    pub image_name: String,
    pub tar_filename: String,
    pub archive_path: Option<String>,
    pub loaded: bool,
    pub skipped: bool,
    pub message: String,
}

/// The set of container images bundled with the application.
pub const BUNDLE_IMAGES: &[BundleImageDef] = &[
    BundleImageDef {
        tar_filename: "python-dev.tar.gz",
        image_name: "cratebay-python-dev:v1",
    },
    BundleImageDef {
        tar_filename: "node-dev.tar.gz",
        image_name: "cratebay-node-dev:v1",
    },
    BundleImageDef {
        tar_filename: "rust-dev.tar.gz",
        image_name: "cratebay-rust-dev:v1",
    },
    BundleImageDef {
        tar_filename: "ubuntu-base.tar.gz",
        image_name: "cratebay-ubuntu-base:v1",
    },
];

/// Candidate directories for bundle image archives.
pub fn candidate_bundle_image_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(dir) = std::env::var("CRATEBAY_BUNDLE_IMAGES_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            dirs.push(PathBuf::from(trimmed));
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            dirs.push(exe_dir.join("bundle-images"));
            dirs.push(exe_dir.join("../Resources/bundle-images"));
            dirs.push(exe_dir.join("../../Resources/bundle-images"));
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("bundle-images"));
        dirs.push(cwd.join("crates/cratebay-gui/src-tauri/bundle-images"));
    }

    dedup_paths(dirs)
}

/// Find the first existing bundle image directory.
pub fn find_bundle_image_dir() -> Option<PathBuf> {
    candidate_bundle_image_dirs()
        .into_iter()
        .find(|dir| dir.is_dir())
}

/// Load pre-packaged images from a Tauri resource directory.
pub async fn load_bundle_images_from_resource_dir(
    docker: &Docker,
    resource_dir: &Path,
) -> Vec<BundleImageLoadResult> {
    let bundle_dir = resource_dir.join("bundle-images");
    if !bundle_dir.is_dir() {
        tracing::debug!(
            "No bundle-images directory found at {:?}, skipping preload",
            bundle_dir
        );
        return Vec::new();
    }

    load_bundle_images_from_dir(docker, &bundle_dir).await
}

/// Load pre-packaged image archives from a directory.
pub async fn load_bundle_images_from_dir(
    docker: &Docker,
    bundle_dir: &Path,
) -> Vec<BundleImageLoadResult> {
    let mut results = Vec::new();

    for def in BUNDLE_IMAGES {
        results.push(load_one_bundle_image(docker, bundle_dir, def).await);
    }

    results
}

async fn load_one_bundle_image(
    docker: &Docker,
    bundle_dir: &Path,
    def: &BundleImageDef,
) -> BundleImageLoadResult {
    let archive_path = bundle_dir.join(def.tar_filename);
    let archive_path_string = archive_path.to_string_lossy().to_string();

    match container::image_exists(docker, def.image_name).await {
        Ok(true) => {
            return BundleImageLoadResult {
                image_name: def.image_name.to_string(),
                tar_filename: def.tar_filename.to_string(),
                archive_path: Some(archive_path_string),
                loaded: false,
                skipped: true,
                message: "already present".to_string(),
            };
        }
        Ok(false) => {}
        Err(e) => {
            return BundleImageLoadResult {
                image_name: def.image_name.to_string(),
                tar_filename: def.tar_filename.to_string(),
                archive_path: Some(archive_path_string),
                loaded: false,
                skipped: false,
                message: format!("failed to inspect image: {}", e),
            };
        }
    }

    if !archive_path.is_file() {
        return missing_archive_result(def, archive_path_string);
    }

    tracing::info!(
        "Loading bundle image {} from {:?}",
        def.image_name,
        archive_path
    );

    match container::image_load_from_tar(docker, &archive_path_string).await {
        Ok(names) => BundleImageLoadResult {
            image_name: def.image_name.to_string(),
            tar_filename: def.tar_filename.to_string(),
            archive_path: Some(archive_path_string),
            loaded: true,
            skipped: false,
            message: if names.is_empty() {
                "loaded archive".to_string()
            } else {
                format!("loaded {}", names.join(", "))
            },
        },
        Err(e) => BundleImageLoadResult {
            image_name: def.image_name.to_string(),
            tar_filename: def.tar_filename.to_string(),
            archive_path: Some(archive_path_string),
            loaded: false,
            skipped: false,
            message: format_load_error(e),
        },
    }
}

fn missing_archive_result(def: &BundleImageDef, archive_path: String) -> BundleImageLoadResult {
    BundleImageLoadResult {
        image_name: def.image_name.to_string(),
        tar_filename: def.tar_filename.to_string(),
        archive_path: Some(archive_path),
        loaded: false,
        skipped: false,
        message: "archive not found".to_string(),
    }
}

fn format_load_error(error: AppError) -> String {
    format!("failed to load archive: {}", error)
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::new();
    for path in paths {
        if !deduped.iter().any(|existing| existing == &path) {
            deduped.push(path);
        }
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_image_defs_are_unique() {
        let mut names = BUNDLE_IMAGES
            .iter()
            .map(|def| def.image_name)
            .collect::<Vec<_>>();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before);
    }

    #[test]
    fn candidate_dirs_include_env_override() {
        let _lock = ENV_LOCK.lock().expect("env lock poisoned");
        std::env::set_var("CRATEBAY_BUNDLE_IMAGES_DIR", "/tmp/cratebay-bundle-test");
        let dirs = candidate_bundle_image_dirs();
        std::env::remove_var("CRATEBAY_BUNDLE_IMAGES_DIR");
        assert!(dirs
            .iter()
            .any(|dir| dir == Path::new("/tmp/cratebay-bundle-test")));
    }

    #[test]
    fn missing_bundle_archive_is_a_failure_not_a_skip() {
        let result = missing_archive_result(&BUNDLE_IMAGES[0], "/missing/python-dev.tar.gz".into());

        assert!(!result.loaded);
        assert!(!result.skipped);
        assert_eq!(result.message, "archive not found");
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}

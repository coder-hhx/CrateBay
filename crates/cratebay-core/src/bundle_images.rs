//! Pre-packaged container image archives shipped with CrateBay.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use bollard::Docker;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};

use crate::runtime::{self, RuntimeManager};
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

/// Load pre-packaged images from a Tauri resource directory through CrateBay Engine native APIs.
pub async fn load_bundle_images_from_resource_dir_native(
    runtime_mgr: &dyn RuntimeManager,
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

    load_bundle_images_from_dir_native(runtime_mgr, &bundle_dir).await
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

/// Load pre-packaged image archives through CrateBay Engine native APIs.
pub async fn load_bundle_images_from_dir_native(
    runtime_mgr: &dyn RuntimeManager,
    bundle_dir: &Path,
) -> Vec<BundleImageLoadResult> {
    let images = match runtime::query_built_in_native_images(runtime_mgr) {
        Ok(images) => images,
        Err(error) => {
            return BUNDLE_IMAGES
                .iter()
                .map(|def| BundleImageLoadResult {
                    image_name: def.image_name.to_string(),
                    tar_filename: def.tar_filename.to_string(),
                    archive_path: Some(
                        bundle_dir
                            .join(def.tar_filename)
                            .to_string_lossy()
                            .to_string(),
                    ),
                    loaded: false,
                    skipped: false,
                    message: format!("failed to list native images: {}", error),
                })
                .collect();
        }
    };

    let existing_refs = images
        .items
        .iter()
        .flat_map(native_image_refs)
        .collect::<Vec<_>>();

    BUNDLE_IMAGES
        .iter()
        .map(|def| load_one_bundle_image_native(runtime_mgr, bundle_dir, def, &existing_refs))
        .collect()
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

fn load_one_bundle_image_native(
    runtime_mgr: &dyn RuntimeManager,
    bundle_dir: &Path,
    def: &BundleImageDef,
    existing_refs: &[String],
) -> BundleImageLoadResult {
    let archive_path = bundle_dir.join(def.tar_filename);
    let archive_path_string = archive_path.to_string_lossy().to_string();

    if existing_refs
        .iter()
        .any(|image| image_ref_matches(image, def.image_name))
    {
        return BundleImageLoadResult {
            image_name: def.image_name.to_string(),
            tar_filename: def.tar_filename.to_string(),
            archive_path: Some(archive_path_string),
            loaded: false,
            skipped: true,
            message: "already present".to_string(),
        };
    }

    if !archive_path.is_file() {
        return missing_archive_result(def, archive_path_string);
    }

    tracing::info!(
        "Loading bundle image {} from {:?} through CrateBay Engine native import",
        def.image_name,
        archive_path
    );

    match prepare_bundle_archive_for_import(&archive_path).and_then(|archive| {
        runtime::query_built_in_native_image_import_file(runtime_mgr, archive.path())
    }) {
        Ok(payload) => BundleImageLoadResult {
            image_name: def.image_name.to_string(),
            tar_filename: def.tar_filename.to_string(),
            archive_path: Some(archive_path_string),
            loaded: true,
            skipped: false,
            message: native_import_message(&payload),
        },
        Err(error) => BundleImageLoadResult {
            image_name: def.image_name.to_string(),
            tar_filename: def.tar_filename.to_string(),
            archive_path: Some(archive_path_string),
            loaded: false,
            skipped: false,
            message: format_load_error(error),
        },
    }
}

fn native_image_refs(image: &runtime::NativeImageSummary) -> Vec<String> {
    let mut refs = image
        .tags
        .iter()
        .filter(|tag| !tag.trim().is_empty() && tag.as_str() != "<none>:<none>")
        .cloned()
        .collect::<Vec<_>>();

    if !image.repository.trim().is_empty() {
        if image.tag.trim().is_empty() || image.tag == "<none>" {
            refs.push(image.repository.clone());
        } else {
            refs.push(format!("{}:{}", image.repository, image.tag));
        }
    }

    refs.sort();
    refs.dedup();
    refs
}

fn image_ref_matches(existing: &str, wanted: &str) -> bool {
    let existing = existing.trim();
    let wanted = wanted.trim();
    existing == wanted
        || existing
            .strip_prefix("docker.io/library/")
            .is_some_and(|short| short == wanted)
}

fn native_import_message(payload: &serde_json::Value) -> String {
    let images = payload
        .get("images")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if images.is_empty() {
        "loaded archive through CrateBay Engine".to_string()
    } else {
        format!("loaded {}", images.join(", "))
    }
}

struct PreparedImportArchive {
    path: PathBuf,
    remove_on_drop: bool,
}

impl PreparedImportArchive {
    fn borrowed(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            remove_on_drop: false,
        }
    }

    fn temporary(path: PathBuf) -> Self {
        Self {
            path,
            remove_on_drop: true,
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PreparedImportArchive {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn prepare_bundle_archive_for_import(path: &Path) -> Result<PreparedImportArchive, AppError> {
    if !is_gzip_archive(path) {
        return Ok(PreparedImportArchive::borrowed(path));
    }

    let input = fs::File::open(path).map_err(AppError::from)?;
    let mut decoder = GzDecoder::new(input);
    let output_path = std::env::temp_dir().join(format!(
        "cratebay-bundle-import-{}.tar",
        uuid::Uuid::new_v4()
    ));
    let mut output = fs::File::create(&output_path).map_err(AppError::from)?;
    io::copy(&mut decoder, &mut output).map_err(AppError::from)?;
    Ok(PreparedImportArchive::temporary(output_path))
}

fn is_gzip_archive(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".tar.gz") || lower.ends_with(".tgz") || lower.ends_with(".gz")
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

    #[test]
    fn native_image_refs_include_tags_and_repo_tag() {
        let refs = native_image_refs(&runtime::NativeImageSummary {
            id: "sha256:abc123".to_string(),
            repository: "cratebay-python-dev".to_string(),
            tag: "v1".to_string(),
            tags: vec![
                "docker.io/library/cratebay-python-dev:v1".to_string(),
                "cratebay-python-dev:v1".to_string(),
            ],
            digests: Vec::new(),
            size_bytes: 1,
            created: 0,
            labels: serde_json::json!({}),
            managed_by: "cratebay".to_string(),
        });

        assert!(refs.iter().any(|tag| tag == "cratebay-python-dev:v1"));
        assert!(refs
            .iter()
            .any(|tag| tag == "docker.io/library/cratebay-python-dev:v1"));
    }

    #[test]
    fn native_import_message_reports_imported_images() {
        let message = native_import_message(&serde_json::json!({
            "api": "cratebay.image.import.v1",
            "backend": "containerd",
            "images": ["unpacking cratebay-python-dev:v1...done"]
        }));

        assert_eq!(message, "loaded unpacking cratebay-python-dev:v1...done");
    }

    #[test]
    fn prepare_bundle_archive_streams_gzip_to_temporary_tar() {
        use std::io::Write;

        let temp = tempfile::tempdir().expect("temp dir");
        let gzip_path = temp.path().join("image.tar.gz");
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"tar payload").expect("write gzip body");
        fs::write(&gzip_path, encoder.finish().expect("finish gzip")).expect("write gzip file");

        let prepared = prepare_bundle_archive_for_import(&gzip_path).expect("prepare gzip archive");
        assert_ne!(prepared.path(), gzip_path.as_path());
        assert_eq!(
            fs::read(prepared.path()).expect("read prepared tar"),
            b"tar payload"
        );
        let prepared_path = prepared.path().to_path_buf();
        drop(prepared);
        assert!(!prepared_path.exists());
    }

    #[test]
    fn image_ref_matches_containerd_library_prefix() {
        assert!(image_ref_matches(
            "docker.io/library/cratebay-python-dev:v1",
            "cratebay-python-dev:v1",
        ));
        assert!(image_ref_matches(
            "cratebay-python-dev:v1",
            "cratebay-python-dev:v1",
        ));
        assert!(!image_ref_matches(
            "docker.io/library/cratebay-node-dev:v1",
            "cratebay-python-dev:v1",
        ));
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}

use anyhow::Result;
use bollard::Docker;
use cratebay_core::bundle_images::BundleImageLoadResult;
use std::path::PathBuf;

use cratebay_core::bundle_images;
use cratebay_core::container;
use cratebay_core::models::ImageSearchResult;
use cratebay_core::runtime::RuntimeManager;

use super::{print_structured, OutputFormat};

pub fn print_search_results(results: &[ImageSearchResult], format: &OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Table => {
            println!(
                "{:<40} {:>7} {:>8} {:<8} DESCRIPTION",
                "REFERENCE", "STARS", "SOURCE", "OFFICIAL"
            );
            for r in results {
                let stars = r.stars.unwrap_or(0);
                println!(
                    "{:<40} {:>7} {:>8} {:<8} {}",
                    r.reference,
                    stars,
                    r.source,
                    if r.official { "yes" } else { "no" },
                    r.description
                );
            }
            Ok(())
        }
        _ => print_structured(results, format),
    }
}

pub async fn list(docker: &Docker, format: &OutputFormat) -> Result<()> {
    let images = container::image_list(docker).await?;

    match format {
        OutputFormat::Table => {
            println!("{:<20} {:<50} {:>10}", "ID", "TAGS", "SIZE");
            for img in images {
                let id = img.id.trim_start_matches("sha256:");
                let short = id.chars().take(12).collect::<String>();
                let tags = if img.repo_tags.is_empty() {
                    "<none>".to_string()
                } else {
                    img.repo_tags.join(",")
                };
                println!("{:<20} {:<50} {:>10}", short, tags, img.size_human);
            }
            Ok(())
        }
        _ => print_structured(&images, format),
    }
}

pub async fn search(
    docker: &Docker,
    query: &str,
    limit: Option<u32>,
    format: &OutputFormat,
) -> Result<()> {
    let results = container::image_search(docker, query, limit.map(u64::from)).await?;
    print_search_results(&results, format)
}

pub async fn pull(docker: &Docker, image: &str) -> Result<()> {
    eprintln!("Pulling image: {}", image);

    let cb: container::PullProgressCallback = std::sync::Arc::new(|progress| {
        if progress.total_bytes > 0 {
            let pct = (progress.current_bytes as f64 / progress.total_bytes as f64) * 100.0;
            eprintln!("{:>6.1}% {}", pct, progress.status);
        } else {
            eprintln!("{}", progress.status);
        }
    });

    container::image_pull(docker, image, None, Some(cb)).await?;
    println!("Pulled {}", image);
    Ok(())
}

pub async fn delete(docker: &Docker, id: &str, force: bool) -> Result<()> {
    container::image_remove(docker, id, force).await?;
    println!("Deleted {}", id);
    Ok(())
}

pub async fn export(docker: &Docker, images: Vec<String>, output: &str) -> Result<()> {
    let bytes = container::image_export_to_tar(docker, &images, output).await?;
    println!(
        "Exported {} image(s) to {} ({} bytes)",
        images.len(),
        output,
        bytes
    );
    Ok(())
}

pub async fn import(docker: &Docker, input: &str, format: &OutputFormat) -> Result<()> {
    let loaded = container::image_load_from_tar(docker, input).await?;

    match format {
        OutputFormat::Table => {
            if loaded.is_empty() {
                println!("Imported image archive from {}", input);
            } else {
                println!("Imported from {}:", input);
                for image in &loaded {
                    println!("  {}", image);
                }
            }
            Ok(())
        }
        _ => print_structured(&loaded, format),
    }
}

pub async fn preload_bundled(
    docker: &Docker,
    dir: Option<String>,
    format: &OutputFormat,
) -> Result<()> {
    let bundle_dir = match dir {
        Some(dir) => PathBuf::from(dir),
        None => bundle_images::find_bundle_image_dir().ok_or_else(|| {
            anyhow::anyhow!(
                "No bundle-images directory found. Set CRATEBAY_BUNDLE_IMAGES_DIR or pass --dir."
            )
        })?,
    };

    let results = bundle_images::load_bundle_images_from_dir(docker, &bundle_dir).await;

    match format {
        OutputFormat::Table => {
            println!("Bundle image directory: {}", bundle_dir.display());
            println!("{:<28} {:<10} MESSAGE", "IMAGE", "STATUS",);
            for result in &results {
                let status = if result.loaded {
                    "loaded"
                } else if result.skipped {
                    "skipped"
                } else {
                    "failed"
                };
                println!(
                    "{:<28} {:<10} {}",
                    result.image_name, status, result.message
                );
            }
            Ok(())
        }
        _ => print_structured(&results, format),
    }?;

    if bundle_preload_failed(&results) {
        std::process::exit(1);
    }

    Ok(())
}

pub async fn preload_bundled_native(
    runtime: &dyn RuntimeManager,
    dir: Option<String>,
    format: &OutputFormat,
) -> Result<()> {
    let bundle_dir = match dir {
        Some(dir) => PathBuf::from(dir),
        None => bundle_images::find_bundle_image_dir().ok_or_else(|| {
            anyhow::anyhow!(
                "No bundle-images directory found. Set CRATEBAY_BUNDLE_IMAGES_DIR or pass --dir."
            )
        })?,
    };

    let results = bundle_images::load_bundle_images_from_dir_native(runtime, &bundle_dir).await;

    match format {
        OutputFormat::Table => {
            println!("Bundle image directory: {}", bundle_dir.display());
            println!("{:<28} {:<10} MESSAGE", "IMAGE", "STATUS",);
            for result in &results {
                let status = if result.loaded {
                    "loaded"
                } else if result.skipped {
                    "skipped"
                } else {
                    "failed"
                };
                println!(
                    "{:<28} {:<10} {}",
                    result.image_name, status, result.message
                );
            }
            Ok(())
        }
        _ => print_structured(&results, format),
    }?;

    if bundle_preload_failed(&results) {
        std::process::exit(1);
    }

    Ok(())
}

pub async fn inspect(docker: &Docker, id: &str, format: &OutputFormat) -> Result<()> {
    let detail = container::image_inspect(docker, id).await?;
    match format {
        OutputFormat::Table => {
            println!("ID: {}", detail.id);
            println!("RepoTags: {}", detail.repo_tags.join(", "));
            println!("SizeBytes: {}", detail.size_bytes);
            println!("Created: {}", detail.created);
            Ok(())
        }
        _ => print_structured(&detail, format),
    }
}

pub async fn tag(docker: &Docker, source: &str, target: &str) -> Result<()> {
    container::image_tag(docker, source, target).await?;
    println!("Tagged {} as {}", source, target);
    Ok(())
}

pub async fn pack_container(docker: &Docker, container_id: &str, image: &str) -> Result<()> {
    container::image_commit_container(docker, container_id, image).await?;
    println!("Packed container {} into {}", container_id, image);
    Ok(())
}

fn bundle_preload_failed(results: &[BundleImageLoadResult]) -> bool {
    results
        .iter()
        .any(|result| !result.loaded && !result.skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(loaded: bool, skipped: bool) -> BundleImageLoadResult {
        BundleImageLoadResult {
            image_name: "cratebay-test:v1".to_string(),
            tar_filename: "test.tar.gz".to_string(),
            archive_path: None,
            loaded,
            skipped,
            message: "test".to_string(),
        }
    }

    #[test]
    fn bundle_preload_failure_detects_failed_archives() {
        assert!(!bundle_preload_failed(&[result(true, false)]));
        assert!(!bundle_preload_failed(&[result(false, true)]));
        assert!(bundle_preload_failed(&[result(false, false)]));
    }
}

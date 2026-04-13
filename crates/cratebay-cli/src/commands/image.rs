use anyhow::Result;
use bollard::container::Config;
use bollard::image::CommitContainerOptions;
use bollard::Docker;

use cratebay_core::container;
use cratebay_core::models::ImageSearchResult;

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

pub async fn delete(docker: &Docker, id: &str) -> Result<()> {
    container::image_remove(docker, id, false).await?;
    println!("Deleted {}", id);
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

pub async fn pack_container(docker: &Docker, container_id: &str, image: &str) -> Result<()> {
    let (repo, tag) = split_repo_and_tag(image);
    let options = CommitContainerOptions {
        container: container_id.to_string(),
        repo,
        tag,
        pause: false,
        ..Default::default()
    };
    docker
        .commit_container(options, Config::<String>::default())
        .await?;
    println!("Packed container {} into {}", container_id, image);
    Ok(())
}

fn split_repo_and_tag(reference: &str) -> (String, String) {
    let last_slash = reference.rfind('/');
    let last_colon = reference.rfind(':');

    match last_colon {
        Some(colon_index)
            if last_slash
                .map(|slash_index| colon_index > slash_index)
                .unwrap_or(true) =>
        {
            (
                reference[..colon_index].to_string(),
                reference[colon_index + 1..].to_string(),
            )
        }
        _ => (reference.to_string(), "latest".to_string()),
    }
}

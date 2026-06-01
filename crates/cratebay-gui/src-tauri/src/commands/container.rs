//! Container management Tauri commands.

use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::AppState;
use cratebay_core::error::AppError;
use cratebay_core::models::AuditAction;
use cratebay_core::models::{
    ContainerCreateRequest, ContainerDetail, ContainerInfo, ContainerListFilters, ContainerStats,
    ExecResult, ImageInspectInfo, ImageSearchResult, LocalImageInfo, LogEntry, LogOptions,
};
use cratebay_core::MutexExt;
use cratebay_core::{audit, bundle_images, container, storage, validation};

/// List available container templates.
#[tauri::command]
pub async fn container_templates(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let db = state.db.lock_or_recover()?;
    storage::list_templates(&db)
}

/// List all containers, optionally filtered.
#[tauri::command]
pub async fn container_list(
    state: State<'_, AppState>,
    filters: Option<ContainerListFilters>,
) -> Result<Vec<ContainerInfo>, AppError> {
    let docker = state.ensure_docker_once().await?;
    container::list(&docker, true, filters).await
}

/// Create a new container.
///
/// The caller should ensure the image is available locally before calling.
/// Use `image_pull` to pull images before creation.
#[tauri::command]
pub async fn container_create(
    state: State<'_, AppState>,
    request: ContainerCreateRequest,
) -> Result<ContainerInfo, AppError> {
    let docker = state.ensure_docker_once().await?;

    // Validate input
    validation::validate_container_name(&request.name)?;
    if let (Some(cpu), Some(mem)) = (request.cpu_cores, request.memory_mb) {
        validation::validate_resource_limits(cpu, mem)?;
    }

    let result = container::create(&docker, request).await?;

    // Audit
    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerCreate, &result.id, None, "user")?;

    Ok(result)
}

/// Start a stopped container.
#[tauri::command]
pub async fn container_start(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let docker = state.ensure_docker_once().await?;
    container::start(&docker, &id).await?;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerStart, &id, None, "user")?;
    Ok(())
}

/// Stop a running container.
#[tauri::command]
pub async fn container_stop(
    state: State<'_, AppState>,
    id: String,
    timeout: Option<u32>,
) -> Result<(), AppError> {
    let docker = state.ensure_docker_once().await?;
    container::stop(&docker, &id, timeout).await?;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerStop, &id, None, "user")?;
    Ok(())
}

/// Remove a container.
#[tauri::command]
pub async fn container_delete(
    state: State<'_, AppState>,
    id: String,
    force: Option<bool>,
) -> Result<(), AppError> {
    let docker = state.ensure_docker_once().await?;
    container::delete(&docker, &id, force.unwrap_or(false)).await?;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerDelete, &id, None, "user")?;
    Ok(())
}

/// Execute a command inside a running container.
#[tauri::command]
pub async fn container_exec(
    state: State<'_, AppState>,
    id: String,
    cmd: Vec<String>,
    working_dir: Option<String>,
) -> Result<ExecResult, AppError> {
    let docker = state.ensure_docker_once().await?;
    let result = container::exec(&docker, &id, cmd, working_dir).await?;

    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerExec, &id, None, "user")?;

    Ok(result)
}

/// Get container logs.
#[tauri::command]
pub async fn container_logs(
    state: State<'_, AppState>,
    id: String,
    options: Option<LogOptions>,
) -> Result<Vec<LogEntry>, AppError> {
    let docker = state.ensure_docker_once().await?;
    container::logs(&docker, &id, options).await
}

/// Inspect a container for detailed information.
#[tauri::command]
pub async fn container_inspect(
    state: State<'_, AppState>,
    id: String,
) -> Result<ContainerDetail, AppError> {
    let docker = state.ensure_docker_once().await?;
    container::inspect(&docker, &id).await
}

/// Get real-time resource usage snapshot for a container.
#[tauri::command]
pub async fn container_stats(
    state: State<'_, AppState>,
    id: String,
) -> Result<ContainerStats, AppError> {
    let docker = state.ensure_docker_once().await?;
    container::stats(&docker, &id).await
}

/// Execute a command with streaming output via Tauri Events.
///
/// Output is emitted as events on `exec:stream:{channel_id}`.
#[tauri::command]
pub async fn container_exec_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    cmd: Vec<String>,
    channel_id: String,
    working_dir: Option<String>,
) -> Result<(), AppError> {
    let docker = state.ensure_docker_once().await?;

    let event_name = format!("exec:stream:{}", channel_id);
    let app_handle = app.clone();

    container::exec_stream(&docker, &id, cmd, working_dir, move |chunk| {
        let _ = app_handle.emit(&event_name, &chunk);
    })
    .await?;

    // Audit
    let db = state.db.lock_or_recover()?;
    audit::log_action(&db, &AuditAction::ContainerExec, &id, None, "user")?;

    Ok(())
}

/// List local Docker images.
#[tauri::command]
pub async fn image_list(state: State<'_, AppState>) -> Result<Vec<LocalImageInfo>, AppError> {
    let docker = state.ensure_docker_once().await?;
    container::image_list(&docker).await
}

/// Search images from registry (Docker Hub via Docker API).
#[tauri::command]
pub async fn image_search(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<Vec<ImageSearchResult>, AppError> {
    let term = query.trim();
    let limit = limit.map(u64::from);

    // Prefer Docker Engine search when an explicit host or already-running
    // built-in runtime is reachable. Avoid provisioning/starting the runtime
    // just for image search. When DOCKER_HOST is explicit, surface connection
    // failures instead of silently falling back to Docker Hub.
    if let Ok(docker) = state.require_docker() {
        if cratebay_core::docker::is_available(&docker).await {
            return container::image_search(&docker, term, limit).await;
        }
    }

    if let Some(host) = cratebay_core::docker::explicit_host_override() {
        let docker = cratebay_core::docker::connect_host(&host).await?;
        return container::image_search(&docker, term, limit).await;
    }

    if let Some(docker) = cratebay_core::docker::try_connect().await {
        return container::image_search(&docker, term, limit).await;
    }

    container::image_search_dockerhub(term, limit).await
}

/// Inspect a local image by id or reference.
#[tauri::command]
pub async fn image_inspect(
    state: State<'_, AppState>,
    id: String,
) -> Result<ImageInspectInfo, AppError> {
    let docker = state.ensure_docker_once().await?;
    container::image_inspect(&docker, &id).await
}

/// Remove a local image.
#[tauri::command]
pub async fn image_remove(
    state: State<'_, AppState>,
    id: String,
    force: Option<bool>,
) -> Result<(), AppError> {
    let docker = state.ensure_docker_once().await?;
    container::image_remove(&docker, &id, force.unwrap_or(false)).await
}

/// Tag a local image with a new `repo:tag`.
#[tauri::command]
pub async fn image_tag(
    state: State<'_, AppState>,
    source: String,
    target: String,
) -> Result<(), AppError> {
    let docker = state.ensure_docker_once().await?;
    container::image_tag(&docker, &source, &target).await
}

/// Commit a container into a new local image tag.
#[tauri::command]
pub async fn image_pack_container(
    state: State<'_, AppState>,
    container: String,
    image: String,
) -> Result<String, AppError> {
    let docker = state.ensure_docker_once().await?;
    container::image_commit_container(&docker, &container, &image).await
}

/// Export one or more local images to a tar archive.
#[tauri::command]
pub async fn image_export(
    state: State<'_, AppState>,
    images: Vec<String>,
    output: String,
) -> Result<u64, AppError> {
    let docker = state.ensure_docker_once().await?;
    container::image_export_to_tar(&docker, &images, &output).await
}

/// Import images from a tar archive.
#[tauri::command]
pub async fn image_import(
    state: State<'_, AppState>,
    input: String,
) -> Result<Vec<String>, AppError> {
    let docker = state.ensure_docker_once().await?;
    container::image_load_from_tar(&docker, &input).await
}

/// Load bundled CrateBay container images into the built-in runtime.
#[tauri::command]
pub async fn image_preload_bundled(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<bundle_images::BundleImageLoadResult>, AppError> {
    let docker = state.ensure_docker_once().await?;
    let bundle_dir = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("bundle-images"))
        .filter(|dir| dir.is_dir())
        .or_else(bundle_images::find_bundle_image_dir)
        .ok_or_else(|| {
            AppError::Runtime(
                "No bundle-images directory found. Set CRATEBAY_BUNDLE_IMAGES_DIR or include bundle-images in the app resources."
                    .to_string(),
            )
        })?;

    Ok(bundle_images::load_bundle_images_from_dir(&docker, &bundle_dir).await)
}

/// Pull a Docker image (non-blocking).
///
/// Spawns the pull operation in the background so it doesn't block other Tauri commands.
/// Progress and completion are reported via `image:pull:{channel_id}` events.
///
/// Returns immediately with the channel_id for the frontend to listen on.
#[tauri::command]
pub async fn image_pull(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    image: String,
    mirrors: Option<Vec<String>>,
    channel_id: Option<String>,
) -> Result<String, AppError> {
    let docker = state.ensure_docker_once().await?;
    let channel_id = channel_id.unwrap_or_else(|| format!("pull-{}", uuid::Uuid::new_v4()));
    let ch_id = channel_id.clone();
    let app_handle = app.clone();
    let image_clone = image.clone();

    // Emit start event
    let _ = app.emit(
        &crate::events::image_pull_progress_event(&channel_id),
        &crate::events::ImagePullProgress {
            current_layer: 0,
            total_layers: 0,
            progress_percent: 0,
            status: format!("开始拉取镜像 {}", &image),
            complete: false,
            error: None,
            current_bytes: 0,
            total_bytes: 0,
        },
    );

    // Spawn background task — does NOT block the Tauri command handler
    tokio::spawn(async move {
        let app = app_handle;
        let event_name = crate::events::image_pull_progress_event(&ch_id);

        // Progress callback that emits Tauri events
        let app_for_progress = app.clone();
        let event_for_progress = event_name.clone();
        let progress_cb: container::PullProgressCallback = std::sync::Arc::new(move |progress| {
            let percent = if progress.total_bytes > 0 {
                ((progress.current_bytes as f64 / progress.total_bytes as f64) * 100.0) as u32
            } else {
                0
            };
            let status = translate_pull_status(&progress.status);
            let _ = app_for_progress.emit(
                &event_for_progress,
                &crate::events::ImagePullProgress {
                    current_layer: 0,
                    total_layers: 0,
                    progress_percent: percent,
                    status,
                    complete: false,
                    error: None,
                    current_bytes: progress.current_bytes,
                    total_bytes: progress.total_bytes,
                },
            );
        });

        let result = match mirrors {
            Some(ref m) if !m.is_empty() => {
                container::image_pull_with_mirrors(&docker, &image_clone, m, Some(progress_cb))
                    .await
            }
            _ => container::image_pull(&docker, &image_clone, None, Some(progress_cb)).await,
        };

        match result {
            Ok(()) => {
                let _ = app.emit(
                    &event_name,
                    &crate::events::ImagePullProgress {
                        current_layer: 0,
                        total_layers: 0,
                        progress_percent: 100,
                        status: format!("镜像 {} 拉取完成", &image_clone),
                        complete: true,
                        error: None,
                        current_bytes: 0,
                        total_bytes: 0,
                    },
                );
            }
            Err(e) => {
                let error_msg = e.to_string();
                tracing::error!("Image pull failed for {}: {}", image_clone, error_msg);
                let _ = app.emit(
                    &event_name,
                    &crate::events::ImagePullProgress {
                        current_layer: 0,
                        total_layers: 0,
                        progress_percent: 0,
                        status: format!("镜像拉取失败: {}", error_msg),
                        complete: true,
                        error: Some(error_msg),
                        current_bytes: 0,
                        total_bytes: 0,
                    },
                );
            }
        }
    });

    // Return immediately with the channel_id
    Ok(channel_id)
}

/// Translate Docker pull status messages to Chinese.
fn translate_pull_status(status: &str) -> String {
    // Docker API status messages are like "Downloading", "Extracting",
    // "Pull complete", "Pulling fs layer", "Verifying Checksum", "Download complete",
    // "Already exists", "Waiting", "Digest: sha256:...", "Pulling from library/xxx"
    let s = status.trim();
    if let Some(image) = s.strip_prefix("Pulling from ") {
        return format!("正在拉取 {}", image);
    }
    if s.starts_with("Digest:") {
        return s.to_string(); // Keep digest as-is
    }
    match s {
        "Downloading" => "下载中".to_string(),
        "Extracting" => "解压中".to_string(),
        "Download complete" => "下载完成".to_string(),
        "Pull complete" => "拉取完成".to_string(),
        "Pulling fs layer" => "拉取层".to_string(),
        "Verifying Checksum" => "校验中".to_string(),
        "Already exists" => "已存在".to_string(),
        "Waiting" => "等待中".to_string(),
        "Retrying" => "重试中".to_string(),
        _ => s.to_string(),
    }
}

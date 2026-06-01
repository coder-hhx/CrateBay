//! Application state managed by Tauri.

use bollard::Docker;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;

use cratebay_core::engine::EnsureOptions;
use cratebay_core::error::AppError;
use cratebay_core::runtime::RuntimeManager;

/// Shared application state accessible from all Tauri commands.
pub struct AppState {
    /// Docker client (optional — Docker may not be available).
    /// Wrapped in Mutex so it can be updated after runtime starts.
    pub docker: Arc<Mutex<Option<Arc<Docker>>>>,

    /// Source label for the current Docker client.
    pub docker_source: Arc<Mutex<Option<String>>>,

    /// In-process single-flight guard for Docker initialisation.
    ///
    /// Only one caller runs `engine::ensure_docker()` at a time, but failed
    /// attempts are not cached. This lets the GUI recover after a temporary
    /// runtime start/connect failure without requiring an app restart.
    pub docker_init_lock: Arc<AsyncMutex<()>>,

    /// SQLite database connection.
    pub db: Arc<Mutex<Connection>>,

    /// Application data directory (~/.cratebay/).
    pub data_dir: PathBuf,

    /// Built-in container runtime manager (platform-specific).
    pub runtime: Arc<dyn RuntimeManager>,
}

impl AppState {
    /// Get a clone of the Docker client Arc, or error if unavailable.
    pub fn require_docker(&self) -> Result<Arc<Docker>, AppError> {
        let guard = self
            .docker
            .lock()
            .map_err(|e| AppError::Runtime(format!("Docker state mutex poisoned: {}", e)))?;
        guard.clone().ok_or_else(|| {
            AppError::Docker(bollard::errors::Error::DockerResponseServerError {
                status_code: 503,
                message: "Docker is not available. Please start CrateBay Runtime first."
                    .to_string(),
            })
        })
    }

    /// Ensure Docker is available, with in-process single-flight deduplication.
    ///
    /// When Docker is not yet connected, only the **first** concurrent caller
    /// runs the full `engine::ensure_docker()` start sequence. All other
    /// concurrent callers await that same future.
    ///
    /// # Fast path
    /// If the shared Docker client is already present, it is returned
    /// **immediately without a ping** to avoid per-command serialisation.
    pub async fn ensure_docker_once(&self) -> Result<Arc<Docker>, AppError> {
        // Fast path: a client is already stored — return it immediately.
        if let Ok(docker) = self.require_docker() {
            return Ok(docker);
        }

        // Single-flight init: exactly one concurrent caller runs ensure_docker.
        let _init_guard = self.docker_init_lock.lock().await;

        // Another caller may have completed while we were waiting for the lock.
        if let Ok(docker) = self.require_docker() {
            return Ok(docker);
        }

        let options = EnsureOptions {
            lock_wait_timeout: Duration::from_secs(60),
            ..Default::default()
        };
        let docker = cratebay_core::engine::ensure_docker(self.runtime.as_ref(), options).await?;
        self.set_docker(Some(docker.clone()), Some("builtin".to_string()));
        Ok(docker)
    }

    /// Update the Docker client (e.g., after runtime starts).
    pub fn set_docker(&self, docker: Option<Arc<Docker>>, source: Option<String>) {
        if let Ok(mut guard) = self.docker.lock() {
            *guard = docker;
        }
        if let Ok(mut guard) = self.docker_source.lock() {
            *guard = source;
        }
    }

    /// Get the current Docker source label, if any.
    pub fn docker_source(&self) -> Option<String> {
        self.docker_source
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Check if Docker is currently available.
    pub fn has_docker(&self) -> bool {
        self.docker.lock().map(|g| g.is_some()).unwrap_or(false)
    }
}

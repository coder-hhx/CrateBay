//! Application state managed by Tauri.

use bollard::Docker;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Mutex as AsyncMutex;

use cratebay_core::engine::EnsureOptions;
use cratebay_core::error::AppError;
use cratebay_core::runtime::{self, RuntimeManager};

pub struct ContainerTerminalSession {
    pub container_id: String,
    pub closed: Arc<AtomicBool>,
}

/// Shared application state accessible from all Tauri commands.
pub struct AppState {
    /// Engine compatibility client (optional — runtime may not be available).
    /// Wrapped in Mutex so it can be updated after runtime starts.
    pub engine_compatibility: Arc<Mutex<Option<Arc<Docker>>>>,

    /// Source label for the current engine compatibility client.
    pub engine_compatibility_source: Arc<Mutex<Option<String>>>,

    /// In-process single-flight guard for engine initialisation.
    ///
    /// Only one caller runs Engine startup/adoption at a time, but failed
    /// attempts are not cached. This lets the GUI recover after a temporary
    /// runtime start/connect failure without requiring an app restart.
    pub engine_init_lock: Arc<AsyncMutex<()>>,

    /// SQLite database connection.
    pub db: Arc<Mutex<Connection>>,

    /// Application data directory (~/.cratebay/).
    pub data_dir: PathBuf,

    /// Built-in container runtime manager (platform-specific).
    pub runtime: Arc<dyn RuntimeManager>,

    /// Interactive container terminal sessions keyed by frontend-generated id.
    pub terminal_sessions: Arc<AsyncMutex<HashMap<String, ContainerTerminalSession>>>,
}

impl AppState {
    /// Ensure the native CrateBay Engine API is available.
    ///
    /// Native management commands use this path so their readiness check is the
    /// CrateBay `/cratebay/engine` contract, not the Docker-compatible endpoint.
    pub async fn ensure_native_engine_once(&self) -> Result<(), AppError> {
        // Fast path: avoid serialising native commands once the Engine contract is live.
        if runtime::query_built_in_ready_engine_status(self.runtime.as_ref()).is_ok() {
            return Ok(());
        }

        // Single-flight init: exactly one concurrent caller starts/adopts the Engine.
        let _init_guard = self.engine_init_lock.lock().await;

        // Another caller may have completed while we were waiting for the lock.
        if runtime::query_built_in_ready_engine_status(self.runtime.as_ref()).is_ok() {
            return Ok(());
        }

        let options = EnsureOptions {
            lock_wait_timeout: Duration::from_secs(60),
            ..Default::default()
        };
        cratebay_core::engine::ensure_engine_contract(self.runtime.as_ref(), options).await?;
        Ok(())
    }

    /// Update the engine compatibility client (e.g., after runtime starts).
    pub fn set_engine_compatibility(&self, docker: Option<Arc<Docker>>, source: Option<String>) {
        if let Ok(mut guard) = self.engine_compatibility.lock() {
            *guard = docker;
        }
        if let Ok(mut guard) = self.engine_compatibility_source.lock() {
            *guard = source;
        }
    }

    /// Get the current engine source label, if any.
    pub fn engine_compatibility_source(&self) -> Option<String> {
        self.engine_compatibility_source
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }
}

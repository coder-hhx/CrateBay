//! Tauri event definitions for streaming and real-time updates.

/// Event names used for Tauri event system.
pub mod event_names {
    /// Prefix for image pull progress events. The full event name is
    /// `image:pull:{channel_id}` where channel_id is unique per pull operation.
    pub const IMAGE_PULL_PROGRESS_PREFIX: &str = "image:pull";

    #[allow(dead_code)]
    pub const CONTAINER_STATUS_CHANGE: &str = "container:status-change";
    pub const RUNTIME_HEALTH: &str = "runtime:health";
    pub const RUNTIME_PROVISION: &str = "runtime:provision";
    pub const ENGINE_CONNECTED: &str = "engine:connected";
}

/// Build a scoped image pull progress event name.
pub fn image_pull_progress_event(channel_id: &str) -> String {
    format!("{}:{}", event_names::IMAGE_PULL_PROGRESS_PREFIX, channel_id)
}

/// Image pull progress update event payload.
#[derive(serde::Serialize, Clone, Debug)]
pub struct ImagePullProgress {
    /// Current layer being pulled.
    pub current_layer: u32,
    /// Total layers to pull.
    pub total_layers: u32,
    /// Overall progress percentage (0-100).
    pub progress_percent: u32,
    /// Human-readable status message.
    pub status: String,
    /// Whether pull is complete (either success or failure).
    pub complete: bool,
    /// Error message if pull failed. None means success (when complete=true).
    pub error: Option<String>,
    /// Bytes downloaded so far (current chunk/layer).
    pub current_bytes: u64,
    /// Total bytes expected (current chunk/layer).
    pub total_bytes: u64,
}

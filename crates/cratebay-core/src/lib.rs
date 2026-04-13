//! CrateBay Core — shared library for all CrateBay binaries.
//!
//! This crate contains business logic, storage, runtime management,
//! and Docker integration. Binary crates (gui, cli, mcp) depend on this.

pub mod audit;
pub mod container;
pub mod docker;
pub mod engine;
pub mod error;
pub mod fsutil;
pub mod images;
pub mod llm_proxy;
pub mod mcp;
pub mod models;
pub mod proxy;
pub mod runtime;
pub mod sandbox;
pub mod storage;
pub mod validation;

// Re-export commonly used types
pub use error::AppError;
pub use models::*;

/// Extension trait for safe mutex locking (never `.lock().unwrap()`).
pub trait MutexExt<T> {
    fn lock_or_recover(&self) -> Result<std::sync::MutexGuard<'_, T>, AppError>;
}

impl<T> MutexExt<T> for std::sync::Mutex<T> {
    fn lock_or_recover(&self) -> Result<std::sync::MutexGuard<'_, T>, AppError> {
        self.lock().map_err(|e| {
            tracing::error!("Mutex poisoned, attempting recovery: {}", e);
            AppError::Runtime(format!("Mutex poisoned: {}", e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_display_validation() {
        let err = AppError::Validation("invalid name".to_string());
        assert_eq!(err.to_string(), "Validation error: invalid name");
    }

    #[test]
    fn app_error_display_not_found() {
        let err = AppError::NotFound {
            entity: "container".to_string(),
            id: "abc123".to_string(),
        };
        assert_eq!(err.to_string(), "Not found: container with id abc123");
    }

    #[test]
    fn app_error_display_permission_denied() {
        let err = AppError::PermissionDenied("host volume mount".to_string());
        assert_eq!(err.to_string(), "Permission denied: host volume mount");
    }

    #[test]
    fn app_error_serializes_as_string() {
        let err = AppError::Validation("test".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"Validation error: test\"");
    }

    #[test]
    fn mutex_ext_works() {
        let mutex = std::sync::Mutex::new(42);
        let guard = mutex.lock_or_recover().unwrap();
        assert_eq!(*guard, 42);
    }
}

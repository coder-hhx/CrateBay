//! Input validation utilities.

use crate::error::AppError;
use std::path::{Path, PathBuf};

/// Validate that a name is non-empty and within length limits.
pub fn validate_name(name: &str, max_len: usize) -> Result<(), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::Validation("Name cannot be empty".into()));
    }
    if name.len() > max_len {
        return Err(AppError::Validation(format!(
            "Name too long: {} chars (max {})",
            name.len(),
            max_len
        )));
    }
    Ok(())
}

/// Validate container name (alphanumeric + hyphens/underscores, 1-64 chars).
pub fn validate_container_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() || name.len() > 64 {
        return Err(AppError::Validation(
            "Container name must be 1-64 characters".into(),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::Validation(
            "Container name must contain only alphanumeric characters, hyphens, or underscores"
                .into(),
        ));
    }
    Ok(())
}

/// Validate that a URL is well-formed.
pub fn validate_url(url: &str) -> Result<(), AppError> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(AppError::Validation(format!(
            "Invalid URL: must start with http:// or https://, got: {}",
            url
        )));
    }
    Ok(())
}

/// Validate file path is within workspace root (prevent path traversal).
pub fn validate_path_within_root(path: &Path, root: &Path) -> Result<PathBuf, AppError> {
    let canonical = path
        .canonicalize()
        .map_err(|_| AppError::Validation(format!("Invalid path: {}", path.display())))?;
    let root_canonical = root
        .canonicalize()
        .map_err(|_| AppError::Validation(format!("Invalid root: {}", root.display())))?;

    if !canonical.starts_with(&root_canonical) {
        return Err(AppError::PermissionDenied(
            "Path traversal detected: path is outside workspace root".into(),
        ));
    }
    Ok(canonical)
}

/// Validate resource limits for containers.
pub fn validate_resource_limits(cpu: u32, memory_mb: u64) -> Result<(), AppError> {
    if cpu == 0 || cpu > 16 {
        return Err(AppError::Validation("CPU cores must be 1-16".into()));
    }
    if !(256..=65536).contains(&memory_mb) {
        return Err(AppError::Validation("Memory must be 256-65536 MB".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name() {
        assert!(validate_name("hello", 100).is_ok());
        assert!(validate_name("", 100).is_err());
        assert!(validate_name("   ", 100).is_err());
        assert!(validate_name("a", 0).is_err());
        assert!(validate_name("abcde", 3).is_err());
    }

    #[test]
    fn test_validate_container_name() {
        assert!(validate_container_name("my-container").is_ok());
        assert!(validate_container_name("my_container_123").is_ok());
        assert!(validate_container_name("").is_err());
        assert!(validate_container_name("my container").is_err());
        assert!(validate_container_name("my.container").is_err());
        let long_name = "a".repeat(65);
        assert!(validate_container_name(&long_name).is_err());
    }

    #[test]
    fn test_validate_url() {
        assert!(validate_url("https://registry-1.docker.io").is_ok());
        assert!(validate_url("http://localhost:8080").is_ok());
        assert!(validate_url("ftp://bad.com").is_err());
        assert!(validate_url("not-a-url").is_err());
    }

    #[test]
    fn test_validate_resource_limits() {
        assert!(validate_resource_limits(2, 1024).is_ok());
        assert!(validate_resource_limits(0, 1024).is_err());
        assert!(validate_resource_limits(17, 1024).is_err());
        assert!(validate_resource_limits(2, 128).is_err());
        assert!(validate_resource_limits(2, 70000).is_err());
    }
}

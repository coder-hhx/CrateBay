//! Audit logging for all modification operations.
//!
//! Records all significant actions to the audit_log table for
//! accountability and debugging.

use rusqlite::params;

use crate::error::AppError;
use crate::models::AuditAction;

/// Log an audit event to the database.
pub fn log_action(
    conn: &rusqlite::Connection,
    action: &AuditAction,
    target: &str,
    details: Option<&str>,
    user: &str,
) -> Result<(), AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO audit_log (id, action, target, details, user)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, action.as_str(), target, details, user],
    )?;
    Ok(())
}

/// Log an audit event with a specific timestamp (for testing or replay).
pub fn log_action_with_timestamp(
    conn: &rusqlite::Connection,
    action: &AuditAction,
    target: &str,
    details: Option<&str>,
    user: &str,
    timestamp: &str,
) -> Result<(), AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO audit_log (id, timestamp, action, target, details, user)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, timestamp, action.as_str(), target, details, user],
    )?;
    Ok(())
}

/// Rotate audit logs older than the specified number of days.
pub fn rotate_audit_log(
    conn: &rusqlite::Connection,
    older_than_days: u32,
) -> Result<u32, AppError> {
    let deleted = conn.execute(
        "DELETE FROM audit_log WHERE timestamp < datetime('now', ?1)",
        params![format!("-{} days", older_than_days)],
    )?;
    Ok(deleted as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage;

    fn setup_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        storage::migrate(&conn).unwrap();
        conn
    }

    #[test]
    fn test_log_action() {
        let conn = setup_db();
        log_action(
            &conn,
            &AuditAction::ContainerCreate,
            "container-123",
            Some("Created node-dev container"),
            "user",
        )
        .unwrap();

        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_log_action_various_types() {
        let conn = setup_db();

        let actions = vec![
            AuditAction::ContainerCreate,
            AuditAction::ContainerStart,
            AuditAction::ContainerStop,
            AuditAction::ContainerDelete,
            AuditAction::SettingsUpdate,
        ];

        for action in &actions {
            log_action(&conn, action, "test-target", None, "user").unwrap();
        }

        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, actions.len() as u32);
    }
}

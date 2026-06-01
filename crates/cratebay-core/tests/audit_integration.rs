//! Integration tests for the audit logging module.
//!
//! Tests cover audit log writing, querying with various filters,
//! timestamp-based insertion, and log rotation.

use rusqlite::Connection;

use cratebay_core::audit;
use cratebay_core::models::AuditAction;
use cratebay_core::storage;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    storage::migrate(&conn).unwrap();
    conn
}

// ─── Basic Audit Logging ────────────────────────────────────────────

#[test]
fn audit_log_action_writes_to_db() {
    let conn = setup_db();

    audit::log_action(
        &conn,
        &AuditAction::ContainerCreate,
        "container-abc",
        Some("Created node-dev container"),
        "testuser",
    )
    .unwrap();

    let count: u32 = conn
        .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn audit_log_all_action_types() {
    let conn = setup_db();

    let actions = vec![
        (AuditAction::ContainerCreate, "container.create"),
        (AuditAction::ContainerStart, "container.start"),
        (AuditAction::ContainerStop, "container.stop"),
        (AuditAction::ContainerDelete, "container.delete"),
        (AuditAction::ContainerExec, "container.exec"),
        (AuditAction::SettingsUpdate, "settings.update"),
    ];

    for (action, expected_str) in &actions {
        assert_eq!(
            action.as_str(),
            *expected_str,
            "AuditAction::{:?} should serialize to '{}'",
            action,
            expected_str
        );
        audit::log_action(&conn, action, "test", None, "user").unwrap();
    }

    let count: u32 = conn
        .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 6);
}

#[test]
fn audit_log_with_null_details() {
    let conn = setup_db();

    audit::log_action(&conn, &AuditAction::SettingsUpdate, "theme", None, "user").unwrap();

    let details: Option<String> = conn
        .query_row("SELECT details FROM audit_log LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(details.is_none());
}

// ─── Querying with list_audit_logs ──────────────────────────────────

#[test]
fn list_audit_logs_no_filter() {
    let conn = setup_db();

    // Insert multiple events
    audit::log_action(&conn, &AuditAction::ContainerCreate, "c1", None, "user").unwrap();
    audit::log_action(&conn, &AuditAction::ContainerStart, "c1", None, "user").unwrap();
    audit::log_action(&conn, &AuditAction::ContainerStop, "c1", None, "user").unwrap();

    let logs = storage::list_audit_logs(&conn, None, None, 100).unwrap();
    assert_eq!(logs.len(), 3);
}

#[test]
fn list_audit_logs_filter_by_action() {
    let conn = setup_db();

    audit::log_action(&conn, &AuditAction::ContainerCreate, "c1", None, "user").unwrap();
    audit::log_action(&conn, &AuditAction::ContainerCreate, "c2", None, "user").unwrap();
    audit::log_action(&conn, &AuditAction::ContainerStop, "c1", None, "user").unwrap();

    let logs = storage::list_audit_logs(&conn, Some("container.create"), None, 100).unwrap();
    assert_eq!(logs.len(), 2);
    for log in &logs {
        assert_eq!(log["action"].as_str().unwrap(), "container.create");
    }
}

#[test]
fn list_audit_logs_filter_by_target() {
    let conn = setup_db();

    audit::log_action(
        &conn,
        &AuditAction::ContainerCreate,
        "container-a",
        None,
        "user",
    )
    .unwrap();
    audit::log_action(
        &conn,
        &AuditAction::ContainerStart,
        "container-a",
        None,
        "user",
    )
    .unwrap();
    audit::log_action(
        &conn,
        &AuditAction::ContainerCreate,
        "container-b",
        None,
        "user",
    )
    .unwrap();

    let logs = storage::list_audit_logs(&conn, None, Some("container-a"), 100).unwrap();
    assert_eq!(logs.len(), 2);
    for log in &logs {
        assert_eq!(log["target"].as_str().unwrap(), "container-a");
    }
}

#[test]
fn list_audit_logs_filter_by_action_and_target() {
    let conn = setup_db();

    audit::log_action(&conn, &AuditAction::ContainerCreate, "c1", None, "user").unwrap();
    audit::log_action(&conn, &AuditAction::ContainerStart, "c1", None, "user").unwrap();
    audit::log_action(&conn, &AuditAction::ContainerCreate, "c2", None, "user").unwrap();

    let logs = storage::list_audit_logs(&conn, Some("container.create"), Some("c1"), 100).unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0]["action"].as_str().unwrap(), "container.create");
    assert_eq!(logs[0]["target"].as_str().unwrap(), "c1");
}

#[test]
fn list_audit_logs_respects_limit() {
    let conn = setup_db();

    for i in 0..10 {
        audit::log_action(
            &conn,
            &AuditAction::SettingsUpdate,
            &format!("key-{}", i),
            None,
            "user",
        )
        .unwrap();
    }

    let logs = storage::list_audit_logs(&conn, None, None, 3).unwrap();
    assert_eq!(logs.len(), 3);
}

// ─── Timestamp-based Insertion ──────────────────────────────────────

#[test]
fn log_action_with_custom_timestamp() {
    let conn = setup_db();

    audit::log_action_with_timestamp(
        &conn,
        &AuditAction::ContainerCreate,
        "c1",
        Some("Custom time"),
        "user",
        "2025-01-01T00:00:00Z",
    )
    .unwrap();

    let timestamp: String = conn
        .query_row("SELECT timestamp FROM audit_log LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(timestamp, "2025-01-01T00:00:00Z");
}

// ─── Log Rotation ───────────────────────────────────────────────────

#[test]
fn rotate_audit_log_deletes_old_entries() {
    let conn = setup_db();

    // Insert an entry with an old timestamp
    audit::log_action_with_timestamp(
        &conn,
        &AuditAction::SettingsUpdate,
        "old-setting",
        None,
        "user",
        "2020-01-01T00:00:00Z",
    )
    .unwrap();

    // Insert a recent entry
    audit::log_action(
        &conn,
        &AuditAction::SettingsUpdate,
        "new-setting",
        None,
        "user",
    )
    .unwrap();

    let before_count: u32 = conn
        .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
        .unwrap();
    assert_eq!(before_count, 2);

    // Rotate: delete entries older than 30 days
    let deleted = audit::rotate_audit_log(&conn, 30).unwrap();
    assert!(deleted >= 1, "Should have deleted at least the old entry");

    let after_count: u32 = conn
        .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
        .unwrap();
    assert!(
        after_count < before_count,
        "Count should decrease after rotation"
    );
}

#[test]
fn rotate_audit_log_keeps_recent_entries() {
    let conn = setup_db();

    // Insert only recent entries
    audit::log_action(&conn, &AuditAction::ContainerCreate, "c1", None, "user").unwrap();
    audit::log_action(&conn, &AuditAction::ContainerStart, "c1", None, "user").unwrap();

    let deleted = audit::rotate_audit_log(&conn, 30).unwrap();
    assert_eq!(deleted, 0, "No entries should be deleted");

    let count: u32 = conn
        .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 2);
}

// ─── Audit Log Metadata ────────────────────────────────────────────

#[test]
fn audit_log_stores_all_fields() {
    let conn = setup_db();

    audit::log_action(
        &conn,
        &AuditAction::SettingsUpdate,
        "theme",
        Some("dark"),
        "admin",
    )
    .unwrap();

    let logs = storage::list_audit_logs(&conn, None, None, 10).unwrap();
    assert_eq!(logs.len(), 1);

    let log = &logs[0];
    assert!(log["id"].as_str().is_some());
    assert!(log["timestamp"].as_str().is_some());
    assert_eq!(log["action"].as_str().unwrap(), "settings.update");
    assert_eq!(log["target"].as_str().unwrap(), "theme");
    assert_eq!(log["details"].as_str().unwrap(), "dark");
    assert_eq!(log["user"].as_str().unwrap(), "admin");
}

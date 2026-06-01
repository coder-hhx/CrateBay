//! Integration tests for the SQLite storage layer.

use rusqlite::Connection;

use cratebay_core::storage;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;",
    )
    .unwrap();
    storage::migrate(&conn).unwrap();
    conn
}

#[test]
fn pragma_wal_mode_set() {
    let conn = setup_db();
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert!(mode == "wal" || mode == "memory");
}

#[test]
fn pragma_foreign_keys_enabled() {
    let conn = setup_db();
    let fk: i32 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    assert_eq!(fk, 1);
}

#[test]
fn migration_creates_expected_tables() {
    let conn = setup_db();

    let expected_tables = [
        "_migrations",
        "container_templates",
        "settings",
        "audit_log",
    ];

    for table_name in &expected_tables {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
                rusqlite::params![table_name],
                |row| row.get(0),
            )
            .unwrap();
        assert!(exists, "Expected table '{}' to exist", table_name);
    }
}

#[test]
fn migration_seeds_default_settings() {
    let conn = setup_db();

    assert_eq!(
        storage::get_setting(&conn, "theme").unwrap(),
        Some("system".to_string())
    );
    assert_eq!(
        storage::get_setting(&conn, "language").unwrap(),
        Some("en".to_string())
    );
    assert_eq!(
        storage::get_setting(&conn, "runtime.auto_start").unwrap(),
        Some("true".to_string())
    );
}

#[test]
fn migration_seeds_default_templates() {
    let conn = setup_db();
    let templates = storage::list_templates(&conn).unwrap();
    assert_eq!(templates.len(), 4);

    let ids: Vec<String> = templates
        .iter()
        .filter_map(|t| t["id"].as_str().map(String::from))
        .collect();
    assert!(ids.contains(&"node-dev".to_string()));
    assert!(ids.contains(&"python-dev".to_string()));
    assert!(ids.contains(&"rust-dev".to_string()));
    assert!(ids.contains(&"ubuntu".to_string()));
}

#[test]
fn settings_get_all_returns_sorted() {
    let conn = setup_db();
    storage::set_setting(&conn, "aaa.first", "1").unwrap();
    storage::set_setting(&conn, "zzz.last", "2").unwrap();

    let all = storage::get_all_settings(&conn).unwrap();
    let keys: Vec<&str> = all.iter().map(|(k, _)| k.as_str()).collect();
    let mut sorted_keys = keys.clone();
    sorted_keys.sort();
    assert_eq!(keys, sorted_keys);
}

#[test]
fn settings_upsert_overwrites() {
    let conn = setup_db();

    storage::set_setting(&conn, "test_key", "value1").unwrap();
    assert_eq!(
        storage::get_setting(&conn, "test_key").unwrap(),
        Some("value1".to_string())
    );

    storage::set_setting(&conn, "test_key", "value2").unwrap();
    assert_eq!(
        storage::get_setting(&conn, "test_key").unwrap(),
        Some("value2".to_string())
    );

    let count: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM settings WHERE key = 'test_key'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn disabled_templates_are_not_listed() {
    let conn = setup_db();
    conn.execute(
        "INSERT INTO container_templates
         (id, name, description, image, enabled, sort_order)
         VALUES ('hidden', 'Hidden', '', 'alpine:latest', 0, -1)",
        [],
    )
    .unwrap();

    let templates = storage::list_templates(&conn).unwrap();
    assert!(templates.iter().all(|t| t["id"] != "hidden"));
}

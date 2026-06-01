//! SQLite storage layer.
//!
//! Database stored at `~/.cratebay/cratebay.db` with WAL mode.
//! Includes migration system, settings CRUD, container templates, audit logs,
//! and platform-aware data path helpers.

use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

use crate::error::AppError;
// ─── Migration System ───────────────────────────────────────────────

/// A numbered database migration.
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// All migrations, applied in order.
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial_schema",
    sql: include_str!("../migrations/001_initial_schema.sql"),
}];

/// Get the default database path: `~/.cratebay/cratebay.db`
pub fn default_db_path() -> Result<PathBuf, AppError> {
    let home = home_dir()?;
    let db_dir = home.join(".cratebay");
    std::fs::create_dir_all(&db_dir)?;
    Ok(db_dir.join("cratebay.db"))
}

/// Open a database connection with recommended PRAGMA settings.
pub fn open(path: &Path) -> Result<Connection, AppError> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(path)?;

    // Apply performance and safety PRAGMAs
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         PRAGMA cache_size = -2000;
         PRAGMA temp_store = MEMORY;",
    )?;

    Ok(conn)
}

/// Run all pending database migrations.
pub fn migrate(conn: &Connection) -> Result<(), AppError> {
    // Create migrations tracking table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            version     INTEGER PRIMARY KEY,
            name        TEXT NOT NULL,
            applied_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        );",
    )?;

    let current_version: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM _migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    for migration in MIGRATIONS {
        if migration.version > current_version {
            tracing::info!(
                "Applying migration v{}: {}",
                migration.version,
                migration.name
            );

            // Run migration in a transaction
            conn.execute_batch("BEGIN;")?;
            match conn.execute_batch(migration.sql) {
                Ok(()) => {
                    conn.execute(
                        "INSERT INTO _migrations (version, name) VALUES (?1, ?2)",
                        params![migration.version, migration.name],
                    )?;
                    conn.execute_batch("COMMIT;")?;
                    tracing::info!("Migration v{} applied successfully", migration.version);
                }
                Err(e) => {
                    let _ = conn.execute_batch("ROLLBACK;");
                    return Err(AppError::Database(e));
                }
            }
        }
    }

    Ok(())
}

/// Open and migrate database in one step.
pub fn init(path: &Path) -> Result<Connection, AppError> {
    let conn = open(path)?;
    migrate(&conn)?;
    Ok(conn)
}

// ─── Settings CRUD ──────────────────────────────────────────────────

/// Get a setting value by key.
pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, AppError> {
    let result = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(result)
}

/// Set a setting value (insert or update).
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO settings (key, value, updated_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
         ON CONFLICT(key) DO UPDATE SET
             value = excluded.value,
             updated_at = excluded.updated_at",
        params![key, value],
    )?;
    Ok(())
}

/// Get all settings as key-value pairs.
pub fn get_all_settings(conn: &Connection) -> Result<Vec<(String, String)>, AppError> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

// ─── Container Template Operations ──────────────────────────────────

/// List all container templates.
pub fn list_templates(conn: &Connection) -> Result<Vec<serde_json::Value>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, image, command, env, ports, volumes,
                cpu_cores, memory_mb, working_dir, labels, enabled, sort_order
         FROM container_templates
         WHERE enabled = 1
         ORDER BY sort_order, name",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "name": row.get::<_, String>(1)?,
            "description": row.get::<_, String>(2)?,
            "image": row.get::<_, String>(3)?,
            "command": row.get::<_, Option<String>>(4)?,
            "env": row.get::<_, String>(5)?,
            "ports": row.get::<_, String>(6)?,
            "volumes": row.get::<_, String>(7)?,
            "cpu_cores": row.get::<_, i32>(8)?,
            "memory_mb": row.get::<_, i64>(9)?,
            "working_dir": row.get::<_, Option<String>>(10)?,
            "labels": row.get::<_, String>(11)?,
        }))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

// ─── Audit Log Operations ───────────────────────────────────────────

/// Query audit logs with optional filtering.
pub fn list_audit_logs(
    conn: &Connection,
    action: Option<&str>,
    target: Option<&str>,
    limit: u32,
) -> Result<Vec<serde_json::Value>, AppError> {
    let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match (action, target) {
        (Some(a), Some(t)) => (
            "SELECT id, timestamp, action, target, details, user
                 FROM audit_log WHERE action = ?1 AND target = ?2
                 ORDER BY timestamp DESC LIMIT ?3"
                .to_string(),
            vec![
                Box::new(a.to_string()),
                Box::new(t.to_string()),
                Box::new(limit),
            ],
        ),
        (Some(a), None) => (
            "SELECT id, timestamp, action, target, details, user
                 FROM audit_log WHERE action = ?1
                 ORDER BY timestamp DESC LIMIT ?2"
                .to_string(),
            vec![Box::new(a.to_string()), Box::new(limit)],
        ),
        (None, Some(t)) => (
            "SELECT id, timestamp, action, target, details, user
                 FROM audit_log WHERE target = ?1
                 ORDER BY timestamp DESC LIMIT ?2"
                .to_string(),
            vec![Box::new(t.to_string()), Box::new(limit)],
        ),
        (None, None) => (
            "SELECT id, timestamp, action, target, details, user
                 FROM audit_log ORDER BY timestamp DESC LIMIT ?1"
                .to_string(),
            vec![Box::new(limit)],
        ),
    };

    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        params_vec.iter().map(|v| v.as_ref()).collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "timestamp": row.get::<_, String>(1)?,
            "action": row.get::<_, String>(2)?,
            "target": row.get::<_, String>(3)?,
            "details": row.get::<_, Option<String>>(4)?,
            "user": row.get::<_, String>(5)?,
        }))
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

// ─── Path Utility Functions ──────────────────────────────────────────
//
// Platform-aware directory helpers used by the runtime, image management,
// and logging subsystems.  These mirror the helpers from the v1 `store`
// module and are the canonical way to locate CrateBay data on disk.

/// CrateBay configuration directory.
///
/// Override with `CRATEBAY_CONFIG_DIR`.  Platform defaults:
/// - macOS:   `~/Library/Application Support/com.cratebay.app`
/// - Linux:   `$XDG_CONFIG_HOME/cratebay` or `~/.config/cratebay`
/// - Windows: `%APPDATA%\cratebay`
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CRATEBAY_CONFIG_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("com.cratebay.app");
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("cratebay");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config").join("cratebay");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("cratebay");
        }
    }

    std::env::temp_dir().join("cratebay")
}

/// CrateBay persistent data directory.
///
/// Override with `CRATEBAY_DATA_DIR`.  Platform defaults:
/// - Linux: `$XDG_DATA_HOME/cratebay` or `~/.local/share/cratebay`
/// - macOS / Windows: same as [`config_dir()`]
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CRATEBAY_DATA_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg).join("cratebay");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("cratebay");
        }
    }

    // macOS / Windows default: same as config_dir.
    config_dir()
}

/// CrateBay log directory.
///
/// Override with `CRATEBAY_LOG_DIR`.
pub fn log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CRATEBAY_LOG_DIR") {
        return PathBuf::from(dir);
    }

    #[cfg(target_os = "linux")]
    {
        return data_dir();
    }

    #[cfg(not(target_os = "linux"))]
    {
        config_dir()
    }
}

/// Console log path for a runtime VM.
pub fn vm_console_log_path(vm_id: &str) -> PathBuf {
    data_dir().join("vms").join(vm_id).join("console.log")
}

/// Write `bytes` atomically: writes to a temporary file then renames.
///
/// Creates parent directories if necessary.  Safe for concurrent use
/// from multiple processes.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;

    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("tmp");
    let unique = format!(
        "{}.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        file_name
    );
    let tmp_path = dir.join(format!(".{}.tmp", unique));

    {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(bytes)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }

    match std::fs::rename(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            // Windows may fail rename if destination exists.
            if path.exists() {
                let _ = std::fs::remove_file(path);
                std::fs::rename(&tmp_path, path).map_err(|_| e)?;
                return Ok(());
            }
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────

fn home_dir() -> Result<PathBuf, AppError> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|_| AppError::Runtime("Cannot determine home directory".into()))
}

// ─── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;",
        )
        .unwrap();
        migrate(&conn).unwrap();
        conn
    }

    #[test]
    fn test_migrations_apply_cleanly() {
        let conn = setup_db();
        // Verify _migrations table has our migration
        let version: u32 = conn
            .query_row("SELECT MAX(version) FROM _migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn test_migrations_idempotent() {
        let conn = setup_db();
        // Running migrate again should be a no-op
        migrate(&conn).unwrap();
        let count: u32 = conn
            .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_settings_crud() {
        let conn = setup_db();

        // Default settings should exist
        let theme = get_setting(&conn, "theme").unwrap();
        assert_eq!(theme, Some("system".to_string()));

        // Update setting
        set_setting(&conn, "theme", "dark").unwrap();
        let theme = get_setting(&conn, "theme").unwrap();
        assert_eq!(theme, Some("dark".to_string()));

        // New setting
        set_setting(&conn, "custom.key", "custom_value").unwrap();
        let value = get_setting(&conn, "custom.key").unwrap();
        assert_eq!(value, Some("custom_value".to_string()));

        // Non-existent setting
        let missing = get_setting(&conn, "nonexistent").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_default_templates_seeded() {
        let conn = setup_db();
        let templates = list_templates(&conn).unwrap();
        assert_eq!(templates.len(), 4);
    }

    // ── SQL injection prevention tests (testing-spec.md §7.4) ──

    #[test]
    fn test_sql_injection_in_setting_key() {
        let conn = setup_db();
        let malicious_key = "key'; DROP TABLE settings; --";
        set_setting(&conn, malicious_key, "value").unwrap();
        let value = get_setting(&conn, malicious_key).unwrap();
        assert_eq!(value, Some("value".to_string()));
    }

    #[test]
    fn test_sql_injection_in_setting_value() {
        let conn = setup_db();
        let malicious_content = "Hello'; DROP TABLE settings; --";
        set_setting(&conn, "message.content", malicious_content).unwrap();
        let value = get_setting(&conn, "message.content").unwrap();
        assert_eq!(value, Some(malicious_content.to_string()));
    }

    // ── Path utility function tests ──

    #[test]
    fn test_config_dir_returns_nonempty() {
        let dir = config_dir();
        assert!(
            !dir.as_os_str().is_empty(),
            "config_dir should not be empty"
        );
    }

    #[test]
    fn test_data_dir_returns_nonempty() {
        let dir = data_dir();
        assert!(!dir.as_os_str().is_empty(), "data_dir should not be empty");
    }

    #[test]
    fn test_log_dir_returns_nonempty() {
        let dir = log_dir();
        assert!(!dir.as_os_str().is_empty(), "log_dir should not be empty");
    }

    #[test]
    fn test_vm_console_log_path_contains_vm_id() {
        let path = vm_console_log_path("test-vm-42");
        let s = path.to_string_lossy();
        assert!(s.contains("test-vm-42"), "path should contain VM id");
        assert!(
            s.ends_with("console.log"),
            "path should end with console.log"
        );
    }

    #[test]
    fn test_write_atomic_creates_file_and_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub").join("test.txt");

        write_atomic(&path, b"hello").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("hello"));
    }

    #[test]
    fn test_write_atomic_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("over.txt");

        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("second"));
    }
}

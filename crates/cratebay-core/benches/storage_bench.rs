//! SQLite storage layer benchmarks.
//!
//! Measures settings CRUD throughput for the image/runtime-focused storage
//! surface.

use cratebay_core::storage;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Create an in-memory database with schema applied.
fn setup_db() -> rusqlite::Connection {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         PRAGMA cache_size = -2000;
         PRAGMA temp_store = MEMORY;",
    )
    .unwrap();
    storage::migrate(&conn).unwrap();
    conn
}

fn bench_setting_write(c: &mut Criterion) {
    let conn = setup_db();
    let mut counter = 0u64;

    c.bench_function("set_setting", |b| {
        b.iter(|| {
            counter += 1;
            let key = format!("bench.setting.{counter}");
            storage::set_setting(black_box(&conn), black_box(&key), black_box("value")).unwrap();
        })
    });
}

fn bench_setting_read(c: &mut Criterion) {
    let conn = setup_db();
    storage::set_setting(&conn, "bench.setting.read", "value").unwrap();

    c.bench_function("get_setting", |b| {
        b.iter(|| {
            let value =
                storage::get_setting(black_box(&conn), black_box("bench.setting.read")).unwrap();
            assert_eq!(value.as_deref(), Some("value"));
        })
    });
}

fn bench_settings_list(c: &mut Criterion) {
    let conn = setup_db();

    for i in 0..100 {
        storage::set_setting(&conn, &format!("bench.setting.{i}"), "value").unwrap();
    }

    c.bench_function("list_settings_100", |b| {
        b.iter(|| {
            let settings = storage::get_all_settings(black_box(&conn)).unwrap();
            assert!(settings.len() >= 100);
        })
    });
}

criterion_group!(
    benches,
    bench_setting_write,
    bench_setting_read,
    bench_settings_list,
);
criterion_main!(benches);

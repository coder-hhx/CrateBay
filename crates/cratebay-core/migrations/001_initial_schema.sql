-- Migration 001: Initial schema
-- Creates all core tables for CrateBay v2

-- Container Templates
CREATE TABLE IF NOT EXISTS container_templates (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    image       TEXT NOT NULL,
    command     TEXT,
    env         TEXT NOT NULL DEFAULT '[]',
    ports       TEXT NOT NULL DEFAULT '[]',
    volumes     TEXT NOT NULL DEFAULT '[]',
    cpu_cores   INTEGER NOT NULL DEFAULT 2,
    memory_mb   INTEGER NOT NULL DEFAULT 1024,
    working_dir TEXT,
    labels      TEXT NOT NULL DEFAULT '{}',
    enabled     INTEGER NOT NULL DEFAULT 1,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Settings (key-value)
CREATE TABLE IF NOT EXISTS settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Audit Log
CREATE TABLE IF NOT EXISTS audit_log (
    id          TEXT PRIMARY KEY,
    timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    action      TEXT NOT NULL,
    target      TEXT NOT NULL DEFAULT '',
    details     TEXT,
    user        TEXT NOT NULL DEFAULT 'user',
    ip_address  TEXT,
    session_id  TEXT
);

CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp
    ON audit_log(timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_audit_log_action
    ON audit_log(action, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_audit_log_target
    ON audit_log(target, timestamp DESC);

-- Seed default data

INSERT OR IGNORE INTO container_templates (id, name, description, image, cpu_cores, memory_mb, env)
VALUES
    ('node-dev', 'Node.js Development', 'Node.js 20 with common dev tools',
     'node:20-alpine', 2, 1024, '["NODE_ENV=development"]'),
    ('python-dev', 'Python Development', 'Python 3.12 with pip',
     'python:3.12-slim', 2, 1024, '[]'),
    ('rust-dev', 'Rust Development', 'Rust stable with cargo',
     'rust:1-slim', 2, 2048, '[]'),
    ('ubuntu', 'Ubuntu Shell', 'Ubuntu 24.04 general-purpose',
     'ubuntu:24.04', 1, 512, '[]');

INSERT OR IGNORE INTO settings (key, value)
VALUES
    ('theme', 'system'),
    ('language', 'en'),
    ('runtime.auto_start', 'true'),
    ('runtime.cpu_cores', '2'),
    ('runtime.memory_mb', '2048');

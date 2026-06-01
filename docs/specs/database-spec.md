# Database Spec

CrateBay uses SQLite for local settings, audit events, and lightweight metadata.
The database lives in the CrateBay data directory.

## Current Tables

- `settings`: key/value settings used by the desktop app and backend.
- `container_templates`: optional presets for creating containers.
- `audit_log`: local history for important actions.

## Settings

Settings should remain simple JSON-compatible values. Keep keys stable across
versions and migrate when behavior changes.

Common settings:

- language
- theme
- registry mirrors
- runtime connectivity options

## Audit Log

Audit records should describe actions that change local state or containers:

- container create/start/stop/delete/exec
- image pull/remove/tag/import/export
- runtime start/stop
- settings update

## Migration Rules

- Migrations must be idempotent.
- Avoid stale tables for removed product areas.
- Keep destructive migrations explicit and documented.

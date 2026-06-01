# Spec Consistency Checklist

Use this checklist after changing public behavior.

## Product Surface

- Top-level navigation only exposes Containers, Images, and Settings.
- Pods are available as a secondary tab under Containers.
- Runtime controls are visible from Settings or status surfaces.
- No removed feature appears in app copy, tests, docs, or command help.

## CLI

- `runtime` commands cover status, start, stop, and provision.
- `image` commands cover list, search, pull, inspect, tag/pack, export, import,
  and delete. Bundled image preload reports per-image results and fails the CLI
  process when expected archives cannot be loaded.
- `container` commands cover list, create, start, stop, delete, exec, logs, and
  inspect. Structured exec supports timeout, bounded output, caller-friendly
  exit-code handling, and timeout/truncation-state reporting.
- `run` commands cover one-shot container execution with output capture, timeout,
  auto-pull, cleanup, environment variables, bind mounts, resource limits, and
  pod attachment. Output capture is bounded for embedded callers, and callers
  can opt out of propagating the container exit code to the CLI process.
- `pod` commands cover list, create, inspect, add, remove, and delete.
- Structured commands support `--json`, `--format json`, and `--format yaml`
  where useful.

## Core

- Docker-facing behavior lives in `cratebay-core`.
- CLI and Tauri command layers stay thin.
- Runtime startup is centralized through the engine/runtime modules.
- Image archive operations stream or handle files safely and return clear
  errors.

## Desktop

- Tauri command registrations match the functions still used by React.
- Frontend stores only keep state for current pages.
- Tests do not reference removed routes, stores, or components.
- Images page exposes import/export actions.
- Pods tab under Containers exposes create/delete/inspect/add/remove workflows.
- macOS release app bundles include valid bundled image resources.

## Storage

- SQLite migrations only contain tables that are still used.
- Settings keys match the desktop settings page and CLI behavior.
- Audit actions reflect container, image, runtime, and settings operations.

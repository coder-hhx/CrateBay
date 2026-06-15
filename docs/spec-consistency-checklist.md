# Spec Consistency Checklist

Use this checklist after changing public behavior.

## Product Surface

- Top-level navigation exposes Dashboard, Containers, Images, Pods, Volumes,
  Networks, and Settings.
- Pods, Volumes, and Networks are first-class resource pages.
- Runtime lifecycle controls, including provision/start/stop/restart, are
  visible from Settings or status surfaces.
- Runtime HTTP proxy controls include bridge, bind host/port, and guest host
  settings on both desktop and CLI surfaces.
- Settings diagnostics and Engine maintenance refreshes use the same aggregate
  runtime diagnostics snapshot exposed to CLI automation.
- Dashboard, Settings diagnostics, and CLI runtime status expose the same
  runtime resource usage fields: CPU, memory, disk, and managed container
  count.
- Runtime HTTP proxy port validation rejects invalid ports consistently across
  desktop and CLI surfaces.
- No removed feature appears in app copy, tests, docs, or command help.

## CLI

- `runtime` commands cover status, diagnostics, start, stop, restart,
  provision, and persisted HTTP proxy settings.
- Default resource-management commands auto-start/adopt the built-in runtime
  through the native Engine contract; explicit `--engine-host` remains
  compatibility mode, while status/diagnostics and registry search stay
  non-starting.
- `runtime diagnostics` mirrors the desktop diagnostics/maintenance snapshot
  surface and keeps per-section structured errors for offline Engine state.
- `image` commands cover list, search, pull, inspect, tag/pack, export, import,
  and delete. Bundled image preload reports per-image results and fails the CLI
  process when expected archives cannot be loaded.
- `container` commands cover list, create, start, stop, delete, exec, logs,
  stats, inspect, and native terminal session operations. Structured exec
  supports timeout, bounded output, caller-friendly
  exit-code handling, and timeout/truncation-state reporting.
- `run` commands cover one-shot container execution with output capture, timeout,
  auto-pull, cleanup, environment variables, bind mounts, resource limits, and
  pod attachment. Output capture is bounded for embedded callers, and callers
  can opt out of propagating the container exit code to the CLI process.
- `pod` commands cover list, create, inspect, add, remove, and delete.
- `volume` commands cover create, list, inspect, and remove.
- `network` commands cover create, list, inspect, and remove.
- Structured commands support `--json`, `--format json`, and `--format yaml`
  where useful.
- `settings` commands cover list, get, set, and reset for the persisted desktop
  Settings keys used by CLI automation.
- `update check` follows the desktop updater release selection rules and
  reports GitHub Release plus `latest.json` manifest metadata without starting
  the runtime. Its default prerelease behavior comes from the same persisted
  `includePrereleases` setting used by desktop Settings.

## Core

- Engine compatibility behavior lives in `cratebay-core`.
- CLI and Tauri command layers stay thin.
- Runtime startup is centralized through the engine/runtime modules.
- Image archive operations stream or handle files safely and return clear
  errors.

## Desktop

- Tauri command registrations match the functions still used by React.
- Frontend stores only keep state for current pages.
- Tests do not reference removed routes, stores, or components.
- Images page exposes import/export actions.
- Pods page exposes create/delete/inspect/add/remove workflows.
- Volumes page exposes list/create/inspect/delete workflows.
- Networks page exposes list/create/inspect/delete workflows.
- macOS release app bundles include valid bundled image resources.

## Storage

- SQLite migrations only contain tables that are still used.
- Settings keys match the desktop settings page and CLI behavior.
- Audit actions reflect container, image, pod, volume, network, runtime, and
  settings operations.

# Architecture

CrateBay is split into a desktop app, a CLI, shared Rust core logic, and a
built-in runtime.

The CLI plus built-in runtime must remain usable without launching the desktop
app. The GUI is a richer control plane over the same core operations.

```text
GUI (Tauri + React)      CLI (cratebay)
        \                 /
         \               /
          cratebay-core
              |
      CrateBay Engine API
              |
       Built-in runtime
```

## Crates

| Crate | Role |
|---|---|
| `cratebay-core` | CrateBay Engine operations, runtime coordination, storage, validation |
| `cratebay-cli` | Command parsing, table/json/yaml output |
| `cratebay-gui/src-tauri` | Desktop command wrappers and app lifecycle |
| `cratebay-gui/src` | React UI for dashboard, containers, images, pods, volumes, networks, settings |
| `cratebay-vz` | macOS virtualization runtime |
| `cratebay-guest-agent` | Guest-side CrateBay Engine compatibility bridge |

## Product Surface

- Images: list, search, pull, inspect, tag, export, import, delete.
- Containers: create, start, stop, delete, exec, logs, inspect, stats.
- Pods: managed network groups for related containers.
- Volumes: persistent Engine volume create, list, inspect, delete.
- Networks: managed Engine network create, list, inspect, delete.
- Runtime: status, provision, start, stop.
- Settings: theme, language, registry mirrors, runtime connectivity.

## Boundaries

- Core owns behavior.
- CLI owns argument parsing and output.
- Tauri commands wrap core behavior for the desktop app.
- React owns interaction state and rendering.
- Runtime modules own platform-specific VM details.

## Pod Model

Pods are CrateBay Engine managed CNI network groups labeled as CrateBay-managed
resources. Containers can be attached to or detached from a pod without changing
the container image or runtime.

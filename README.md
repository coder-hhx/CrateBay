# CrateBay

Open-source, cross-platform container and image management.

CrateBay is a desktop alternative to Docker Desktop and OrbStack. It focuses on local container workflows: browse and remove images, search registries, pull new images, create containers, inspect logs, manage volumes and networks, and run a built-in runtime across macOS, Linux, and Windows.

The CLI and built-in runtime form the minimum usable unit: the desktop app is a
visual control plane on top of the same core container operations.
External Docker-compatible endpoints are supported as explicit compatibility
overrides, but the built-in runtime is the default path.

## Why CrateBay?

- **Open source** — MIT licensed and free to use
- **Cross-platform** — macOS, Linux, and Windows
- **Built-in runtime** — starts a local VM-backed container engine when needed
- **Image-first workflow** — search, pull, inspect, tag, and remove images from one UI
- **Container management** — create, start, stop, inspect, exec, and view logs
- **Pod grouping** — manage CrateBay/CNI network-backed pods for related containers
- **Volumes and networks** — create, inspect, and remove persistent volumes and managed networks from GUI or CLI
- **Registry mirrors** — configure mirror hosts for faster Docker Hub pulls

## Quick Start

```bash
cd crates/cratebay-gui
pnpm install
pnpm tauri dev
```

CLI examples:

```bash
cratebay runtime start
cratebay image search alpine
cratebay image pull alpine:latest
cratebay run alpine:latest -- echo hello
cratebay run --network none --read-only --memory 512 alpine:latest -- sh -lc "pwd && id"
cratebay run --entrypoint /bin/sh alpine:latest -- -lc "echo from custom entrypoint"
cratebay --json run --max-output-bytes 1048576 alpine:latest -- sh -lc "echo bounded output"
cratebay --json run --no-propagate-exit-code alpine:latest -- sh -lc "exit 42"
cratebay image preload-bundled
cratebay image export --output alpine.tar alpine:latest
cratebay image import alpine.tar
cratebay pod create demo-pod
cratebay volume create demo-cache
cratebay network create demo-net
cratebay container create demo --image alpine:latest --entrypoint /bin/sh --command "sleep 3600" --pod demo-pod --publish 8080:80 --volume "$PWD:/workspace:ro"
cratebay image pack-container demo cratebay/demo:latest
cratebay image tag cratebay/demo:latest cratebay/demo:dev
cratebay container list --all
```

## Desktop App

The CrateBay desktop app provides:

- **Containers** — lifecycle actions, templates, logs, terminal access, and resource details
- **Images** — local image list, registry search, pull progress, inspect, tag, export, import, and delete
- **Pods** — network-based container grouping
- **Volumes** — persistent Engine volume lifecycle and inspection
- **Networks** — managed Engine network lifecycle and inspection
- **Runtime** — start/stop/restart the built-in engine and configure HTTP proxy settings
- **Settings** — language, theme, registry mirrors, and runtime connectivity

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  CrateBay                                            │
│                                                      │
│  ┌──────────────┐        ┌───────────────────────┐  │
│  │ GUI App      │        │ cratebay-cli           │  │
│  │ Tauri + React│        │ command line           │  │
│  └──────┬───────┘        └──────────┬────────────┘  │
│         └───────────────┬───────────┘               │
│                         │                           │
│              ┌──────────▼──────────┐                │
│              │   cratebay-core     │                │
│              │ Engine + storage    │                │
│              └──────────┬──────────┘                │
│                         │                           │
│              ┌──────────▼──────────┐                │
│              │  Built-in Runtime   │                │
│              │  macOS: VZ          │                │
│              │  Linux: KVM/QEMU    │                │
│              │  Windows: WSL2      │                │
│              └─────────────────────┘                │
└─────────────────────────────────────────────────────┘
```

**Tech stack**: Tauri v2 | React 19 | Rust | containerd | runc | CNI | bollard compatibility client | SQLite

## Compared To

| | CrateBay | Docker Desktop | OrbStack | Podman Desktop |
|---|---|---|---|---|
| Open source | MIT | No | No | Yes |
| Cross-platform | macOS/Win/Linux | macOS/Win/Linux | macOS only | macOS/Win/Linux |
| Built-in runtime | Yes | Yes | Yes | Yes |
| Image management | Yes | Yes | Yes | Yes |
| Container logs/exec | Yes | Yes | Yes | Yes |
| Cost | Free | Free / paid tiers | Free / paid tiers | Free |

## Status

v0.9.0 focuses on image management, pod grouping, volume and network lifecycle, container lifecycle, one-shot CLI runs, and the built-in runtime.

## License

[MIT](LICENSE)

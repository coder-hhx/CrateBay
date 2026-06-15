# Built-In Runtime Verification Matrix

This matrix verifies the current CrateBay product shape: CLI plus built-in
runtime as the minimum usable unit, with the desktop app as a visual control
plane over the same image, container, pod, volume, network, and runtime
operations.

## Preconditions

- No `--engine-host`, `--docker-host`, or `DOCKER_HOST` override unless
  explicitly testing compatibility mode.
- Runtime assets are present or can be built by the smoke script.
- On macOS, `cratebay-vz` can be signed with the Virtualization entitlement.
- On Linux, QEMU/KVM runtime helper assets are present or buildable.
- On Windows, WSL2 runtime assets are present or downloaded from CI artifacts.

## Required Automated Checks

| Area | Command | Expected Result |
|---|---|---|
| Product surface | `./scripts/product-surface-guard.sh` | No removed conversational or legacy execution product wording. |
| Tauri surface | `./scripts/verify-tauri-command-surface.sh` | Frontend invokes, backend handlers, and E2E mocks agree. |
| Rust core workspace | `cargo test --workspace --exclude cratebay-gui --exclude cratebay-vz -- --test-threads=1` | Core, CLI, guest agent, and Engine adapter tests pass. |
| GUI backend | `cargo clippy -p cratebay-gui --all-targets -- -D warnings` and `cargo test -p cratebay-gui -- --test-threads=1` | Tauri command wrappers and desktop runtime wiring pass lint and tests. |
| VZ runner | `cargo clippy -p cratebay-vz --all-targets -- -D warnings` and `cargo test -p cratebay-vz -- --test-threads=1` on macOS | macOS runner proxy and helper logic pass lint and tests against the real Virtualization.framework bridge path. |
| Frontend build | `pnpm run build` from `crates/cratebay-gui` | Production frontend bundle builds. |
| Frontend unit tests | `pnpm run test:unit` from `crates/cratebay-gui` | App, stores, containers, pods, volumes, networks, settings, and runtime health tests pass. |
| Desktop E2E | `pnpm exec playwright test` from `crates/cratebay-gui` | Images, containers, pods, volumes, networks, settings, and navigation flows pass. |
| CLI + runtime | `./scripts/runtime-smoke-cli-only.sh` | Isolated built-in runtime starts and runs CLI lifecycle checks. |
| Registry pull | `./scripts/runtime-smoke-local-registry.sh` | Built-in runtime starts a registry container and pulls back from it. |

## CLI Runtime Smoke Coverage

`runtime-smoke-cli-only.sh` verifies:

- structured command errors for embedded callers
- on-demand built-in runtime startup
- Engine compatibility endpoint readiness from the built-in runtime
- offline image import
- `cratebay run` with network isolation, read-only root, entrypoint override,
  bounded output, and timeout exit code `124`
- pod create, inspect, add, remove, and delete
- container create, start, exec, logs, stop, and delete
- image list, inspect, pack-container, tag, export, and import
- volume create, list, inspect, and remove
- network create, list, inspect, and remove
- cleanup of containers, pods, volumes, networks, images, runtime data, and test sockets

`runtime-smoke-local-registry.sh` extends this by running a tiny registry inside
the built-in runtime and pulling an image through `cratebay image pull` without
depending on Docker Hub.

## Manual Desktop Checks

| Page | Operation | Expected Result |
|---|---|---|
| Images | Load bundled images | Bundled images are imported or skipped with a clear result. |
| Images | Search Docker Hub | Results appear without starting the runtime solely for search. |
| Images | Pull image | Pull progress is shown and the image appears in the local list. |
| Images | Tag/export/import/delete | Local image operations update the list without stale rows. |
| Containers | Create from image | Form supports image, command, env, ports, volumes, pod, CPU, memory, user, and read-only root. |
| Containers | Start/stop/delete | Lifecycle buttons update state and errors stay visible. |
| Containers | Logs/terminal/details | Details panel opens without leaving the list. |
| Containers | Package image | Container filesystem commits to a local image tag. |
| Pods | Create/delete pod | Managed pod networks are created and removed. |
| Pods | Attach/detach container | Membership updates by full ID, short ID, or normalized name. |
| Volumes | Create/inspect/delete volume | Persistent Engine volumes update without stale rows. |
| Networks | Create/inspect/delete network | Managed Engine networks update without stale rows. |
| Settings | Runtime control | Start, stop, restart, status, and proxy settings operate on the built-in runtime. |

## Compatibility Mode

External Docker-compatible endpoints are compatibility overrides, not the
default product path. Test them only by passing `--engine-host`,
`--docker-host`, or setting `DOCKER_HOST`.

Examples:

```bash
cratebay --engine-host tcp://127.0.0.1:2375 system engine-status
cratebay --docker-host tcp://127.0.0.1:2375 system docker-status
DOCKER_HOST=unix:///path/to/docker.sock cratebay image list
```

Commands that can fall back without starting the runtime, such as
`cratebay image search --source auto`, must still surface explicit host
connection failures instead of silently using Docker Hub.

The default path must continue to ignore Docker Desktop, Colima, OrbStack,
Podman, and Docker context names unless an explicit endpoint is provided.

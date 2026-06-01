# Getting Started

CrateBay is a local container and image management app with a built-in runtime.
Use it as a lightweight desktop control plane or as a CLI for repeatable local
container workflows.

## Requirements

- macOS, Linux, or Windows
- Rust toolchain
- Node.js 20+
- pnpm

## Desktop App

```bash
cd crates/cratebay-gui
pnpm install
pnpm tauri dev
```

The app starts with the container workflow: images, containers, runtime status,
and settings.

## CLI

Build the CLI from the workspace root:

```bash
cargo build -p cratebay-cli
```

Common commands:

```bash
cratebay runtime status
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
cratebay container create demo --image alpine:latest --entrypoint /bin/sh --command "sleep 3600" --pod demo-pod --publish 8080:80 --volume "$PWD:/workspace:ro"
cratebay image pack-container demo cratebay/demo:latest
cratebay image tag cratebay/demo:latest cratebay/demo:dev
cratebay container list --all
```

## Runtime Notes

Container and image commands use the built-in runtime by default. If the runtime
is not already running, CrateBay provisions and starts it on demand.

For compatibility or diagnostics, pass `--docker-host` or set `DOCKER_HOST` to
target an explicit Docker-compatible endpoint.

## Verification

```bash
cargo check --workspace
cargo test --workspace --exclude cratebay-gui --exclude cratebay-vz -- --test-threads=1
cd crates/cratebay-gui
pnpm run build
pnpm run test:unit
```

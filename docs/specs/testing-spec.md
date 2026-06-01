# Testing Spec

## Rust

Run from the workspace root:

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace --exclude cratebay-gui --exclude cratebay-vz -- --test-threads=1
cargo test -p cratebay-gui -- --test-threads=1
```

Use unit tests for pure parsing and mapping logic. Keep Docker-dependent tests
ignored or guarded unless CI has a Docker-compatible endpoint available.

## Frontend

Run from `crates/cratebay-gui`:

```bash
pnpm run build
pnpm run lint
pnpm run test:unit
```

Tests should cover:

- navigation between active pages
- image list/search/pull state
- container lifecycle store actions
- settings persistence
- runtime health display

## CLI Smoke Checks

The release gate should include the built-in runtime smoke:

```bash
./scripts/runtime-smoke-cli-only.sh
./scripts/runtime-smoke-local-registry.sh
./scripts/build-bundle-images.sh
```

This starts an isolated built-in runtime, generates a tiny local Linux image,
loads it through `cratebay image import`, and verifies the CLI-only product
path: `cratebay run`, timeout exit code `124`, pod add/remove, container
exec/logs, image pack/tag/export/import, and volume lifecycle. The one-shot
run also verifies `--entrypoint` so CLI callers can override image defaults
when using CrateBay as an embedded execution tool, and verifies
`--max-output-bytes` so structured results stay bounded for upper-layer tools.
Bundle image preload smoke should fail explicitly when the expected bundled
archives are missing, while still returning structured per-image diagnostics.
The offline image path avoids Docker Hub flakes; set
`CRATEBAY_SMOKE_RUNTIME_IMAGE=image:tag` only when intentionally testing
registry pulls.
The bundle-image build step should generate and verify the offline archives
for Python, Node, Rust, and Ubuntu so the packaged app keeps its built-in
images available.

The one-shot smoke should also cover the caller-friendly structured run mode,
where `--json` plus `--no-propagate-exit-code` keeps the CLI process successful
after a completed run while still reporting `exitCode` and `timedOut` in the
payload.

The container exec path should use the same caller-friendly structured mode so
agent-style callers can read `exitCode`, `timedOut`, and truncation flags
without treating container failures as CLI infrastructure failures.

The exec path should also support a bounded timeout, bounded output, and
surface `124` on timeout, matching the one-shot run contract.

After the built-in runtime is available:

```bash
cratebay runtime status
cratebay image search alpine --limit 3
cratebay image pull alpine:latest
cratebay run alpine:latest -- echo hello
cratebay run --network none --read-only --memory 512 alpine:latest -- sh -lc "pwd && id"
cratebay run --entrypoint /bin/sh alpine:latest -- -lc "echo from custom entrypoint"
cratebay image export --output /tmp/alpine.tar alpine:latest
cratebay image import /tmp/alpine.tar
cratebay container create smoke --image alpine:latest --command "sleep 60"
cratebay container list --all
cratebay pod create smoke-pod
cratebay pod add smoke-pod smoke
cratebay pod inspect smoke-pod
cratebay pod delete smoke-pod --force
cratebay container delete smoke --force
```

## Release Gate

- No removed product surface appears in app code or docs.
- Desktop build and unit tests pass.
- Workspace Rust checks pass.
- Runtime asset scripts still produce expected files.
- Release artifact verification confirms macOS app bundles include valid
  bundled image resources.

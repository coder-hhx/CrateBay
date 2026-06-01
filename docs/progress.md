# Progress

Updated: 2026-05-31

## Current Direction

CrateBay is now a container and image management app. The product surface is
centered on images, containers, pods, settings, and the built-in runtime.

## Completed

- Removed the former conversational/product layer from the app surface.
- Reduced desktop navigation to the management views that remain relevant.
- Kept runtime management, Docker connectivity, storage, and system commands.
- Kept bundled runtime images and bundle image loading.
- Added bundle-image generation and verification for the offline archives.
- Added CLI image archive commands:
  - `cratebay image export --output image.tar IMAGE...`
  - `cratebay image import image.tar`
- Added CLI pod commands backed by Docker networks:
  - `cratebay pod list`
  - `cratebay pod create NAME`
  - `cratebay pod inspect NAME`
  - `cratebay pod add NAME CONTAINER`
  - `cratebay pod remove NAME CONTAINER`
  - `cratebay pod delete NAME --force`
- Added one-shot CLI run commands for embedded/local automation:
  - `cratebay run IMAGE -- COMMAND...`
  - `cratebay container run IMAGE -- COMMAND...`
  - captured stdout/stderr are bounded by `--max-output-bytes` by default so
    upper-layer tools can consume structured results safely
  - structured CLI mode now emits machine-readable command errors with
    `ok`, `kind`, and `error` fields
- Added desktop Pod management commands and page.
- Added desktop image import/export controls.

## In Progress

- Tighten desktop Pod and image archive UX with native file pickers when a dialog
  plugin is added.
- Continue pruning obsolete documentation and tests as the new product shape
  stabilizes.

## Verification Baseline

Run these before merging broad changes:

```bash
cargo fmt --check
./scripts/product-surface-guard.sh
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace --exclude cratebay-gui --exclude cratebay-vz -- --test-threads=1
cargo test -p cratebay-gui -- --test-threads=1
cd crates/cratebay-gui
pnpm run build
pnpm run lint
pnpm run test:unit
pnpm exec playwright test e2e/containers-list.spec.ts e2e/images-management.spec.ts e2e/pods-management.spec.ts --project=chromium
cd ../..
./scripts/runtime-smoke-cli-only.sh
```

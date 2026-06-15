# Development Workflow

## Local Setup

```bash
cargo check --workspace
cd crates/cratebay-gui
pnpm install
pnpm tauri dev
```

Use the workspace root for Rust commands and `crates/cratebay-gui` for frontend
commands.

## Change Flow

1. Read the local module before editing.
2. Keep changes scoped to the current product surface.
3. Prefer existing helpers in `cratebay-core`.
4. Add focused tests when behavior changes.
5. Run formatting and checks before handoff.

## Verification

Rust:

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace --exclude cratebay-gui --exclude cratebay-vz -- --test-threads=1
```

Desktop:

```bash
cd crates/cratebay-gui
pnpm run build
pnpm run lint
pnpm run test:unit
```

## Module Boundaries

- `cratebay-core`: CrateBay Engine operations, runtime coordination, storage, validation.
- `cratebay-cli`: command parsing and output formatting.
- `cratebay-gui/src-tauri`: Tauri command wrappers and desktop runtime wiring.
- `cratebay-gui/src`: React pages, stores, and components.
- `cratebay-vz`: macOS virtualization runtime.
- `cratebay-guest-agent`: guest-side CrateBay Engine compatibility bridge.

## Review Checklist

- No removed product surface appears in code, command help, tests, or docs.
- CLI output is useful in table mode and structured mode.
- Runtime-dependent paths handle unavailable Engine endpoints clearly.
- GUI text matches the actual pages and controls.

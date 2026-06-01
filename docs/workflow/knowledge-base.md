# Documentation Map

This directory keeps the project shape easy to recover after long gaps.

## Primary Files

- `README.md`: public overview and quick start.
- `docs/getting-started.md`: local development and first commands.
- `docs/ROADMAP.md`: current product direction.
- `docs/progress.md`: current implementation state.

## Specs

- `docs/specs/architecture.md`: system layout and boundaries.
- `docs/specs/frontend-spec.md`: desktop UI structure.
- `docs/specs/backend-spec.md`: Rust backend and command surface.
- `docs/specs/database-spec.md`: local storage schema.
- `docs/specs/runtime-spec.md`: built-in runtime strategy.
- `docs/specs/api-spec.md`: CLI and Tauri command contracts.
- `docs/specs/testing-spec.md`: verification strategy.

## References

- `docs/references/tech-decisions.md`: technical decisions.
- `docs/references/glossary.md`: shared vocabulary.

## Maintenance Rules

- Keep docs aligned with the current app surface.
- Remove stale pages when behavior is removed.
- Prefer concise specs over historical implementation notes.
- Update examples when CLI flags or command names change.

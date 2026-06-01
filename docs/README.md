# CrateBay Documentation Index

> Version: 1.2.0 | Updated: 2026-05-31

CrateBay documentation focuses on container and image management, the built-in runtime, storage, API contracts, and testing.

## Documentation Map

```
docs/
├── README.md
├── getting-started.md
├── ROADMAP.md
├── progress.md
├── spec-consistency-checklist.md
├── specs/
│   ├── architecture.md
│   ├── frontend-spec.md
│   ├── backend-spec.md
│   ├── database-spec.md
│   ├── runtime-spec.md
│   ├── api-spec.md
│   └── testing-spec.md
├── workflow/
│   ├── dev-workflow.md
│   └── knowledge-base.md
└── references/
    ├── tech-decisions.md
    └── glossary.md
```

## Core Specs

| # | Document | Purpose |
|---|---|---|
| 1 | [architecture.md](specs/architecture.md) | System overview and module boundaries |
| 2 | [frontend-spec.md](specs/frontend-spec.md) | React/Tauri UI structure and component contracts |
| 3 | [backend-spec.md](specs/backend-spec.md) | Rust backend and Tauri command design |
| 4 | [database-spec.md](specs/database-spec.md) | SQLite schema and migrations |
| 5 | [runtime-spec.md](specs/runtime-spec.md) | Built-in runtime lifecycle and platform strategy |
| 6 | [api-spec.md](specs/api-spec.md) | Tauri command surface and payload shapes |
| 7 | [testing-spec.md](specs/testing-spec.md) | Test strategy and verification matrix |

## Workflow Docs

| # | Document | Purpose |
|---|---|---|
| 1 | [dev-workflow.md](workflow/dev-workflow.md) | Day-to-day implementation flow |
| 2 | [knowledge-base.md](workflow/knowledge-base.md) | Project knowledge organization |

## References

| # | Document | Purpose |
|---|---|---|
| 1 | [tech-decisions.md](references/tech-decisions.md) | Technical decisions and rationale |
| 2 | [glossary.md](references/glossary.md) | Product and implementation terminology |

## Reading Order

1. `README.md`
2. `runtime-spec.md`
3. `architecture.md`
4. `backend-spec.md`
5. `frontend-spec.md`
6. `api-spec.md`
7. `database-spec.md`
8. `testing-spec.md`

## Notes

- Keep this index in sync when docs are added or removed.
- Update the version and date whenever the structure changes.

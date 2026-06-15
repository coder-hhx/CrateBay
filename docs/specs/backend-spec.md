# Backend Spec

The Rust backend is organized around thin command layers over `cratebay-core`.

## Core Modules

| Module | Role |
|---|---|
| `container.rs` | Container lifecycle, logs, exec, image operations |
| `pod.rs` | Managed network backed pod operations |
| `engine.rs` | Native CrateBay Engine API helpers and built-in runtime access |
| `docker.rs` | Engine compatibility client and explicit compatibility host helpers |
| `runtime/` | Built-in runtime lifecycle |
| `storage.rs` | SQLite settings and audit storage |
| `validation.rs` | Input validation helpers |

## Tauri Commands

Desktop commands should:

1. Resolve or start the native CrateBay Engine contract through app state.
2. Use native CrateBay Engine APIs for containers, images, pods, volumes,
   networks, exec, logs, stats, and terminal operations.
3. Keep the Docker-compatible endpoint as an explicit compatibility surface,
   not as the readiness gate for native management commands.
4. Call `cratebay-core`.
5. Return serializable data or `AppError`.

## CLI Commands

CLI commands should:

1. Parse arguments with `clap`.
2. Use the native CrateBay Engine API by default.
3. Use Engine-compatible endpoints only when `--engine-host`, legacy
   `--docker-host`, or `DOCKER_HOST` is explicitly set.
4. Call `cratebay-core`.
5. Print table output by default and structured output for automation.
6. Preserve structured error output for `--json` and `--format json|yaml` so
   callers can distinguish validation, runtime, compatibility, and command failures
   without parsing prose.
7. For one-shot runs and container exec, support a caller-friendly mode that
   keeps the CLI process successful after infrastructure success while
   reporting the executed process exit code, timeout state, and output
   truncation state in structured output.

## Error Handling

Use `AppError` for core errors. Prefer validation errors for bad user input,
runtime errors for timeouts, and compatibility errors for CrateBay Engine API
responses.
In structured CLI mode, errors are emitted on stderr as:

```json
{
  "ok": false,
  "kind": "validation",
  "error": "Validation error: bad input"
}
```

## Timeouts

Engine compatibility list/inspect operations should have short timeouts. Pull,
import, export, and runtime startup paths may take longer but should report
clear progress or errors.

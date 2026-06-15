# API Spec

This spec tracks the command contracts exposed by the desktop backend and CLI.

## Tauri Commands

### Containers

- `container_list(all?, filters?) -> ContainerInfo[]`
- `container_create(request) -> ContainerInfo`
- `container_start(id) -> void`
- `container_stop(id, timeout?) -> void`
- `container_delete(id, force?) -> void`
- `container_exec(id, cmd, workingDir?, timeout?, maxOutputBytes?) -> ExecResult`
- `container_exec_stream(id, cmd, channelId, workingDir?) -> void`
- `container_terminal_open(id, sessionId, cols?, rows?) -> void`
- `container_terminal_input(sessionId, data) -> void`
- `container_terminal_resize(sessionId, cols, rows) -> void`
- `container_terminal_close(sessionId) -> void`
- `container_logs(id, options?: { tail?, timestamps? }) -> LogEntry[]`
- `container_inspect(id) -> ContainerDetail`
- `container_stats(id) -> ContainerStats`

`container_create` accepts image/name plus optional command, environment,
working directory, CPU, memory, published ports, bind mounts, pod name, labels,
template id, and auto-start preference.

`container_terminal_open` emits `terminal:stream:{sessionId}` events with
`type: "Output" | "Done" | "Error"` payloads.

### Images

- `image_list() -> LocalImageInfo[]`
- `image_search(query, limit?) -> ImageSearchResult[]`
- `image_pull(image, mirrors?, channelId?) -> channelId`
- `image_inspect(id) -> ImageInspectInfo`
- `image_remove(id, force?) -> void`
- `image_tag(source, target) -> void`
- `image_pack_container(container, image) -> imageId`
- `image_export(images, output) -> bytesWritten`
- `image_import(input) -> string[]`
- `image_preload_bundled() -> BundleImageLoadResult[]`

### Pods

- `pod_list() -> PodInfo[]`
- `pod_create(name, driver?, internal?, enableIpv6?) -> PodInfo`
- `pod_inspect(name) -> PodInfo`
- `pod_delete(name, force?) -> void`
- `pod_add_container(name, container) -> void`
- `pod_remove_container(name, container, force?) -> void`

### Volumes

- `volume_list() -> VolumeInfo[]`
- `volume_create(name, driver?) -> VolumeInfo`
- `volume_inspect(name) -> VolumeInfo`
- `volume_delete(name, force?) -> void`

### Networks

- `network_list() -> NetworkInfo[]`
- `network_create(name, driver?, internal?, enableIpv6?) -> NetworkInfo`
- `network_inspect(id) -> NetworkInfo`
- `network_delete(id, force?) -> void`

### Storage And System

- `settings_get(key) -> string?`
- `settings_update(key, value) -> void`
- `system_info() -> SystemInfo`
- `engine_status() -> EngineEndpointStatus`
- `docker_status() -> DockerStatus` (compatibility alias)
- `runtime_status() -> RuntimeStatusInfo`
- `runtime_start() -> string`
- `runtime_provision() -> string`
- `runtime_stop() -> string`
- `runtime_restart() -> string`
- `runtime_diagnostics(pruneExitedContainers?) -> RuntimeDiagnosticsInfo`

`EngineEndpointStatus` uses `engineSource` as the primary source field and keeps
`source` as a compatibility alias. Built-in runtime responses should report
`engineSource: "builtin"` and the canonical Engine endpoint path.

`RuntimeStatusInfo` uses `engineResponsive`, `compatibilityResponsive`,
`compatibilityVersion`, and `engineSource` as primary readiness/source fields.
`dockerResponsive` and `dockerSource` remain compatibility aliases for older
clients. `engineSource` is filled only by the native Engine source or native
readiness reconciliation; compatibility-only `dockerSource` must not backfill
it. `resourceUsage` carries runtime CPU, memory, disk, and managed container
count and is surfaced by Dashboard, Settings diagnostics, and `cratebay runtime
status`.

`RuntimeDiagnosticsInfo` is the desktop aggregate snapshot used by Settings. It
includes `ok`, `runtime`, `engineContract`, `substrate`, `storageGc`,
`shimTasks`, and `generatedAtUnix`; each diagnostics section carries `ok`,
`value`, and `error` so the UI can keep partial offline diagnostics visible.

`runtime:health` events follow the same shape: `engine_responsive`,
`compatibility_responsive`, `compatibility_version`, and `engine_source` are
primary; `docker_responsive`, `docker_version`, and `docker_source` are legacy
aliases. Compatibility-only `docker_source` must not backfill `engine_source` in
backend responses or frontend readiness/source helpers. Frontend consumers must
accept native-only health events that omit legacy `docker_*` aliases and should
normalize snake_case/camelCase health fields before updating runtime state.

## CLI Commands

```text
cratebay runtime status|diagnostics|start|stop|restart|provision
cratebay runtime proxy show|set|clear
cratebay settings list|get|set|reset
cratebay update check [--include-prerelease|--stable] [--repository OWNER/REPO]
cratebay run IMAGE -- COMMAND...
cratebay image list|search|pull|inspect|tag|delete|remove|rmi|pack-container|export|import|preload-bundled
cratebay container list|create|run|start|stop|delete|remove|rm|exec|logs|stats|inspect|terminal-open|terminal-input|terminal-read|terminal-resize|terminal-close
cratebay pod list|create|inspect|add|remove|delete
cratebay volume create|list|inspect|remove|delete
cratebay network create|list|inspect|remove|delete
cratebay system info|engine-status
```

By default, CLI management commands that operate on CrateBay resources
(`run`, `container`, `pod`, `image` except registry search, `volume`,
`network`, and `engine` management subcommands) start or adopt the built-in
runtime and verify the native `/cratebay/engine` contract before issuing
native `/cratebay/*` calls.
Explicit `--engine-host`/`--docker-host` keeps those commands in compatibility
mode. Read-only diagnostics/status commands, `engine status`, `engine
substrate`, dry-run Engine maintenance commands, and `image search` do not
implicitly start the runtime.

`cratebay image pack-container CONTAINER IMAGE` commits a container filesystem
into a new local image tag.

`cratebay settings` manages the same persisted keys used by desktop Settings.
Only known desktop settings keys are accepted. `registryMirrors` accepts JSON,
comma-separated, or newline-separated input and is stored as a JSON array.

`cratebay runtime diagnostics` returns the CLI counterpart to the desktop
Settings diagnostics and Engine maintenance panels. It aggregates runtime
status, native Engine contract, substrate details, storage GC dry-run output,
and shim task inventory without starting the runtime. Structured output keeps
per-section `ok` and `error` fields so automation can distinguish offline
diagnostics from command failure. If lifecycle state is stale but the native
Engine contract is reachable, runtime status is reconciled to ready for
diagnostic output. The nested `runtime.resourceUsage` object uses the same
shape as desktop `runtime_status`.

`cratebay update check` mirrors the desktop update check selection rules for
GitHub Releases with a `latest.json` Tauri updater manifest. It reports
current/latest versions, channel, release metadata, manifest URL, and
availability in table, JSON, or YAML output. By default it reads the same
persisted `includePrereleases` setting as desktop Settings; `--include-prerelease`
and `--stable` override that value for a single check. Desktop remains
responsible for installing signed updater artifacts.

`cratebay image tag SOURCE TARGET` adds a new `repo:tag` reference to a local
image.

`cratebay container remove|rm`, `cratebay image remove|rmi`,
`cratebay volume delete`, and `cratebay network delete` are lifecycle aliases
that match common CLI and desktop management verbs without breaking existing
automation.

`cratebay image preload-bundled [--dir DIR]` loads CrateBay's bundled container
image archives into the built-in runtime, making the same images available to
CLI-only workflows. The CLI returns a non-zero exit code if any expected
archive fails to load while still printing per-image results.

`cratebay run` is suitable for automation: it streams captured stdout/stderr,
returns the container exit code, returns `124` on timeout, and supports
`--entrypoint`, `--network bridge|none|host|NETWORK`, `--user UID[:GID]`,
`--read-only`, resource limits, env vars, published ports, bind mounts, and structured
`--format json|yaml` results (`--json` is a shortcut for `--format json`). Captured
stdout/stderr are bounded by `--max-output-bytes` per stream by default; pass
`0` to disable truncation. Pass `--no-propagate-exit-code` when an embedded
caller should receive a successful CLI process after infrastructure success and
read the container result from `exitCode`/`timedOut` in the structured payload.
`cratebay container exec` and `cratebay engine exec` support `--timeout SECS`,
`--max-output-bytes`, return the same
`exitCode`/`timedOut`/`stdoutTruncated`/`stderrTruncated` fields in structured
output, and `--no-propagate-exit-code` makes them caller-friendly for embedded
tooling that wants the CLI process to stay successful after infrastructure
success. Both exec paths default to bounded output and use
`--max-output-bytes 0` to disable truncation.
When structured output is selected (`--json` or `--format json|yaml`),
command-level failures also return a structured error on stderr:

```json
{
  "ok": false,
  "kind": "runtime",
  "error": "Runtime error: Unsupported Engine host format: bad-host"
}
```

Known `kind` values include `command`, `engineCompatibility`, `database`, `validation`,
`notFound`, `runtime`, `io`, `serialization`, and `permissionDenied`.

`cratebay container create` supports `--entrypoint`, `--command`,
`--working-dir`, repeated `--env`, repeated
`--publish/-p HOST:CONTAINER[/tcp|udp|sctp]`, repeated
`--volume/-v HOST:CONTAINER[:ro|rw]`, `--pod`, `--network bridge|none|host|NETWORK`,
`--user UID[:GID]`, `--read-only`, `--cpu`, `--memory`, and `--no-start`.

## Output

The CLI defaults to table output. Commands returning structured data should also
support:

```bash
--format json
--json
--format yaml
```

## Events

Image pull emits progress events by channel id:

```text
image:pull:{channelId}
```

Future long-running import/export UI can use the same channel pattern.

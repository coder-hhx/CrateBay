# API Spec

This spec tracks the command contracts exposed by the desktop backend and CLI.

## Tauri Commands

### Containers

- `container_list(all?, filters?) -> ContainerInfo[]`
- `container_create(request) -> ContainerInfo`
- `container_start(id) -> void`
- `container_stop(id, timeout?) -> void`
- `container_delete(id, force?) -> void`
- `container_exec(id, command, workingDir?) -> ExecResult`
- `container_exec_stream(id, command, workingDir?, eventId?) -> void`
- `container_logs(id, options?) -> LogEntry[]`
- `container_inspect(id) -> ContainerDetail`
- `container_stats(id) -> ContainerStats`

`container_create` accepts image/name plus optional command, environment,
working directory, CPU, memory, published ports, bind mounts, pod name, labels,
template id, and auto-start preference.

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
- `pod_create(name) -> PodInfo`
- `pod_inspect(name) -> PodInfo`
- `pod_delete(name, force?) -> void`
- `pod_add_container(name, container) -> void`
- `pod_remove_container(name, container, force?) -> void`

### Storage And System

- `settings_get(key) -> string?`
- `settings_update(key, value) -> void`
- `system_info() -> SystemInfo`
- `docker_status() -> DockerStatus`
- `runtime_status() -> RuntimeStatusInfo`
- `runtime_start() -> string`
- `runtime_stop() -> string`

## CLI Commands

```text
cratebay runtime status|start|stop|provision
cratebay run IMAGE -- COMMAND...
cratebay image list|search|pull|inspect|tag|delete|pack-container|export|import|preload-bundled
cratebay container list|create|run|start|stop|delete|exec|logs|inspect
cratebay pod list|create|inspect|add|remove|delete
cratebay volume create|list|inspect|remove
cratebay system info|docker-status
```

`cratebay image pack-container CONTAINER IMAGE` commits a container filesystem
into a new local image tag.

`cratebay image tag SOURCE TARGET` adds a new `repo:tag` reference to a local
image.

`cratebay image preload-bundled [--dir DIR]` loads CrateBay's bundled container
image archives into the built-in runtime, making the same images available to
CLI-only workflows. The CLI returns a non-zero exit code if any expected
archive fails to load while still printing per-image results.

`cratebay run` is suitable for automation: it streams captured stdout/stderr,
returns the container exit code, returns `124` on timeout, and supports
`--entrypoint`, `--network bridge|none|host`, `--user UID[:GID]`,
`--read-only`, resource limits, env vars, bind mounts, and structured
`--format json|yaml` results (`--json` is a shortcut for `--format json`). Captured
stdout/stderr are bounded by `--max-output-bytes` per stream by default; pass
`0` to disable truncation. Pass `--no-propagate-exit-code` when an embedded
caller should receive a successful CLI process after infrastructure success and
read the container result from `exitCode`/`timedOut` in the structured payload.
`cratebay container exec` supports `--timeout SECS`, `--max-output-bytes`,
returns the same `exitCode`/`timedOut`/`stdoutTruncated`/`stderrTruncated`
fields in structured output, and `--no-propagate-exit-code` makes it
caller-friendly for embedded tooling that wants the CLI process to stay
successful.
When structured output is selected (`--json` or `--format json|yaml`),
command-level failures also return a structured error on stderr:

```json
{
  "ok": false,
  "kind": "runtime",
  "error": "Runtime error: Unsupported Docker host format: bad-host"
}
```

Known `kind` values include `command`, `docker`, `database`, `validation`,
`notFound`, `runtime`, `io`, `serialization`, and `permissionDenied`.

`cratebay container create` supports `--entrypoint`, `--command`,
`--working-dir`, repeated `--env`, repeated
`--publish/-p HOST:CONTAINER[/tcp|udp]`, repeated
`--volume/-v HOST:CONTAINER[:ro|rw]`, `--pod`, `--network bridge|none|host`,
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

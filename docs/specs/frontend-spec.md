# Frontend Spec

The desktop app is a compact management tool, not a landing page. The first
screen should expose the working UI.

## Navigation

- Dashboard
- Containers
- Images
- Pods
- Volumes
- Networks
- Settings

Runtime status can appear in the top bar or Settings. Pods, Volumes, and
Networks are first-class resource pages for the same CrateBay Engine backing
store exposed by the CLI.

## Pages

### Images

- Local image table with tags, size, and actions.
- Registry search and pull progress.
- Inspect, tag, delete, export, and import actions.

### Containers

- Running/stopped container list.
- Pods tab for managed pod groups and container membership.
- Create form with image, command, pod, port mappings, bind mounts, CPU,
  memory, and working directory fields.
- Start, stop, delete, package-as-image, logs, stats, inspect, exec, and
  terminal actions.
- Detail panel should expose recent logs with tail/timestamp controls, resource
  monitoring, structured exec, and an interactive terminal without leaving the
  container list.

### Pods

- Managed pod list with container membership.
- Create and delete pods.
- Attach existing containers to a pod.
- Detach containers from a pod without deleting the container.

### Volumes

- Persistent Engine volume list with driver, scope, and mountpoint.
- Create, inspect, and delete volumes.

### Networks

- Managed Engine network list with driver, scope, flags, and container count.
- Create, inspect, and delete networks.

### Settings

- Theme and language.
- Registry mirror list.
- Runtime status and runtime actions.
- CrateBay Engine endpoint diagnostics.
- Runtime resource diagnostics showing CPU, memory, disk, and runtime-reported
  managed container count from the same payload used by Dashboard and CLI.

### Dashboard

- Management counters for containers, images, pods, volumes, and networks.
- Runtime monitoring for CPU, memory, and disk usage.
- Engine details including backend runtime, OCI runtime, network stack, native
  API, endpoint, uptime, and runtime-reported managed container count.

## State

- `appStore`: navigation, theme, runtime summary.
- Runtime summary state treats native Engine readiness/source and compatibility
  API reachability/source as separate signals; legacy `docker*` source fields
  must not imply built-in runtime readiness.
- Runtime health event handling normalizes snake_case and camelCase payloads so
  backend serialization details do not change the visible Engine status.
- `containerStore`: containers, images for create forms, lifecycle actions.
- `settingsStore`: persisted preferences and mirror hosts.
- Pull progress state is isolated from generic app state.

## Design Notes

- Keep operational pages dense and scannable.
- Avoid marketing sections inside the app shell.
- Use icon buttons for repeated row actions.
- Keep cards for repeated items or dialogs, not page sections.

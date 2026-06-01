# Frontend Spec

The desktop app is a compact management tool, not a landing page. The first
screen should expose the working UI.

## Navigation

- Containers
- Images
- Settings

Pods are presented as a secondary tab under Containers.

Runtime status can appear in the top bar or Settings. Pod UI can be introduced
as a compact network grouping page for creating pods and attaching containers.

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
- Start, stop, delete, package-as-image, logs, inspect, and terminal actions.
- Detail panel should expose recent logs and an exec terminal without leaving
  the container list.

### Pods

- Managed pod list with container membership.
- Create and delete pods.
- Attach existing containers to a pod.
- Detach containers from a pod without deleting the container.

### Settings

- Theme and language.
- Registry mirror list.
- Runtime status and runtime actions.
- Docker connectivity diagnostics.

## State

- `appStore`: navigation, theme, runtime summary.
- `containerStore`: containers, images for create forms, lifecycle actions.
- `settingsStore`: persisted preferences and mirror hosts.
- Pull progress state is isolated from generic app state.

## Design Notes

- Keep operational pages dense and scannable.
- Avoid marketing sections inside the app shell.
- Use icon buttons for repeated row actions.
- Keep cards for repeated items or dialogs, not page sections.

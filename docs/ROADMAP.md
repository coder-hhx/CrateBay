# Roadmap

CrateBay is moving toward a focused OrbStack-style container manager: a clean
desktop app, a scriptable CLI, and a built-in runtime that works without Docker
Desktop.

## Near Term

- Finish the image workflow: list, search, pull, inspect, tag, export, import,
  and remove.
- Keep container lifecycle actions reliable: create, start, stop, delete, exec,
  logs, inspect, and stats.
- Keep pod grouping reliable through Docker network backed pods.
- Keep the CLI and runtime usable without the desktop app.
- Keep bundled development images loadable from the app bundle.

## Desktop

- Keep first-level navigation to Containers, Images, and Settings.
- Keep Pods as a secondary tab under Containers.
- Keep polished import/export and pack-container controls in the Images view.
- Keep Pods visible as a first-class grouping view inside Containers, not a hidden filter.
- Keep runtime health visible without making runtime management the whole app.

## Runtime

- Built-in runtime remains the primary path.
- External Docker-compatible endpoints remain explicit compatibility overrides.
- Runtime provisioning must remain deterministic and easy to verify in CI.

## Packaging

- Ship built-in runtime assets for supported platforms.
- Keep bundle image generation reproducible through scripts.
- Preserve a single workspace build path for desktop, CLI, core, runtime, and
  guest helper crates.

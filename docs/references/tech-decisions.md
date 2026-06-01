# Technical Decisions

## Built-In Runtime First

CrateBay owns a built-in runtime path for macOS, Linux, and Windows. External
Docker-compatible endpoints are explicit compatibility overrides; runtime fixes
and packaging work should prioritize the built-in path.

The CLI plus built-in runtime is the minimum supported product shape. Desktop
features should not be required for basic image, container, pod, or one-shot run
workflows.

## Docker API Through Bollard

Container, image, volume, network, and pod operations use `bollard` from
`cratebay-core`. GUI and CLI layers should call core helpers instead of reaching
directly into Docker APIs.

## Pods As Managed Networks

Docker does not provide a native pod object. CrateBay models pods as managed
attachable bridge networks with labels:

- `com.cratebay.managed=true`
- `com.cratebay.pod=true`

This gives the app a grouping primitive while staying compatible with Docker.

## SQLite For Local State

SQLite remains the local persistence layer for settings, audit events, and
runtime metadata. Data lives under the CrateBay data directory and should remain
portable across app upgrades.

## Tauri + React Desktop

Tauri keeps system access in Rust while React owns the UI. Tauri commands should
be narrow wrappers over core behavior, and React stores should represent only the
active desktop pages.

## Image Archives

Image export/import follows Docker archive semantics. CLI commands mirror common
Docker habits:

- `cratebay image export --output bundle.tar IMAGE...`
- `cratebay image import bundle.tar`

Core functions should handle file errors clearly and preserve archive bytes.

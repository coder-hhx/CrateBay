# Changelog

## 0.9.0

CrateBay is now focused on local container, image, pod, and runtime management.

### Changed

- Removed the former conversational product surface from the app direction.
- Reduced desktop navigation to Images, Containers, Pods, and Settings.
- Updated documentation to match the container management product.
- Kept the built-in runtime as the primary engine path.

### Added

- Image archive commands:
  - `cratebay image export --output archive.tar IMAGE...`
  - `cratebay image import archive.tar`
- Pod commands backed by managed Docker networks:
  - `cratebay pod list`
  - `cratebay pod create NAME`
  - `cratebay pod inspect NAME`
  - `cratebay pod add NAME CONTAINER`
  - `cratebay pod remove NAME CONTAINER`
  - `cratebay pod delete NAME --force`
- One-shot container execution:
  - `cratebay run IMAGE -- COMMAND...`
  - `cratebay container run IMAGE -- COMMAND...`
- Tauri backend commands for image export/import.

### Kept

- Container lifecycle commands.
- Image list/search/pull/inspect/tag/delete.
- Volume commands.
- Runtime status/start/stop/provision.
- Built-in runtime assets and bundle image loading.

# Glossary

## CrateBay

An open-source desktop and CLI tool for local container, image, pod, and runtime
management.

## Built-In Runtime

The VM-backed container runtime managed by CrateBay. Platform implementations
live under `cratebay-core/src/runtime` and `cratebay-vz`.

## Image

A Docker-compatible image reference, such as `alpine:latest` or
`ghcr.io/example/app:1.0`.

## Image Archive

A tar archive produced by Docker-compatible save/export operations and loaded by
Docker-compatible import/load operations.

## Container

A running or stopped process environment created from an image and managed by a
Docker-compatible engine.

## Pod

A CrateBay-managed container group backed by an attachable Docker bridge
network.

## Bundle Image

A pre-built image archive shipped with the desktop app and loaded into the
runtime when needed.

## Tauri Command

A Rust function exposed to the React desktop app through Tauri IPC.

## Guest Helper

The small binary installed inside the runtime guest to bridge Docker socket
access back to the host-side app.

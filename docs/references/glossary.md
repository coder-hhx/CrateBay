# Glossary

## CrateBay

An open-source desktop and CLI tool for local container, image, pod, and runtime
management.

## Built-In Runtime

The VM-backed container runtime managed by CrateBay. Platform implementations
live under `cratebay-core/src/runtime` and `cratebay-vz`.

## Image

A container image reference, such as `alpine:latest` or
`ghcr.io/example/app:1.0`.

## Image Archive

A tar archive produced by OCI/Docker-compatible save/export operations and
loaded by OCI/Docker-compatible import/load operations.

## Container

A running or stopped process environment created from an image and managed by
CrateBay Engine.

## Pod

A CrateBay-managed container group backed by an attachable Engine network.

## Bundle Image

A pre-built image archive shipped with the desktop app and loaded into the
runtime when needed.

## Tauri Command

A Rust function exposed to the React desktop app through Tauri IPC.

## Guest Helper

The small binary installed inside the runtime guest to bridge the CrateBay
Engine compatibility endpoint back to the host-side app.

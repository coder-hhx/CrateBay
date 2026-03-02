# CargoBay Roadmap

> **English** · [中文](../README.zh.md)

## v0.1.0 — Foundation (Done)

- ✅ Docker container management (list, start, stop, remove, run)
- ✅ Image search (Docker Hub, Quay), tag listing, import/push
- ✅ Package container as image (docker commit)
- ✅ GUI Dashboard with container overview
- ✅ CLI with Docker/VM/Image/Mount subcommands
- ✅ Dark/Light theme + i18n (EN/ZH)
- ✅ Docker socket auto-detection (Colima, OrbStack, Docker Desktop)

## v0.2.0 — VM & Networking (Done)

- ✅ Virtualization.framework FFI (Swift bridge) for real VM start/stop
- ✅ OS image download and management
- ✅ VM console (serial output)
- ✅ VM port forwarding (TCP proxy)
- ✅ VirtioFS mount management (UI + daemon)
- ✅ VM/container resource monitoring (CPU / memory / disk / network)

## v0.3.0 — Developer Experience (Done)

- ✅ Container log streaming (real-time follow)
- ✅ Container exec / terminal integration
- ✅ Container environment variable viewer
- ✅ Local image management (list, remove, tag, inspect)
- ✅ Docker volume management (list, create, inspect, remove)

## v0.4.0 — Kubernetes (Done)

- ✅ K3s integration (on-demand download, install, start, stop, uninstall)
- ✅ Kubernetes dashboard (pods, services, deployments, namespace selector, pod logs)
- ✅ Auto-update checker (GitHub releases)
- ✅ Official website (GitHub Pages)

## v1.0.0 — Production Ready (In Progress)

- ⬜ Real VM execution on macOS (Virtualization.framework end-to-end)
- ⬜ ACPI graceful shutdown
- ⬜ Real VirtioFS mount implementation
- ⬜ Linux (KVM) VM backend
- ⬜ Windows (Hyper-V) VM backend
- ⬜ CI/CD pipeline (GitHub Actions, cross-platform builds)
- ⬜ Comprehensive test suite
- ⬜ Shell completion (bash, zsh, fish)
- ⬜ Plugin system
- ⬜ Security audit
- ⬜ Performance optimization (<20MB install, <200MB idle RAM, <3s startup)

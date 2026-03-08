# Release Notes — 2026-03-08

## Product direction

This update refocuses CrateBay around an AI-first local workflow.

- Primary workflow: local models, managed agent sandboxes, MCP servers, provider profiles, and CLI bridges.
- Secondary substrate: containers, images, and volumes remain available behind the AI workflow.
- Post-v1 tracks: VMs and Kubernetes stay visible as future expansion areas, but are no longer part of the default v1 path.

## What changed

### Dashboard

- Reworked the desktop dashboard into an AI-first control plane.
- Added primary cards for Sandboxes, Models, MCP, and Provider Profiles.
- Added runtime overview cards for CPU, memory, and GPU occupancy.
- Added active AI workload visibility for running sandboxes.
- Added deep links from dashboard cards into the relevant AI Hub and Settings tabs.

### AI Hub

- Expanded local model management around Ollama-first workflows.
- Added GPU telemetry for local model runtimes.
- Added sandbox runtime usage visibility during inspect flows.
- Added per-sandbox GPU attribution when NVIDIA tooling is available on Linux.
- Kept MCP lifecycle, logs, export, and registry management in the same control plane.

### Sandboxes

- Continued the managed sandbox path as the core local runtime primitive for agents.
- Added runtime usage reporting for CPU, memory, and GPU-related workload attribution.
- Preserved create / start / stop / inspect / exec / cleanup flows.

### Provider canaries

- Added controlled provider canary automation and dedicated smoke scripts.
- Added opt-in provider and Ollama daemon smoke coverage for CI / dedicated runners.
- Avoided persisting test-only secrets into the desktop app keychain.

## Validation status

Validated in this workspace:

- `cargo test -p cratebay-gui --no-run`
- targeted Vitest coverage for `App`, `Dashboard`, `AiHub`, and `Settings`
- `git diff --check`
- `node --check website/script.js`

## Scope note

This release note reflects the current v1-prioritized product scope, not the earlier broader scope that included real VM and K3s runtime runners in the default milestone.

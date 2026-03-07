# Validation Matrix

> This document answers one question: how do we automatically prove that core CrateBay workflows are usable, correct, and regression-safe before v1.0?

## Coverage Rules

A feature is not considered release-ready until it is covered by as many of the following layers as apply:

- **Contract / unit** — parameter validation, formatting, command routing, safety policy.
- **Browser E2E** — full GUI flow in the built frontend with deterministic Tauri bridge mocks.
- **Desktop smoke** — real Tauri desktop shell launched through `tauri-driver`.
- **Real dependency integration** — Docker / VM / provider / MCP runtime state validated against real dependencies or protocol-compatible runtime canaries.
- **Release gate** — CI and release scripts fail if the required layers regress.

## Matrix

| Area | Key flows | Contract / unit | Browser E2E | Desktop smoke | Real dependency integration | Current status |
|---|---|---:|---:|---:|---:|---|
| App shell | Top-level navigation, modal shell | ✅ | ✅ | ✅ | n/a | Browser E2E + Linux desktop smoke are both gated in CI |
| Containers | Create / list / env / lifecycle | ✅ | ✅ | ⏳ | ✅ | Real Docker CLI smoke runs in Linux CI via `scripts/docker-runtime-smoke.sh` |
| Images | Search / run / pack / inspect | ✅ | ✅ | ⏳ | ✅ | Real registry + Docker image smoke runs in Linux CI |
| Volumes | Create / inspect / remove | ✅ | ✅ | ⏳ | ✅ | Real Docker volume lifecycle is asserted in Linux CI |
| VMs | Create / port forwarding / login command / console | ✅ | ✅ | ⏳ | ⏳ | GUI/browser coverage is in place; real hypervisor runners still needed |
| Kubernetes | Pod list / log viewing | ✅ | ✅ | ⏳ | ⏳ | Browser-covered; real K3s runner still pending |
| AI Hub Models | Status / list / pull / delete / storage | ✅ | ✅ | ⏳ | ⏳ | Runtime canary now exercises the real HTTP + local CLI path with a protocol-compatible Ollama stub; real Ollama daemon CI is still pending |
| AI Hub Sandboxes | Create / execute / restart / delete / audit | ✅ | ✅ | ⏳ | ✅ | Real Docker-backed sandbox smoke now runs in CI; sandbox creation also auto-pulls missing images |
| MCP | Registry / start-stop / export / logs | ✅ | ✅ | ⏳ | ✅ | Real local process lifecycle smoke now runs in CI via backend tests |
| Assistant / Skills | Plan / run / schema validation / confirmation denial | ✅ | ✅ | ⏳ | ⏳ | Contract + browser coverage is strong; controlled provider canaries still pending |
| Security guardrails | Schema validation / destructive confirmation | ✅ | ✅ | ⏳ | n/a | Covered |

## What Is Automated Today

- `crates/cratebay-gui/e2e/` covers the major GUI flows end-to-end in a deterministic environment.
- `crates/cratebay-gui/src-tauri/tests/desktop_smoke.rs` plus `scripts/run-desktop-e2e-linux.sh` cover the real Tauri shell on Linux CI.
- `scripts/docker-runtime-smoke.sh` validates the real Docker daemon through CrateBay CLI flows: image search, container run/start/stop/remove, env inspection, image pack, and volume lifecycle.
- `scripts/ai-runtime-smoke.sh` validates AI Hub runtime paths: Ollama-compatible canary, MCP process lifecycle, and Docker-backed sandbox lifecycle.
- `scripts/ci-local.sh` and `scripts/release-readiness.sh` now include these runtime smokes as release gates.

## What Still Needs To Reach v1.0 Confidence

1. **Real VM backend runners**
   - Linux KVM self-hosted runner
   - macOS Virtualization.framework self-hosted runner
   - Windows Hyper-V self-hosted runner
2. **Real Kubernetes runner**
   - Linux CI or self-hosted environment with a disposable K3s cluster
3. **Controlled provider canaries**
   - OpenAI / Claude / Codex bridge smoke against locked-down test credentials
   - Real Ollama daemon smoke on a dedicated Linux runner
4. **Desktop deepening**
   - Extend desktop-shell smoke from navigation to runtime actions on Linux CI

## Commands

```bash
# Browser E2E
cd crates/cratebay-gui
npm run test:e2e:install
npx playwright test

# Real Docker CLI smoke
./scripts/docker-runtime-smoke.sh

# AI Hub runtime smoke
./scripts/ai-runtime-smoke.sh

# Linux-only desktop shell smoke
./scripts/run-desktop-e2e-linux.sh

# Full local gate
./scripts/ci-local.sh
```

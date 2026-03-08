# Validation Matrix

> This document answers one question: how do we automatically prove that core CrateBay workflows are usable, correct, and regression-safe before v1.0?

## Coverage Rules

A feature is not considered release-ready until it is covered by as many of the following layers as apply:

- **Contract / unit** — parameter validation, formatting, command routing, safety policy.
- **Browser E2E** — full GUI flow in the built frontend with deterministic Tauri bridge mocks.
- **Desktop smoke** — real Tauri desktop shell launched through `tauri-driver`.
- **Real dependency integration** — Docker / VM / provider / MCP runtime state validated against real dependencies or protocol-compatible runtime canaries.
- **Release gate** — CI and release scripts fail if the required layers regress.

## Public v1 Product Scope

- **Primary v1 surfaces** — AI Hub Sandboxes, AI Hub Models, MCP, and provider / CLI bridges.
- **Supporting runtime** — Containers, images, and volumes remain in scope as the enabling runtime for those AI workflows.
- **Experimental / post-v1** — VMs and Kubernetes remain visible in the product and matrix, but they are not release-blocking for the AI-first v1 until dedicated runtime runners exist.

## Matrix

| Area | Key flows | Contract / unit | Browser E2E | Desktop smoke | Real dependency integration | Current status |
|---|---|---:|---:|---:|---:|---|
| App shell | Top-level navigation, modal shell | ✅ | ✅ | ✅ | n/a | Browser E2E + Linux desktop smoke are both gated in CI |
| Containers | Create / list / env / lifecycle | ✅ | ✅ | ✅ | ✅ | Real Docker CLI smoke runs in Linux CI, and Linux desktop smoke now exercises create → env → login command → stop/start → remove through the real Tauri shell |
| Images | Search / run / pack / inspect | ✅ | ✅ | ⏳ | ✅ | Real registry + Docker image smoke runs in Linux CI |
| Volumes | Create / inspect / remove | ✅ | ✅ | ⏳ | ✅ | Real Docker volume lifecycle is asserted in Linux CI |
| VMs | Create / port forwarding / login command / console | ✅ | ✅ | ⏳ | ⏳ | Browser-covered today, but explicitly treated as an experimental post-v1 surface until dedicated hypervisor runners exist |
| Kubernetes | Pod list / log viewing | ✅ | ✅ | ⏳ | ⏳ | Browser-covered today, but kept on the post-v1 track until a real K3s runner is wired in |
| AI Hub Models | Status / list / pull / delete / storage | ✅ | ✅ | ⏳ | ⏳ | Runtime canary now exercises the real HTTP + local CLI path with a protocol-compatible Ollama stub; real Ollama daemon CI is still pending |
| AI Hub Sandboxes | Create / execute / restart / delete / audit | ✅ | ✅ | ⏳ | ✅ | Real Docker-backed sandbox smoke now runs in CI; sandbox creation also auto-pulls missing images |
| MCP | Registry / start-stop / export / logs | ✅ | ✅ | ✅ | ✅ | Real local process lifecycle smoke runs in CI via backend tests, and Linux desktop smoke now covers create → save → start → logs → stop in the real shell |
| Assistant / Skills | Plan / run / schema validation / confirmation denial | ✅ | ✅ | ⏳ | ⏳ | Contract + browser coverage is strong; the v1 release proof point is the provider / CLI bridge canary path rather than a broader agent platform claim |
| Security guardrails | Schema validation / destructive confirmation | ✅ | ✅ | ⏳ | n/a | Covered |

## What Is Automated Today

- `crates/cratebay-gui/e2e/` covers the major GUI flows end-to-end in a deterministic environment.
- `crates/cratebay-gui/src-tauri/tests/desktop_smoke.rs` plus `scripts/run-desktop-e2e-linux.sh` cover the real Tauri shell on Linux CI.
- `scripts/docker-runtime-smoke.sh` validates the real Docker daemon through CrateBay CLI flows: image search, container run/start/stop/remove, env inspection, image pack, and volume lifecycle.
- `scripts/ai-runtime-smoke.sh` validates AI Hub runtime paths: Ollama-compatible canary, MCP process lifecycle, and Docker-backed sandbox lifecycle.
- `scripts/ci-local.sh` and `scripts/release-readiness.sh` now include these runtime smokes as release gates.
- `scripts/provider-canary-smoke.sh`, `scripts/ollama-daemon-smoke.sh`, and `.github/workflows/provider-canary.yml` now wire controlled provider probes onto dedicated runners without persisting secrets into the app keychain.

## What Still Needs To Reach v1.0 Confidence

1. **Controlled provider canaries**
   - OpenAI / Anthropic `ai_test_connection` smoke against locked-down test credentials
   - Codex / Claude bridge smoke against explicit allowlisted binaries and prompts
   - Real Ollama daemon smoke on a dedicated Linux runner
2. **Keep the AI-first runtime chain green together**
   - Linux desktop smoke for real shell interactions
   - Docker runtime smoke for the supporting container engine
   - AI runtime smoke for local model, sandbox, and MCP process flows

## Public v1.0 Closure Plan

### P0 — Finish the provider evidence chain

- **Completed already**: Linux desktop smoke now reaches real runtime actions in CI for container lifecycle and MCP lifecycle flows through the actual Tauri shell.
- **Scope**: add minimal real-provider probes for OpenAI / Anthropic profiles and Codex / Claude CLI bridge paths, plus a real Ollama daemon probe on a dedicated Linux runner.
- **Safety rules**:
  - Use locked-down test credentials and canary-only profiles.
  - Use a minimal prompt such as `Reply with PONG.` and short timeouts.
  - Persist only request IDs, latency, and pass/fail summaries; never secrets or full model output.
- **Acceptance criteria**:
  - `ai_test_connection` succeeds against each supported canary profile.
  - Codex / Claude bridge presets can execute a read-only allowlisted smoke command.
  - A real Ollama daemon smoke validates status / list / pull / delete against a tiny or pre-seeded model.
- **Suggested artifacts**:
  - `scripts/provider-canary-smoke.sh`
  - `scripts/ollama-daemon-smoke.sh`
  - `.github/workflows/provider-canary.yml`

## Post-v1 Expansion Tracks

### Kubernetes

- **Scope**: provision a disposable Linux K3s environment on a dedicated CI or self-hosted runner and validate CrateBay against the real cluster.
- **Acceptance criteria**:
  - `cratebay k3s status` reports `installed=true`, `running=true`, and `node_count>=1`.
  - CrateBay can list real pods and fetch real pod logs from that cluster.
  - Kubernetes is no longer proven only by browser mocks.
- **Suggested artifacts**:
  - `scripts/k3s-runtime-smoke.sh`
  - `.github/workflows/k3s-runtime-smoke.yml`
- **Scope note**:
  - Treat Kubernetes as experimental / post-v1 until a dedicated Linux runner is stable.

### VM Backends

- **Scope**: add self-hosted runtime runners for Linux KVM, macOS Virtualization.framework, and Windows Hyper-V.
- **Minimum guest workflow per backend**:
  - Create a VM from a known-good OS image or fixed kernel/initrd pair.
  - Start and stop the VM successfully.
  - Add, list, and remove a port forward.
  - Verify the login command points to the expected forwarded port.
  - Assert that the console log is created and non-empty.
  - Delete the VM and confirm cleanup.
- **Acceptance criteria**:
  - Each backend has a dedicated runtime smoke script.
  - Each self-hosted job uploads console logs and runner diagnostics for failures.
  - Release-readiness is not considered complete until all target VM backends have a green runtime smoke path.
- **Suggested artifacts**:
  - `scripts/vm-runtime-smoke-linux-kvm.sh`
  - `scripts/vm-runtime-smoke-macos-vz.sh`
  - `scripts/vm-runtime-smoke-windows-hyperv.ps1`
  - `.github/workflows/vm-runtime-smoke.yml`
- **Rollout recommendation**:
  - Start as `workflow_dispatch` on self-hosted runners.
  - Promote to a required release gate once runner stability is proven.

## v1.0 Exit Criteria

- Linux desktop smoke proves at least one real runtime action chain through the actual Tauri shell.
- Docker runtime smoke remains green for the supporting container engine.
- AI runtime smoke remains green for local model, sandbox, and MCP process flows.
- Controlled provider canaries pass on dedicated infrastructure without leaking secrets or high-cost prompts.
- VMs and Kubernetes remain explicitly outside the v1 release gate until dedicated runners exist.

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

# Controlled provider canaries
./scripts/provider-canary-smoke.sh
./scripts/ollama-daemon-smoke.sh

# Full local gate
./scripts/ci-local.sh
```

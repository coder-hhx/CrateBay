# Progress

Updated: 2026-06-15

## Current Direction

CrateBay is now a container and image management app. The product surface is
centered on images, containers, pods, volumes, networks, settings, and the
built-in runtime.

## Completed

- Removed the former conversational/product layer from the app surface.
- Reduced desktop navigation to the management views that remain relevant.
- Kept runtime management, Engine connectivity, storage, and system commands.
- Kept bundled runtime images and bundle image loading.
- Added bundle-image generation and verification for the offline archives.
- Added CLI image archive commands:
  - `cratebay image export --output image.tar IMAGE...`
  - `cratebay image import image.tar`
- Added CLI pod commands backed by CrateBay Engine managed networks:
  - `cratebay pod list`
  - `cratebay pod create NAME [--driver DRIVER] [--internal] [--ipv6]`
  - `cratebay pod inspect NAME`
  - `cratebay pod add NAME CONTAINER`
  - `cratebay pod remove NAME CONTAINER`
  - `cratebay pod delete NAME --force`
- Added CLI network commands backed by the native CrateBay Engine by default:
  - `cratebay network list`
  - `cratebay network create NAME`
  - `cratebay network inspect NAME`
  - `cratebay network remove NAME`
- Added CLI volume commands backed by the native CrateBay Engine by default,
  including `cratebay volume create NAME --driver DRIVER`.
- Added one-shot CLI run commands for embedded/local automation:
  - `cratebay run IMAGE -- COMMAND...`
  - `cratebay container run IMAGE -- COMMAND...`
  - GUI one-shot Run now exposes the matching native request controls for
    name, entrypoint, environment, published ports, volumes, CPU/memory,
    working directory, pod or network, user, read-only rootfs, pull policy,
    keep-after-exit, timeout, and output limits.
  - captured stdout/stderr are bounded by `--max-output-bytes` by default so
    upper-layer tools can consume structured results safely
  - structured CLI mode now emits machine-readable command errors with
    `ok`, `kind`, and `error` fields
- Added desktop Pod management commands and page.
- Added desktop pod network driver, internal-network, and IPv6 creation
  controls, aligned with `cratebay pod create` options.
- Threaded native pod force removal through CLI, Tauri, and the desktop Pods
  delete confirmation, and made the native Engine reject non-forced removal
  while CrateBay-managed containers still reference the pod network, with the
  desktop delete dialog surfacing the native rejection reason inline.
- Added desktop Volume management commands and page.
- Added desktop volume driver selection for Engine volume creation, aligned
  with `cratebay volume create NAME --driver DRIVER`.
- Threaded native volume force removal through CLI, Tauri, and the desktop
  Volumes delete confirmation, and made the native Engine reject non-forced
  removal while CrateBay-managed containers still reference the volume, with
  the desktop delete dialog surfacing the native rejection reason inline.
- Added desktop Network management commands and page.
- Added desktop network driver, internal-network, and IPv6 creation controls,
  aligned with `cratebay network create` options.
- Threaded native network force removal through CLI, Tauri, and the desktop
  Networks delete confirmation, and made the native Engine reject non-forced
  removal while CrateBay-managed containers still reference the network, with
  the desktop delete dialog surfacing the native rejection reason inline.
- Connected desktop container Create and one-shot Run network selectors to
  CrateBay-managed networks from the Networks page, while retaining built-in
  `bridge`, `none`, and `host` modes.
- Aligned desktop container Create port publishing with the CLI/native Engine
  path by accepting TCP, UDP, and SCTP publish specs.
- Added one-shot Run port publishing across CLI, Tauri, GUI, and the native
  Engine request path, using the same TCP, UDP, and SCTP publish spec format.
- Aligned CLI container Create and one-shot Run network flags with the GUI by
  accepting CrateBay-managed network names in addition to built-in `bridge`,
  `none`, and `host` modes.
- Removed the stale core restriction that rejected CrateBay-managed network
  names, so Create and one-shot Run now pass custom network names through to
  the native Engine path consistently.
- Added CLI and native helper guards so container Create and one-shot Run keep
  Pod attachment and explicit network selection mutually exclusive.
- Connected desktop container Create and one-shot Run volume mount controls to
  CrateBay-managed volumes from the Volumes page, while retaining manual
  `source:/container[:ro|rw]` mount entry.
- Added desktop image import/export controls.
- Added native file pickers for desktop image import/export archive paths.
- Added desktop native Engine maintenance controls for substrate inspection,
  storage GC dry-run/apply, and shim task reaping.
- Added desktop Engine contract inspection aligned with the CLI `engine status`
  surface, including Engine kind, native API, namespace, and compatibility API
  state.
- Added desktop `runtime_diagnostics` aggregation so the Settings diagnostics
  and Engine maintenance refresh path use one coherent runtime, Engine
  contract, substrate, storage GC, and shim task snapshot.
- Added visible desktop Engine maintenance operation feedback for manual refresh,
  storage GC, and shim task reaping, including timestamps and result metrics.
- Added visible desktop image operation feedback for bundled image loading,
  archive import/export, and image tagging, including timestamps and result
  metrics.
- Added a desktop Images workflow for packing a selected container filesystem
  into a local image tag through the native `image_pack_container` command.
- Added desktop Images Engine-offline recovery for local image management, so
  the page surfaces the native runtime-start action instead of silently showing
  an empty image list when implicit runtime auto-start is disabled.
- Localized the desktop image pull task popover, including its accessible
  trigger label, so registry pull feedback follows the selected GUI language.
- Localized the desktop Container detail panel controls, spec labels, copy
  affordances, and relative timestamps so the high-frequency management view
  follows the selected GUI language.
- Localized desktop container monitoring CPU core units and timestamp locale
  formatting, plus memory and image-inspect OS labels, so live stats and
  image metadata follow the selected GUI language.
- Localized the desktop Images local-list timeout and relative-time labels, so
  local image inventory feedback follows the selected GUI language.
- Localized the desktop Images native archive picker labels and search-timeout
  feedback, so import/export and registry-search failures follow the selected
  GUI language.
- Localized desktop container store action feedback, including refresh
  fallbacks, image-pull placeholders, create/start/stop/delete notifications,
  and global uncaught-error notification titles; pull task rows now avoid
  leaking backend status strings into the active GUI language.
- Normalized backend image-pull progress event status strings to English so
  future GUI surfaces and embedded consumers do not receive localized Chinese
  runtime progress text from the Engine path.
- Localized the shared desktop dialog close controls, including the default
  icon-only accessible label and optional footer close button, so modal chrome
  follows the selected GUI language across resource workflows.
- Localized the global React error boundary fallback, including the crash title,
  description, details summary, and reload action, so the desktop app's last
  resort error surface follows the selected GUI language instead of hardcoded
  English.
- Routed shared desktop Tauri-operation fallback errors and container delete
  dialog fallback errors through typed i18n, keeping backend error details
  intact while avoiding hardcoded English for null or opaque failures.
- Extended the product surface guard to fail on CJK GUI source text outside the
  Simplified Chinese locale and tests, keeping user-facing copy routed through
  typed i18n.
- Extended the runtime native guard to scan the native Engine adapter source
  for real Docker daemon/package/service dependencies while still allowing
  Docker Hub image references and explicit compatibility API names.
- Verified `cratebay-engine-adapter` as a workspace package with direct
  `cargo check`, 132 focused unit/contract tests, and `clippy -D warnings`, so
  the self-managed containerd/runc/CNI Engine adapter stays covered by the Rust
  validation chain.
- Tightened the workspace Rust verification chain by clearing clippy warnings in
  the guest agent, macOS runtime adoption checks, and native Engine log-follow
  helper; `cargo clippy --workspace --exclude cratebay-gui --exclude cratebay-vz
  --all-targets -- -D warnings` and the matching workspace tests now cover
  core, CLI, guest-agent, and Engine adapter together.
- Tightened the GUI backend and VZ runner lint/test chain, clearing Tauri
  backend clippy warnings and folding core workspace, GUI backend, and VZ
  runner checks into one GitHub CI Rust job so each native management surface
  is explicitly covered without duplicate Tauri backend jobs.
- Split GitHub CI VZ runner verification onto macOS so `cratebay-vz` clippy and
  tests exercise the real Virtualization.framework bridge path instead of only
  the Linux unsupported-platform stub.
- Aligned `scripts/ci-local.sh` with the CI Rust verification layout so local
  runs cover core workspace clippy/tests, GUI backend clippy/tests, and VZ
  runner clippy/tests as separate gates.
- Aligned CLI image pulls with GUI Settings by using persisted registry
  mirrors when `cratebay image pull` is run without explicit `--mirror` flags.
- Added the same registry mirror behavior to `cratebay engine pull-image`,
  including explicit `--mirror` overrides for native Engine diagnostics.
- Shared the default registry mirror list with the CLI so fresh installs pull
  through the same mirrors as the GUI until the user changes Settings.
- Threaded registry mirrors into native container Create and one-shot Run
  automatic image pulls across GUI, CLI, and the Engine adapter, including
  pending-container record persistence for later starts.
- Threaded native image force removal through the desktop Images delete
  confirmation and top-level `cratebay image delete --force`, and made the
  native Engine reject non-forced image removal while CrateBay-managed
  containers still reference the image, with GUI feedback surfacing the native
  rejection reason.
- Added a desktop Container detail Exec tab backed by the native
  `container_exec` command, aligning the GUI with CLI `container exec` and
  `engine exec` management workflows.
- Aligned native `cratebay engine exec` with `container exec` embedded-caller
  semantics by exposing `--no-propagate-exit-code` alongside timeout and
  bounded-output controls.
- Added matching desktop Exec timeout and max-output controls, with the native
  Engine timeout/truncation flags preserved in the GUI result state.
- Added desktop container Logs tail and timestamp controls aligned with the CLI
  `container logs --tail --timestamps` management workflow.
- Added top-level `cratebay container stats` as the CLI counterpart to desktop
  container monitoring and native `cratebay engine stats`.
- Added top-level `cratebay container terminal-*` commands as CLI counterparts
  to the desktop native terminal workflow, backed by the self-managed CrateBay
  Engine.
- Added `cratebay runtime restart` as the CLI counterpart to the desktop
  Settings runtime restart workflow, with single structured output for embedded
  callers.
- Added `cratebay runtime proxy show|set|clear` and wired CLI runtime
  start/restart to the same persisted proxy settings used by desktop Settings.
- Added `cratebay runtime diagnostics` as the CLI counterpart to desktop
  Settings diagnostics and Engine maintenance snapshots, aggregating runtime
  status, Engine contract, substrate, storage GC dry-run, and shim task
  sections with per-section structured errors. CLI runtime status now also
  reconciles stale lifecycle state with native Engine contract reachability.
- Added top-level `cratebay settings list|get|set|reset` so CLI users can
  manage the same persisted Settings keys as the desktop app, including
  registry mirror updates consumed by image pull/create/run workflows.
- Added top-level `cratebay update check` so CLI users can inspect the same
  GitHub Release plus `latest.json` updater manifest metadata used by desktop
  Settings, with structured output for automation and the same persisted
  `includePrereleases` default as the desktop update panel.
- Threaded desktop container deletion through the same non-forced-by-default
  removal semantics as the native Engine, with an explicit force checkbox and
  visible native rejection feedback for running containers.
- Added visible desktop runtime control feedback for Engine VM start, stop,
  restart, and proxy-save workflows, including refreshed state, endpoint, and
  completion timestamps.
- Added native Tauri `runtime_restart` so desktop restart uses the same
  backend lifecycle operation shape as CLI `cratebay runtime restart` instead
  of front-end stop/start chaining.
- Added native Tauri `runtime_provision` and a desktop Runtime control for
  pre-downloading/preparing the self-managed runtime image without starting the
  Engine VM, matching CLI `cratebay runtime provision`.
- Expanded desktop Runtime proxy settings to cover the same proxy bridge, bind
  host/port, and guest host fields as CLI `cratebay runtime proxy set`.
- Added a desktop Runtime proxy reset action backed by the shared default proxy
  values, aligning GUI Settings with CLI `cratebay runtime proxy clear`.
- Added Windows/WSL runtime resource probes for CPU percent and disk usage by
  parsing guest `/proc/stat` and `df -B1` output, improving Dashboard and
  Settings runtime diagnostics for the self-managed WSL Engine.
- Added macOS/VZ runtime resource probes for runner process CPU/RSS and
  host-allocated runtime disk usage, replacing the previous zero-only
  Dashboard and Settings diagnostics placeholders.
- Added runtime resource usage to CLI `cratebay runtime status` and the runtime
  section of `cratebay runtime diagnostics`, including CPU, memory, disk, and
  managed container counts for parity with desktop monitoring.
- Added the runtime-reported managed container count to the desktop Dashboard
  Engine details so runtime resource probes are visible in the GUI as well as
  the CLI.
- Added live CPU, memory, disk, and runtime container usage rows to desktop
  Settings runtime diagnostics, reusing the same runtime resource payload as
  Dashboard and CLI status.
- Updated macOS/VZ runtime resource usage to count containers through the
  native CrateBay Engine container list first, keeping the compatibility API
  count only as a fallback.
- Updated Linux/KVM runtime resource usage to report host-allocated runtime
  disk usage and native Engine container counts, with compatibility API count
  as a fallback.
- Updated Windows/WSL runtime container counting to use the native CrateBay
  Engine container list first, replacing the previous guest shell
  `curl | grep | wc` compatibility-only probe with a Rust-side JSON fallback.
- Added native Engine contract readiness as the first-class runtime bring-up
  target, so GUI native management commands verify `/cratebay/engine` rather
  than depending on the Docker-compatible endpoint before issuing `/cratebay/*`
  calls.
- Updated desktop background runtime auto-start to use the native Engine
  contract as its success gate before emitting Engine connected events and
  preloading bundled images.
- Added native Engine auto-start/adoption for default CLI management commands
  (`run`, `container`, `pod`, `image` except registry search, `volume`,
  `network`, and `engine` management subcommands) while keeping explicit
  `--engine-host` as compatibility mode and leaving read-only
  diagnostics/status commands non-starting.
- Split CLI `engine` auto-start policy so contract/substrate inspection,
  storage GC dry-run, shim inventory, and shim reap dry-run stay side-effect
  free, while mutating maintenance and native resource management subcommands
  still ensure the strict CrateBay Engine contract first.
- Tightened macOS/VZ, Linux/KVM, and Windows/WSL runtime readiness so
  `RuntimeState::Ready`, health `engine_responsive`, and native auto-start wait
  on the strict `/cratebay/engine` contract (`cratebay.engine.v1` +
  `cratebay-containerd`); compatibility `_ping` is now kept for legacy fields,
  endpoint selection, and Bollard-compatible clients rather than native Ready.
- Tightened Tauri native command readiness and desktop `runtime_start` waits to
  use the same strict native Engine contract helper, while leaving raw Engine
  contract queries available for diagnostics and `engine status` inspection.
- Added a runtime native guard that scans runtime asset builders and bundled
  runtime resource docs so the built-in runtime cannot regress to real Docker
  daemon/package/service dependencies while compatibility aliases remain
  explicit.
- Split desktop runtime readiness helpers so the GUI treats only the strict
  native CrateBay Engine contract as management-ready, while compatibility
  endpoint pings remain a separate legacy signal; Dashboard counters now avoid
  native management calls when only the compatibility endpoint is online.
- Updated desktop chrome status indicators to show Engine Ready only for native
  Engine readiness instead of compatibility-only endpoint reachability.
- Scoped the GUI app store setters so `engineConnected` tracks only the native
  CrateBay Engine contract and the legacy `dockerConnected` flag tracks only
  compatibility endpoint reachability.
- Tightened desktop runtime health downgrade grace so compatibility-only
  endpoint connectivity can no longer preserve or imply native Engine readiness
  during transient health events.
- Tightened Tauri runtime health source reconciliation so compatibility-only
  endpoint responsiveness only fills the legacy compatibility source and can
  no longer backfill native `engine_source`.
- Tightened CLI and Tauri `runtime_status` source reconciliation so
  compatibility-only `docker_source` remains a legacy compatibility source and
  can no longer backfill primary native `engine_source`.
- Tightened desktop runtime source helpers so runtime status and health event
  handling no longer promote compatibility-only `docker_source` into built-in
  runtime readiness/source state.
- Relaxed desktop `runtime:health` typing so native-only events can omit legacy
  `docker_*` aliases without losing Engine readiness.
- Normalized desktop `runtime:health` state reads across snake_case and
  camelCase event payloads.
- Normalized desktop Engine endpoint status reads across Rust/Tauri camelCase
  payloads and snake_case test payloads so Dashboard and Settings reliably show
  the native API version and endpoint path.
- Corrected Dashboard runtime hints so a native Engine endpoint without
  runtime metadata is labeled as native API readiness rather than as a
  compatibility API endpoint.
- Updated desktop unit/E2E Tauri mocks so the default online path uses the
  native CrateBay Engine contract instead of Docker-compatible API versions.
- Added non-breaking CLI lifecycle aliases (`container remove|rm`,
  `image remove|rmi`, `volume delete`, and `network delete`), aligning common
  CLI verbs with the desktop/Tauri management surface while preserving
  existing automation.
- Isolated CLI runtime smoke HTTP proxy ports from the user's normal runtime:
  smoke runs now set an explicit high `CRATEBAY_RUNTIME_HTTP_PROXY_BIND_PORT`,
  macOS bridge mode derives a high port from `CRATEBAY_DATA_DIR`, and the CLI
  no longer overwrites explicit runtime proxy environment overrides with
  persisted default settings during native Engine auto-start.
- Updated the CLI-only runtime smoke assertions to match current native CLI
  output while keeping the checks object/API-focused; the smoke now validates
  the native Engine contract plus container, pod, image, volume, network, PTY,
  and image export/import workflows end to end without using Docker as the
  runtime.
- Added a CLI-only local registry smoke path and native loopback registry
  importer: the smoke mini-registry now serves standard `sha256:` blob routes
  and digest headers, while the Engine adapter pulls loopback OCI Registry v2
  images over HTTP, verifies digests, imports the image through containerd, and
  reports the `cratebay-loopback-registry` backend. Validated with
  `CRATEBAY_ALPINE_MIRROR=https://mirrors.aliyun.com/alpine ./scripts/runtime-smoke-local-registry.sh`.

## In Progress

- Continue closing GUI/CLI parity gaps across container creation, image
  lifecycle, and runtime diagnostics.
- Continue tightening the OrbStack-inspired desktop visual system while the
  management workflows stabilize.

## Verification Baseline

Run these before merging broad changes:

```bash
cargo fmt --check
./scripts/product-surface-guard.sh
./scripts/runtime-native-guard.sh
./scripts/verify-tauri-command-surface.sh
cargo check --workspace
cargo clippy --workspace --exclude cratebay-gui --exclude cratebay-vz --all-targets -- -D warnings
cargo clippy -p cratebay-gui --all-targets -- -D warnings
cargo clippy -p cratebay-vz --all-targets -- -D warnings
cargo test --workspace --exclude cratebay-gui --exclude cratebay-vz -- --test-threads=1
cargo test -p cratebay-gui -- --test-threads=1
cargo test -p cratebay-vz -- --test-threads=1
cd crates/cratebay-gui
pnpm run build
pnpm run lint
pnpm run test:unit
pnpm exec playwright test e2e/containers-list.spec.ts e2e/images-management.spec.ts e2e/pods-management.spec.ts e2e/volumes-management.spec.ts e2e/networks-management.spec.ts --project=chromium
cd ../..
./scripts/runtime-smoke-cli-only.sh
CRATEBAY_ALPINE_MIRROR=https://mirrors.aliyun.com/alpine ./scripts/runtime-smoke-local-registry.sh
```

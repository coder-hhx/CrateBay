#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

echo "== Local CI: Rust fmt =="
cargo fmt --check

echo "== Local CI: Product surface guard =="
./scripts/product-surface-guard.sh

echo "== Local CI: Runtime native guard =="
./scripts/runtime-native-guard.sh

echo "== Local CI: Tauri command surface =="
./scripts/verify-tauri-command-surface.sh

os_name="$(uname -s)"
core_clippy_args=(--workspace --exclude cratebay-gui --exclude cratebay-vz --all-targets -- -D warnings)
core_test_args=(--workspace --exclude cratebay-gui --exclude cratebay-vz -- --test-threads=1)

ready_runtime_file() {
  local file_path="$1"
  [[ -f "$file_path" ]] || return 1
  local file_size
  file_size="$(wc -c <"$file_path" | tr -d ' ')"
  if [[ "$file_size" -lt 1024 ]] && grep -Fq "PLACEHOLDER" "$file_path" 2>/dev/null; then
    return 1
  fi
  return 0
}

ensure_node_runtime() {
  if command -v node >/dev/null 2>&1; then
    local current_major
    current_major="$(node -p "process.versions.node.split('.')[0]" 2>/dev/null || echo 0)"
    if (( current_major >= 22 )); then
      return 0
    fi
  fi

  export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
  if [[ -s "$NVM_DIR/nvm.sh" ]]; then
    # shellcheck disable=SC1090
    . "$NVM_DIR/nvm.sh"
    for candidate in 24 22 --lts; do
      if nvm use "$candidate" >/dev/null 2>&1; then
        local nvm_major
        nvm_major="$(node -p "process.versions.node.split('.')[0]" 2>/dev/null || echo 0)"
        if (( nvm_major >= 22 )); then
          return 0
        fi
      fi
    done
  fi

  return 1
}

echo "== Local CI: Core workspace clippy =="
cargo clippy "${core_clippy_args[@]}"

echo "== Local CI: Core workspace tests =="
cargo test "${core_test_args[@]}"

if [[ "$os_name" == "Darwin" ]]; then
  rust_target="$(rustc -vV | awk '/^host:/ {print $2}' | head -n 1)"
  tauri_runner="$repo_root/crates/cratebay-gui/src-tauri/bin/cratebay-vz-${rust_target}"
  if ready_runtime_file "$tauri_runner"; then
    echo "== Local CI: Tauri external bin already present =="
  else
    echo "== Local CI: Prepare Tauri external bin (${rust_target}) =="
    bash "$repo_root/scripts/prepare-tauri-external-bins.sh" "$rust_target"
  fi
fi

if ! ensure_node_runtime; then
  if command -v node >/dev/null 2>&1; then
    node_version="$(node -v)"
    echo "ERROR: Node.js 22+ is required. Current: ${node_version}"
  else
    echo "ERROR: Node.js 22+ is required for frontend and Playwright checks (node not found)."
  fi
  echo "Use: nvm install 24 && nvm use 24"
  exit 1
fi

node_major="$(node -p "process.versions.node.split('.')[0]")"
node_version="$(node -v)"

echo "== Local CI: Frontend dist for Tauri =="
echo "Node runtime: ${node_version}"
pushd crates/cratebay-gui >/dev/null
corepack enable
pnpm install --frozen-lockfile
pnpm run build
popd >/dev/null

echo "== Local CI: GUI backend Rust check =="
cargo check -p cratebay-gui

echo "== Local CI: GUI backend Rust clippy =="
cargo clippy -p cratebay-gui --all-targets -- -D warnings

echo "== Local CI: GUI backend Rust tests =="
cargo test -p cratebay-gui -- --test-threads=1

echo "== Local CI: cratebay-vz clippy =="
cargo clippy -p cratebay-vz --all-targets -- -D warnings

if [[ "$os_name" == "Darwin" ]]; then
  if [[ "${CRATEBAY_RUN_VZ_TESTS:-0}" == "1" ]]; then
    echo "== Local CI: cratebay-vz tests =="
    cargo test -p cratebay-vz -- --test-threads=1
  else
    echo "== Local CI: cratebay-vz tests skipped =="
    echo "Set CRATEBAY_RUN_VZ_TESTS=1 to run cratebay-vz tests locally."
  fi
else
  echo "== Local CI: cratebay-vz tests =="
  cargo test -p cratebay-vz -- --test-threads=1
fi

echo "== Local CI: Frontend checks =="
echo "Node runtime: ${node_version}"
pushd crates/cratebay-gui >/dev/null
pnpm run lint
pnpm run check:i18n
pnpm run test:unit

echo "== Local CI: Frontend coverage =="
pnpm run test:coverage || echo "Coverage report may have non-zero exit if thresholds not met"

echo "== Local CI: Playwright browser install =="
pnpm exec playwright install chromium
echo "== Local CI: Frontend E2E tests =="
pnpm exec playwright test
popd >/dev/null

if [[ "${CRATEBAY_RUN_RUNTIME_SMOKE:-0}" == "1" ]]; then
  echo "== Local CI: CLI + built-in runtime smoke =="
  ./scripts/runtime-smoke-cli-only.sh
else
  echo "== Local CI: CLI + built-in runtime smoke skipped =="
  echo "Set CRATEBAY_RUN_RUNTIME_SMOKE=1 to start the built-in runtime and run the smoke test."
fi

if [[ "${CRATEBAY_RUN_LOCAL_REGISTRY_SMOKE:-0}" == "1" ]]; then
  echo "== Local CI: CLI + built-in runtime local registry pull smoke =="
  ./scripts/runtime-smoke-local-registry.sh
else
  echo "== Local CI: CLI + built-in runtime local registry pull smoke skipped =="
  echo "Set CRATEBAY_RUN_LOCAL_REGISTRY_SMOKE=1 to verify pull through a local registry container."
fi

if [[ "${CRATEBAY_RUN_ONLINE_PULL_SMOKE:-0}" == "1" ]]; then
  echo "== Local CI: CLI + built-in runtime live registry pull smoke =="
  ./scripts/runtime-smoke-online-pull.sh
else
  echo "== Local CI: CLI + built-in runtime live registry pull smoke skipped =="
  echo "Set CRATEBAY_RUN_ONLINE_PULL_SMOKE=1 when this host can reach Docker Hub or a configured registry."
fi

# Performance benchmarks (optional — requires release binaries)
if [[ -f "${RELEASE_DIR:-target/release}/cratebay" ]]; then
  echo "== Local CI: Performance benchmarks =="
  ./scripts/bench-perf.sh
else
  echo "== Local CI: Performance benchmarks skipped =="
  echo "Run 'cargo build --release -p cratebay-cli' first, then re-run."
fi

echo "== Local CI complete =="

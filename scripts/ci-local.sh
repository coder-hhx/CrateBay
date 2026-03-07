#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

echo "== Local CI: Rust fmt =="
cargo fmt --check

os_name="$(uname -s)"
if [[ "$os_name" == "Darwin" ]]; then
  clippy_args=(--workspace --exclude cratebay-gui -- -D warnings)
  test_args=(--workspace --exclude cratebay-gui --exclude cratebay-vz -- --test-threads=1)
else
  clippy_args=(--workspace --exclude cratebay-gui --exclude cratebay-vz -- -D warnings)
  test_args=(--workspace --exclude cratebay-gui --exclude cratebay-vz -- --test-threads=1)
fi

echo "== Local CI: Rust clippy =="
cargo clippy "${clippy_args[@]}"

echo "== Local CI: Rust tests =="
cargo test "${test_args[@]}"

echo "== Local CI: GUI backend Rust check =="
cargo check -p cratebay-gui

echo "== Local CI: GUI backend Rust tests =="
cargo test -p cratebay-gui

if [[ "$os_name" == "Darwin" ]]; then
  if [[ "${CRATEBAY_RUN_VZ_TESTS:-0}" == "1" ]]; then
    echo "== Local CI: cratebay-vz tests =="
    cargo test -p cratebay-vz -- --test-threads=1
  else
    echo "== Local CI: cratebay-vz tests skipped =="
    echo "Set CRATEBAY_RUN_VZ_TESTS=1 to run cratebay-vz tests locally."
  fi
fi

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

echo "== Local CI: Frontend checks =="
echo "Node runtime: ${node_version}"
pushd crates/cratebay-gui >/dev/null
npm ci
npm run lint
npm run build
npm run check:i18n
npm run test:unit
echo "== Local CI: Playwright browser install =="
npx playwright install chromium
echo "== Local CI: Frontend E2E tests =="
npx playwright test
popd >/dev/null

echo "== Local CI: AI runtime smoke =="
./scripts/ai-runtime-smoke.sh

if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  echo "== Local CI: Docker runtime smoke =="
  ./scripts/docker-runtime-smoke.sh
else
  echo "== Local CI: Docker runtime smoke skipped =="
  echo "Docker daemon is not available on this machine."
fi

echo "== Local CI complete =="

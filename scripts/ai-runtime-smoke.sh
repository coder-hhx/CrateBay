#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ERROR: required command '$1' not found"
    exit 1
  fi
}

require_cmd cargo

run_test() {
  local name="$1"
  shift
  echo "== $name =="
  cargo test -p cratebay-gui "$@" -- --ignored --nocapture --test-threads=1
}

run_test "AI Hub Ollama canary" ollama_runtime_canary_smoke
run_test "AI Hub MCP runtime smoke" mcp_runtime_smoke_lifecycle

if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  run_test "AI Hub sandbox runtime smoke" sandbox_runtime_smoke_lifecycle
else
  echo "== AI Hub sandbox runtime smoke skipped =="
  echo "Docker daemon is not available on this machine."
fi

echo "AI runtime smoke: PASS"

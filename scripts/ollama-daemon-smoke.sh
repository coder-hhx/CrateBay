#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

artifact_dir="$repo_root/dist/ollama-daemon-smoke"
log_file="$artifact_dir/ollama-daemon-smoke.log"
summary_file="$artifact_dir/summary.txt"
shim_dir="$artifact_dir/bin"
mkdir -p "$artifact_dir"
: > "$log_file"
: > "$summary_file"
exec > >(tee -a "$log_file") 2>&1

export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "ERROR: required command '$1' not found"
    exit 1
  fi
}

require_cmd cargo

echo "== Ollama canary discovery =="
if [[ -z "${CRATEBAY_CANARY_OLLAMA_BIN:-}" ]]; then
  echo "No Ollama binary configured; skipping daemon canary."
  printf 'status=skipped\nreason=no ollama binary configured\n' > "$summary_file"
  exit 0
fi

if [[ -x "$CRATEBAY_CANARY_OLLAMA_BIN" ]]; then
  mkdir -p "$shim_dir"
  ln -sf "$CRATEBAY_CANARY_OLLAMA_BIN" "$shim_dir/ollama"
  export PATH="$shim_dir:$PATH"
  echo "using shimmed Ollama binary from CRATEBAY_CANARY_OLLAMA_BIN"
elif command -v "$CRATEBAY_CANARY_OLLAMA_BIN" >/dev/null 2>&1; then
  export CRATEBAY_CANARY_OLLAMA_BIN="$(command -v "$CRATEBAY_CANARY_OLLAMA_BIN")"
  echo "using Ollama command from PATH: $CRATEBAY_CANARY_OLLAMA_BIN"
else
  echo "ERROR: CRATEBAY_CANARY_OLLAMA_BIN is not executable: $CRATEBAY_CANARY_OLLAMA_BIN"
  exit 1
fi

echo "== Ollama canary compile check =="
cargo test -p cratebay-gui --no-run

echo "== Ollama daemon canary =="
cargo test -p cratebay-gui ollama_real_daemon_canary_smoke -- --ignored --nocapture --test-threads=1

printf 'status=pass\ntest=Ollama daemon canary\n' > "$summary_file"
echo "Ollama daemon smoke: PASS"

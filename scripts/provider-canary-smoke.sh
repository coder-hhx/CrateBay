#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

artifact_dir="$repo_root/dist/provider-canary"
log_file="$artifact_dir/provider-canary.log"
summary_file="$artifact_dir/summary.txt"
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

has_env() {
  local key="$1"
  [[ -n "${!key:-}" ]]
}

resolve_bin_env() {
  local env_name="$1"
  local current="${!env_name:-}"

  if [[ -z "$current" ]]; then
    return 1
  fi

  if [[ -x "$current" ]]; then
    export "$env_name=$current"
    return 0
  fi
  if command -v "$current" >/dev/null 2>&1; then
    export "$env_name=$(command -v "$current")"
    return 0
  fi

  echo "WARN: $env_name is set but not executable: $current"
  return 1
}

add_test() {
  labels+=("$1")
  filters+=("$2")
}

require_cmd cargo

declare -a labels=()
declare -a filters=()

echo "== Provider canary discovery =="
if has_env CRATEBAY_CANARY_OPENAI_API_KEY; then
  add_test "OpenAI provider canary" "openai_provider_canary_real_connection"
else
  echo "skip: OpenAI canary not configured"
fi

if has_env CRATEBAY_CANARY_ANTHROPIC_API_KEY; then
  add_test "Anthropic provider canary" "anthropic_provider_canary_real_connection"
else
  echo "skip: Anthropic canary not configured"
fi

if resolve_bin_env CRATEBAY_CANARY_CODEX_BIN; then
  add_test "Codex CLI bridge canary" "codex_cli_bridge_canary"
else
  echo "skip: Codex CLI bridge canary not configured"
fi

if resolve_bin_env CRATEBAY_CANARY_CLAUDE_BIN; then
  add_test "Claude CLI bridge canary" "claude_cli_bridge_canary"
else
  echo "skip: Claude CLI bridge canary not configured"
fi

if [[ "${#filters[@]}" -eq 0 ]]; then
  echo "No controlled provider canaries are configured; skipping."
  printf 'status=skipped\nreason=no provider credentials or bridge binaries configured\n' > "$summary_file"
  exit 0
fi

echo "== Provider canary compile check =="
cargo test -p cratebay-gui --no-run

summary_lines=("status=pass" "count=${#filters[@]}")
for i in "${!filters[@]}"; do
  label="${labels[$i]}"
  filter="${filters[$i]}"
  echo "== $label =="
  cargo test -p cratebay-gui "$filter" -- --ignored --nocapture --test-threads=1
  summary_lines+=("test_$((i + 1))=$label")
done

printf '%s\n' "${summary_lines[@]}" > "$summary_file"
echo "Provider canary smoke: PASS (${#filters[@]} checks)"

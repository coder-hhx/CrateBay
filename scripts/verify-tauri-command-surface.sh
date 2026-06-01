#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

frontend_invokes="$tmp_dir/frontend-invokes.txt"
registered_commands="$tmp_dir/registered-commands.txt"
mocked_commands="$tmp_dir/mocked-commands.txt"
missing_handlers="$tmp_dir/missing-handlers.txt"
missing_mocks="$tmp_dir/missing-mocks.txt"
frontend_file_list="$tmp_dir/frontend-files.txt"

rg --files crates/cratebay-gui/src \
  | rg '\.(ts|tsx)$' \
  >"$frontend_file_list"

if [[ ! -s "$frontend_file_list" ]]; then
  echo "ERROR: no frontend source files found" >&2
  exit 1
fi

while IFS= read -r file_path; do
  perl -ne 'while (/invoke(?:<[^>]+>)?\(\s*"([^"]+)"/g) { print "$1\n" unless $1 =~ /\$/ }' "$file_path"
done <"$frontend_file_list" | sort -u >"$frontend_invokes"

perl -ne 'while (/commands::[A-Za-z0-9_]+::([A-Za-z0-9_]+)/g) { print "$1\n" }' \
  crates/cratebay-gui/src-tauri/src/main.rs \
  | sort -u >"$registered_commands"

perl -ne 'while (/case\s+"([^"]+)"\s*:/g) { print "$1\n" }' \
  crates/cratebay-gui/e2e/tauri-mock.ts \
  | sort -u >"$mocked_commands"

comm -23 "$frontend_invokes" "$registered_commands" >"$missing_handlers"
comm -23 "$frontend_invokes" "$mocked_commands" >"$missing_mocks"

status=0

if [[ -s "$missing_handlers" ]]; then
  echo "ERROR: frontend invokes missing Tauri handler registration:" >&2
  sed 's/^/  - /' "$missing_handlers" >&2
  status=1
fi

if [[ -s "$missing_mocks" ]]; then
  echo "ERROR: frontend invokes missing E2E mock coverage:" >&2
  sed 's/^/  - /' "$missing_mocks" >&2
  status=1
fi

if [[ "$status" -ne 0 ]]; then
  exit "$status"
fi

echo "Tauri command surface: PASS ($(wc -l <"$frontend_invokes" | tr -d ' ') frontend commands)"

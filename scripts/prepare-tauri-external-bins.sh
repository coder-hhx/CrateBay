#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

has_virtualization_entitlements() {
  local binary_path="$1"
  command -v codesign >/dev/null 2>&1 || return 1
  # Try both XML plist (:-) and human-readable (-) formats for macOS compat
  local output
  output="$(codesign -d --entitlements :- "$binary_path" 2>&1; codesign -d --entitlements - "$binary_path" 2>&1)"
  echo "$output" | grep -Fq "com.apple.security.virtualization"
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "SKIP: prepare-tauri-external-bins.sh is macOS-only."
  exit 0
fi

target="${1:-}"
if [[ -z "$target" ]]; then
  target="$(rustc -vV | awk '/^host:/ {print $2}' | head -n 1)"
fi
if [[ -z "$target" ]]; then
  echo "ERROR: failed to resolve Rust host target." >&2
  exit 1
fi

bin_dir="crates/cratebay-gui/src-tauri/bin"
mkdir -p "$bin_dir"

echo "== Build cratebay-vz (${target}) =="
cargo build --release --target "$target" -p cratebay-vz

src="target/${target}/release/cratebay-vz"
if [[ ! -f "$src" ]]; then
  # Fallback for builds without explicit --target.
  src="target/release/cratebay-vz"
fi
if [[ ! -f "$src" ]]; then
  echo "ERROR: cratebay-vz binary not found (checked: target/${target}/release/cratebay-vz, target/release/cratebay-vz)" >&2
  exit 1
fi

dst="${bin_dir}/cratebay-vz-${target}"
cp "$src" "$dst"
chmod +x "$dst"

if [[ "${CRATEBAY_SKIP_CODESIGN:-0}" != "1" ]]; then
  if command -v codesign >/dev/null 2>&1; then
    entitlements="$repo_root/scripts/macos-entitlements.plist"
    if [[ -f "$entitlements" ]]; then
      identity="${CRATEBAY_CODESIGN_IDENTITY:--}"
      echo "== Codesign cratebay-vz (${target}) =="
      codesign --force --sign "$identity" --options runtime --entitlements "$entitlements" "$dst"
    else
      echo "WARN: entitlements plist not found: $entitlements"
    fi
  else
    echo "WARN: codesign not available; cratebay-vz may fail on newer macOS versions."
  fi
fi

if [[ "${CRATEBAY_SKIP_CODESIGN:-0}" != "1" ]] && ! has_virtualization_entitlements "$dst"; then
  echo "ERROR: cratebay-vz was staged without virtualization entitlements: $dst" >&2
  exit 1
fi

echo "External bin ready: ${dst}"

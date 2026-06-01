#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
bundle_dir="${1:-$repo_root/crates/cratebay-gui/src-tauri/bundle-images}"
if [[ "$#" -gt 0 ]]; then
  shift
fi

if [[ "$#" -gt 0 ]]; then
  required=("$@")
else
  required=(
    python-dev.tar.gz
    node-dev.tar.gz
    rust-dev.tar.gz
    ubuntu-base.tar.gz
  )
fi

status=0
echo "Bundle image directory: $bundle_dir"

expected_image_for_archive() {
  case "$1" in
    python-dev.tar.gz) echo "cratebay-python-dev:v1" ;;
    node-dev.tar.gz) echo "cratebay-node-dev:v1" ;;
    rust-dev.tar.gz) echo "cratebay-rust-dev:v1" ;;
    ubuntu-base.tar.gz) echo "cratebay-ubuntu-base:v1" ;;
    *) echo "" ;;
  esac
}

for archive in "${required[@]}"; do
  path="$bundle_dir/$archive"
  if [[ ! -f "$path" ]]; then
    echo "MISSING $archive"
    status=1
    continue
  fi

  size="$(wc -c <"$path" | tr -d ' ')"
  if [[ "$size" -le 0 ]]; then
    echo "EMPTY   $archive"
    status=1
    continue
  fi

  if ! gzip -t "$path" 2>/dev/null; then
    echo "BADGZIP $archive"
    status=1
    continue
  fi

  if ! tar_listing="$(gzip -dc "$path" | tar -tf - 2>/dev/null)"; then
    echo "BADTAR  $archive"
    status=1
    continue
  fi

  if ! printf '%s\n' "$tar_listing" | grep -Fxq "manifest.json"; then
    echo "BADTAR  $archive (missing manifest.json)"
    status=1
    continue
  fi

  expected_image="$(expected_image_for_archive "$archive")"
  if [[ -n "$expected_image" ]]; then
    manifest="$(gzip -dc "$path" | tar -xOf - manifest.json 2>/dev/null || true)"
    if ! printf '%s\n' "$manifest" | grep -Fq "\"$expected_image\""; then
      echo "BADTAG  $archive (missing $expected_image)"
      status=1
      continue
    fi
  fi

  echo "OK      $archive ($size bytes)"
done

if [[ "$status" -ne 0 ]]; then
  echo "Bundle image verification failed." >&2
  exit "$status"
fi

echo "Bundle image verification: PASS"

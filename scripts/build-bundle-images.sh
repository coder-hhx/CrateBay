#!/bin/bash
# Prepare CrateBay container images and export them as tar.gz for offline bundling.
#
# Uses the CrateBay CLI and built-in runtime. Set CRATEBAY_BIN to point at an
# existing CLI binary, or let the script build target/debug/cratebay.
#
# Usage:
#   ./scripts/build-bundle-images.sh [image...]
#
# Examples:
#   ./scripts/build-bundle-images.sh              # build all
#   ./scripts/build-bundle-images.sh python node   # build only python and node

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_ROOT/crates/cratebay-gui/src-tauri/bundle-images"
CRATEBAY_BIN="${CRATEBAY_BIN:-$PROJECT_ROOT/target/debug/cratebay}"
CRATEBAY_BUNDLE_IMAGE_MIRRORS="${CRATEBAY_BUNDLE_IMAGE_MIRRORS:-docker.1ms.run docker.xuanyuan.me dockerhub.icu}"
CRATEBAY_BUNDLE_PULL_RETRIES="${CRATEBAY_BUNDLE_PULL_RETRIES:-3}"
TEMP_DIR="$(mktemp -d)"
suffix="$(date +%s)-$$"
runtime_started=0
own_data_dir=0
own_socket_path=0

if [ -z "${CRATEBAY_DATA_DIR:-}" ]; then
  export CRATEBAY_DATA_DIR="$PROJECT_ROOT/target/bundle-images-runtime-${suffix}"
  own_data_dir=1
fi
if [ -z "${CRATEBAY_DOCKER_SOCKET_PATH:-}" ]; then
  export CRATEBAY_DOCKER_SOCKET_PATH="/tmp/cratebay-bundle-images-${suffix}.sock"
  own_socket_path=1
fi
export CRATEBAY_DOCKER_PROXY_PORT="${CRATEBAY_DOCKER_PROXY_PORT:-$((43000 + ($$ % 10000)))}"
export CRATEBAY_LINUX_DOCKER_PORT="${CRATEBAY_LINUX_DOCKER_PORT:-$CRATEBAY_DOCKER_PROXY_PORT}"

cleanup() {
  set +e
  if [ "$runtime_started" = "1" ] && [ "${CRATEBAY_KEEP_BUNDLE_RUNTIME:-0}" != "1" ]; then
    "$CRATEBAY_BIN" runtime stop >/dev/null 2>&1 || true
  fi
  if [ "$own_socket_path" = "1" ]; then
    rm -f "$CRATEBAY_DOCKER_SOCKET_PATH" >/dev/null 2>&1 || true
  fi
  rm -rf "$TEMP_DIR"
  if [ "$own_data_dir" = "1" ] && [ "${CRATEBAY_KEEP_BUNDLE_RUNTIME:-0}" != "1" ] && [ -d "$CRATEBAY_DATA_DIR" ]; then
    rm -rf "$CRATEBAY_DATA_DIR"
  fi
}
trap cleanup EXIT

has_registry_prefix() {
  local image="$1"
  [[ "$image" == */* ]] || return 1
  local first_component="${image%%/*}"
  [[ "$first_component" == *.* || "$first_component" == *:* || "$first_component" == "localhost" ]]
}

rewrite_image_for_mirror() {
  local image="$1"
  local mirror="$2"
  mirror="${mirror%/}"

  if has_registry_prefix "$image"; then
    printf '%s\n' "$image"
    return 0
  fi

  if [[ "$image" == */* ]]; then
    printf '%s/%s\n' "$mirror" "$image"
  else
    printf '%s/library/%s\n' "$mirror" "$image"
  fi
}

pull_with_retries() {
  local image_ref="$1"
  local attempt=1
  local max_attempts="$CRATEBAY_BUNDLE_PULL_RETRIES"

  while true; do
    if "$CRATEBAY_BIN" image pull "$image_ref"; then
      return 0
    fi

    if [ "$attempt" -ge "$max_attempts" ]; then
      return 1
    fi

    echo "WARN: Pull attempt $attempt failed for $image_ref; retrying..." >&2
    sleep "$attempt"
    attempt=$((attempt + 1))
  done
}

mkdir -p "$OUTPUT_DIR"

if [ ! -x "$CRATEBAY_BIN" ]; then
  echo "Building CrateBay CLI..."
  cargo build -p cratebay-cli >/dev/null
fi

if [ ! -x "$CRATEBAY_BIN" ]; then
  echo "ERROR: CrateBay CLI not found at $CRATEBAY_BIN"
  exit 1
fi

echo "Starting CrateBay built-in runtime..."
"$CRATEBAY_BIN" runtime start
runtime_started=1

pull_and_export() {
  local pull_image="$1"
  local tag_image="$2"
  local archive="$3"
  local raw_tar="$TEMP_DIR/${tag_image//[:\/]/-}.tar"
  local -a candidates=()
  local mirror candidate pulled_ref=""

  if [ -n "$CRATEBAY_BUNDLE_IMAGE_MIRRORS" ]; then
    for mirror in $CRATEBAY_BUNDLE_IMAGE_MIRRORS; do
      candidate="$(rewrite_image_for_mirror "$pull_image" "$mirror")"
      candidates+=("$candidate")
    done
  fi
  candidates+=("$pull_image")

  for candidate in "${candidates[@]}"; do
    echo "=== Pulling $candidate ==="
    if pull_with_retries "$candidate"; then
      pulled_ref="$candidate"
      break
    fi
    echo "WARN: Candidate failed: $candidate" >&2
  done

  if [ -z "$pulled_ref" ]; then
    echo "ERROR: Failed to pull $pull_image from all configured mirrors and direct Docker Hub" >&2
    exit 1
  fi

  echo "=== Tagging as $tag_image ==="
  "$CRATEBAY_BIN" image tag "$pulled_ref" "$tag_image"

  echo "=== Exporting $tag_image -> $archive ==="
  "$CRATEBAY_BIN" image export --output "$raw_tar" "$tag_image"
  gzip -c "$raw_tar" > "$archive"

  local size
  size=$(du -h "$archive" | cut -f1)
  echo "=== Done: $archive ($size) ==="
  echo
}

# Determine which images to build
if [ $# -gt 0 ]; then
  TARGETS=("$@")
else
  TARGETS=(python node rust ubuntu)
fi

built_archives=()

for name in "${TARGETS[@]}"; do
  case "$name" in
    python)
      pull_and_export "python:3.12-slim-bookworm" "cratebay-python-dev:v1" "$OUTPUT_DIR/python-dev.tar.gz"
      built_archives+=("python-dev.tar.gz")
      ;;
    node)
      pull_and_export "node:20-slim" "cratebay-node-dev:v1" "$OUTPUT_DIR/node-dev.tar.gz"
      built_archives+=("node-dev.tar.gz")
      ;;
    rust)
      pull_and_export "rust:1-slim-bookworm" "cratebay-rust-dev:v1" "$OUTPUT_DIR/rust-dev.tar.gz"
      built_archives+=("rust-dev.tar.gz")
      ;;
    ubuntu)
      pull_and_export "ubuntu:24.04" "cratebay-ubuntu-base:v1" "$OUTPUT_DIR/ubuntu-base.tar.gz"
      built_archives+=("ubuntu-base.tar.gz")
      ;;
    *)
      echo "ERROR: Unknown image '$name'. Available: python node rust ubuntu"
      exit 1
      ;;
  esac
done

echo "All images built successfully:"
ls -lh "$OUTPUT_DIR"/*.tar.gz 2>/dev/null || echo "(no tar.gz files found)"
"$PROJECT_ROOT/scripts/verify-bundle-images.sh" "$OUTPUT_DIR" "${built_archives[@]}"

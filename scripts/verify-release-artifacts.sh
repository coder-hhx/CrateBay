#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob
shopt -s globstar 2>/dev/null || true

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

mode="ci"
runner_os=""
target=""
artifacts_root="artifacts"

usage() {
  cat <<USAGE
Usage:
  bash scripts/verify-release-artifacts.sh --mode ci --os <macOS|Linux|Windows> --target <rust-target>
  bash scripts/verify-release-artifacts.sh --mode downloaded [--artifacts-root <dir>]
USAGE
}

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

require_match() {
  local label="$1"
  shift
  matched_files=()
  for pattern in "$@"; do
    for file in $pattern; do
      matched_files+=("$file")
    done
  done
  if [[ ${#matched_files[@]} -eq 0 ]]; then
    fail "$label missing (checked: $*)"
  fi
  echo "OK: $label"
  printf '  %s\n' "${matched_files[@]}"
}

verify_macos_app_bundle_images() {
  local label="$1"
  shift

  require_match "$label" "$@"
  local app_bundle
  for app_bundle in "${matched_files[@]}"; do
    verify_app_bundle_images "$app_bundle"
  done
}

verify_app_bundle_images() {
  local app_bundle="$1"
  local candidates=()
  local candidate

  [[ -d "$app_bundle" ]] || fail "macOS app bundle is not a directory: $app_bundle"

  while IFS= read -r -d '' candidate; do
    candidates+=("$candidate")
  done < <(find "$app_bundle" -type d -name bundle-images -print0)

  if [[ ${#candidates[@]} -eq 0 ]]; then
    fail "Bundled image resources missing from app bundle: $app_bundle"
  fi

  for candidate in "${candidates[@]}"; do
    if bash scripts/verify-bundle-images.sh "$candidate"; then
      echo "OK: bundled image resources in $app_bundle"
      echo "  $candidate"
      return 0
    fi
  done

  fail "Bundled image resources are present but invalid in app bundle: $app_bundle"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      mode="$2"
      shift 2
      ;;
    --os)
      runner_os="$2"
      shift 2
      ;;
    --target)
      target="$2"
      shift 2
      ;;
    --artifacts-root)
      artifacts_root="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      fail "Unknown argument: $1"
      ;;
  esac
done

verify_ci() {
  [[ -n "$runner_os" ]] || fail "--os is required for --mode ci"
  [[ -n "$target" ]] || fail "--target is required for --mode ci"

  local bundle_root="target/${target}/release/bundle"
  [[ -d "$bundle_root" ]] || fail "Bundle root not found: $bundle_root"

  case "$runner_os" in
    macOS)
      verify_macos_app_bundle_images "macOS app bundle" "$bundle_root/macos/*.app"
      require_match "macOS dmg bundle" "$bundle_root/dmg/*.dmg"
      ;;
    Linux)
      require_match "Linux deb bundle" "$bundle_root/deb/*.deb"
      require_match "Linux AppImage bundle" "$bundle_root/appimage/*.AppImage"
      ;;
    Windows)
      require_match "Windows msi bundle" "$bundle_root/msi/*.msi"
      require_match "Windows nsis bundle" "$bundle_root/nsis/*.exe"
      ;;
    *)
      fail "Unsupported --os value: $runner_os"
      ;;
  esac
}

verify_downloaded() {
  [[ -d "$artifacts_root" ]] || fail "Artifacts root not found: $artifacts_root"

  if [[ -d "$artifacts_root/bundle-images" ]]; then
    fail "Internal bundle-images artifact should not be published as a standalone release asset"
  fi

  require_match "macOS aarch64 CLI artifact" "$artifacts_root/cratebay-macos-aarch64/cratebay" "$artifacts_root/cratebay-macos-aarch64/**/cratebay"
  require_match "macOS x86_64 CLI artifact" "$artifacts_root/cratebay-macos-x86_64/cratebay" "$artifacts_root/cratebay-macos-x86_64/**/cratebay"
  require_match "Linux x86_64 CLI artifact" "$artifacts_root/cratebay-linux-x86_64/cratebay" "$artifacts_root/cratebay-linux-x86_64/**/cratebay"
  require_match "Linux aarch64 CLI artifact" "$artifacts_root/cratebay-linux-aarch64/cratebay" "$artifacts_root/cratebay-linux-aarch64/**/cratebay"
  require_match "Windows x86_64 CLI artifact" "$artifacts_root/cratebay-windows-x86_64/cratebay.exe" "$artifacts_root/cratebay-windows-x86_64/**/cratebay.exe"
  require_match "Windows aarch64 CLI artifact" "$artifacts_root/cratebay-windows-aarch64/cratebay.exe" "$artifacts_root/cratebay-windows-aarch64/**/cratebay.exe"

  verify_macos_app_bundle_images "macOS aarch64 app bundle" "$artifacts_root/cratebay-gui-macos-aarch64/**/*.app"
  require_match "macOS aarch64 dmg bundle" "$artifacts_root/cratebay-gui-macos-aarch64/**/*.dmg"
  verify_macos_app_bundle_images "macOS x86_64 app bundle" "$artifacts_root/cratebay-gui-macos-x86_64/**/*.app"
  require_match "macOS x86_64 dmg bundle" "$artifacts_root/cratebay-gui-macos-x86_64/**/*.dmg"
  require_match "Linux x86_64 deb bundle" "$artifacts_root/cratebay-gui-linux-x86_64/**/*.deb"
  require_match "Linux x86_64 AppImage bundle" "$artifacts_root/cratebay-gui-linux-x86_64/**/*.AppImage"
  require_match "Linux aarch64 deb bundle" "$artifacts_root/cratebay-gui-linux-aarch64/**/*.deb"
  require_match "Linux aarch64 AppImage bundle" "$artifacts_root/cratebay-gui-linux-aarch64/**/*.AppImage"
  require_match "Windows x86_64 msi bundle" "$artifacts_root/cratebay-gui-windows-x86_64/**/*.msi"
  require_match "Windows x86_64 nsis bundle" "$artifacts_root/cratebay-gui-windows-x86_64/**/*.exe"
  require_match "Windows aarch64 msi bundle" "$artifacts_root/cratebay-gui-windows-aarch64/**/*.msi"
  require_match "Windows aarch64 nsis bundle" "$artifacts_root/cratebay-gui-windows-aarch64/**/*.exe"
}

case "$mode" in
  ci)
    verify_ci
    ;;
  downloaded)
    verify_downloaded
    ;;
  *)
    fail "Unsupported --mode value: $mode"
    ;;
esac

echo "Release artifact verification: PASS"

#!/usr/bin/env bash
set -euo pipefail
shopt -s nullglob globstar

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
  local matches=()
  for pattern in "$@"; do
    for file in $pattern; do
      matches+=("$file")
    done
  done
  if [[ ${#matches[@]} -eq 0 ]]; then
    fail "$label missing (checked: $*)"
  fi
  echo "OK: $label"
  printf '  %s\n' "${matches[@]}"
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
      require_match "macOS app bundle" "$bundle_root/macos/*.app"
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

  require_match "macOS aarch64 CLI artifact" "$artifacts_root/cratebay-macos-aarch64/**/cratebay"
  require_match "macOS x86_64 CLI artifact" "$artifacts_root/cratebay-macos-x86_64/**/cratebay"
  require_match "Linux x86_64 CLI artifact" "$artifacts_root/cratebay-linux-x86_64/**/cratebay"
  require_match "Linux aarch64 CLI artifact" "$artifacts_root/cratebay-linux-aarch64/**/cratebay"
  require_match "Windows x86_64 CLI artifact" "$artifacts_root/cratebay-windows-x86_64/**/cratebay.exe"
  require_match "Windows aarch64 CLI artifact" "$artifacts_root/cratebay-windows-aarch64/**/cratebay.exe"

  require_match "macOS aarch64 app bundle" "$artifacts_root/cratebay-gui-macos-aarch64/**/*.app"
  require_match "macOS aarch64 dmg bundle" "$artifacts_root/cratebay-gui-macos-aarch64/**/*.dmg"
  require_match "macOS x86_64 app bundle" "$artifacts_root/cratebay-gui-macos-x86_64/**/*.app"
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

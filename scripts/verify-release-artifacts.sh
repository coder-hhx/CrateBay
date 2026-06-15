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

require_headless_zip() {
  local platform="$1"
  shift
  local required_entries=("$@")

  require_match \
    "Headless ${platform} zip" \
    "$artifacts_root/**/CrateBay-*-headless-${platform}.zip" \
    "$artifacts_root/CrateBay-*-headless-${platform}.zip"
  local zip_path="${matched_files[0]}"

  require_match \
    "Headless ${platform} checksum" \
    "$artifacts_root/**/CrateBay-*-headless-${platform}.zip.sha256" \
    "$artifacts_root/CrateBay-*-headless-${platform}.zip.sha256"

  python3 - "$zip_path" "${required_entries[@]}" <<'PY'
import sys
from zipfile import ZipFile

zip_path = sys.argv[1]
required = sys.argv[2:]
with ZipFile(zip_path) as archive:
    names = set(archive.namelist())
missing = [entry for entry in required if entry not in names]
if missing:
    raise SystemExit(f"{zip_path} missing required entries: {', '.join(missing)}")
print(f"OK: headless package contents in {zip_path}")
for entry in required:
    print(f"  {entry}")
PY
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

  require_headless_zip \
    "macos-aarch64" \
    "bin/cratebay" \
    "bin/cratebay-vz" \
    "resources/runtime-images/cratebay-runtime-aarch64/vmlinuz" \
    "resources/runtime-images/cratebay-runtime-aarch64/initramfs"
  require_headless_zip \
    "macos-x86_64" \
    "bin/cratebay" \
    "bin/cratebay-vz" \
    "resources/runtime-images/cratebay-runtime-x86_64/vmlinuz" \
    "resources/runtime-images/cratebay-runtime-x86_64/initramfs"
  require_headless_zip \
    "linux-x86_64" \
    "bin/cratebay" \
    "resources/runtime-images/cratebay-runtime-x86_64/vmlinuz" \
    "resources/runtime-images/cratebay-runtime-x86_64/initramfs" \
    "resources/runtime-linux/cratebay-runtime-linux-x86_64/qemu-system-x86_64"
  require_headless_zip \
    "windows-x86_64" \
    "bin/cratebay.exe" \
    "resources/runtime-wsl/cratebay-runtime-wsl-x86_64/rootfs.tar"

  require_match "macOS aarch64 dmg bundle" "$artifacts_root/**/CrateBay-*-macOS-aarch64.dmg" "$artifacts_root/CrateBay-*-macOS-aarch64.dmg"
  require_match "macOS x86_64 dmg bundle" "$artifacts_root/**/CrateBay-*-macOS-x86_64.dmg" "$artifacts_root/CrateBay-*-macOS-x86_64.dmg"
  require_match "Linux x86_64 deb bundle" "$artifacts_root/**/CrateBay-*-Linux-x86_64.deb" "$artifacts_root/CrateBay-*-Linux-x86_64.deb"
  require_match "Linux x86_64 AppImage bundle" "$artifacts_root/**/CrateBay-*-Linux-x86_64.AppImage" "$artifacts_root/CrateBay-*-Linux-x86_64.AppImage"
  require_match "Windows x86_64 msi bundle" "$artifacts_root/**/CrateBay-*-Windows-x64.msi" "$artifacts_root/CrateBay-*-Windows-x64.msi"
  require_match "Windows x86_64 nsis bundle" "$artifacts_root/**/CrateBay-*-Windows-x64-Setup.exe" "$artifacts_root/CrateBay-*-Windows-x64-Setup.exe"
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

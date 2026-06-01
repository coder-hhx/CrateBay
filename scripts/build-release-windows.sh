#!/usr/bin/env bash
#
# build-release-windows.sh — Build CrateBay release artifacts for Windows
#
# Produces:
#   dist/cratebay.exe              — CLI binary
#   dist/CrateBay_<ver>_x64.msi   — MSI installer (GUI + runtime assets)
#   dist/CrateBay_<ver>_x64-setup.exe — NSIS installer (GUI + runtime assets)
#
# Prerequisites:
#   - Rust stable toolchain (MSVC)
#   - Node.js + npm
#   - protoc (Protocol Buffers compiler)
#
# Usage:
#   bash scripts/build-release-windows.sh [--skip-gui]
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

VERSION="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$REPO_ROOT/Cargo.toml" | head -n 1)"
if [[ -z "${VERSION}" ]]; then
    echo "ERROR: Failed to resolve version from workspace Cargo.toml"
    exit 1
fi
ARCH="x86_64"
RUST_TARGET="x86_64-pc-windows-msvc"

GUI_CRATE="crates/cratebay-gui"
BUNDLE_IMAGES_DIR="$REPO_ROOT/$GUI_CRATE/src-tauri/bundle-images"
DIST_DIR="$REPO_ROOT/dist"

ensure_node_runtime() {
    if command -v node >/dev/null 2>&1; then
        local current_major
        current_major="$(node -p "process.versions.node.split('.')[0]" 2>/dev/null || echo 0)"
        if (( current_major >= 22 )); then
            return 0
        fi
    fi

    export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
    if [[ -s "$NVM_DIR/nvm.sh" ]]; then
        # shellcheck disable=SC1090
        . "$NVM_DIR/nvm.sh"
        for candidate in 24 22 --lts; do
            if nvm use "$candidate" >/dev/null 2>&1; then
                local nvm_major
                nvm_major="$(node -p "process.versions.node.split('.')[0]" 2>/dev/null || echo 0)"
                if (( nvm_major >= 22 )); then
                    return 0
                fi
            fi
        done
    fi

    return 1
}

SKIP_GUI=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-gui) SKIP_GUI=true; shift ;;
        *) echo "Unknown argument: $1"; exit 2 ;;
    esac
done

echo "=== CrateBay Windows Release Build ==="
echo "  Version : $VERSION"
echo "  Arch    : $ARCH"
echo "  Target  : $RUST_TARGET"
echo ""

if ! ensure_node_runtime; then
    if command -v node >/dev/null 2>&1; then
        echo "ERROR: Node.js 22+ is required. Current: $(node -v)"
    else
        echo "ERROR: Node.js 22+ is required."
    fi
    echo "Use: nvm install 24 && nvm use 24"
    exit 1
fi

# ── Step 0: Build bundled runtime assets (Windows WSL2) ──────────────────────
echo "── [0/5] Building CrateBay WSL Runtime assets ──"
bash scripts/build-runtime-assets-wsl.sh "$ARCH"

# ── Step 1: Build CLI ────────────────────────────────────────────────────────
echo "── [1/6] Building CLI (release) ──"
cargo build --release -p cratebay-cli

echo "  ✓ target/release/cratebay.exe"

# Verify binaries exist
for bin in cratebay.exe; do
    if [[ ! -f "target/release/$bin" ]]; then
        echo "ERROR: target/release/$bin not found"
        exit 1
    fi
done

echo ""
echo "── [2/6] Ensuring bundled container images ──"
if [[ "$SKIP_GUI" == "true" ]]; then
    echo "  Skipped (--skip-gui)"
else
    if bash scripts/verify-bundle-images.sh "$BUNDLE_IMAGES_DIR"; then
        echo "  ✓ bundled images already present"
    else
        echo "  Building bundled images with CLI + built-in runtime..."
        CRATEBAY_BIN="$REPO_ROOT/target/release/cratebay.exe" bash scripts/build-bundle-images.sh
    fi
fi

if [[ "$SKIP_GUI" == "true" ]]; then
    echo ""
    echo "── [3/6] Skipping frontend dependencies (--skip-gui) ──"
    echo "── [4/6] Skipping Tauri build (--skip-gui) ──"
else
    # ── Step 3: Install frontend dependencies ────────────────────────────────
    echo ""
    echo "── [3/6] Installing frontend dependencies ──"
    corepack enable
    (cd "$GUI_CRATE" && pnpm install --frozen-lockfile)

    # ── Step 4: Build Tauri app ──────────────────────────────────────────────
    echo ""
    echo "── [4/6] Building Tauri app ──"
    (cd "$GUI_CRATE" && pnpm tauri build)
fi

# ── Step 5: Collect CLI binary ───────────────────────────────────────────────
echo ""
echo "── [5/6] Collecting CLI binary ──"
mkdir -p "$DIST_DIR"

cp "target/release/cratebay.exe" "$DIST_DIR/cratebay.exe"
echo "  ✓ $DIST_DIR/cratebay.exe"

# ── Step 6: Collect GUI installers ───────────────────────────────────────────
echo ""
echo "── [6/6] Collecting GUI installers ──"

if [[ "$SKIP_GUI" == "true" ]]; then
    echo "  Skipped (--skip-gui)"
else
    FOUND_INSTALLER=false

    # Collect MSI installer
    for msi in target/release/bundle/msi/*.msi; do
        if [[ -f "$msi" ]]; then
            BASENAME="$(basename "$msi")"
            cp "$msi" "$DIST_DIR/$BASENAME"
            echo "  ✓ $DIST_DIR/$BASENAME"
            FOUND_INSTALLER=true
        fi
    done

    # Collect NSIS installer
    for nsis in target/release/bundle/nsis/*.exe; do
        if [[ -f "$nsis" ]]; then
            BASENAME="$(basename "$nsis")"
            cp "$nsis" "$DIST_DIR/$BASENAME"
            echo "  ✓ $DIST_DIR/$BASENAME"
            FOUND_INSTALLER=true
        fi
    done

    if [[ "$FOUND_INSTALLER" == "false" ]]; then
        echo "  WARNING: No MSI or NSIS installers found under target/release/bundle/"
    fi
fi

# ── Summary ──────────────────────────────────────────────────────────────────
echo ""
echo "=== Build Complete ==="
echo ""
echo "Artifacts in $DIST_DIR:"

# List artifacts with sizes
for f in "$DIST_DIR"/*; do
    if [[ -f "$f" ]]; then
        SIZE=$(du -h "$f" | awk '{print $1}')
        printf "  %-50s %s\n" "$(basename "$f")" "$SIZE"
    fi
done

echo ""
echo "Next steps:"
echo "  1. Test CLI: ./dist/cratebay.exe system info"
if [[ "$SKIP_GUI" == "false" ]]; then
    echo "  2. Install GUI: double-click the MSI or NSIS installer"
fi

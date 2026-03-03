#!/usr/bin/env bash
#
# build-release-windows.sh — Build CrateBay release artifacts for Windows
#
# Produces:
#   dist/cratebay.exe              — CLI binary
#   dist/cratebay-daemon.exe       — Daemon binary
#   dist/CrateBay_<ver>_x64.msi   — MSI installer (GUI + daemon)
#   dist/CrateBay_<ver>_x64-setup.exe — NSIS installer (GUI + daemon)
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

VERSION="0.1.0"
ARCH="x86_64"
RUST_TARGET="x86_64-pc-windows-msvc"

GUI_CRATE="crates/cratebay-gui"
TAURI_DIR="$GUI_CRATE/src-tauri"
DIST_DIR="$REPO_ROOT/dist"

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

# ── Step 1: Build daemon & CLI ───────────────────────────────────────────────
echo "── [1/5] Building daemon and CLI (release) ──"
cargo build --release -p cratebay-daemon -p cratebay-cli

echo "  ✓ target/release/cratebay.exe"
echo "  ✓ target/release/cratebay-daemon.exe"

# Verify binaries exist
for bin in cratebay.exe cratebay-daemon.exe; do
    if [[ ! -f "target/release/$bin" ]]; then
        echo "ERROR: target/release/$bin not found"
        exit 1
    fi
done

if [[ "$SKIP_GUI" == "true" ]]; then
    echo ""
    echo "── [2/5] Skipping frontend dependencies (--skip-gui) ──"
    echo "── [3/5] Skipping Tauri build (--skip-gui) ──"
else
    # ── Step 2: Install frontend dependencies ────────────────────────────────
    echo ""
    echo "── [2/5] Installing frontend dependencies ──"
    (cd "$GUI_CRATE" && npm ci)

    # ── Step 3: Build Tauri app ──────────────────────────────────────────────
    echo ""
    echo "── [3/5] Building Tauri app ──"
    (cd "$GUI_CRATE" && npx tauri build)
fi

# ── Step 4: Collect CLI & daemon binaries ────────────────────────────────────
echo ""
echo "── [4/5] Collecting CLI & daemon binaries ──"
mkdir -p "$DIST_DIR"

cp "target/release/cratebay.exe" "$DIST_DIR/cratebay.exe"
echo "  ✓ $DIST_DIR/cratebay.exe"

cp "target/release/cratebay-daemon.exe" "$DIST_DIR/cratebay-daemon.exe"
echo "  ✓ $DIST_DIR/cratebay-daemon.exe"

# ── Step 5: Collect GUI installers ───────────────────────────────────────────
echo ""
echo "── [5/5] Collecting GUI installers ──"

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
echo "  1. Test CLI:    ./dist/cratebay.exe status"
echo "  2. Test daemon: ./dist/cratebay-daemon.exe"
if [[ "$SKIP_GUI" == "false" ]]; then
    echo "  3. Install GUI: double-click the MSI or NSIS installer"
fi

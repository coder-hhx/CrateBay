#!/usr/bin/env bash
#
# build-release-macos.sh — Build CrateBay macOS release artifacts
#
# Produces:
#   dist/CrateBay_<version>_<arch>.dmg  — GUI app with built-in runtime assets
#   dist/cratebay                       — CLI binary
#
# Usage:
#   ./scripts/build-release-macos.sh
#
set -euo pipefail

# Ensure Cargo/Rust toolchain is on PATH
if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
fi

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

VERSION="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$REPO_ROOT/Cargo.toml" | head -n 1)"
if [[ -z "${VERSION}" ]]; then
    echo "ERROR: Failed to resolve version from workspace Cargo.toml"
    exit 1
fi
ARCH="$(uname -m)"                           # aarch64 or x86_64
RUST_TARGET="$(rustc -vV | grep host | awk '{print $2}')"  # e.g. aarch64-apple-darwin

GUI_CRATE="crates/cratebay-gui"
BUNDLE_IMAGES_DIR="$REPO_ROOT/$GUI_CRATE/src-tauri/bundle-images"

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

if ! ensure_node_runtime; then
    if command -v node >/dev/null 2>&1; then
        echo "ERROR: Node.js 22+ is required. Current: $(node -v)"
    else
        echo "ERROR: Node.js 22+ is required."
    fi
    echo "Use: nvm install 24 && nvm use 24"
    exit 1
fi

echo "=== CrateBay macOS Release Build ==="
echo "  Version : $VERSION"
echo "  Arch    : $ARCH"
echo "  Target  : $RUST_TARGET"
echo ""

# ── Step 0: Build bundled runtime assets (macOS) ─────────────────────────────
runtime_arch="$ARCH"
if [[ "$runtime_arch" == "arm64" ]]; then
    runtime_arch="aarch64"
fi

echo "── [0/5] Building CrateBay Runtime assets ──"
bash scripts/build-runtime-assets-alpine.sh "$runtime_arch"

# ── Step 1: Build CLI ────────────────────────────────────────────────────────
echo "── [1/6] Building CLI (release) ──"
cargo build --release -p cratebay-cli

echo "  ✓ target/release/cratebay"

# ── Step 2: Ensure bundled container images ──────────────────────────────────
echo ""
echo "── [2/6] Ensuring bundled container images ──"
if bash scripts/verify-bundle-images.sh "$BUNDLE_IMAGES_DIR"; then
    echo "  ✓ bundled images already present"
else
    echo "  Building bundled images with CLI + built-in runtime..."
    CRATEBAY_BIN="$REPO_ROOT/target/release/cratebay" bash scripts/build-bundle-images.sh
fi

# ── Step 3: Install frontend dependencies ────────────────────────────────────
echo ""
echo "── [3/6] Installing frontend dependencies ──"
corepack enable
(cd "$GUI_CRATE" && pnpm install --frozen-lockfile)

# ── Step 4: Build Tauri app ──────────────────────────────────────────────────
echo ""
echo "── [4/6] Building Tauri app ──"
(cd "$GUI_CRATE" && pnpm tauri build)

# Locate the .app bundle produced by Tauri
# Workspace builds place bundles under the workspace root target/ directory
BUNDLE_DIR="target/release/bundle/macos"
APP_BUNDLE="$(find "$BUNDLE_DIR" -name '*.app' -maxdepth 1 | head -1)"
if [ -z "$APP_BUNDLE" ]; then
    echo "ERROR: Could not find .app bundle under $BUNDLE_DIR"
    exit 1
fi
APP_NAME="$(basename "$APP_BUNDLE")"
echo "  ✓ $APP_BUNDLE"

# Verify bundle structure
echo ""
echo "  Bundle Contents/MacOS/:"
ls -la "$APP_BUNDLE/Contents/MacOS/"

# ── Step 5: Create DMG ───────────────────────────────────────────────────────
echo ""
echo "── [5/6] Creating DMG ──"
DIST_DIR="$REPO_ROOT/dist"
mkdir -p "$DIST_DIR"

DMG_NAME="CrateBay_${VERSION}_${ARCH}.dmg"
DMG_PATH="$DIST_DIR/$DMG_NAME"

# Remove old DMG if present
rm -f "$DMG_PATH"

TAURI_DMG=""
if [[ -d "target/release/bundle/dmg" ]]; then
    TAURI_DMG="$(find target/release/bundle/dmg -maxdepth 1 -name '*.dmg' | head -n 1)"
fi

if [[ -n "$TAURI_DMG" ]]; then
    cp "$TAURI_DMG" "$DMG_PATH"
else
    # Create a temporary directory for DMG contents when Tauri did not emit one.
    DMG_STAGE="$(mktemp -d)"
    cp -R "$APP_BUNDLE" "$DMG_STAGE/"
    ln -s /Applications "$DMG_STAGE/Applications"

    hdiutil create \
        -volname "CrateBay $VERSION" \
        -srcfolder "$DMG_STAGE" \
        -ov \
        -format UDZO \
        "$DMG_PATH"

    rm -rf "$DMG_STAGE"
fi
echo "  ✓ $DMG_PATH"

# ── Step 6: Collect CLI binary ───────────────────────────────────────────────
echo ""
echo "── [6/6] Collecting CLI binary ──"
cp "target/release/cratebay" "$DIST_DIR/cratebay"
echo "  ✓ $DIST_DIR/cratebay"

# ── Summary ──────────────────────────────────────────────────────────────────
echo ""
echo "=== Build Complete ==="
echo ""
echo "Artifacts:"
echo "  GUI (DMG): $DMG_PATH"
echo "  CLI:       $DIST_DIR/cratebay"
echo ""
echo "DMG size: $(du -h "$DMG_PATH" | awk '{print $1}')"
echo "CLI size: $(du -h "$DIST_DIR/cratebay" | awk '{print $1}')"
echo ""
echo "Next steps:"
echo "  1. Open the DMG and drag CrateBay to Applications"
echo "  2. Launch CrateBay from Applications"
echo "  3. Test CLI: ./dist/cratebay system info"

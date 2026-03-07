#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "desktop Tauri WebDriver smoke currently runs on Linux only"
  exit 1
fi

export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
if [[ -s "$NVM_DIR/nvm.sh" ]]; then
  # shellcheck disable=SC1090
  . "$NVM_DIR/nvm.sh"
  nvm use 24 >/dev/null 2>&1 || nvm use 22 >/dev/null 2>&1 || nvm use --lts >/dev/null 2>&1 || true
fi

if ! command -v node >/dev/null 2>&1; then
  echo "Node.js 22+ is required"
  exit 1
fi

if ! command -v tauri-driver >/dev/null 2>&1; then
  echo "tauri-driver is required in PATH"
  exit 1
fi

if ! command -v WebKitWebDriver >/dev/null 2>&1; then
  echo "WebKitWebDriver is required in PATH"
  exit 1
fi

pushd crates/cratebay-gui >/dev/null
npm ci
npm run build
popd >/dev/null

cargo build -p cratebay-gui

app_path="$repo_root/target/debug/cratebay-gui"
if [[ ! -x "$app_path" ]]; then
  echo "Built app not found at $app_path"
  exit 1
fi

tauri-driver --native-driver "$(command -v WebKitWebDriver)" > /tmp/cratebay-tauri-driver.log 2>&1 &
driver_pid=$!
trap 'kill "$driver_pid" >/dev/null 2>&1 || true' EXIT
sleep 3

CRATEBAY_DESKTOP_E2E_APP="$app_path" TAURI_DRIVER_URL="http://127.0.0.1:4444" cargo test -p cratebay-gui --test desktop_smoke -- --ignored --nocapture

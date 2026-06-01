#!/usr/bin/env bash
set -u -o pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

timestamp="$(date +"%Y%m%d-%H%M%S")"
report_dir="$repo_root/dist/release-readiness"
report_file="$report_dir/report-$timestamp.log"
mkdir -p "$report_dir"
touch "$report_file"

status=0
release_stage="${CRATEBAY_RELEASE_STAGE:-ga}"

case "$release_stage" in
  preview|ga) ;;
  *)
    echo "ERROR: Unsupported CRATEBAY_RELEASE_STAGE '$release_stage' (expected preview|ga)"
    exit 2
    ;;
esac

run_check() {
  local name="$1"
  shift

  echo
  echo "== $name =="
  echo "== $name ==" >>"$report_file"
  echo "[$(date -u +"%Y-%m-%dT%H:%M:%SZ")] $name" >>"$report_file"

  if "$@" >>"$report_file" 2>&1; then
    echo "[PASS] $name"
    echo "[PASS] $name" >>"$report_file"
  else
    echo "[FAIL] $name"
    echo "[FAIL] $name" >>"$report_file"
    status=1
  fi
}

echo "CrateBay release-readiness gate (stage=$release_stage)"
echo "Report: $report_file"

run_check "Product surface guard" ./scripts/product-surface-guard.sh
run_check "Tauri command surface guard" ./scripts/verify-tauri-command-surface.sh
run_check "Local CI gate (Rust + frontend + Playwright E2E)" ./scripts/ci-local.sh
run_check "Tauri GUI check" cargo check -p cratebay-gui
run_check "Tauri GUI tests" cargo test -p cratebay-gui
run_check "CLI + built-in runtime smoke" ./scripts/runtime-smoke-cli-only.sh
run_check "CLI + built-in runtime local registry pull smoke" ./scripts/runtime-smoke-local-registry.sh
run_check "Bundled image build and verification" ./scripts/build-bundle-images.sh

if [[ "$release_stage" == "preview" ]]; then
  run_check "Release wording guard (must not claim released)" \
    bash -c "if rg -n -i '(已发布|正式发布|已上线|正式上线|is\\s+now\\s+live|now\\s+live|officially\\s+released|already\\s+released|已经发布)' README.md README.zh.md website/index.html website/script.js; then exit 1; fi"
  run_check "Coming-soon wording guard (required)" \
    bash -c "rg -n -i '(coming\\s+soon|即将发布|即将提供|即将推出)' README.md README.zh.md website/index.html website/script.js"
else
  run_check "GA wording guard (must not claim coming soon)" \
    bash -c "if rg -n -i '(coming\\s+soon|即将发布|即将提供|即将推出)' README.md README.zh.md website/index.html website/script.js; then exit 1; fi"
fi

echo
if [[ $status -eq 0 ]]; then
  echo "Release-readiness: PASS"
else
  echo "Release-readiness: FAIL (see $report_file)"
fi

exit $status

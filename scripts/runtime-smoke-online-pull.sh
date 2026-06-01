#!/usr/bin/env bash
set -euo pipefail

# Live registry smoke for the CrateBay CLI plus built-in runtime.
#
# This verifies the real image pull path by reusing the CLI/runtime smoke with
# a registry-backed image. It is intentionally separate from
# runtime-smoke-cli-only.sh because it depends on external network access.
#
# Useful overrides:
#   CRATEBAY_ONLINE_PULL_IMAGE=busybox:latest
#   CRATEBAY_ONLINE_PULL_REGISTRY_URL=https://registry-1.docker.io/v2/

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

image="${CRATEBAY_ONLINE_PULL_IMAGE:-busybox:latest}"
registry_url="${CRATEBAY_ONLINE_PULL_REGISTRY_URL:-https://registry-1.docker.io/v2/}"

echo "== Online registry preflight =="
echo "Registry: ${registry_url}"
if ! command -v curl >/dev/null 2>&1; then
  echo "ERROR: curl is required for online registry preflight." >&2
  exit 1
fi

if ! curl --fail --silent --show-error --max-time 10 --head "$registry_url" >/dev/null; then
  echo "ERROR: registry preflight failed: ${registry_url}" >&2
  echo "Host network must be able to reach the registry before validating live pulls." >&2
  exit 1
fi

echo "== Live pull smoke image: ${image} =="
CRATEBAY_SMOKE_RUNTIME_IMAGE="$image" \
CRATEBAY_SMOKE_OFFLINE_IMAGE=0 \
  "$repo_root/scripts/runtime-smoke-cli-only.sh"

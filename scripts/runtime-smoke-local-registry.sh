#!/usr/bin/env bash
set -euo pipefail

# Local registry smoke for the CrateBay CLI plus built-in runtime.
#
# This is the fully offline pull proof: it starts a registry container inside
# the built-in runtime, then pulls the smoke image back from that registry.

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

CRATEBAY_SMOKE_LOCAL_REGISTRY=1 \
  "$repo_root/scripts/runtime-smoke-cli-only.sh"

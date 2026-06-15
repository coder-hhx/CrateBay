#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

paths=(
  README.md
  README.zh.md
  AGENTS.md
  CHANGELOG.md
  CHANGELOG.zh.md
  docs
  website
  scripts
  .github
  .githooks
  .gitignore
  .codebuddy
  .mcp.json
  .cursorrules
  .windsurfrules
  CLAUDE.md
  GEMINI.md
  crates
)

existing_paths=()
for path in "${paths[@]}"; do
  if [[ -e "$path" ]]; then
    existing_paths+=("$path")
  fi
done

if [[ ${#existing_paths[@]} -eq 0 ]]; then
  echo "Product surface guard: no paths to scan"
  exit 0
fi

forbidden_pattern='(Local AI|AI Sandbox|ChatPage|\bchat\b|\bLLM\b|\bMCP\b|Anthropic|Claude|Gemini|Ollama|cratebay-mcp|provider-canary|setup-ai|ai-runtime)'

if rg \
  --hidden \
  --line-number \
  --ignore-case \
  --glob '!scripts/product-surface-guard.sh' \
  --glob '!scripts/release/**' \
  --glob '!**/node_modules/**' \
  --glob '!**/target/**' \
  --glob '!**/dist/**' \
  --glob '!**/playwright-report/**' \
  --glob '!**/pnpm-lock.yaml' \
  --glob '!**/package-lock.json' \
  --glob '!Cargo.lock' \
  --glob '!**/*.png' \
  --glob '!**/*.ico' \
  --glob '!**/*.icns' \
  --regexp "$forbidden_pattern" \
  -- "${existing_paths[@]}"; then
  echo
  echo "Product surface guard: found removed AI/MCP/chat product wording." >&2
  echo "Keep CrateBay focused on containers, images, pods, volumes, networks, CLI, and the built-in runtime." >&2
  exit 1
fi

if [[ -d crates/cratebay-gui/src ]]; then
  if rg \
    --hidden \
    --line-number \
    --pcre2 \
    --glob '!**/locales/zh-CN.ts' \
    --glob '!**/__tests__/**' \
    --glob '!**/__mocks__/**' \
    --glob '!**/*.png' \
    --glob '!**/*.ico' \
    --glob '!**/*.icns' \
    --regexp '[\x{4e00}-\x{9fff}]' \
    -- crates/cratebay-gui/src; then
    echo
    echo "Product surface guard: found CJK text outside the Simplified Chinese locale or tests." >&2
    echo "Route GUI user-facing copy through the typed i18n locales instead of hard-coding localized strings." >&2
    exit 1
  fi
fi

echo "Product surface guard: PASS"

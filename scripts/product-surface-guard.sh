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

forbidden_pattern='(\bAI\b|\bLLM\b|\bMCP\b|Local AI|AI Sandbox|ChatPage|\bchat\b|sandbox|OpenAI|Anthropic|Claude|Gemini|Ollama|cratebay-mcp|provider-canary|setup-ai|ai-runtime)'

if rg \
  --hidden \
  --line-number \
  --ignore-case \
  --glob '!scripts/product-surface-guard.sh' \
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
  echo "Product surface guard: found removed AI/MCP/chat/sandbox product wording." >&2
  echo "Keep CrateBay focused on images, containers, pods, CLI, and the built-in runtime." >&2
  exit 1
fi

echo "Product surface guard: PASS"

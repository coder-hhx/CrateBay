# AI Skills Runtime

> Public product notes only. Detailed planning and release sequencing are maintained privately.

## Goal

Provide a stable, executable skill registry shared by:

- Assistant quick actions
- Assistant step execution adapters (`assistant_step`)
- MCP action execution (`mcp_action`)
- Agent/CLI bridge presets (`agent_cli_preset`)
- Managed sandbox actions (`sandbox_action`)

CrateBay v1.0 focuses on **CrateBay-managed sandboxes** and **direct local execution surfaces**. External sandbox compatibility remains experimental.

## Data Model

`AiSettings` includes a `skills` array. Each skill contains:

- `id`: stable unique skill id
- `display_name`: user-facing name
- `description`: short behavior summary
- `tags`: category hints for grouping or routing
- `executor`: adapter type (`assistant_step`, `mcp_action`, `agent_cli_preset`, `sandbox_action`)
- `target`: command / action / preset target
- `input_schema`: JSON-schema-like object used for runtime validation
- `enabled`: toggle flag

## Built-in Defaults

Current default entries include:

- `assistant-container-diagnose`
- `mcp-k8s-pods-read`
- `managed-sandbox-list`
- `managed-sandbox-command`
- `agent-cli-codex-prompt`
- `agent-cli-claude-prompt`
- `agent-cli-openclaw-plan`

## Runtime Guardrails

Built-in execution now includes:

- schema-based input validation before dispatch
- MCP allowlist / confirmation policy enforcement
- CLI allowlist enforcement for Agent/CLI presets
- confirmation gates for write / destructive sandbox and assistant actions
- audit logging with request ids for skill execution

## Current UI Surface

### Settings → AI

The Skills Registry now supports:

- viewing built-in skill metadata
- enabling or disabling skills
- editing per-skill input payloads
- dry-run execution for Agent/CLI skills
- direct execution through `ai_skill_execute`
- per-skill result and error panels

### AI Hub → Assistant

Assistant now supports two complementary flows:

- **Plan mode** — generate a step-by-step action plan, inspect args, then execute each step safely
- **Quick Skills** — run enabled skills directly (for example Codex / Claude prompts or managed sandbox commands) without generating a plan first

## Next Areas

Public follow-up areas after the current runtime surface:

1. richer schema keywords and stricter validation feedback
2. skill import / export for team presets
3. reusable skill packs and policy templates
4. deeper orchestration between plans, models, sandboxes, and MCP sessions

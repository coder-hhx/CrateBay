# CrateBay

Open-source, cross-platform container management with built-in AI sandbox.

CrateBay is an alternative to Docker Desktop and OrbStack — fully open-source, works on macOS/Windows/Linux, and has built-in AI code execution capabilities. Manage containers and images through a desktop GUI, or let AI agents run code safely in local sandboxes via MCP protocol.

## Why CrateBay?

- **Open source** — MIT licensed, free forever. Docker Desktop is proprietary; OrbStack is macOS-only
- **Cross-platform** — macOS, Windows, Linux. No platform lock-in
- **Built-in AI** — Chat with AI that can execute code in sandboxes. No other container tool does this
- **No Docker required** — built-in VM runtime (macOS: Virtualization.framework, Linux: KVM, Windows: WSL2)
- **MCP compatible** — connect Claude Desktop, Cursor, Windsurf to run code via MCP protocol
- **Zero cost** — no cloud bills, no usage limits, code never leaves your machine

## How It Works

```
Your AI Agent                    CrateBay                         Local VM
(Claude, Cursor, etc.)           (MCP Server)                     (Docker inside VM)
       │                              │                                │
       │  "run this Python script"    │                                │
       ├─────────────────────────────►│                                │
       │                              │  create sandbox + exec code    │
       │                              ├───────────────────────────────►│
       │                              │                                │
       │                              │  stdout/stderr + exit code     │
       │       result                 │◄───────────────────────────────┤
       │◄─────────────────────────────┤                                │
```

## Quick Start

### 1. Install

```bash
# macOS (Apple Silicon & Intel)
brew install --cask cratebay

# Or download from Releases
```

### 2. Connect to Your AI

Add to your MCP client config (e.g. Claude Desktop `claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "cratebay": {
      "command": "cratebay-mcp"
    }
  }
}
```

### 3. Use It

Tell your AI:

> "Create a Python sandbox and run: print('Hello from CrateBay')"

CrateBay handles the rest — VM startup, container creation, code execution, result delivery.

## Features

### MCP Server — Let Any AI Run Code

The `cratebay-mcp` binary exposes sandbox tools via the Model Context Protocol:

| Tool | What It Does |
|------|-------------|
| `sandbox_run_code` | Create sandbox + execute code + return result (one-shot) |
| `sandbox_create` | Create a persistent sandbox from template |
| `sandbox_exec` | Run a command in an existing sandbox |
| `sandbox_install` | Install packages (pip, npm, apt) |
| `sandbox_upload` / `sandbox_download` | Transfer files in/out of sandbox |
| `sandbox_list` | List running sandboxes |
| `sandbox_stop` / `sandbox_delete` | Lifecycle management |

### Desktop App — Visual Sandbox Management

The CrateBay desktop app provides:

- **Chat interface** — talk to an AI assistant that manages sandboxes through natural language
- **Sandbox dashboard** — see running sandboxes, resource usage, logs
- **Image management** — search, pull, and manage container images
- **MCP server management** — connect external MCP tool servers
- **Settings** — LLM provider config, runtime settings, registry mirrors

### CLI — Headless Operations

```bash
cratebay sandbox create --template python-dev
cratebay sandbox exec <id> -- python -c "print('hello')"
cratebay sandbox list
cratebay sandbox stop <id>
```

### Pre-built Sandbox Templates

| Template | Image | Use Case |
|----------|-------|----------|
| `python-dev` | Python 3.12 + pip | Data analysis, scripting, ML |
| `node-dev` | Node.js 20 + npm | Web development, scripting |
| `rust-dev` | Rust stable + cargo | Systems programming |
| `ubuntu-base` | Ubuntu 24.04 | General purpose |

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  CrateBay                                            │
│                                                      │
│  ┌──────────────┐  ┌──────────┐  ┌───────────────┐  │
│  │ cratebay-mcp │  │ GUI App  │  │ cratebay-cli  │  │
│  │ (MCP Server) │  │ (Tauri)  │  │ (CLI)         │  │
│  └──────┬───────┘  └────┬─────┘  └──────┬────────┘  │
│         └───────────────┼───────────────┘            │
│                         │                            │
│              ┌──────────▼──────────┐                 │
│              │   cratebay-core     │                 │
│              │   (Rust library)    │                 │
│              └──────────┬──────────┘                 │
│                         │                            │
│              ┌──────────▼──────────┐                 │
│              │  Built-in Runtime   │                 │
│              │  macOS: VZ.framework│                 │
│              │  Linux: KVM/QEMU   │                 │
│              │  Windows: WSL2     │                 │
│              │       ↓            │                 │
│              │  Docker in VM      │                 │
│              └─────────────────────┘                 │
└─────────────────────────────────────────────────────┘
```

**Tech stack**: Tauri v2 | React 19 | Rust | bollard | SQLite | pi-agent-core

## Compared To

| | CrateBay | Docker Desktop | OrbStack | E2B |
|---|---|---|---|---|
| Open source | MIT | No | No | Partial |
| Cross-platform | macOS/Win/Linux | macOS/Win/Linux | macOS only | Cloud |
| Container mgmt | Yes | Yes | Yes | No |
| AI chat + sandbox | Yes | No | No | API only |
| MCP support | Yes | No | No | No |
| Cost | Free | Free / $5+/mo | Free / $8/mo | $0.01/min |
| No Docker needed | Yes (built-in VM) | Is Docker | Requires Docker | N/A |

## Status

v0.9.0 → v1.0.0 — Container management + AI ChatPage with sandbox execution.

See [docs/progress.md](docs/progress.md) for detailed development status and [docs/ROADMAP.md](docs/ROADMAP.md) for the release plan.

## Contributing

This project uses [AGENTS.md](AGENTS.md) for AI-assisted development. See [docs/](docs/) for technical specs and workflow guides.

## License

[MIT](LICENSE)

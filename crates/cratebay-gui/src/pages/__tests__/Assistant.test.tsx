import { describe, it, expect, vi, beforeEach } from "vitest"
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { invoke } from "@tauri-apps/api/core"
import { Assistant } from "../Assistant"
import { messages } from "../../i18n/messages"
import type {
  AiSettings,
  AiSkillExecutionResult,
  AssistantPlanResult,
  AssistantStepExecutionResult,
} from "../../types"

const t = (key: string) => messages.en[key] || key

const mockPlan: AssistantPlanResult = {
  request_id: "ai-plan-1",
  strategy: "heuristic",
  notes: "test plan",
  fallback_used: true,
  steps: [
    {
      id: "step-1",
      title: "Stop container",
      command: "stop_container",
      args: { id: "abc123" },
      risk_level: "write",
      requires_confirmation: true,
      explain: "Stops a running container.",
    },
  ],
}

const mockAiSettings: AiSettings = {
  active_profile_id: "openai-default",
  profiles: [
    {
      id: "openai-default",
      provider_id: "openai",
      display_name: "OpenAI",
      model: "gpt-4o-mini",
      base_url: "https://api.openai.com/v1",
      api_key_ref: "cratebay.ai.openai.default",
      headers: {},
    },
  ],
  skills: [
    {
      id: "managed-sandbox-command",
      display_name: "Managed Sandbox Command",
      description: "Run a command inside a sandbox",
      tags: ["sandbox", "managed"],
      executor: "sandbox_action",
      target: "sandbox_exec",
      input_schema: {
        type: "object",
        properties: { id: { type: "string" }, command: { type: "string" } },
      },
      enabled: true,
    },
    {
      id: "agent-cli-codex-prompt",
      display_name: "Codex CLI Prompt",
      description: "Invoke codex preset",
      tags: ["agent-cli", "codex"],
      executor: "agent_cli_preset",
      target: "codex",
      input_schema: {
        type: "object",
        properties: { prompt: { type: "string" } },
      },
      enabled: true,
    },
  ],
  security_policy: {
    destructive_action_confirmation: true,
    mcp_remote_enabled: false,
    mcp_allowed_actions: ["list_containers"],
    mcp_auth_token_ref: "",
    mcp_audit_enabled: true,
    cli_command_allowlist: ["codex", "openclaw"],
  },
  mcp_servers: [],
  opensandbox: {
    enabled: false,
    base_url: "http://127.0.0.1:8080",
    config_path: "~/.cratebay/opensandbox.toml",
  },
}

describe("Assistant", () => {
  beforeEach(() => {
    vi.clearAllMocks()
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "load_ai_settings") return mockAiSettings
      return null
    })
  })

  it("routes step execution through assistant_execute_step", async () => {
    const user = userEvent.setup()
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true)
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "load_ai_settings") return mockAiSettings
      if (command === "ai_generate_plan") return mockPlan
      if (command === "assistant_execute_step") {
        const out: AssistantStepExecutionResult = {
          ok: true,
          request_id: "ai-exec-1",
          command: "stop_container",
          risk_level: "write",
          output: { ok: true },
        }
        return out
      }
      return null
    })

    render(<Assistant t={t} />)
    await screen.findByTestId("assistant-skill-card-agent-cli-codex-prompt")
    await user.type(screen.getByPlaceholderText(t("assistantPromptPlaceholder")), "stop web")
    await user.click(screen.getByRole("button", { name: t("assistantGeneratePlan") }))
    await screen.findByText("Stop container")

    await user.click(screen.getByRole("button", { name: t("assistantRunStep") }))

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("assistant_execute_step", {
        command: "stop_container",
        args: { id: "abc123" },
        riskLevel: "write",
        requiresConfirmation: true,
        confirmed: true,
      })
    )
    expect(confirmSpy).toHaveBeenCalled()
    expect(screen.getByTestId("assistant-step-result-step-1")).toHaveTextContent(
      /request_id=ai-exec-1/
    )
    expect(invoke).not.toHaveBeenCalledWith("stop_container", expect.anything())
  })

  it("does not execute when confirmation is denied", async () => {
    const user = userEvent.setup()
    vi.spyOn(window, "confirm").mockReturnValue(false)
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "load_ai_settings") return mockAiSettings
      if (command === "ai_generate_plan") return mockPlan
      return null
    })

    render(<Assistant t={t} />)
    await user.type(screen.getByPlaceholderText(t("assistantPromptPlaceholder")), "stop web")
    await user.click(screen.getByRole("button", { name: t("assistantGeneratePlan") }))
    await screen.findByText("Stop container")
    await user.click(screen.getByRole("button", { name: t("assistantRunStep") }))

    expect(invoke).not.toHaveBeenCalledWith("assistant_execute_step", expect.anything())
  })

  it("shows invalid args error for malformed step JSON", async () => {
    const user = userEvent.setup()
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "load_ai_settings") return mockAiSettings
      if (command === "ai_generate_plan") return mockPlan
      return null
    })

    render(<Assistant t={t} />)
    await user.type(screen.getByPlaceholderText(t("assistantPromptPlaceholder")), "stop web")
    await user.click(screen.getByRole("button", { name: t("assistantGeneratePlan") }))
    await screen.findByText("Stop container")

    fireEvent.change(screen.getByTestId("assistant-step-args-step-1"), {
      target: { value: "{invalid json}" },
    })
    await user.click(screen.getByRole("button", { name: t("assistantRunStep") }))

    expect(screen.getByTestId("assistant-step-result-step-1")).toHaveTextContent(/Invalid args JSON/)
    expect(invoke).not.toHaveBeenCalledWith("assistant_execute_step", expect.anything())
  })

  it("executes codex quick skill directly from assistant", async () => {
    const user = userEvent.setup()
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "load_ai_settings") return mockAiSettings
      if (command === "ai_skill_execute") {
        const out: AiSkillExecutionResult = {
          ok: true,
          skill_id: "agent-cli-codex-prompt",
          executor: "agent_cli_preset",
          target: "codex",
          request_id: "skill-1",
          output: {
            ok: true,
            command_line: "codex exec summarize repo",
          },
        }
        return out
      }
      return null
    })

    render(<Assistant t={t} />)
    const card = await screen.findByTestId("assistant-skill-card-agent-cli-codex-prompt")
    await user.clear(within(card).getByTestId("assistant-skill-input-agent-cli-codex-prompt"))
    await user.type(
      within(card).getByTestId("assistant-skill-input-agent-cli-codex-prompt"),
      "summarize repo"
    )
    await user.click(within(card).getByRole("button", { name: t("aiSkillRun") }))

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("ai_skill_execute", {
        skillId: "agent-cli-codex-prompt",
        input: { prompt: "summarize repo" },
        dryRun: true,
        confirmed: null,
      })
    )
    expect(screen.getByTestId("assistant-skill-result-agent-cli-codex-prompt")).toHaveTextContent(
      "codex exec summarize repo"
    )
  })

  it("executes sandbox quick skill with confirmation", async () => {
    const user = userEvent.setup()
    vi.spyOn(window, "confirm").mockReturnValue(true)
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "load_ai_settings") return mockAiSettings
      if (command === "ai_skill_execute") {
        const out: AiSkillExecutionResult = {
          ok: true,
          skill_id: "managed-sandbox-command",
          executor: "sandbox_action",
          target: "sandbox_exec",
          request_id: "skill-2",
          output: { ok: true, output: "hello from sandbox" },
        }
        return out
      }
      return null
    })

    render(<Assistant t={t} />)
    const card = await screen.findByTestId("assistant-skill-card-managed-sandbox-command")
    fireEvent.change(within(card).getByTestId("assistant-skill-input-managed-sandbox-command"), {
      target: { value: '{"id":"sandbox-1","command":"echo hello from sandbox"}' },
    })
    await user.click(within(card).getByRole("button", { name: t("aiSkillRun") }))

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("ai_skill_execute", {
        skillId: "managed-sandbox-command",
        input: { id: "sandbox-1", command: "echo hello from sandbox" },
        dryRun: null,
        confirmed: true,
      })
    )
    expect(screen.getByTestId("assistant-skill-result-managed-sandbox-command")).toHaveTextContent(
      "hello from sandbox"
    )
  })
})

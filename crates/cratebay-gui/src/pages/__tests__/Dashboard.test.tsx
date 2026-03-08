import { describe, it, expect, vi, beforeEach } from "vitest"
import { render, screen, within, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { invoke } from "@tauri-apps/api/core"
import { Dashboard } from "../Dashboard"
import { messages } from "../../i18n/messages"
import type {
  AiSettings,
  ContainerInfo,
  GpuStatusDto,
  McpServerStatusDto,
  OllamaModelDto,
  OllamaStatusDto,
  SandboxInfoDto,
  SandboxRuntimeUsageDto,
} from "../../types"

const t = (key: string) => messages.en[key] || key

const mockContainer = (overrides?: Partial<ContainerInfo>): ContainerInfo => ({
  id: "abc123",
  name: "web-server",
  image: "nginx:latest",
  state: "running",
  status: "Up 2 hours",
  ports: "0.0.0.0:80->80/tcp",
  ...overrides,
})

const mockSandbox = (overrides?: Partial<SandboxInfoDto>): SandboxInfoDto => ({
  id: "sandbox-1",
  short_id: "sandbox-1",
  name: "Agent Sandbox",
  image: "python:3.12",
  state: "running",
  status: "Up 5 minutes",
  template_id: "python-agent",
  owner: "test",
  created_at: "2026-03-06T00:00:00Z",
  expires_at: "2026-03-06T08:00:00Z",
  ttl_hours: 8,
  cpu_cores: 2,
  memory_mb: 2048,
  is_expired: false,
  ...overrides,
})

const baseOllamaStatus: OllamaStatusDto = {
  installed: true,
  running: true,
  version: "0.6.2",
  base_url: "http://127.0.0.1:11434",
}

const baseModels: OllamaModelDto[] = [
  {
    name: "qwen2.5:7b",
    size_bytes: 7516192768,
    size_human: "7.0 GB",
    modified_at: "2026-03-06T01:00:00Z",
    digest: "sha256:qwen",
    family: "qwen2.5",
    parameter_size: "7B",
    quantization_level: "Q4_K_M",
  },
  {
    name: "llama3.2:3b",
    size_bytes: 2147483648,
    size_human: "2.0 GB",
    modified_at: "2026-03-06T02:00:00Z",
    digest: "sha256:llama",
    family: "llama3.2",
    parameter_size: "3B",
    quantization_level: "Q4_K_M",
  },
]

const baseMcpServers: McpServerStatusDto[] = [
  {
    id: "filesystem",
    name: "Filesystem MCP",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
    env: [],
    working_dir: "/tmp",
    enabled: true,
    notes: "",
    running: true,
    status: "running",
    started_at: "2026-03-06T03:00:00Z",
    exit_code: null,
  },
]

const baseAiSettings: AiSettings = {
  active_profile_id: "openai-default",
  profiles: [
    {
      id: "openai-default",
      provider_id: "openai",
      display_name: "OpenAI Default",
      model: "gpt-5-mini",
      base_url: "https://api.openai.com/v1",
      api_key_ref: "openai",
      headers: {},
    },
    {
      id: "anthropic-default",
      provider_id: "anthropic",
      display_name: "Anthropic Default",
      model: "claude-sonnet",
      base_url: "https://api.anthropic.com",
      api_key_ref: "anthropic",
      headers: {},
    },
  ],
  skills: [],
  security_policy: {
    destructive_action_confirmation: true,
    mcp_remote_enabled: false,
    mcp_allowed_actions: [],
    mcp_auth_token_ref: "",
    mcp_audit_enabled: true,
    cli_command_allowlist: ["codex", "claude"],
  },
  mcp_servers: baseMcpServers,
  opensandbox: {
    enabled: true,
    base_url: "http://127.0.0.1:8080",
    config_path: "/Users/test/.cratebay/opensandbox.toml",
  },
}

const baseGpuStatus: GpuStatusDto = {
  available: true,
  utilization_supported: true,
  backend: "nvidia-smi",
  message: "Live GPU telemetry is available for 1 device(s).",
  devices: [
    {
      index: 0,
      name: "NVIDIA RTX 4090",
      utilization_percent: 62,
      memory_used_bytes: 6442450944,
      memory_total_bytes: 25769803776,
      memory_used_human: "6.0 GB",
      memory_total_human: "24.0 GB",
      temperature_celsius: 58,
      power_watts: 210.5,
    },
  ],
}

const baseSandboxRuntimeUsage: SandboxRuntimeUsageDto = {
  running: true,
  cpu_percent: 18.5,
  memory_usage_mb: 256,
  memory_limit_mb: 1024,
  memory_percent: 25,
  network_rx_bytes: 1024,
  network_tx_bytes: 2048,
  gpu_attribution_supported: true,
  gpu_message: "Matched 2 GPU process(es) across 1 device(s).",
  gpu_processes: [
    {
      gpu_index: 0,
      gpu_name: "NVIDIA RTX 4090",
      pid: 4242,
      process_name: "python",
      memory_used_bytes: 2147483648,
      memory_used_human: "2.0 GB",
    },
    {
      gpu_index: 0,
      gpu_name: "NVIDIA RTX 4090",
      pid: 4243,
      process_name: "uvicorn",
      memory_used_bytes: 1073741824,
      memory_used_human: "1.0 GB",
    },
  ],
  gpu_memory_used_bytes: 3221225472,
  gpu_memory_used_human: "3.0 GB",
}

const defaultProps = {
  containers: [] as ContainerInfo[],
  running: [] as ContainerInfo[],
  imgResultsCount: 0,
  installedImagesCount: 0,
  volumesCount: 0,
  onNavigate: vi.fn(),
  onOpenAiTab: vi.fn(),
  onOpenSettingsTab: vi.fn(),
  t,
}

function setupInvoke({
  sandboxes = [mockSandbox()],
  models = baseModels,
  gpu = baseGpuStatus,
  mcpServers = baseMcpServers,
  aiSettings = baseAiSettings,
  ollamaStatus = baseOllamaStatus,
}: {
  sandboxes?: SandboxInfoDto[]
  models?: OllamaModelDto[]
  gpu?: GpuStatusDto | null
  mcpServers?: McpServerStatusDto[]
  aiSettings?: AiSettings | null
  ollamaStatus?: OllamaStatusDto | null
} = {}) {
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "container_stats") {
      return {
        cpu_percent: 25,
        memory_usage_mb: 128,
        memory_limit_mb: 512,
        memory_percent: 25,
        network_rx_bytes: 0,
        network_tx_bytes: 0,
      }
    }
    if (command === "sandbox_list") return sandboxes
    if (command === "ollama_status") return ollamaStatus
    if (command === "ollama_list_models") return models
    if (command === "mcp_list_servers") return mcpServers
    if (command === "load_ai_settings") return aiSettings
    if (command === "gpu_status") return gpu
    if (command === "sandbox_runtime_usage") {
      const { id } = args as { id: string }
      if (id === "sandbox-1") return baseSandboxRuntimeUsage
      return {
        ...baseSandboxRuntimeUsage,
        gpu_processes: [],
        gpu_memory_used_bytes: 0,
        gpu_memory_used_human: "0 B",
      }
    }
    return null
  })
}

describe("Dashboard", () => {
  beforeEach(() => {
    vi.clearAllMocks()
    setupInvoke()
  })

  it("renders AI-first overview cards", async () => {
    render(<Dashboard {...defaultProps} />)

    expect(screen.getByText(t("dashboardAiOverview"))).toBeInTheDocument()
    expect(screen.getByTestId("dashboard-card-sandboxes")).toBeInTheDocument()
    expect(screen.getByTestId("dashboard-card-models")).toBeInTheDocument()
    expect(screen.getByTestId("dashboard-card-mcp")).toBeInTheDocument()
    expect(screen.getByTestId("dashboard-card-ai-settings")).toBeInTheDocument()

    await waitFor(() => {
      expect(within(screen.getByTestId("dashboard-card-sandboxes")).getByText("1")).toBeInTheDocument()
      expect(within(screen.getByTestId("dashboard-card-models")).getByText("2")).toBeInTheDocument()
      expect(within(screen.getByTestId("dashboard-card-ai-settings")).getByText("2")).toBeInTheDocument()
    })

    expect(screen.queryByTestId("dashboard-card-vms")).not.toBeInTheDocument()
  })

  it("deep-links primary cards into AI Hub and AI settings tabs", async () => {
    const user = userEvent.setup()
    const onOpenAiTab = vi.fn()
    const onOpenSettingsTab = vi.fn()
    render(
      <Dashboard
        {...defaultProps}
        onOpenAiTab={onOpenAiTab}
        onOpenSettingsTab={onOpenSettingsTab}
      />
    )

    await user.click(screen.getByTestId("dashboard-card-sandboxes"))
    await user.click(screen.getByTestId("dashboard-card-models"))
    await user.click(screen.getByTestId("dashboard-card-mcp"))
    await user.click(screen.getByTestId("dashboard-card-ai-settings"))

    expect(onOpenAiTab).toHaveBeenCalledWith("sandboxes")
    expect(onOpenAiTab).toHaveBeenCalledWith("models")
    expect(onOpenAiTab).toHaveBeenCalledWith("mcp")
    expect(onOpenSettingsTab).toHaveBeenCalledWith("ai")
  })

  it("keeps containers, images, and volumes as secondary runtime cards", async () => {
    const containers = [mockContainer(), mockContainer({ id: "def456", name: "api" })]
    render(
      <Dashboard
        {...defaultProps}
        containers={containers}
        running={containers}
        installedImagesCount={8}
        imgResultsCount={15}
        volumesCount={3}
      />
    )

    expect(screen.getByText(t("dashboardEngineOverview"))).toBeInTheDocument()
    expect(within(screen.getByTestId("dashboard-card-containers")).getByText("2")).toBeInTheDocument()
    expect(within(screen.getByTestId("dashboard-card-images")).getByText("8")).toBeInTheDocument()
    expect(within(screen.getByTestId("dashboard-card-volumes")).getByText("3")).toBeInTheDocument()
    expect(within(screen.getByTestId("dashboard-card-images")).getByText(new RegExp(`15\\s+${t("searchResults")}`, "i"))).toBeInTheDocument()
  })

  it("shows CPU, memory, and GPU runtime overview", async () => {
    const running = [mockContainer()]
    render(<Dashboard {...defaultProps} containers={running} running={running} />)

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("container_stats", { id: "abc123" })
    })

    const gpuCard = screen.getByTestId("dashboard-metric-gpu")
    expect(screen.getByTestId("dashboard-metric-cpu")).toBeInTheDocument()
    expect(screen.getByTestId("dashboard-metric-memory")).toBeInTheDocument()
    expect(within(gpuCard).getByText(/1 gpu devices/i)).toBeInTheDocument()
    expect(within(gpuCard).getByText(/62% .* 6 GB \/ 24 GB .* 1 sandboxes .* 2 gpu processes .* 3 GB/i)).toBeInTheDocument()
  })

  it("shows running sandboxes in the active AI workloads panel", async () => {
    const sandboxes = [
      mockSandbox(),
      mockSandbox({ id: "sandbox-2", short_id: "sandbox-2", name: "Codegen Sandbox", template_id: "node-agent" }),
    ]
    setupInvoke({ sandboxes })

    render(<Dashboard {...defaultProps} />)

    await waitFor(() => {
      expect(screen.getAllByTestId("dashboard-sandbox-item")).toHaveLength(2)
    })
    expect(screen.getByText("Agent Sandbox")).toBeInTheDocument()
    expect(screen.getByText("Codegen Sandbox")).toBeInTheDocument()
  })

  it("deep-links empty AI workload state to sandboxes tab", async () => {
    const user = userEvent.setup()
    const onOpenAiTab = vi.fn()
    setupInvoke({ sandboxes: [mockSandbox({ state: "exited", status: "Exited" })] })

    render(<Dashboard {...defaultProps} onOpenAiTab={onOpenAiTab} />)

    expect(await screen.findByText(t("dashboardNoAiWorkloads"))).toBeInTheDocument()
    await user.click(screen.getByRole("button", { name: t("dashboardOpenAiHub") }))
    expect(onOpenAiTab).toHaveBeenCalledWith("sandboxes")
  })
})

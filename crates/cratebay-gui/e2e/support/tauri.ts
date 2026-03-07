import type { Page } from "@playwright/test"

type Json = null | boolean | number | string | Json[] | { [key: string]: Json }

type MockState = {
  appVersion: string
  containers: Array<Record<string, Json>>
  containerEnv: Record<string, Array<Record<string, Json>>>
  containerLogs: Record<string, string>
  localImages: Array<Record<string, Json>>
  imageSearchResults: Array<Record<string, Json>>
  imageTags: Record<string, string[]>
  volumes: Array<Record<string, Json>>
  osImages: Array<Record<string, Json>>
  vms: Array<Record<string, Json>>
  k3sStatus: Record<string, Json>
  namespaces: string[]
  pods: Array<Record<string, Json>>
  services: Array<Record<string, Json>>
  deployments: Array<Record<string, Json>>
  podLogs: Record<string, string>
  ollamaStatus: Record<string, Json>
  ollamaStorage: Record<string, Json>
  ollamaModels: Array<Record<string, Json>>
  sandboxTemplates: Array<Record<string, Json>>
  sandboxes: Array<Record<string, Json>>
  sandboxAudit: Array<Record<string, Json>>
  sandboxExecOutput: Record<string, string>
  mcpServers: Array<Record<string, Json>>
  mcpLogs: Record<string, string[]>
  aiSettings: Record<string, Json>
  agentPresets: Array<Record<string, Json>>
  updates: Record<string, Json>
  secretExists: Record<string, boolean>
  invokeCalls: Array<Record<string, Json>>
  counters: Record<string, number>
}

type InstallOptions = {
  confirmResult?: boolean
  state?: Partial<MockState>
}

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T

const mergeState = <T extends Record<string, unknown>>(base: T, patch?: Partial<T>): T => {
  if (!patch) return clone(base)
  const next = clone(base)
  for (const [key, value] of Object.entries(patch)) {
    if (value === undefined) continue
    ;(next as Record<string, unknown>)[key] = clone(value)
  }
  return next
}

export const defaultMockState = (): MockState => ({
  appVersion: "0.8.0",
  containers: [
    {
      id: "ctr-web",
      name: "web",
      image: "nginx:latest",
      state: "running",
      status: "Up 2 minutes",
      ports: "0.0.0.0:8080->80/tcp",
    },
  ],
  containerEnv: {
    "ctr-web": [{ key: "NODE_ENV", value: "development" }],
  },
  containerLogs: {
    "ctr-web": "web server booted\nready on :80",
  },
  localImages: [
    {
      id: "img-nginx",
      repo_tags: ["nginx:latest"],
      size_bytes: 145000000,
      size_human: "145 MB",
      created: 1709800000,
    },
  ],
  imageSearchResults: [
    {
      source: "docker_hub",
      reference: "redis:7",
      description: "Redis cache",
      stars: 12000,
      pulls: 1000000,
      official: true,
    },
    {
      source: "docker_hub",
      reference: "postgres:16",
      description: "PostgreSQL database",
      stars: 9000,
      pulls: 800000,
      official: true,
    },
  ],
  imageTags: {
    "redis:7": ["7", "7.2", "latest"],
    "postgres:16": ["16", "latest"],
  },
  volumes: [
    {
      name: "app-data",
      driver: "local",
      mountpoint: "/var/lib/docker/volumes/app-data/_data",
      created_at: "2026-03-07T00:00:00Z",
      labels: { app: "cratebay" },
      options: {},
      scope: "local",
    },
  ],
  osImages: [
    {
      id: "ubuntu-22.04",
      name: "Ubuntu",
      version: "22.04",
      arch: "x86_64",
      size_bytes: 512000000,
      status: "ready",
      default_cmdline: "",
    },
  ],
  vms: [
    {
      id: "vm-dev",
      name: "dev-vm",
      state: "stopped",
      cpus: 2,
      memory_mb: 4096,
      disk_gb: 40,
      rosetta_enabled: false,
      mounts: [],
      port_forwards: [],
      os_image: "ubuntu-22.04",
    },
  ],
  k3sStatus: {
    installed: true,
    running: true,
    version: "v1.31.4+k3s1",
    node_count: 1,
    kubeconfig_path: "/Users/test/.cratebay/k3s/kubeconfig.yaml",
  },
  namespaces: ["default", "kube-system"],
  pods: [
    {
      name: "api-6d8d6f9c7c-z9q2w",
      namespace: "default",
      status: "Running",
      ready: "1/1",
      restarts: 0,
      age: "4m",
    },
  ],
  services: [
    {
      name: "api",
      namespace: "default",
      service_type: "ClusterIP",
      cluster_ip: "10.43.0.10",
      ports: "80/TCP",
    },
  ],
  deployments: [
    {
      name: "api",
      namespace: "default",
      ready: "1/1",
      up_to_date: 1,
      available: 1,
      age: "4m",
    },
  ],
  podLogs: {
    "default/api-6d8d6f9c7c-z9q2w": "server started\nhealth check ok",
  },
  ollamaStatus: {
    installed: true,
    running: true,
    version: "0.6.2",
    base_url: "http://127.0.0.1:11434",
  },
  ollamaStorage: {
    path: "/Users/test/.ollama/models",
    exists: true,
    model_count: 1,
    total_size_bytes: 7516192768,
    total_size_human: "7.0 GB",
  },
  ollamaModels: [
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
  ],
  sandboxTemplates: [
    {
      id: "node-dev",
      name: "Node Dev",
      description: "Node sandbox",
      image: "node:22",
      default_command: "sleep infinity",
      cpu_default: 2,
      memory_mb_default: 2048,
      ttl_hours_default: 8,
      tags: ["node"],
    },
  ],
  sandboxes: [],
  sandboxAudit: [],
  sandboxExecOutput: {},
  mcpServers: [
    {
      id: "local-mcp-1",
      name: "Filesystem MCP",
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
      env: ["DEBUG=1"],
      working_dir: "/tmp",
      enabled: true,
      notes: "local dev",
      running: false,
      status: "stopped",
      pid: null,
      started_at: "",
      exit_code: null,
    },
  ],
  mcpLogs: {
    "local-mcp-1": ["filesystem server ready"],
  },
  aiSettings: {
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
        id: "managed-sandbox-list",
        display_name: "Managed Sandbox List",
        description: "List managed sandboxes",
        tags: ["sandbox", "managed"],
        executor: "assistant_step",
        target: "sandbox_list",
        input_schema: { type: "object", properties: {}, additionalProperties: false },
        enabled: true,
      },
      {
        id: "managed-sandbox-command",
        display_name: "Managed Sandbox Command",
        description: "Run a command inside a sandbox",
        tags: ["sandbox", "managed"],
        executor: "sandbox_action",
        target: "sandbox_exec",
        input_schema: {
          type: "object",
          properties: {
            id: { type: "string", minLength: 1 },
            command: { type: "string", minLength: 1 },
          },
          required: ["id", "command"],
          additionalProperties: false,
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
          properties: { prompt: { type: "string", minLength: 1 } },
          required: ["prompt"],
          additionalProperties: false,
        },
        enabled: true,
      },
      {
        id: "agent-cli-claude-prompt",
        display_name: "Claude Code Prompt",
        description: "Invoke claude preset",
        tags: ["agent-cli", "claude"],
        executor: "agent_cli_preset",
        target: "claude",
        input_schema: {
          type: "object",
          properties: { prompt: { type: "string", minLength: 1 } },
          required: ["prompt"],
          additionalProperties: false,
        },
        enabled: true,
      },
      {
        id: "agent-cli-openclaw-plan",
        display_name: "OpenClaw CLI Plan",
        description: "Invoke openclaw preset for planning",
        tags: ["agent-cli", "openclaw"],
        executor: "agent_cli_preset",
        target: "openclaw",
        input_schema: {
          type: "object",
          properties: { prompt: { type: "string", minLength: 1 } },
          required: ["prompt"],
          additionalProperties: false,
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
      cli_command_allowlist: ["codex", "claude", "openclaw"],
    },
    mcp_servers: [
      {
        id: "local-mcp-1",
        name: "Filesystem MCP",
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
        env: ["DEBUG=1"],
        working_dir: "/tmp",
        enabled: true,
        notes: "local dev",
      },
    ],
    opensandbox: {
      enabled: false,
      base_url: "http://127.0.0.1:8080",
      config_path: "~/.cratebay/opensandbox.toml",
    },
  },
  agentPresets: [
    {
      id: "codex",
      name: "Codex CLI",
      description: "Codex CLI preset",
      command: "codex",
      args_template: ["exec", "{{prompt}}"],
      timeout_sec: 180,
      dangerous: false,
    },
    {
      id: "claude",
      name: "Claude Code",
      description: "Claude Code preset",
      command: "claude",
      args_template: ["-p", "{{prompt}}"],
      timeout_sec: 180,
      dangerous: false,
    },
    {
      id: "openclaw",
      name: "OpenClaw CLI",
      description: "OpenClaw CLI preset",
      command: "openclaw",
      args_template: ["run", "--prompt", "{{prompt}}"],
      timeout_sec: 180,
      dangerous: false,
    },
  ],
  updates: {
    available: false,
    current_version: "0.8.0",
    latest_version: "0.8.0",
    release_notes: "",
    download_url: "https://github.com/coder-hhx/CrateBay/releases",
  },
  secretExists: {
    "cratebay.ai.openai.default": true,
  },
  invokeCalls: [],
  counters: {
    container: 1,
    image: 1,
    volume: 1,
    sandbox: 1,
    vm: 1,
    request: 1,
  },
})

export async function installTauriMock(page: Page, options: InstallOptions = {}) {
  const state = mergeState(defaultMockState(), options.state)
  const confirmResult = options.confirmResult ?? true
  await page.addInitScript(
    ({ initialState, initialConfirmResult }) => {
      const clone = (value: unknown) => JSON.parse(JSON.stringify(value))
      const state = clone(initialState) as MockState & Record<string, unknown>
      let confirmResult = initialConfirmResult
      const callbacks = new Map<number, { callback: (payload: unknown) => void; once: boolean }>()
      const listeners = new Map<string, Array<{ eventId: number; handlerId: number }>>()
      let callbackSeq = 1
      let eventSeq = 1

      const nextCounter = (key: string) => {
        const counters = state.counters as Record<string, number>
        counters[key] = (counters[key] ?? 0) + 1
        return counters[key]
      }
      const nextRequestId = () => `e2e-${nextCounter("request")}`
      const defaultStats = {
        cpu_percent: 3.2,
        memory_usage_mb: 128,
        memory_limit_mb: 1024,
        memory_percent: 12.5,
        network_rx_bytes: 4096,
        network_tx_bytes: 8192,
        disk_usage_gb: 2.5,
      }
      const findSkill = (skillId: string) =>
        ((state.aiSettings as Record<string, unknown>).skills as Array<Record<string, unknown>>).find(
          (skill) => skill.id === skillId
        )
      const schemaValueType = (value: unknown) => {
        if (value === null) return "null"
        if (Array.isArray(value)) return "array"
        return typeof value
      }
      const normalizeSkillInput = (skill: Record<string, unknown>, input: unknown) => {
        if (skill.executor !== "agent_cli_preset") return input
        if (typeof input === "string") {
          return { prompt: input.trim() }
        }
        if (input && typeof input === "object" && !Array.isArray(input)) {
          const next = clone(input) as Record<string, unknown>
          if (typeof next.prompt === "string") next.prompt = next.prompt.trim()
          return next
        }
        return input
      }
      const validateSkillInput = (path: string, value: unknown, schema: unknown): void => {
        if (!schema || typeof schema !== "object" || Array.isArray(schema)) return
        const schemaObj = schema as Record<string, unknown>
        if (Object.keys(schemaObj).length === 0) return
        const expectedType = typeof schemaObj.type === "string" ? schemaObj.type : null
        if (expectedType === "object") {
          if (!value || typeof value !== "object" || Array.isArray(value)) {
            throw new Error(`${path} must be an object, got ${schemaValueType(value)}`)
          }
          const valueObj = value as Record<string, unknown>
          const properties = schemaObj.properties && typeof schemaObj.properties === "object" && !Array.isArray(schemaObj.properties)
            ? (schemaObj.properties as Record<string, unknown>)
            : {}
          const required = Array.isArray(schemaObj.required) ? schemaObj.required : []
          for (const field of required) {
            if (typeof field === "string" && !(field in valueObj)) {
              throw new Error(`${path}.${field} is required`)
            }
          }
          const additionalProperties = schemaObj.additionalProperties !== false
          if (!additionalProperties) {
            for (const key of Object.keys(valueObj)) {
              if (!(key in properties)) {
                throw new Error(`${path}.${key} is not allowed`)
              }
            }
          }
          for (const [key, propertySchema] of Object.entries(properties)) {
            if (key in valueObj) {
              validateSkillInput(`${path}.${key}`, valueObj[key], propertySchema)
            }
          }
          return
        }
        if (expectedType === "string") {
          if (typeof value !== "string") {
            throw new Error(`${path} must be a string, got ${schemaValueType(value)}`)
          }
          const minLength = typeof schemaObj.minLength === "number" ? schemaObj.minLength : 0
          if (value.length < minLength) {
            throw new Error(`${path} must be at least ${minLength} characters`)
          }
          return
        }
      }
      const emitEvent = (event: string, payload: unknown) => {
        const entries = listeners.get(event) ?? []
        for (const item of entries) {
          const callback = callbacks.get(item.handlerId)
          if (!callback) continue
          callback.callback({ event, id: item.eventId, payload })
          if (callback.once) callbacks.delete(item.handlerId)
        }
      }
      const listContainers = () => clone(state.containers)
      const createContainer = (image: string, name?: string | null) => {
        const index = nextCounter("container")
        const container = {
          id: `ctr-${index}`,
          name: name?.trim() || `${image.split(":")[0].replace(/[^a-z0-9-]/gi, "-")}-${index}`,
          image,
          state: "running",
          status: "Up less than a second",
          ports: "",
        }
        ;(state.containers as Array<Record<string, unknown>>).unshift(container)
        ;(state.containerEnv as Record<string, unknown>)[container.id] = []
        ;(state.containerLogs as Record<string, unknown>)[container.id] = `container ${container.name} booted`
        return container
      }
      const sandboxInspect = (id: string) => {
        const sandbox = (state.sandboxes as Array<Record<string, unknown>>).find((item) => item.id === id)
        if (!sandbox) throw new Error(`Sandbox not found: ${id}`)
        return {
          id: sandbox.id,
          short_id: sandbox.short_id,
          name: sandbox.name,
          image: sandbox.image,
          template_id: sandbox.template_id,
          owner: sandbox.owner,
          created_at: sandbox.created_at,
          expires_at: sandbox.expires_at,
          ttl_hours: sandbox.ttl_hours,
          cpu_cores: sandbox.cpu_cores,
          memory_mb: sandbox.memory_mb,
          running: sandbox.state === "running",
          command: sandbox.command,
          env: sandbox.env,
        }
      }
      const handleInvoke = async (cmd: string, args: Record<string, unknown> = {}) => {
        ;(state.invokeCalls as Array<Record<string, unknown>>).push({ cmd, args: clone(args) })

        switch (cmd) {
          case "set_window_theme":
          case "open_release_page":
          case "plugin:window|set_size":
          case "plugin:window|set_position":
          case "plugin:window|maximize":
          case "plugin:window|toggle_maximize":
          case "plugin:window|minimize":
          case "plugin:window|close":
            return null
          case "plugin:window|is_maximized":
            return false
          case "plugin:window|inner_size":
            return { width: 1600, height: 1100 }
          case "plugin:window|scale_factor":
            return 1
          case "plugin:event|listen": {
            const event = String(args.event ?? "")
            const handlerId = Number(args.handler)
            const eventId = eventSeq++
            const items = listeners.get(event) ?? []
            items.push({ eventId, handlerId })
            listeners.set(event, items)
            return eventId
          }
          case "plugin:event|unlisten": {
            const event = String(args.event ?? "")
            const eventId = Number(args.eventId)
            const items = (listeners.get(event) ?? []).filter((item) => item.eventId !== eventId)
            listeners.set(event, items)
            return null
          }
          case "plugin:event|emit":
            emitEvent(String(args.event ?? ""), args.payload)
            return null
          case "plugin:event|emit_to":
            emitEvent(String(args.event ?? ""), args.payload)
            return null
          case "plugin:dialog|open":
            return "/tmp/mock-import.tar"
          case "plugin:dialog|save":
            return "/tmp/mock-export.json"
          case "check_update":
            return clone(state.updates)
          case "docker_runtime_quick_setup":
            return { ok: true, request_id: nextRequestId(), message: "Docker runtime ready" }
          case "list_containers":
            return listContainers()
          case "container_stats":
            return {
              cpu_percent: defaultStats.cpu_percent,
              memory_usage_mb: defaultStats.memory_usage_mb,
              memory_limit_mb: defaultStats.memory_limit_mb,
              memory_percent: defaultStats.memory_percent,
              network_rx_bytes: defaultStats.network_rx_bytes,
              network_tx_bytes: defaultStats.network_tx_bytes,
            }
          case "start_container": {
            const item = (state.containers as Array<Record<string, unknown>>).find((container) => container.id === args.id)
            if (item) {
              item.state = "running"
              item.status = "Up less than a second"
            }
            return null
          }
          case "stop_container": {
            const item = (state.containers as Array<Record<string, unknown>>).find((container) => container.id === args.id)
            if (item) {
              item.state = "exited"
              item.status = "Exited (0) 1 second ago"
            }
            return null
          }
          case "remove_container": {
            state.containers = (state.containers as Array<Record<string, unknown>>).filter((container) => container.id !== args.id)
            return null
          }
          case "container_env":
            return clone((state.containerEnv as Record<string, unknown>)[String(args.id)] ?? [])
          case "container_logs":
            return String((state.containerLogs as Record<string, unknown>)[String(args.id)] ?? "")
          case "container_logs_stream":
            emitEvent(`container-log:${String(args.id)}`, { line: String((state.containerLogs as Record<string, unknown>)[String(args.id)] ?? "") })
            return null
          case "container_logs_stream_stop":
            return null
          case "container_exec":
            return `Executed in ${String(args.containerId)}: ${String(args.command ?? "")}`
          case "container_exec_interactive_cmd":
            return `docker exec -it ${String(args.containerId)} /bin/sh`
          case "container_login_cmd":
            return `docker exec -it ${String(args.container)} ${String(args.shell ?? "/bin/sh")}`
          case "image_list":
            return clone(state.localImages)
          case "image_remove": {
            state.localImages = (state.localImages as Array<Record<string, unknown>>).filter((image) => image.id !== args.id)
            return null
          }
          case "image_inspect": {
            const image = (state.localImages as Array<Record<string, unknown>>).find((item) => item.id === args.id)
            const repoTags = Array.isArray(image?.repo_tags) ? image?.repo_tags : [String(args.id)]
            return {
              id: args.id,
              repo_tags: repoTags,
              size_bytes: image?.size_bytes ?? 145000000,
              created: "2026-03-07T00:00:00Z",
              architecture: "amd64",
              os: "linux",
              docker_version: "27.0.0",
              layers: 6,
            }
          }
          case "image_tag":
            return null
          case "image_search": {
            const query = String(args.query ?? "").toLowerCase()
            return clone((state.imageSearchResults as Array<Record<string, unknown>>).filter((item) => String(item.reference).toLowerCase().includes(query)))
          }
          case "image_tags":
            return clone((state.imageTags as Record<string, unknown>)[String(args.reference)] ?? ["latest"])
          case "docker_run": {
            const result = createContainer(String(args.image ?? "nginx:latest"), (args.name as string | null) ?? null)
            return {
              id: result.id,
              name: result.name,
              image: result.image,
              login_cmd: `docker exec -it ${result.name} /bin/sh`,
            }
          }
          case "image_load":
            return "Loaded image archive"
          case "image_push":
            return `Pushed ${String(args.reference ?? "")}`
          case "volume_list":
            return clone(state.volumes)
          case "volume_create": {
            const volume = {
              name: String(args.name ?? `volume-${nextCounter("volume")}`),
              driver: String(args.driver ?? "local"),
              mountpoint: `/var/lib/docker/volumes/${String(args.name)}/_data`,
              created_at: "2026-03-07T00:00:00Z",
              labels: {},
              options: {},
              scope: "local",
            }
            ;(state.volumes as Array<Record<string, unknown>>).unshift(volume)
            return clone(volume)
          }
          case "volume_inspect": {
            const volume = (state.volumes as Array<Record<string, unknown>>).find((item) => item.name === args.name)
            if (!volume) throw new Error(`Volume not found: ${String(args.name)}`)
            return clone(volume)
          }
          case "volume_remove": {
            state.volumes = (state.volumes as Array<Record<string, unknown>>).filter((item) => item.name !== args.name)
            return null
          }
          case "vm_list":
            return clone(state.vms)
          case "vm_stats":
            return {
              cpu_percent: defaultStats.cpu_percent,
              memory_usage_mb: defaultStats.memory_usage_mb,
              disk_usage_gb: defaultStats.disk_usage_gb,
            }
          case "image_catalog":
            return clone(state.osImages)
          case "image_download_status":
            return { image_id: String(args.imageId ?? ""), current_file: "", bytes_downloaded: 0, bytes_total: 0, done: true, error: null }
          case "image_download_os": {
            const image = (state.osImages as Array<Record<string, unknown>>).find((item) => item.id === args.imageId)
            if (image) image.status = "ready"
            return null
          }
          case "image_delete_os": {
            const image = (state.osImages as Array<Record<string, unknown>>).find((item) => item.id === args.imageId)
            if (image) image.status = "not_downloaded"
            return null
          }
          case "vm_create": {
            const vmIndex = nextCounter("vm")
            const vm = {
              id: `vm-${vmIndex}`,
              name: String(args.name ?? `vm-${vmIndex}`),
              state: "stopped",
              cpus: Number(args.cpus ?? 2),
              memory_mb: Number(args.memoryMb ?? 4096),
              disk_gb: Number(args.diskGb ?? 40),
              rosetta_enabled: Boolean(args.rosetta ?? false),
              mounts: [],
              port_forwards: [],
              os_image: args.osImage ?? null,
            }
            ;(state.vms as Array<Record<string, unknown>>).unshift(vm)
            return vm.id
          }
          case "vm_start": {
            const vm = (state.vms as Array<Record<string, unknown>>).find((item) => item.id === args.id)
            if (vm) vm.state = "running"
            return null
          }
          case "vm_stop": {
            const vm = (state.vms as Array<Record<string, unknown>>).find((item) => item.id === args.id)
            if (vm) vm.state = "stopped"
            return null
          }
          case "vm_delete": {
            state.vms = (state.vms as Array<Record<string, unknown>>).filter((item) => item.id !== args.id)
            return null
          }
          case "vm_login_cmd": {
            const portForwards = Array.isArray(args.portForwards) ? args.portForwards : []
            const detectedPort = (portForwards as Array<Record<string, unknown>>).find(
              (item) => Number(item.guest_port) === 22 && String(item.protocol ?? "tcp").toLowerCase() === "tcp"
            )?.host_port
            const port = args.port ?? detectedPort
            if (port == null) {
              throw new Error("VM login is not available yet. Add a guest port 22 forward or specify an SSH port.")
            }
            return `ssh ${String(args.user ?? "root")}@${String(args.host ?? "127.0.0.1")} -p ${Number(port)}
# VM: ${String(args.name ?? "")}`
          }
          case "vm_mount_add": {
            const vm = (state.vms as Array<Record<string, unknown>>).find((item) => item.id === args.vm)
            if (vm) {
              ;(vm.mounts as Array<Record<string, unknown>>).push({
                tag: String(args.tag),
                host_path: String(args.hostPath),
                guest_path: String(args.guestPath),
                read_only: Boolean(args.readonly),
              })
            }
            return null
          }
          case "vm_mount_remove": {
            const vm = (state.vms as Array<Record<string, unknown>>).find((item) => item.id === args.vm)
            if (vm) {
              vm.mounts = (vm.mounts as Array<Record<string, unknown>>).filter((mount) => mount.tag !== args.tag)
            }
            return null
          }
          case "vm_port_forward_add": {
            const vm = (state.vms as Array<Record<string, unknown>>).find((item) => item.id === args.vmId)
            if (vm) {
              ;(vm.port_forwards as Array<Record<string, unknown>>).push({
                host_port: Number(args.hostPort),
                guest_port: Number(args.guestPort),
                protocol: String(args.protocol ?? "tcp"),
              })
            }
            return null
          }
          case "vm_port_forward_remove": {
            const vm = (state.vms as Array<Record<string, unknown>>).find((item) => item.id === args.vmId)
            if (vm) {
              vm.port_forwards = (vm.port_forwards as Array<Record<string, unknown>>).filter(
                (item) => item.host_port !== args.hostPort
              )
            }
            return null
          }
          case "vm_console":
            return ["boot complete\n", 14]
          case "k3s_status":
            return clone(state.k3sStatus)
          case "k3s_install": {
            ;(state.k3sStatus as Record<string, unknown>).installed = true
            return null
          }
          case "k3s_start": {
            ;(state.k3sStatus as Record<string, unknown>).installed = true
            ;(state.k3sStatus as Record<string, unknown>).running = true
            return null
          }
          case "k3s_stop": {
            ;(state.k3sStatus as Record<string, unknown>).running = false
            return null
          }
          case "k3s_uninstall": {
            ;(state.k3sStatus as Record<string, unknown>).installed = false
            ;(state.k3sStatus as Record<string, unknown>).running = false
            return null
          }
          case "k8s_list_namespaces":
            return clone(state.namespaces)
          case "k8s_list_pods":
            return clone(state.pods)
          case "k8s_list_services":
            return clone(state.services)
          case "k8s_list_deployments":
            return clone(state.deployments)
          case "k8s_pod_logs": {
            const key = `${String(args.namespace ?? "default")}/${String(args.name ?? "")}`
            return String((state.podLogs as Record<string, unknown>)[key] ?? "")
          }
          case "ollama_status":
            return clone(state.ollamaStatus)
          case "ollama_storage_info":
            return {
              ...(clone(state.ollamaStorage) as Record<string, unknown>),
              model_count: (state.ollamaModels as Array<unknown>).length,
            }
          case "ollama_list_models":
            return clone(state.ollamaModels)
          case "ollama_pull_model": {
            const name = String(args.name ?? `model-${nextCounter("image")}`)
            ;(state.ollamaModels as Array<Record<string, unknown>>).push({
              name,
              size_bytes: 2147483648,
              size_human: "2.0 GB",
              modified_at: "2026-03-07T00:00:00Z",
              digest: `sha256:${name}`,
              family: name.split(":")[0],
              parameter_size: "3B",
              quantization_level: "Q4_K_M",
            })
            return { ok: true, message: "Model pulled" }
          }
          case "ollama_delete_model": {
            state.ollamaModels = (state.ollamaModels as Array<Record<string, unknown>>).filter((model) => model.name !== args.name)
            return { ok: true, message: "Model deleted" }
          }
          case "sandbox_templates":
            return clone(state.sandboxTemplates)
          case "sandbox_list":
            return clone(state.sandboxes)
          case "sandbox_audit_list":
            return clone(state.sandboxAudit)
          case "sandbox_create": {
            const request = (args.request ?? {}) as Record<string, unknown>
            const template = (state.sandboxTemplates as Array<Record<string, unknown>>).find((item) => item.id === request.template_id)
            if (!template) throw new Error(`Template not found: ${String(request.template_id)}`)
            const index = nextCounter("sandbox")
            const id = `sandbox-${index}`
            const name = String(request.name ?? `cbx-${String(template.id)}-${index}`)
            const sandbox = {
              id,
              short_id: `sbx${index}`,
              name,
              image: String(request.image ?? template.image),
              state: "running",
              status: "running",
              template_id: String(template.id),
              owner: String(request.owner ?? "tester"),
              created_at: "2026-03-07T00:00:00Z",
              expires_at: "2026-03-07T08:00:00Z",
              ttl_hours: Number(request.ttl_hours ?? template.ttl_hours_default),
              cpu_cores: Number(request.cpu_cores ?? template.cpu_default),
              memory_mb: Number(request.memory_mb ?? template.memory_mb_default),
              is_expired: false,
              command: String(request.command ?? template.default_command),
              env: Array.isArray(request.env) ? request.env : [],
            }
            ;(state.sandboxes as Array<Record<string, unknown>>).unshift(sandbox)
            ;(state.sandboxAudit as Array<Record<string, unknown>>).unshift({
              timestamp: "2026-03-07T00:00:00Z",
              action: "create",
              sandbox_id: id,
              sandbox_name: name,
              level: "info",
              detail: "sandbox created",
            })
            return {
              id,
              short_id: sandbox.short_id,
              name,
              image: sandbox.image,
              login_cmd: `docker exec -it ${name} /bin/sh`,
            }
          }
          case "sandbox_inspect":
            return sandboxInspect(String(args.id))
          case "sandbox_start": {
            const sandbox = (state.sandboxes as Array<Record<string, unknown>>).find((item) => item.id === args.id)
            if (sandbox) {
              sandbox.state = "running"
              sandbox.status = "running"
            }
            return null
          }
          case "sandbox_stop": {
            const sandbox = (state.sandboxes as Array<Record<string, unknown>>).find((item) => item.id === args.id)
            if (sandbox) {
              sandbox.state = "stopped"
              sandbox.status = "stopped"
            }
            return null
          }
          case "sandbox_delete": {
            state.sandboxes = (state.sandboxes as Array<Record<string, unknown>>).filter((item) => item.id !== args.id)
            return null
          }
          case "sandbox_cleanup_expired": {
            const removed = (state.sandboxes as Array<Record<string, unknown>>).filter((item) => item.is_expired)
            state.sandboxes = (state.sandboxes as Array<Record<string, unknown>>).filter((item) => !item.is_expired)
            return {
              removed_count: removed.length,
              removed_names: removed.map((item) => item.name),
              message: removed.length ? "Removed expired sandboxes" : "No expired sandboxes",
            }
          }
          case "sandbox_exec": {
            const id = String(args.id ?? "")
            const command = String(args.command ?? "")
            const output = `sandbox ${id}: ${command}`
            ;(state.sandboxExecOutput as Record<string, unknown>)[id] = output
            ;(state.sandboxAudit as Array<Record<string, unknown>>).unshift({
              timestamp: "2026-03-07T00:05:00Z",
              action: "exec",
              sandbox_id: id,
              sandbox_name: sandboxInspect(id).name,
              level: "info",
              detail: command,
            })
            return { ok: true, output }
          }
          case "load_ai_settings":
            return clone(state.aiSettings)
          case "save_ai_settings":
            state.aiSettings = clone((args.settings ?? state.aiSettings) as Record<string, unknown>)
            return clone(state.aiSettings)
          case "validate_ai_profile":
            return { ok: true, message: "Profile validation passed" }
          case "ai_secret_exists":
            return Boolean((state.secretExists as Record<string, unknown>)[String(args.apiKeyRef)])
          case "ai_secret_set": {
            ;(state.secretExists as Record<string, unknown>)[String(args.apiKeyRef)] = true
            return null
          }
          case "ai_secret_delete": {
            ;(state.secretExists as Record<string, unknown>)[String(args.apiKeyRef)] = false
            return null
          }
          case "ai_test_connection":
            return {
              ok: true,
              request_id: nextRequestId(),
              message: "Connection succeeded",
              latency_ms: 42,
            }
          case "agent_cli_list_presets":
            return clone(state.agentPresets)
          case "agent_cli_run": {
            const presetId = args.presetId ? String(args.presetId) : null
            const command = presetId
              ? ((state.agentPresets as Array<Record<string, unknown>>).find((item) => item.id === presetId)?.command as string)
              : String(args.command ?? "")
            const allowlist = (((state.aiSettings as Record<string, unknown>).security_policy as Record<string, unknown>).cli_command_allowlist as string[]) ?? []
            if (command && allowlist.length > 0 && !allowlist.includes(command.split("/").pop() ?? command)) {
              throw new Error(`command '${command}' blocked by CLI allowlist`)
            }
            const prompt = String(args.prompt ?? "")
            return {
              ok: true,
              request_id: nextRequestId(),
              command_line: `${command} ${prompt}`.trim(),
              exit_code: 0,
              stdout: "",
              stderr: "",
              duration_ms: 0,
            }
          }
          case "ai_skill_execute": {
            const skill = findSkill(String(args.skillId ?? ""))
            if (!skill) throw new Error(`Skill not found: ${String(args.skillId)}`)
            if (skill.enabled === false) throw new Error(`Skill '${String(skill.id)}' is disabled`)
            const input = normalizeSkillInput(skill, args.input ?? {})
            validateSkillInput("input", input, skill.input_schema)
            if (skill.executor === "agent_cli_preset") {
              const prompt = typeof (input as Record<string, unknown>).prompt === "string" ? String((input as Record<string, unknown>).prompt) : ""
              return {
                ok: true,
                skill_id: skill.id,
                executor: skill.executor,
                target: skill.target,
                request_id: nextRequestId(),
                output: { ok: true, command_line: `${String(skill.target)} exec ${prompt}`.trim() },
              }
            }
            if (skill.target === "sandbox_list") {
              return {
                ok: true,
                skill_id: skill.id,
                executor: skill.executor,
                target: skill.target,
                request_id: nextRequestId(),
                output: { ok: true, items: clone(state.sandboxes) },
              }
            }
            if (skill.target === "sandbox_exec") {
              const result = await handleInvoke("sandbox_exec", input as Record<string, unknown>)
              return {
                ok: true,
                skill_id: skill.id,
                executor: skill.executor,
                target: skill.target,
                request_id: nextRequestId(),
                output: result,
              }
            }
            return {
              ok: true,
              skill_id: skill.id,
              executor: skill.executor,
              target: skill.target,
              request_id: nextRequestId(),
              output: { ok: true },
            }
          }
          case "ai_generate_plan":
            return {
              request_id: nextRequestId(),
              strategy: "heuristic",
              notes: `Generated plan for ${String(args.prompt ?? "")}`,
              fallback_used: true,
              steps: [
                {
                  id: "step-1",
                  title: "Stop container",
                  command: "stop_container",
                  args: { id: "ctr-web" },
                  risk_level: "write",
                  requires_confirmation: true,
                  explain: "Stops the running container safely.",
                },
              ],
            }
          case "assistant_execute_step": {
            if (args.command === "stop_container") {
              await handleInvoke("stop_container", { id: (args.args as Record<string, unknown>)?.id })
            }
            if (args.command === "sandbox_exec") {
              await handleInvoke("sandbox_exec", args.args as Record<string, unknown>)
            }
            return {
              ok: true,
              request_id: nextRequestId(),
              command: args.command,
              risk_level: args.riskLevel ?? "read",
              output: { ok: true },
            }
          }
          case "mcp_list_servers":
            return clone(state.mcpServers)
          case "mcp_save_servers": {
            const servers = clone((args.servers ?? []) as Array<Record<string, unknown>>)
            state.mcpServers = servers.map((server) => ({
              ...server,
              running: false,
              status: "stopped",
              pid: null,
              started_at: "",
              exit_code: null,
            }))
            ;(state.aiSettings as Record<string, unknown>).mcp_servers = servers.map((server) => {
              const next = { ...server }
              delete next.running
              delete next.status
              delete next.pid
              delete next.started_at
              delete next.exit_code
              return next
            })
            return clone((state.aiSettings as Record<string, unknown>).mcp_servers)
          }
          case "mcp_start_server": {
            state.mcpServers = (state.mcpServers as Array<Record<string, unknown>>).map((server) =>
              server.id === args.id
                ? { ...server, running: true, status: "running", started_at: "2026-03-07T00:00:00Z", pid: 4242 }
                : server
            )
            return { ok: true, message: "started" }
          }
          case "mcp_stop_server": {
            state.mcpServers = (state.mcpServers as Array<Record<string, unknown>>).map((server) =>
              server.id === args.id
                ? { ...server, running: false, status: "stopped", started_at: "", pid: null }
                : server
            )
            return { ok: true, message: "stopped" }
          }
          case "mcp_server_logs":
            return clone((state.mcpLogs as Record<string, unknown>)[String(args.id)] ?? [])
          case "mcp_export_client_config":
            return JSON.stringify({ client: String(args.client ?? "codex"), servers: state.mcpServers }, null, 2)
          case "opensandbox_status":
            return {
              installed: true,
              enabled: false,
              configured: true,
              reachable: true,
              base_url: "http://127.0.0.1:8080",
              config_path: "/Users/test/.cratebay/opensandbox.toml",
            }
          default:
            throw new Error(`Unhandled Tauri invoke in E2E bridge: ${cmd}`)
        }
      }

      Object.defineProperty(window, "__CRATEBAY_E2E__", {
        configurable: true,
        value: {
          state,
          getInvokeCalls: () => clone(state.invokeCalls),
          setConfirmResult: (value: boolean) => {
            confirmResult = value
          },
        },
      })

      Object.defineProperty(window, "open", {
        configurable: true,
        value: () => null,
      })
      Object.defineProperty(window, "confirm", {
        configurable: true,
        value: () => confirmResult,
      })
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: {
          writeText: async () => undefined,
          readText: async () => "",
        },
      })
      Object.defineProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
        configurable: true,
        value: {
          unregisterListener: () => undefined,
        },
      })
      Object.defineProperty(window, "__TAURI_INTERNALS__", {
        configurable: true,
        value: {
          transformCallback(callback: (payload: unknown) => void, once = false) {
            const id = callbackSeq++
            callbacks.set(id, { callback, once })
            return id
          },
          unregisterCallback(id: number) {
            callbacks.delete(id)
          },
          async invoke(cmd: string, args?: Record<string, unknown>) {
            return handleInvoke(cmd, args ?? {})
          },
        },
      })
    },
    {
      initialState: state,
      initialConfirmResult: confirmResult,
    }
  )
}

export async function gotoApp(page: Page, options: InstallOptions = {}) {
  await installTauriMock(page, options)
  await page.goto("/")
}

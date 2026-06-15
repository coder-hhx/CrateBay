import type { Page } from "@playwright/test";

export interface MockTauriData {
  settings: Record<string, string>;
  containerList: Array<Record<string, unknown>>;
  containerTemplates: Array<Record<string, unknown>>;
  containerLogs: Record<string, Array<Record<string, unknown>>>;
  containerDetails: Record<string, Record<string, unknown>>;
  containerStats: Record<string, Record<string, unknown>>;
  pods: Array<Record<string, any>>;
  volumes: Array<Record<string, any>>;
  networks: Array<Record<string, any>>;
  localImages: Array<Record<string, unknown>>;
  imageSearchResults: Array<Record<string, unknown>>;
  engineStatus: Record<string, unknown>;
  engineSubstrate: Record<string, unknown>;
  engineStorageGc: Record<string, unknown>;
  engineShimTasks: Array<Record<string, unknown>>;
  /** Compatibility override for older tests exercising docker_status alias. */
  dockerStatus: Record<string, unknown>;
  runtimeStatus: Record<string, unknown>;
  invokedCommands: Array<{ command: string; args?: Record<string, unknown> }>;
  commandFailures?: Record<string, string | Array<string>>;
  runtimeAutoStartDisabledCommands?: string[];
}

const DEFAULT_DATE = new Date("2026-03-23T00:00:00.000Z").toISOString();

const defaultMockData: MockTauriData = {
  settings: {
    language: "en",
    theme: "dark",
    registryMirrors: "[]",
    runtimeHttpProxy: "",
    runtimeHttpProxyBridge: "false",
    runtimeHttpProxyBindHost: "0.0.0.0",
    runtimeHttpProxyBindPort: "3128",
    runtimeHttpProxyGuestHost: "192.168.64.1",
  },
  containerList: [
    {
      id: "abc123",
      shortId: "abc123",
      name: "node-01",
      status: "running",
      state: "running",
      image: "node:20-alpine",
      templateId: "node-dev",
      cpuCores: 2,
      memoryMb: 2048,
      ports: [{ hostPort: 3000, containerPort: 3000, protocol: "tcp" }],
      createdAt: DEFAULT_DATE,
      labels: {},
    },
    {
      id: "def456",
      shortId: "def456",
      name: "python-dev",
      status: "stopped",
      state: "stopped",
      image: "python:3.12-slim",
      templateId: "python-dev",
      cpuCores: 1,
      memoryMb: 1024,
      ports: [],
      createdAt: DEFAULT_DATE,
      labels: {},
    },
  ],
  containerTemplates: [
    { id: "node-dev", name: "Node.js", description: "Node.js development", image: "node:20-alpine" },
    { id: "python-dev", name: "Python", description: "Python development", image: "python:3.12-slim" },
    { id: "rust-dev", name: "Rust", description: "Rust development", image: "rust:1-slim" },
  ],
  containerLogs: {
    abc123: [
      {
        stream: "stdout",
        message: "Mock container started",
        timestamp: "2026-03-23T00:00:01.000Z",
      },
      {
        stream: "stdout",
        message: "Listening on 0.0.0.0:3000",
        timestamp: "2026-03-23T00:00:02.000Z",
      },
    ],
    def456: [
      {
        stream: "stderr",
        message: "Container is stopped",
        timestamp: "2026-03-23T00:00:03.000Z",
      },
    ],
  },
  containerDetails: {},
  containerStats: {},
  pods: [
    {
      id: "pod123",
      name: "web-stack",
      driver: "bridge",
      createdAt: DEFAULT_DATE,
      containers: [],
    },
  ],
  volumes: [
    {
      name: "workspace-cache",
      driver: "local",
      mountpoint: "/var/lib/cratebay-engine/volumes/workspace-cache/_data",
      createdAt: DEFAULT_DATE,
      scope: "local",
      labels: { "com.cratebay.volume": "true" },
      options: {},
      managedBy: "cratebay-engine",
    },
  ],
  networks: [
    {
      id: "net-workspace",
      name: "workspace-net",
      driver: "bridge",
      scope: "local",
      internal: false,
      attachable: true,
      labels: { "com.cratebay.network": "true" },
      containers: {
        abc123: {
          name: "node-01",
          endpointId: "endpoint-abc123",
          ipv4Address: "172.18.0.2/16",
        },
      },
      managedBy: "cratebay-engine",
    },
  ],
  localImages: [
    {
      id: "sha256:node",
      repoTags: ["node:20-alpine"],
      sizeBytes: 120_000_000,
      sizeHuman: "120 MB",
      created: 1_700_000_000,
    },
  ],
  imageSearchResults: [
    {
      source: "dockerhub",
      reference: "alpine:latest",
      description: "A minimal Linux distribution",
      stars: 10_000,
      pulls: 1_000_000,
      official: true,
    },
  ],
  engineStatus: {
    connected: true,
    version: "cratebay-containerd",
    api_version: "cratebay.engine.v1",
    os: "linux",
    arch: "arm64",
    engine_source: "builtin",
    source: "builtin",
    socket_path: "/tmp/cratebay/engine.sock",
  },
  engineSubstrate: {
    engine: "CrateBay Engine",
    shim: {
      manager: "cratebay-containerd-shim",
      backend: "containerd task service",
    },
    network: {
      manager: "cratebay-cni",
      stack: "CNI",
    },
    storage: {
      manager: "cratebay-storage",
      volumeCount: 1,
      volumeBytes: 4096,
    },
    daemon: {
      compatibilityEndpoint: "/tmp/cratebay/engine.sock",
    },
    compatibility: {
      dockerDaemon: false,
    },
  },
  engineStorageGc: {
    api: "cratebay.storage.gc.v1",
    applied: false,
    candidateCount: 1,
    reclaimableBytes: 4096,
  },
  engineShimTasks: [
    {
      id: "shim-task-abc123",
      name: "node-01",
      state: "running",
      image: "node:20-alpine",
    },
  ],
  runtimeStatus: {
    state: "ready",
    platform: "macos-vz",
    cpu_cores: 2,
    memory_mb: 2048,
    disk_gb: 20,
    engine_responsive: true,
    compatibility_responsive: true,
    compatibility_version: "cratebay-containerd",
    engine_source: "builtin",
    docker_source: "builtin",
    docker_responsive: true,
    uptime_seconds: 120,
    resource_usage: {
      cpu_percent: 18.5,
      memory_used_mb: 768,
      memory_total_mb: 2048,
      disk_used_gb: 6.5,
      disk_total_gb: 20,
      container_count: 2,
    },
  },
  invokedCommands: [],
  commandFailures: {},
  runtimeAutoStartDisabledCommands: [],
};

export async function installTauriMock(
  page: Page,
  overrides: Partial<MockTauriData> = {},
): Promise<void> {
  const merged: MockTauriData = {
    ...defaultMockData,
    ...overrides,
    settings: { ...defaultMockData.settings, ...(overrides.settings ?? {}) },
    containerList: overrides.containerList ?? structuredClone(defaultMockData.containerList),
    containerTemplates: overrides.containerTemplates ?? structuredClone(defaultMockData.containerTemplates),
    containerLogs: { ...structuredClone(defaultMockData.containerLogs), ...(overrides.containerLogs ?? {}) },
    containerDetails: { ...structuredClone(defaultMockData.containerDetails), ...(overrides.containerDetails ?? {}) },
    containerStats: { ...structuredClone(defaultMockData.containerStats), ...(overrides.containerStats ?? {}) },
    pods: overrides.pods ?? structuredClone(defaultMockData.pods),
    volumes: overrides.volumes ?? structuredClone(defaultMockData.volumes),
    networks: overrides.networks ?? structuredClone(defaultMockData.networks),
    localImages: overrides.localImages ?? structuredClone(defaultMockData.localImages),
    imageSearchResults: overrides.imageSearchResults ?? structuredClone(defaultMockData.imageSearchResults),
    engineStatus: { ...defaultMockData.engineStatus, ...(overrides.engineStatus ?? {}) },
    engineSubstrate: { ...structuredClone(defaultMockData.engineSubstrate), ...(overrides.engineSubstrate ?? {}) },
    engineStorageGc: { ...structuredClone(defaultMockData.engineStorageGc), ...(overrides.engineStorageGc ?? {}) },
    engineShimTasks: overrides.engineShimTasks ?? structuredClone(defaultMockData.engineShimTasks),
    dockerStatus: {
      ...defaultMockData.engineStatus,
      ...(overrides.engineStatus ?? {}),
      ...(overrides.dockerStatus ?? {}),
    },
    runtimeStatus: { ...defaultMockData.runtimeStatus, ...(overrides.runtimeStatus ?? {}) },
    invokedCommands: [],
    commandFailures: structuredClone(overrides.commandFailures ?? defaultMockData.commandFailures ?? {}),
    runtimeAutoStartDisabledCommands: [
      ...(overrides.runtimeAutoStartDisabledCommands ??
        defaultMockData.runtimeAutoStartDisabledCommands ??
        []),
    ],
  };

  await page.addInitScript(({ mockData }) => {
    (window as any).__MOCK_TAURI__ = mockData;
    (window as any).__MOCK_TAURI__.terminalBuffers ??= {};

    (window as any).__MOCK_TAURI_INVOKE__ = async (
      command: string,
      args?: Record<string, unknown>,
    ) => {
      const state = (window as any).__MOCK_TAURI__;
      const defaultDate = "2026-03-23T00:00:00.000Z";
      state.invokedCommands.push({ command, args });

      if (
        state.runtimeAutoStartDisabledCommands?.includes(command) &&
        state.runtimeStartRequested !== true
      ) {
        throw new Error("Implicit runtime start disabled by CRATEBAY_DISABLE_RUNTIME_AUTO_START");
      }

      const maybeFailure = state.commandFailures?.[command];
      if (Array.isArray(maybeFailure)) {
        const message = maybeFailure.shift();
        if (message !== undefined) {
          throw new Error(message);
        }
      } else if (typeof maybeFailure === "string") {
        delete state.commandFailures[command];
        throw new Error(maybeFailure);
      }

      const findContainer = (id: string) =>
        state.containerList.find(
          (container: any) => container.id === id || container.name === id || container.shortId === id,
        ) ?? null;

      const lookupByContainerKey = (
        records: Record<string, unknown>,
        id: string,
        container: any,
      ) =>
        records[id] ??
        (container ? records[container.id] ?? records[container.name] ?? records[container.shortId] : undefined);

      const buildContainerInfo = (id: string) => {
        const container = findContainer(id);
        if (container) return structuredClone(container);
        return {
          id,
          shortId: id.slice(0, 12),
          name: id,
          status: "running",
          state: "running",
          image: "alpine:latest",
          cpuCores: 1,
          memoryMb: 512,
          ports: [],
          createdAt: defaultDate,
          labels: {},
        };
      };

      const dispatchTerminal = (sessionId: string, payload: Record<string, unknown>) => {
        if (sessionId.length === 0) return;
        window.dispatchEvent(
          new CustomEvent(`terminal:stream:${sessionId}`, {
            detail: { payload },
          }),
        );
      };

      const markRuntimeStarted = () => {
        state.runtimeStartRequested = true;
        state.engineStatus = {
          ...state.engineStatus,
          connected: true,
          version: "cratebay-containerd",
          api_version: "cratebay.engine.v1",
          os: "linux",
          arch: "arm64",
          engine_source: "builtin",
          source: "builtin",
          socket_path: "/tmp/cratebay/engine.sock",
        };
        state.dockerStatus = {
          ...state.dockerStatus,
          connected: true,
          version: "cratebay-containerd",
          api_version: "cratebay.engine.v1",
          os: "linux",
          arch: "arm64",
          engine_source: "builtin",
          source: "builtin",
          socket_path: "/tmp/cratebay/engine.sock",
        };
        state.runtimeStatus = {
          ...state.runtimeStatus,
          state: "ready",
          engine_responsive: true,
          compatibility_responsive: true,
          compatibility_version: "cratebay-containerd",
          docker_responsive: true,
          engine_source: "builtin",
          docker_source: "builtin",
        };
      };

      const buildEngineContract = () => ({
        name: "CrateBay Engine",
        kind: "cratebay-containerd",
        adapter: { api: "cratebay.engine.v1" },
        backend: { runtime: "containerd", ociRuntime: "runc", namespace: "cratebay" },
        network: { stack: "CNI" },
        compatibility: { dockerCompatible: true },
      });

      switch (command) {
        case "settings_get": {
          const key = (args?.key as string) ?? "";
          return key in state.settings ? state.settings[key] : null;
        }
        case "settings_update": {
          const key = (args?.key as string) ?? "";
          state.settings[key] = String(args?.value ?? "");
          return null;
        }
        case "container_list":
          return state.containerList;
        case "container_templates":
          return state.containerTemplates;
        case "container_start": {
          const id = String(args?.id ?? "");
          state.containerList = state.containerList.map((container: any) =>
            container.id === id || container.name === id
              ? { ...container, status: "running", state: "running" }
              : container,
          );
          return null;
        }
        case "container_stop": {
          const id = String(args?.id ?? "");
          state.containerList = state.containerList.map((container: any) =>
            container.id === id || container.name === id
              ? { ...container, status: "stopped", state: "stopped" }
              : container,
          );
          return null;
        }
        case "container_delete": {
          const id = String(args?.id ?? "");
          state.containerList = state.containerList.filter(
            (container: any) => container.id !== id && container.name !== id,
          );
          return null;
        }
        case "container_create": {
          const request = (args?.request ?? {}) as any;
          const created = {
            id: `mock-${Date.now()}`,
            shortId: "mock-created",
            name: request.name,
            status: "running",
            state: "running",
            image: request.image,
            cpuCores: request.cpuCores,
            memoryMb: request.memoryMb,
            ports: request.ports ?? [],
            createdAt: new Date().toISOString(),
            labels: {},
          };
          state.containerList.push(created);
          state.containerLogs[created.id] = [
            {
              stream: "stdout",
              message: `${created.name} created from ${created.image}`,
              timestamp: new Date().toISOString(),
            },
          ];
          return created;
        }
        case "container_run": {
          const request = (args?.request ?? {}) as any;
          const name = request.name ?? `mock-run-${Date.now()}`;
          const image = request.image ?? "alpine:latest";
          const command = Array.isArray(request.command)
            ? request.command.map((item: unknown) => String(item)).join(" ")
            : "";
          const result = {
            id: `run-${Date.now()}`,
            name,
            image,
            exitCode: 0,
            stdout: command ? `mock run: ${command}\n` : "mock run complete\n",
            stderr: "",
            stdoutTruncated: false,
            stderrTruncated: false,
            timedOut: false,
          };
          if (request.remove === false) {
            state.containerList.push({
              id: result.id,
              shortId: result.id.slice(0, 12),
              name,
              status: "stopped",
              state: "exited",
              image,
              ports: [],
              createdAt: new Date().toISOString(),
              labels: { "com.cratebay.run": "true" },
            });
          }
          return result;
        }
        case "container_exec": {
          const cmd = Array.isArray(args?.cmd)
            ? (args?.cmd as unknown[]).map((item) => String(item))
            : [];
          return {
            exitCode: 0,
            stdout: `mock exec: ${cmd.join(" ")}\n`,
            stderr: "",
            stdoutTruncated: false,
            stderrTruncated: false,
            timedOut: false,
          };
        }
        case "container_exec_stream": {
          const channelId = String(args?.channel_id ?? args?.channelId ?? "");
          const cmd = Array.isArray(args?.cmd)
            ? (args?.cmd as unknown[]).map((item) => String(item))
            : [];
          const rendered = cmd.length > 0 ? cmd[cmd.length - 1] : cmd.join(" ");
          if (channelId.length > 0) {
            window.setTimeout(() => {
              window.dispatchEvent(
                new CustomEvent(`exec:stream:${channelId}`, {
                  detail: { payload: { type: "Stdout", data: `mock exec: ${rendered}\n` } },
                }),
              );
              window.dispatchEvent(
                new CustomEvent(`exec:stream:${channelId}`, {
                  detail: { payload: { type: "Done", exit_code: 0 } },
                }),
              );
            }, 0);
          }
          return null;
        }
        case "container_terminal_open": {
          const sessionId = String(args?.session_id ?? args?.sessionId ?? "");
          state.terminalBuffers[sessionId] = "";
          window.setTimeout(() => {
            dispatchTerminal(sessionId, {
              type: "Output",
              data: "CrateBay native PTY ready\n$ ",
            });
          }, 0);
          return null;
        }
        case "container_terminal_input": {
          const sessionId = String(args?.session_id ?? args?.sessionId ?? "");
          const data = String(args?.data ?? "");
          state.terminalBuffers[sessionId] = `${state.terminalBuffers[sessionId] ?? ""}${data}`;
          if (data.includes("\r") || data.includes("\n")) {
            const rendered = String(state.terminalBuffers[sessionId] ?? "").trim();
            state.terminalBuffers[sessionId] = "";
            window.setTimeout(() => {
              dispatchTerminal(sessionId, {
                type: "Output",
                data: `\r\nmock terminal: ${rendered}\r\n$ `,
              });
            }, 0);
          }
          return null;
        }
        case "container_terminal_resize":
          return {
            resized: true,
            transport: "cratebay-native-pty",
            cols: args?.cols,
            rows: args?.rows,
          };
        case "container_terminal_close": {
          const sessionId = String(args?.session_id ?? args?.sessionId ?? "");
          delete state.terminalBuffers[sessionId];
          window.setTimeout(() => {
            dispatchTerminal(sessionId, { type: "Done", exit_code: 0 });
          }, 0);
          return null;
        }
        case "container_logs": {
          const id = String(args?.id ?? "");
          const container = findContainer(id);
          const logs = lookupByContainerKey(state.containerLogs, id, container);
          if (Array.isArray(logs)) return structuredClone(logs);
          return [
            {
              stream: "stdout",
              message: `${container?.name ?? id} has no captured logs`,
              timestamp: new Date().toISOString(),
            },
          ];
        }
        case "container_inspect": {
          const id = String(args?.id ?? "");
          const container = findContainer(id);
          const detail = lookupByContainerKey(state.containerDetails, id, container);
          if (detail !== undefined) return structuredClone(detail);
          const info = buildContainerInfo(id);
          const running = info.status === "running" || info.status === "paused";
          return {
            info,
            networkSettings: {
              Networks: {
                bridge: {
                  IPAddress: running ? "172.17.0.2" : "",
                  Gateway: "172.17.0.1",
                },
              },
            },
            mounts: [
              {
                Type: "volume",
                Name: "cratebay-mock-data",
                Destination: "/workspace",
                Mode: "rw",
                RW: true,
              },
            ],
            state: {
              status: info.state,
              running,
              startedAt: running ? defaultDate : null,
              finishedAt: running ? null : defaultDate,
              exitCode: running ? null : 0,
              error: null,
              pid: running ? 4242 : null,
            },
          };
        }
        case "container_stats": {
          const id = String(args?.id ?? "");
          const container = findContainer(id);
          const stats = lookupByContainerKey(state.containerStats, id, container);
          if (stats !== undefined) return structuredClone(stats);
          const running = container?.status === "running" || container?.status === "paused";
          const memoryLimitMb = Number(container?.memoryMb ?? 2048);
          const memoryUsedMb = running ? Math.min(memoryLimitMb, 384) : 0;
          const cpuCoresUsed = running ? 0.42 : 0;
          return {
            id: container?.id ?? id,
            name: container?.name ?? id,
            readAt: new Date().toISOString(),
            cpuPercent: running ? 21 : 0,
            cpuCoresUsed,
            memoryUsedMb,
            memoryLimitMb,
            memoryPercent: memoryLimitMb > 0 ? (memoryUsedMb / memoryLimitMb) * 100 : 0,
          };
        }
        case "image_list":
          return state.localImages;
        case "image_search":
          return state.imageSearchResults;
        case "image_inspect":
          return {
            id: args?.id ?? "sha256:node",
            repoTags: ["node:20-alpine"],
            sizeBytes: 120_000_000,
            created: "2026-03-23T00:00:00.000Z",
            architecture: "arm64",
            os: "linux",
            dockerVersion: "25.0.0",
            layers: 4,
          };
        case "image_pull": {
          const channelId = String(args?.channelId ?? args?.channel_id ?? `pull-${Date.now()}`);
          state.localImages.push({
            id: `sha256:${String(args?.image ?? "pulled").replace(/[^a-z0-9]/gi, "-")}`,
            repoTags: [String(args?.image ?? "pulled:latest")],
            sizeBytes: 5_000_000,
            sizeHuman: "5 MB",
            created: 1_700_000_100,
          });
          window.setTimeout(() => {
            window.dispatchEvent(
              new CustomEvent(`image:pull:${channelId}`, {
                detail: {
                  payload: {
                    current_layer: 1,
                    total_layers: 1,
                    progress_percent: 100,
                    status: "complete",
                    complete: true,
                    error: null,
                    current_bytes: 5_000_000,
                    total_bytes: 5_000_000,
                  },
                },
              }),
            );
          }, 0);
          return channelId;
        }
        case "image_remove": {
          const id = String(args?.id ?? "");
          state.localImages = state.localImages.filter(
            (image: any) => image.id !== id && !image.repoTags.includes(id),
          );
          return null;
        }
        case "image_tag": {
          const source = String(args?.source ?? "");
          const target = String(args?.target ?? "");
          const image = state.localImages.find(
            (item: any) => item.id === source || item.repoTags.includes(source),
          );
          if (image && target.length > 0 && !image.repoTags.includes(target)) {
            image.repoTags.push(target);
          }
          return null;
        }
        case "image_export":
          return 4096;
        case "image_import":
          state.localImages.push({
            id: "sha256:imported",
            repoTags: ["cratebay/imported:test"],
            sizeBytes: 4_096,
            sizeHuman: "4 KB",
            created: 1_700_000_200,
          });
          return ["cratebay/imported:test"];
        case "image_preload_bundled": {
          const defs = [
            ["cratebay-python-dev:v1", "python-dev.tar.gz"],
            ["cratebay-node-dev:v1", "node-dev.tar.gz"],
            ["cratebay-rust-dev:v1", "rust-dev.tar.gz"],
            ["cratebay-ubuntu-base:v1", "ubuntu-base.tar.gz"],
          ];
          return defs.map(([imageName, tarFilename]) => {
            const exists = state.localImages.some((image: any) =>
              image.repoTags.includes(imageName),
            );
            if (!exists) {
              state.localImages.push({
                id: `sha256:${imageName}`,
                repoTags: [imageName],
                sizeBytes: 10_000_000,
                sizeHuman: "10 MB",
                created: 1_700_000_300,
              });
            }
            return {
              imageName,
              tarFilename,
              archivePath: `/mock/bundle-images/${tarFilename}`,
              loaded: !exists,
              skipped: exists,
              message: exists ? "already present" : `loaded ${imageName}`,
            };
          });
        }
        case "image_pack_container":
          state.localImages.push({
            id: `sha256:${String(args?.image ?? "packed").replace(/[^a-z0-9]/gi, "-")}`,
            repoTags: [String(args?.image ?? "cratebay/packed:test")],
            sizeBytes: 12_000_000,
            sizeHuman: "12 MB",
            created: 1_700_000_400,
          });
          return String(args?.image ?? "cratebay/packed:test");
        case "pod_list":
          return state.pods;
        case "pod_create": {
          const name = String(args?.name ?? "");
          const driver = String(args?.driver ?? "bridge") || "bridge";
          const pod = {
            id: `pod-${name}`,
            name,
            driver,
            createdAt: new Date().toISOString(),
            containers: [],
            internal: Boolean(args?.internal),
            enableIpv6: Boolean(args?.enableIpv6),
          };
          state.pods.push(pod);
          return pod;
        }
        case "pod_inspect": {
          const name = String(args?.name ?? "");
          return state.pods.find((pod: any) => pod.name === name) ?? null;
        }
        case "pod_delete": {
          const name = String(args?.name ?? "");
          state.pods = state.pods.filter((pod: any) => pod.name !== name);
          return null;
        }
        case "pod_add_container": {
          const name = String(args?.name ?? "");
          const containerRef = String(args?.container ?? "");
          const pod = state.pods.find((item: any) => item.name === name);
          const container = state.containerList.find(
            (item: any) => item.id === containerRef || item.name === containerRef,
          );
          if (pod && container) {
            pod.containers.push({
              id: container.id,
              name: container.name,
              ipv4Address: "172.18.0.2/16",
              ipv6Address: null,
            });
          }
          return null;
        }
        case "pod_remove_container": {
          const name = String(args?.name ?? "");
          const containerRef = String(args?.container ?? "");
          const pod = state.pods.find((item: any) => item.name === name);
          if (pod) {
            pod.containers = pod.containers.filter(
              (container: any) =>
                container.id !== containerRef && container.name !== containerRef,
            );
          }
          return null;
        }
        case "volume_list":
          return state.volumes;
        case "volume_create": {
          const name = String(args?.name ?? "");
          const driver = String(args?.driver ?? "local") || "local";
          const volume = {
            name,
            driver,
            mountpoint: `/var/lib/cratebay-engine/volumes/${name}/_data`,
            createdAt: new Date().toISOString(),
            scope: "local",
            labels: { "com.cratebay.volume": "true" },
            options: {},
            managedBy: "cratebay-engine",
          };
          state.volumes.push(volume);
          return volume;
        }
        case "volume_inspect": {
          const name = String(args?.name ?? "");
          return state.volumes.find((volume: any) => volume.name === name) ?? null;
        }
        case "volume_delete": {
          const name = String(args?.name ?? "");
          state.volumes = state.volumes.filter((volume: any) => volume.name !== name);
          return null;
        }
        case "network_list":
          return state.networks;
        case "network_create": {
          const name = String(args?.name ?? "");
          const driver = String(args?.driver ?? "bridge") || "bridge";
          const internal = Boolean(args?.internal);
          const network = {
            id: `net-${name}`,
            name,
            driver,
            scope: "local",
            internal,
            attachable: true,
            labels: { "com.cratebay.network": "true" },
            containers: {},
            enableIpv6: Boolean(args?.enableIpv6),
            managedBy: "cratebay-engine",
          };
          state.networks.push(network);
          return network;
        }
        case "network_inspect": {
          const id = String(args?.id ?? "");
          return (
            state.networks.find(
              (network: any) => network.id === id || network.name === id,
            ) ?? null
          );
        }
        case "network_delete": {
          const id = String(args?.id ?? "");
          state.networks = state.networks.filter(
            (network: any) => network.id !== id && network.name !== id,
          );
          return null;
        }
        case "runtime_diagnostics":
          return {
            ok: Boolean(
              state.runtimeStatus.engine_responsive ??
                state.runtimeStatus.docker_responsive ??
                state.engineStatus.connected,
            ),
            runtime: structuredClone(state.runtimeStatus),
            engineContract: { ok: true, value: buildEngineContract(), error: null },
            substrate: { ok: true, value: structuredClone(state.engineSubstrate), error: null },
            storageGc: { ok: true, value: structuredClone(state.engineStorageGc), error: null },
            shimTasks: { ok: true, value: { items: structuredClone(state.engineShimTasks) }, error: null },
            generatedAtUnix: Math.floor(Date.now() / 1000),
          };
        case "engine_contract":
          return buildEngineContract();
        case "engine_substrate":
          return structuredClone(state.engineSubstrate);
        case "engine_storage_gc": {
          const apply = Boolean(args?.apply);
          state.engineStorageGc = {
            ...state.engineStorageGc,
            applied: apply,
            candidateCount: apply ? 0 : state.engineStorageGc.candidateCount,
            reclaimableBytes: apply ? 0 : state.engineStorageGc.reclaimableBytes,
          };
          return structuredClone(state.engineStorageGc);
        }
        case "engine_shim_tasks":
          return { items: structuredClone(state.engineShimTasks) };
        case "engine_shim_reap_task": {
          const id = String(args?.id ?? "");
          if (Boolean(args?.apply)) {
            state.engineShimTasks = state.engineShimTasks.filter((task: any) => task.id !== id);
          }
          return {
            api: "cratebay.shim.reap.v1",
            id,
            applied: Boolean(args?.apply),
            reclaimableBytes: 4096,
          };
        }
        case "engine_status":
          return state.engineStatus;
        case "docker_status":
          return state.dockerStatus;
        case "runtime_status":
          return state.runtimeStatus;
        case "runtime_start":
          markRuntimeStarted();
          return "ok";
        case "runtime_provision":
          state.runtimeStatus = {
            ...state.runtimeStatus,
            state: "provisioned",
            engine_responsive: false,
            compatibility_responsive: false,
            docker_responsive: false,
          };
          return "ok";
        case "runtime_restart":
          markRuntimeStarted();
          return "ok";
        case "runtime_stop":
          state.engineStatus = {
            ...state.engineStatus,
            connected: false,
            socket_path: null,
          };
          state.dockerStatus = {
            ...state.dockerStatus,
            connected: false,
            socket_path: null,
          };
          state.runtimeStatus = {
            ...state.runtimeStatus,
            state: "stopped",
            engine_responsive: false,
            compatibility_responsive: false,
            docker_responsive: false,
          };
          return "ok";
        case "app_update_check":
          return {
            configured: true,
            available: false,
            currentVersion: "0.9.0",
            version: null,
            date: null,
            body: null,
            channel: "stable",
            releaseTag: null,
            releaseName: null,
            releaseUrl: null,
            repository: "cratebay/cratebay",
            message: null,
          };
        case "app_update_install":
          return null;
        case "app_restart":
          return null;
        case "system_info":
          return {
            os: "macos",
            osVersion: "14.0",
            arch: "arm64",
            appVersion: "0.9.0",
            dataDir: "/tmp/cratebay",
            dbPath: "/tmp/cratebay/cratebay.db",
            dbSizeBytes: 0,
            logPath: "/tmp/cratebay/cratebay.log",
          };
        default:
          return null;
      }
    };
  }, { mockData: merged });
}

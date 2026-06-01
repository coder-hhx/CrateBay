import type { Page } from "@playwright/test";

export interface MockTauriData {
  settings: Record<string, string>;
  containerList: Array<Record<string, unknown>>;
  containerTemplates: Array<Record<string, unknown>>;
  containerLogs: Record<string, Array<Record<string, unknown>>>;
  containerDetails: Record<string, Record<string, unknown>>;
  containerStats: Record<string, Record<string, unknown>>;
  pods: Array<Record<string, any>>;
  localImages: Array<Record<string, unknown>>;
  imageSearchResults: Array<Record<string, unknown>>;
  dockerStatus: Record<string, unknown>;
  runtimeStatus: Record<string, unknown>;
  invokedCommands: Array<{ command: string; args?: Record<string, unknown> }>;
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
  dockerStatus: {
    connected: true,
    version: "25.0.0",
    api_version: "1.44",
    os: "linux",
    arch: "arm64",
    source: "runtime",
    socket_path: "/tmp/docker.sock",
  },
  runtimeStatus: {
    state: "ready",
    platform: "macos-vz",
    cpu_cores: 2,
    memory_mb: 2048,
    disk_gb: 20,
    docker_responsive: true,
    uptime_seconds: 120,
    resource_usage: null,
  },
  invokedCommands: [],
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
    localImages: overrides.localImages ?? structuredClone(defaultMockData.localImages),
    imageSearchResults: overrides.imageSearchResults ?? structuredClone(defaultMockData.imageSearchResults),
    dockerStatus: { ...defaultMockData.dockerStatus, ...(overrides.dockerStatus ?? {}) },
    runtimeStatus: { ...defaultMockData.runtimeStatus, ...(overrides.runtimeStatus ?? {}) },
    invokedCommands: [],
  };

  await page.addInitScript(({ mockData }) => {
    (window as any).__MOCK_TAURI__ = mockData;

    (window as any).__MOCK_TAURI_INVOKE__ = async (
      command: string,
      args?: Record<string, unknown>,
    ) => {
      const state = (window as any).__MOCK_TAURI__;
      const defaultDate = "2026-03-23T00:00:00.000Z";
      state.invokedCommands.push({ command, args });

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
        case "container_exec": {
          const cmd = Array.isArray(args?.cmd)
            ? (args?.cmd as unknown[]).map((item) => String(item))
            : [];
          return {
            exitCode: 0,
            stdout: `mock exec: ${cmd.join(" ")}\n`,
            stderr: "",
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
          const pod = {
            id: `pod-${name}`,
            name,
            driver: "bridge",
            createdAt: new Date().toISOString(),
            containers: [],
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
        case "docker_status":
          return state.dockerStatus;
        case "runtime_status":
          return state.runtimeStatus;
        case "runtime_start":
        case "runtime_stop":
          return "ok";
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

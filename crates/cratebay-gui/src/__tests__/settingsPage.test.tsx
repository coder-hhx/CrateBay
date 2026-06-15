import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SettingsPage } from "@/pages/SettingsPage";
import { useAppStore } from "@/stores/appStore";
import { useSettingsStore } from "@/stores/settingsStore";
import {
  DEFAULT_REGISTRY_MIRRORS,
  DEFAULT_RUNTIME_HTTP_PROXY,
  DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST,
  DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT,
  DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE,
  DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST,
} from "@/types/settings";

type RuntimeStatusMock = {
  connected: boolean;
  version: string | null;
  api_version: string | null;
  os: string | null;
  arch: string | null;
  source: string;
  socket_path: string;
  state: string;
  platform: string;
  cpu_cores: number;
  memory_mb: number;
  disk_gb: number;
  engine_responsive: boolean;
  compatibility_responsive: boolean;
  compatibility_version: string | null;
  docker_responsive: boolean;
  uptime_seconds: number | null;
};

let runtimeStatusMock: RuntimeStatusMock = {
  connected: true,
  version: "cratebay-containerd",
  api_version: "cratebay.engine.v1",
  os: "linux",
  arch: "arm64",
  source: "builtin",
  socket_path: "/tmp/cratebay/engine.sock",
  state: "ready",
  platform: "macos-vz",
  cpu_cores: 2,
  memory_mb: 2048,
  disk_gb: 20,
  engine_responsive: true,
  compatibility_responsive: true,
  compatibility_version: "cratebay-containerd",
  docker_responsive: true,
  uptime_seconds: 120,
};
let useCamelCaseStatusPayloads = false;
const runtimeResourceUsageMock = {
  cpu_percent: 18.5,
  memory_used_mb: 768,
  memory_total_mb: 2048,
  disk_used_gb: 6.5,
  disk_total_gb: 20,
  container_count: 2,
};
let engineStorageGcMock = {
  applied: false,
  candidateCount: 1,
  reclaimableBytes: 4096,
};
let engineShimTasksMock = [
  {
    id: "shim-task-abc123",
    name: "node-01",
    state: "running",
    image: "node:20-alpine",
  },
];

const engineContractMock = () => ({
  name: "CrateBay Engine",
  kind: "cratebay-containerd",
  adapter: { api: "cratebay.engine.v1" },
  backend: {
    runtime: "containerd",
    ociRuntime: "runc",
    namespace: "cratebay",
  },
  network: { stack: "CNI" },
  compatibility: { dockerCompatible: true },
});

const engineSubstrateMock = () => ({
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
});

vi.mock("@/lib/tauri", () => ({
  invoke: vi.fn((command: string, args?: Record<string, unknown>) => {
    if (command === "engine_status") {
      if (useCamelCaseStatusPayloads) {
        return Promise.resolve({
          connected: runtimeStatusMock.connected,
          version: runtimeStatusMock.version,
          apiVersion: runtimeStatusMock.api_version,
          os: runtimeStatusMock.os,
          arch: runtimeStatusMock.arch,
          engineSource: "builtin",
          source: runtimeStatusMock.source,
          socketPath: runtimeStatusMock.socket_path,
        });
      }
      return Promise.resolve({
        connected: runtimeStatusMock.connected,
        version: runtimeStatusMock.version,
        api_version: runtimeStatusMock.api_version,
        os: runtimeStatusMock.os,
        arch: runtimeStatusMock.arch,
        source: runtimeStatusMock.source,
        socket_path: runtimeStatusMock.socket_path,
      });
    }
    if (command === "runtime_status") {
      if (useCamelCaseStatusPayloads) {
        return Promise.resolve({
          state: runtimeStatusMock.state,
          platform: runtimeStatusMock.platform,
          cpuCores: runtimeStatusMock.cpu_cores,
          memoryMb: runtimeStatusMock.memory_mb,
          diskGb: runtimeStatusMock.disk_gb,
          engineResponsive: runtimeStatusMock.engine_responsive,
          compatibilityResponsive: runtimeStatusMock.compatibility_responsive,
          compatibilityVersion: runtimeStatusMock.compatibility_version,
          dockerResponsive: runtimeStatusMock.docker_responsive,
          uptimeSeconds: runtimeStatusMock.uptime_seconds,
          resourceUsage: {
            cpuPercent: runtimeResourceUsageMock.cpu_percent,
            memoryUsedMb: runtimeResourceUsageMock.memory_used_mb,
            memoryTotalMb: runtimeResourceUsageMock.memory_total_mb,
            diskUsedGb: runtimeResourceUsageMock.disk_used_gb,
            diskTotalGb: runtimeResourceUsageMock.disk_total_gb,
            containerCount: runtimeResourceUsageMock.container_count,
          },
        });
      }
      return Promise.resolve({
        state: runtimeStatusMock.state,
        platform: runtimeStatusMock.platform,
        cpu_cores: runtimeStatusMock.cpu_cores,
        memory_mb: runtimeStatusMock.memory_mb,
        disk_gb: runtimeStatusMock.disk_gb,
        engine_responsive: runtimeStatusMock.engine_responsive,
        compatibility_responsive: runtimeStatusMock.compatibility_responsive,
        compatibility_version: runtimeStatusMock.compatibility_version,
        docker_responsive: runtimeStatusMock.docker_responsive,
        uptime_seconds: runtimeStatusMock.uptime_seconds,
        resource_usage: runtimeResourceUsageMock,
      });
    }
    if (command === "runtime_start") {
      runtimeStatusMock = {
        ...runtimeStatusMock,
        connected: true,
        socket_path: "/tmp/cratebay/engine.sock",
        state: "ready",
        engine_responsive: true,
        compatibility_responsive: true,
        compatibility_version: "cratebay-containerd",
        docker_responsive: true,
      };
      return Promise.resolve("ok");
    }
    if (command === "runtime_provision") {
      runtimeStatusMock = {
        ...runtimeStatusMock,
        state: "provisioned",
        docker_responsive: false,
      };
      return Promise.resolve("ok");
    }
    if (command === "runtime_stop") {
      runtimeStatusMock = {
        ...runtimeStatusMock,
        connected: false,
        socket_path: "",
        state: "stopped",
        docker_responsive: false,
      };
      return Promise.resolve("ok");
    }
    if (command === "runtime_restart") {
      runtimeStatusMock = {
        ...runtimeStatusMock,
        connected: true,
        socket_path: "/tmp/cratebay/engine.sock",
        state: "ready",
        engine_responsive: true,
        compatibility_responsive: true,
        compatibility_version: "cratebay-containerd",
        docker_responsive: true,
      };
      return Promise.resolve("ok");
    }
    if (command === "runtime_diagnostics") {
      return Promise.resolve({
        ok: runtimeStatusMock.engine_responsive || runtimeStatusMock.compatibility_responsive,
        runtime: {
          state: runtimeStatusMock.state,
          platform: runtimeStatusMock.platform,
          cpu_cores: runtimeStatusMock.cpu_cores,
          memory_mb: runtimeStatusMock.memory_mb,
          disk_gb: runtimeStatusMock.disk_gb,
          engine_responsive: runtimeStatusMock.engine_responsive,
          compatibility_responsive: runtimeStatusMock.compatibility_responsive,
          compatibility_version: runtimeStatusMock.compatibility_version,
          docker_responsive: runtimeStatusMock.docker_responsive,
          uptime_seconds: runtimeStatusMock.uptime_seconds,
          resource_usage: runtimeResourceUsageMock,
        },
        engineContract: { ok: true, value: engineContractMock(), error: null },
        substrate: { ok: true, value: engineSubstrateMock(), error: null },
        storageGc: { ok: true, value: { ...engineStorageGcMock }, error: null },
        shimTasks: { ok: true, value: { items: engineShimTasksMock }, error: null },
        generatedAtUnix: 1,
      });
    }
    if (command === "engine_contract") {
      return Promise.resolve(engineContractMock());
    }
    if (command === "engine_substrate") {
      return Promise.resolve(engineSubstrateMock());
    }
    if (command === "engine_storage_gc") {
      const apply = Boolean(args?.apply);
      if (apply) {
        engineStorageGcMock = {
          ...engineStorageGcMock,
          applied: true,
          candidateCount: 0,
          reclaimableBytes: 0,
        };
      }
      return Promise.resolve({
        ...engineStorageGcMock,
      });
    }
    if (command === "engine_shim_tasks") {
      return Promise.resolve({
        items: engineShimTasksMock,
      });
    }
    if (command === "engine_shim_reap_task") {
      const id = String(args?.id ?? "");
      if (args?.apply) {
        engineShimTasksMock = engineShimTasksMock.filter((task) => task.id !== id);
      }
      return Promise.resolve({
        id,
        applied: true,
        reclaimableBytes: 4096,
      });
    }
    if (command === "app_update_check") {
      return Promise.resolve({
        available: false,
        currentVersion: "0.9.0",
        version: null,
        notes: null,
        pubDate: null,
        releaseUrl: null,
        channel: "stable",
        message: null,
      });
    }
    return Promise.resolve(null);
  }),
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

describe("SettingsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useCamelCaseStatusPayloads = false;
    runtimeStatusMock = {
      connected: true,
      version: "cratebay-containerd",
      api_version: "cratebay.engine.v1",
      os: "linux",
      arch: "arm64",
      source: "builtin",
      socket_path: "/tmp/cratebay/engine.sock",
      state: "ready",
      platform: "macos-vz",
      cpu_cores: 2,
      memory_mb: 2048,
      disk_gb: 20,
      engine_responsive: true,
      compatibility_responsive: true,
      compatibility_version: "cratebay-containerd",
      docker_responsive: true,
      uptime_seconds: 120,
    };
    engineStorageGcMock = {
      applied: false,
      candidateCount: 1,
      reclaimableBytes: 4096,
    };
    engineShimTasksMock = [
      {
        id: "shim-task-abc123",
        name: "node-01",
        state: "running",
        image: "node:20-alpine",
      },
    ];
    useAppStore.setState({
      currentPage: "settings",
      sidebarOpen: true,
      sidebarWidth: 260,
      engineConnected: false,
      dockerConnected: false,
      runtimeStatus: "stopped",
      runtimeLoading: false,
      theme: "dark",
    });
    useSettingsStore.setState({
      settings: {
        language: "en",
        theme: "dark",
        registryMirrors: ["docker.1ms.run"],
        runtimeHttpProxy: "",
        runtimeHttpProxyBridge: false,
        runtimeHttpProxyBindHost: "0.0.0.0",
        runtimeHttpProxyBindPort: 3128,
        runtimeHttpProxyGuestHost: "192.168.64.1",
        includePrereleases: false,
      },
    });
  });

  it("renders the flattened settings sections", async () => {
    render(<SettingsPage />);

    expect(screen.getByTestId("settings-section-general")).toBeInTheDocument();
    expect(screen.getByTestId("settings-section-updates")).toBeInTheDocument();
    expect(screen.getByTestId("settings-section-about")).toBeInTheDocument();
    expect(screen.getByTestId("settings-section-runtime")).toBeInTheDocument();
    await screen.findByText("CrateBay is up to date");
    await screen.findByText("Runtime Diagnostics");
  });

  it("shows runtime controls and registry mirror settings", async () => {
    render(<SettingsPage />);

    await waitFor(() => {
      expect(screen.getByText("Engine VM Control")).toBeInTheDocument();
      expect(screen.getByText("Engine VM HTTP Proxy")).toBeInTheDocument();
      expect(screen.getByText("Registry Mirrors")).toBeInTheDocument();
      expect(screen.getByText("docker.1ms.run")).toBeInTheDocument();
    });
  });

  it("restores default registry mirrors from the settings page", async () => {
    useSettingsStore.setState({
      settings: {
        ...useSettingsStore.getState().settings,
        registryMirrors: ["mirror.local"],
      },
    });

    render(<SettingsPage />);

    await waitFor(() => {
      expect(screen.getByText("Registry Mirrors")).toBeInTheDocument();
      expect(screen.getByText("mirror.local")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Restore Defaults" }));

    await waitFor(() => {
      expect(useSettingsStore.getState().settings.registryMirrors).toEqual(
        DEFAULT_REGISTRY_MIRRORS,
      );
    });
    for (const mirror of DEFAULT_REGISTRY_MIRRORS) {
      expect(screen.getByText(mirror)).toBeInTheDocument();
    }
  });

  it("shows runtime diagnostics", async () => {
    render(<SettingsPage />);

    await waitFor(() => {
      expect(screen.getByText("Runtime Diagnostics")).toBeInTheDocument();
      expect(screen.getByText("Engine Endpoint")).toBeInTheDocument();
      expect(screen.getAllByText("cratebay-containerd").length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("cratebay.engine.v1").length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("/tmp/cratebay/engine.sock").length).toBeGreaterThanOrEqual(1);
      expect(screen.getByText("macos-vz")).toBeInTheDocument();
      expect(screen.getByText("CPU usage")).toBeInTheDocument();
      expect(screen.getByText("18.5%")).toBeInTheDocument();
      expect(screen.getByText("Memory usage")).toBeInTheDocument();
      expect(screen.getByText("768 / 2048 MB")).toBeInTheDocument();
      expect(screen.getByText("Disk usage")).toBeInTheDocument();
      expect(screen.getByText("6.5 / 20 GB")).toBeInTheDocument();
      expect(screen.getByText("Runtime containers").parentElement).toHaveTextContent("2");
    });
  });

  it("shows runtime diagnostics from real Tauri camelCase status payloads", async () => {
    useCamelCaseStatusPayloads = true;
    render(<SettingsPage />);

    await waitFor(() => {
      expect(screen.getByText("Runtime Diagnostics")).toBeInTheDocument();
      expect(screen.getByText("Engine Endpoint")).toBeInTheDocument();
      expect(screen.getAllByText("cratebay.engine.v1").length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("/tmp/cratebay/engine.sock").length).toBeGreaterThanOrEqual(1);
      expect(screen.getByText("macos-vz")).toBeInTheDocument();
      expect(screen.getByText("768 / 2048 MB")).toBeInTheDocument();
      expect(screen.getByText("Runtime containers").parentElement).toHaveTextContent("2");
    });
  });

  it("saves runtime proxy bridge settings from the runtime tab", async () => {
    render(<SettingsPage />);

    await waitFor(() => {
      expect(screen.getByText("Engine VM HTTP Proxy")).toBeInTheDocument();
    });

    const proxyInput = screen.getByPlaceholderText("127.0.0.1:7890");
    fireEvent.change(proxyInput, { target: { value: "http://127.0.0.1:7890" } });
    fireEvent.click(screen.getByLabelText("Enable Proxy Bridge (macOS)"));
    fireEvent.change(screen.getByPlaceholderText("0.0.0.0"), { target: { value: "127.0.0.1" } });
    fireEvent.change(screen.getByPlaceholderText("3128"), { target: { value: "4567" } });
    fireEvent.change(screen.getByPlaceholderText("192.168.64.1"), {
      target: { value: "192.168.64.254" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Save Proxy Settings" }));

    await waitFor(() => {
      expect(useSettingsStore.getState().settings.runtimeHttpProxy).toBe("http://127.0.0.1:7890");
      expect(useSettingsStore.getState().settings.runtimeHttpProxyBridge).toBe(true);
      expect(useSettingsStore.getState().settings.runtimeHttpProxyBindHost).toBe("127.0.0.1");
      expect(useSettingsStore.getState().settings.runtimeHttpProxyBindPort).toBe(4567);
      expect(useSettingsStore.getState().settings.runtimeHttpProxyGuestHost).toBe("192.168.64.254");
    });
    await screen.findByText("Runtime proxy settings saved");
  });

  it("resets runtime proxy bridge settings from the runtime tab", async () => {
    useSettingsStore.setState({
      settings: {
        ...useSettingsStore.getState().settings,
        runtimeHttpProxy: "http://127.0.0.1:7890",
        runtimeHttpProxyBridge: true,
        runtimeHttpProxyBindHost: "127.0.0.1",
        runtimeHttpProxyBindPort: 4567,
        runtimeHttpProxyGuestHost: "192.168.64.254",
      },
    });

    render(<SettingsPage />);

    await waitFor(() => {
      expect(screen.getByText("Engine VM HTTP Proxy")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Reset Proxy Settings" }));

    await waitFor(() => {
      expect(useSettingsStore.getState().settings.runtimeHttpProxy).toBe(DEFAULT_RUNTIME_HTTP_PROXY);
      expect(useSettingsStore.getState().settings.runtimeHttpProxyBridge).toBe(
        DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE,
      );
      expect(useSettingsStore.getState().settings.runtimeHttpProxyBindHost).toBe(
        DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST,
      );
      expect(useSettingsStore.getState().settings.runtimeHttpProxyBindPort).toBe(
        DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT,
      );
      expect(useSettingsStore.getState().settings.runtimeHttpProxyGuestHost).toBe(
        DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST,
      );
    });
    await screen.findByText("Runtime proxy settings cleared");
  });

  it("rejects invalid runtime proxy bridge ports", async () => {
    render(<SettingsPage />);

    await waitFor(() => {
      expect(screen.getByText("Engine VM HTTP Proxy")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText("3128"), { target: { value: "0" } });
    fireEvent.click(screen.getByRole("button", { name: "Save Proxy Settings" }));

    await screen.findByText("Runtime proxy settings failed");
    await screen.findByText("Runtime proxy bind port must be a whole number from 1 to 65535.");
    expect(useSettingsStore.getState().settings.runtimeHttpProxyBindPort).toBe(3128);
  });

  it("shows native Engine maintenance and can run maintenance actions", async () => {
    const { invoke } = await import("@/lib/tauri");
    render(<SettingsPage />);

    await waitFor(() => {
      expect(screen.getByText("Engine Maintenance")).toBeInTheDocument();
      expect(invoke).toHaveBeenCalledWith("runtime_diagnostics", {
        prune_exited_containers: true,
      });
      expect(screen.getByText("Engine Contract")).toBeInTheDocument();
      expect(screen.getAllByText("cratebay-containerd").length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText("cratebay.engine.v1").length).toBeGreaterThanOrEqual(1);
      expect(screen.getByText("cratebay")).toBeInTheDocument();
      expect(screen.getByText("Native Substrate")).toBeInTheDocument();
      expect(screen.getByText("containerd task service")).toBeInTheDocument();
      expect(screen.getByText("node-01")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: /Apply GC/ }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("engine_storage_gc", {
        apply: true,
        prune_exited_containers: true,
      });
    });
    await screen.findByText("Storage GC complete");

    fireEvent.click(screen.getByRole("button", { name: /Reap/ }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("engine_shim_reap_task", {
        id: "shim-task-abc123",
        apply: true,
      });
    });
    await screen.findByText(/Reaped shim task/);
    await screen.findByText("No shim tasks found.");
  });

  it("starts the built-in runtime from the runtime tab", async () => {
    runtimeStatusMock = {
      ...runtimeStatusMock,
      connected: false,
      state: "stopped",
      engine_responsive: false,
      compatibility_responsive: false,
      compatibility_version: null,
      docker_responsive: false,
    };
    const { invoke } = await import("@/lib/tauri");
    render(<SettingsPage />);

    await waitFor(() => {
      expect(screen.getByText("Engine VM Control")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "Start" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("runtime_start");
    });
    await screen.findByText("Engine VM started");
    expect(screen.getByTestId("runtime-operation-result")).toHaveTextContent("State: ready");
    expect(screen.getByTestId("runtime-operation-result")).toHaveTextContent(
      "Endpoint: /tmp/cratebay/engine.sock",
    );
  });

  it("provisions the built-in runtime image from the runtime tab", async () => {
    runtimeStatusMock = {
      ...runtimeStatusMock,
      connected: false,
      state: "none",
      engine_responsive: false,
      compatibility_responsive: false,
      compatibility_version: null,
      docker_responsive: false,
    };
    const { invoke } = await import("@/lib/tauri");
    render(<SettingsPage />);

    await waitFor(() => {
      expect(screen.getByText("Engine VM Control")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "Provision Runtime Image" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("runtime_provision");
    });
    await screen.findByText("Runtime image provisioned");
    expect(screen.getByTestId("runtime-operation-result")).toHaveTextContent("State: provisioned");
  });

  it("restarts the built-in runtime through the native restart command", async () => {
    useAppStore.setState({ runtimeStatus: "running", engineConnected: true });
    const { invoke } = await import("@/lib/tauri");
    render(<SettingsPage />);

    await waitFor(() => {
      expect(screen.getByText("Engine VM Control")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "Restart Engine VM" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("runtime_restart");
    });
    expect(invoke).not.toHaveBeenCalledWith("runtime_stop");
    await screen.findByText("Engine VM restarted");
  });
});

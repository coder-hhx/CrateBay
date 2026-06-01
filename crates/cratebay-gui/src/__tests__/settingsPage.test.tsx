import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SettingsPage } from "@/pages/SettingsPage";
import { useAppStore } from "@/stores/appStore";
import { useSettingsStore } from "@/stores/settingsStore";

let runtimeStatusMock = {
  connected: true,
  version: "25.0.0",
  api_version: "1.44",
  os: "linux",
  arch: "arm64",
  source: "builtin",
  socket_path: "/tmp/docker.sock",
  state: "ready",
  platform: "macos-vz",
  cpu_cores: 2,
  memory_mb: 2048,
  disk_gb: 20,
  docker_responsive: true,
  uptime_seconds: 120,
};

vi.mock("@/lib/tauri", () => ({
  invoke: vi.fn((command: string) => {
    if (command === "docker_status") {
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
      return Promise.resolve({
        state: runtimeStatusMock.state,
        platform: runtimeStatusMock.platform,
        cpu_cores: runtimeStatusMock.cpu_cores,
        memory_mb: runtimeStatusMock.memory_mb,
        disk_gb: runtimeStatusMock.disk_gb,
        docker_responsive: runtimeStatusMock.docker_responsive,
        uptime_seconds: runtimeStatusMock.uptime_seconds,
        resource_usage: null,
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
    runtimeStatusMock = {
      connected: true,
      version: "25.0.0",
      api_version: "1.44",
      os: "linux",
      arch: "arm64",
      source: "builtin",
      socket_path: "/tmp/docker.sock",
      state: "ready",
      platform: "macos-vz",
      cpu_cores: 2,
      memory_mb: 2048,
      disk_gb: 20,
      docker_responsive: true,
      uptime_seconds: 120,
    };
    useAppStore.setState({
      currentPage: "settings",
      sidebarOpen: true,
      sidebarWidth: 260,
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
      },
    });
  });

  it("renders the supported settings tabs only", () => {
    render(<SettingsPage />);

    const tabIds = Array.from(
      document.querySelectorAll('[data-testid^="settings-tab-"]'),
    ).map((el) => el.getAttribute("data-testid"));

    expect(tabIds).toEqual([
      "settings-tab-general",
      "settings-tab-runtime",
      "settings-tab-about",
    ]);
  });

  it("shows runtime controls and registry mirror settings", async () => {
    render(<SettingsPage />);

    fireEvent.mouseDown(screen.getByTestId("settings-tab-runtime"));

    await waitFor(() => {
      expect(screen.getByText("Runtime Control")).toBeInTheDocument();
      expect(screen.getByText("Runtime HTTP Proxy")).toBeInTheDocument();
      expect(screen.getByText("Registry Mirrors")).toBeInTheDocument();
      expect(screen.getByText("docker.1ms.run")).toBeInTheDocument();
    });
  });

  it("shows runtime diagnostics", async () => {
    render(<SettingsPage />);

    fireEvent.mouseDown(screen.getByTestId("settings-tab-runtime"));

    await waitFor(() => {
      expect(screen.getByText("Runtime Diagnostics")).toBeInTheDocument();
      expect(screen.getByText("Docker Endpoint")).toBeInTheDocument();
      expect(screen.getByText("25.0.0")).toBeInTheDocument();
      expect(screen.getByText("1.44")).toBeInTheDocument();
      expect(screen.getByText("/tmp/docker.sock")).toBeInTheDocument();
      expect(screen.getByText("macos-vz")).toBeInTheDocument();
    });
  });

  it("starts the built-in runtime from the runtime tab", async () => {
    runtimeStatusMock = {
      ...runtimeStatusMock,
      connected: false,
      state: "stopped",
      docker_responsive: false,
    };
    const { invoke } = await import("@/lib/tauri");
    render(<SettingsPage />);

    fireEvent.mouseDown(screen.getByTestId("settings-tab-runtime"));
    await waitFor(() => {
      expect(screen.getByText("Runtime Control")).toBeInTheDocument();
    });
    fireEvent.click(screen.getByRole("button", { name: "Start" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("runtime_start");
    });
  });
});

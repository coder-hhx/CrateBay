import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { DashboardPage } from "@/pages/DashboardPage";
import { useSettingsStore } from "@/stores/settingsStore";

const invokeMock = vi.fn();

vi.mock("@/lib/tauri", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("DashboardPage", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("treats disabled runtime auto-start as an offline dashboard state", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "engine_status") {
        return Promise.resolve({
          connected: false,
          version: null,
          api_version: null,
          os: null,
          arch: null,
          engine_source: "builtin",
          source: "builtin",
          socket_path: null,
        });
      }
      if (command === "runtime_status") {
        return Promise.resolve({
          state: "stopped",
          platform: "macos-vz",
          cpu_cores: 2,
          memory_mb: 2048,
          disk_gb: 20,
          engine_responsive: false,
          compatibility_responsive: false,
          docker_responsive: false,
          engine_source: "builtin",
          docker_source: "builtin",
          uptime_seconds: null,
          resource_usage: null,
        });
      }
      if (command === "runtime_start") {
        return Promise.resolve("ok");
      }
      return Promise.reject(
        new Error("Implicit runtime start disabled by CRATEBAY_DISABLE_RUNTIME_AUTO_START"),
      );
    });

    render(<DashboardPage />);

    await waitFor(() => {
      expect(screen.getByText("Offline")).toBeInTheDocument();
    });
    expect(invokeMock).not.toHaveBeenCalledWith("container_list");
    expect(invokeMock).not.toHaveBeenCalledWith("image_list");
    expect(
      screen.queryByText(/CRATEBAY_DISABLE_RUNTIME_AUTO_START/),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Containers")).toBeInTheDocument();
    expect(screen.getAllByText("0").length).toBeGreaterThanOrEqual(5);

    fireEvent.click(screen.getByRole("button", { name: "Start Engine" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("runtime_start");
    });
  });

  it("does not load native counters from a compatibility-only endpoint", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "engine_status") {
        return Promise.resolve({
          connected: true,
          version: "25.0.0",
          api_version: "1.44",
          os: "linux",
          arch: "arm64",
          engine_source: "builtin",
          source: "builtin",
          socket_path: "/tmp/cratebay/engine.sock",
        });
      }
      if (command === "runtime_status") {
        return Promise.resolve({
          state: "starting",
          platform: "macos-vz",
          cpu_cores: 2,
          memory_mb: 2048,
          disk_gb: 20,
          engine_responsive: false,
          compatibility_responsive: true,
          docker_responsive: true,
          engine_source: "builtin",
          docker_source: "builtin",
          uptime_seconds: null,
          resource_usage: null,
        });
      }
      return Promise.reject(new Error(`unexpected native command: ${command}`));
    });

    render(<DashboardPage />);

    await waitFor(() => {
      expect(screen.getByText("Offline")).toBeInTheDocument();
    });

    expect(invokeMock).not.toHaveBeenCalledWith("container_list");
    expect(invokeMock).not.toHaveBeenCalledWith("image_list");
    expect(invokeMock).not.toHaveBeenCalledWith("pod_list");
    expect(invokeMock).not.toHaveBeenCalledWith("volume_list");
    expect(invokeMock).not.toHaveBeenCalledWith("network_list");
  });

  it("loads resource counters from the native management commands", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "engine_status") {
        return Promise.resolve({
          connected: true,
          version: "25.0.0",
          api_version: "1.44",
          os: "linux",
          arch: "arm64",
          engine_source: "builtin",
          source: "builtin",
          socket_path: "/tmp/cratebay/engine.sock",
        });
      }
      if (command === "runtime_status") {
        return Promise.resolve({
          state: "ready",
          platform: "macos-vz",
          cpu_cores: 2,
          memory_mb: 2048,
          disk_gb: 20,
          engine_responsive: true,
          compatibility_responsive: true,
          docker_responsive: true,
          engine_source: "builtin",
          docker_source: "builtin",
          uptime_seconds: 120,
          resource_usage: {
            cpu_percent: 18.5,
            memory_used_mb: 768,
            memory_total_mb: 2048,
            disk_used_gb: 6.5,
            disk_total_gb: 20,
            container_count: 2,
          },
          engine: {
            name: "CrateBay Engine",
            backend: {
              runtime: "containerd",
              ociRuntime: "runc",
            },
            network: {
              stack: "CNI",
            },
            adapter: {
              api: "cratebay.engine.v1",
            },
          },
        });
      }
      if (command === "container_list") {
        return Promise.resolve([
          { id: "abc123", name: "node-01", status: "running" },
          { id: "def456", name: "python-dev", status: "stopped" },
        ]);
      }
      if (command === "image_list") {
        return Promise.resolve([{ id: "sha256:node" }]);
      }
      if (command === "pod_list") {
        return Promise.resolve([{ id: "pod-web", name: "web" }]);
      }
      if (command === "volume_list") {
        return Promise.resolve([{ name: "workspace-cache" }]);
      }
      if (command === "network_list") {
        return Promise.resolve([{ id: "net-workspace", name: "workspace-net" }]);
      }
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });

    render(<DashboardPage />);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("container_list");
      expect(invokeMock).toHaveBeenCalledWith("image_list");
      expect(invokeMock).toHaveBeenCalledWith("pod_list");
      expect(invokeMock).toHaveBeenCalledWith("volume_list");
      expect(invokeMock).toHaveBeenCalledWith("network_list");
    });

    expect(screen.getAllByText("2").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("1 running")).toBeInTheDocument();
    expect(screen.getAllByText("1").length).toBeGreaterThanOrEqual(4);
    expect(screen.getByText("Online")).toBeInTheDocument();
    expect(screen.getByText("cratebay.engine.v1")).toBeInTheDocument();
    expect(screen.getByText("Runtime containers").parentElement).toHaveTextContent("2");
  });

  it("shows endpoint details from real Tauri camelCase status payloads", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "engine_status") {
        return Promise.resolve({
          connected: true,
          version: "cratebay-containerd",
          apiVersion: "cratebay.engine.v1",
          os: "linux",
          arch: "arm64",
          engineSource: "builtin",
          source: "builtin",
          socketPath: "/tmp/cratebay/engine.sock",
        });
      }
      if (command === "runtime_status") {
        return Promise.resolve({
          state: "ready",
          platform: "macos-vz",
          cpuCores: 2,
          memoryMb: 2048,
          diskGb: 20,
          engineResponsive: false,
          compatibilityResponsive: true,
          dockerResponsive: true,
          engineSource: "builtin",
          dockerSource: "builtin",
          uptimeSeconds: 120,
          resourceUsage: {
            cpuPercent: 18.5,
            memoryUsedMb: 768,
            memoryTotalMb: 2048,
            diskUsedGb: 6.5,
            diskTotalGb: 20,
            containerCount: 2,
          },
        });
      }
      if (command === "container_list") return Promise.resolve([]);
      if (command === "image_list") return Promise.resolve([]);
      if (command === "pod_list") return Promise.resolve([]);
      if (command === "volume_list") return Promise.resolve([]);
      if (command === "network_list") return Promise.resolve([]);
      return Promise.reject(new Error(`unexpected command: ${command}`));
    });

    render(<DashboardPage />);

    await waitFor(() => {
      expect(screen.getByText("/tmp/cratebay/engine.sock")).toBeInTheDocument();
    });
    expect(screen.getByText(/native API cratebay\.engine\.v1 · cratebay-containerd/)).toBeInTheDocument();
    expect(screen.queryByText(/compatibility API cratebay-containerd/)).not.toBeInTheDocument();
  });
});

import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ContainerMonitoring } from "@/components/container/ContainerMonitoring";
import { useSettingsStore } from "@/stores/settingsStore";

const invokeMock = vi.fn();

vi.mock("@/lib/tauri", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

describe("ContainerMonitoring", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "zh-CN",
      },
    }));
  });

  it("localizes CPU core units in the active GUI language", async () => {
    invokeMock.mockResolvedValue({
      id: "container-1",
      name: "node-01",
      readAt: "2026-06-15T00:00:00.000Z",
      cpuPercent: 25,
      cpuCoresUsed: 0.5,
      memoryUsedMb: 128,
      memoryLimitMb: 1024,
      memoryPercent: 12.5,
    });

    render(<ContainerMonitoring containerId="container-1" cpuCores={2} memoryMb={1024} />);

    expect(await screen.findByText("CPU")).toBeInTheDocument();
    expect(screen.getByText("内存")).toBeInTheDocument();
    expect(await screen.findByText(/0\.50 \/ 2 核心/)).toBeInTheDocument();
    expect(screen.queryByText(/cores/)).not.toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("container_stats", { id: "container-1" });
  });
});

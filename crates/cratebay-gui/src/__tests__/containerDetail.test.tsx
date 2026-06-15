import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { mockInvoke, resetTauriMocks } from "@/__mocks__/tauriMock";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mockInvoke,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("@/lib/tauri", () => ({
  invoke: mockInvoke,
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

vi.mock("@/components/container/ContainerLogs", () => ({
  ContainerLogs: () => <div data-testid="mock-container-logs" />,
}));
vi.mock("@/components/container/ContainerExec", () => ({
  ContainerExec: () => <div data-testid="mock-container-exec" />,
}));
vi.mock("@/components/container/TerminalView", () => ({
  TerminalView: () => <div data-testid="mock-terminal-view" />,
}));
vi.mock("@/components/container/ContainerMonitoring", () => ({
  ContainerMonitoring: () => <div data-testid="mock-container-monitoring" />,
}));

import { ContainerDetail } from "@/components/container/ContainerDetail";
import { useContainerStore } from "@/stores/containerStore";
import { useSettingsStore } from "@/stores/settingsStore";

describe("ContainerDetail", () => {
  beforeEach(() => {
    resetTauriMocks();
    document.body.innerHTML = "";
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
    useContainerStore.setState({
      containers: [
        {
          id: "container-1",
          shortId: "abc123",
          name: "node-01",
          image: "node:20-alpine",
          status: "running",
          state: "running",
          createdAt: new Date(Date.now() - 5 * 60_000).toISOString(),
          cpuCores: 2,
          memoryMb: 2048,
          ports: [],
          labels: {},
        },
      ],
      selectedContainerId: "container-1",
      loading: false,
      error: null,
      images: [],
      imagesLoading: false,
      templates: [],
      filter: { status: "all", search: "", templateId: null },
    });
  });

  it("renders detail panel labels in English", async () => {
    mockInvoke.mockImplementation((command: string) => {
      if (command === "container_inspect") {
        return Promise.resolve({
          info: {},
          networkSettings: { Networks: {} },
          mounts: [],
          state: {
            status: "running",
            running: true,
            startedAt: null,
            finishedAt: null,
            exitCode: null,
            error: null,
            pid: 1234,
          },
        });
      }
      return Promise.resolve(null);
    });

    render(
      <main>
        <ContainerDetail />
      </main>,
    );

    expect(await screen.findByTestId("container-detail")).toBeInTheDocument();
    expect(screen.getByTitle("Stop")).toBeInTheDocument();
    expect(screen.getByTitle("Delete")).toBeInTheDocument();
    expect(screen.getAllByTitle("Copy").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Specs")).toBeInTheDocument();
    expect(screen.getByText("2 cores")).toBeInTheDocument();
    expect(screen.getByText("5 min ago")).toBeInTheDocument();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("container_inspect", { id: "container-1" });
    });
    expect(screen.queryByText(/规格|核心|刚刚|分钟前|复制|停止|删除/)).not.toBeInTheDocument();
  });
});

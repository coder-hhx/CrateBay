import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ContainersPage } from "@/pages/ContainersPage";
import { useAppStore } from "@/stores/appStore";
import { useContainerStore } from "@/stores/containerStore";
import { useSettingsStore } from "@/stores/settingsStore";

const invokeMock = vi.fn();

vi.mock("@/lib/tauri", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  listen: vi.fn(() => Promise.resolve(() => undefined)),
}));

vi.mock("@/components/container/ContainerCreate", () => ({
  ContainerCreate: () => <button type="button">Create</button>,
}));

vi.mock("@/components/container/ContainerRun", () => ({
  ContainerRun: () => <button type="button">Run</button>,
}));

vi.mock("@/components/container/ContainerDetail", () => ({
  ContainerDetail: () => null,
}));

describe("ContainersPage", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    useContainerStore.setState({
      containers: [],
      loading: false,
      error: null,
      _fetchAbortController: null,
      _transitionalRefreshTimer: null,
      selectedContainerId: null,
      templates: [],
      filter: { status: "all", search: "", templateId: null },
    });
    useAppStore.setState({
      engineConnected: false,
      dockerConnected: false,
      builtinRuntimeReady: false,
      runtimeStatus: "stopped",
    });
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("starts the Engine when container listing finds auto-start disabled", async () => {
    let containerListCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "container_list") {
        containerListCalls += 1;
        if (containerListCalls === 1) {
          return Promise.reject(
            new Error("Implicit runtime start disabled by CRATEBAY_DISABLE_RUNTIME_AUTO_START"),
          );
        }
        return Promise.resolve([
          {
            id: "container-after-start",
            shortId: "container-after-start",
            name: "container-after-start",
            image: "alpine:latest",
            status: "running",
            state: "running",
            createdAt: "2026-06-14T00:00:00Z",
            ports: [],
            labels: {},
          },
        ]);
      }
      if (command === "container_templates") {
        return Promise.resolve([]);
      }
      if (command === "runtime_start") {
        return Promise.resolve("ok");
      }
      return Promise.resolve(null);
    });

    render(<ContainersPage />);

    expect(await screen.findByText("Engine is offline")).toBeInTheDocument();
    expect(screen.queryByText(/CRATEBAY_DISABLE_RUNTIME_AUTO_START/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Start Engine" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("runtime_start");
    });
    expect(await screen.findByText("container-after-start")).toBeInTheDocument();
  });
});

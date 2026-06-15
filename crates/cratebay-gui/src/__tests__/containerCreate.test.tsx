import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { mockInvoke, resetTauriMocks } from "@/__mocks__/tauriMock";

vi.mock("@/lib/tauri", () => ({
  invoke: mockInvoke,
  listen: vi.fn(() => Promise.resolve(() => undefined)),
  isTauri: vi.fn(() => false),
}));

import { ContainerCreate } from "@/components/container/ContainerCreate";
import { useContainerStore } from "@/stores/containerStore";
import { useSettingsStore } from "@/stores/settingsStore";

describe("ContainerCreate", () => {
  beforeEach(() => {
    resetTauriMocks();
    useContainerStore.setState({
      containers: [],
      images: [],
      imagesLoading: false,
      loading: false,
      error: null,
      selectedContainerId: null,
      templates: [],
      filter: { status: "all", search: "", templateId: null },
      _fetchAbortController: null,
      _transitionalRefreshTimer: null,
    });
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
        registryMirrors: [],
      },
    }));
  });

  it("submits SCTP port mappings through the native create request", async () => {
    mockInvoke.mockImplementation((command) => {
      if (command === "image_list") {
        return Promise.resolve([
          {
            id: "img-1",
            repoTags: ["alpine:latest"],
            sizeBytes: 1024,
            sizeHuman: "1 KB",
            created: 1,
          },
        ]);
      }
      if (command === "pod_list" || command === "network_list" || command === "volume_list") {
        return Promise.resolve([]);
      }
      if (command === "container_create") {
        return Promise.resolve({
          id: "container-1",
          shortId: "container-1",
          name: "sctp-box",
          image: "alpine:latest",
          status: "running",
          state: "running",
          createdAt: "2026-06-15T00:00:00Z",
          ports: [],
          labels: {},
        });
      }
      if (command === "container_list") {
        return Promise.resolve([]);
      }
      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    render(<ContainerCreate />);

    fireEvent.click(screen.getByRole("button", { name: "Create Container" }));
    fireEvent.change(await screen.findByLabelText("Name (optional)"), {
      target: { value: "sctp-box" },
    });
    fireEvent.change(screen.getByLabelText("Select image"), {
      target: { value: "alpine:latest" },
    });
    fireEvent.change(screen.getByLabelText("Publish port"), {
      target: { value: "5000:5000/sctp" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create Container" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("container_create", {
        request: expect.objectContaining({
          name: "sctp-box",
          image: "alpine:latest",
          ports: [
            {
              hostPort: 5000,
              containerPort: 5000,
              protocol: "sctp",
            },
          ],
          autoStart: true,
        }),
      });
    });
  });
});

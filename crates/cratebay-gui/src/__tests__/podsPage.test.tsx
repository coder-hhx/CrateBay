import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ContainerInfo } from "@/types/container";
import type { PodContainerInfo } from "@/types/pod";
import { useContainerStore } from "@/stores/containerStore";
import { useSettingsStore } from "@/stores/settingsStore";

const invokeMock = vi.fn();

vi.mock("@/lib/tauri", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  listen: vi.fn(() => Promise.resolve(() => undefined)),
}));

import { isContainerAttachedToPod, PodsPage } from "@/pages/PodsPage";

const makeContainer = (overrides: Partial<ContainerInfo> = {}): ContainerInfo => ({
  id: "abcdef1234567890",
  shortId: "abcdef123456",
  name: "node-01",
  image: "node:20-alpine",
  status: "running",
  state: "running",
  createdAt: "2026-03-23T00:00:00.000Z",
  ports: [],
  labels: {},
  ...overrides,
});

const makePodContainer = (
  overrides: Partial<PodContainerInfo> = {},
): PodContainerInfo => ({
  id: "abcdef1234567890",
  name: "node-01",
  ipv4Address: null,
  ipv6Address: null,
  ...overrides,
});

describe("PodsPage helpers", () => {
  it("matches pod membership by full or short container id", () => {
    const container = makeContainer();

    expect(isContainerAttachedToPod(container, [makePodContainer()])).toBe(true);
    expect(
      isContainerAttachedToPod(container, [
        makePodContainer({ id: "abcdef123456", name: "" }),
      ]),
    ).toBe(true);
  });

  it("matches pod membership by normalized container name", () => {
    const container = makeContainer({ id: "1111111111112222", shortId: "111111111111" });

    expect(
      isContainerAttachedToPod(container, [
        makePodContainer({ id: "3333333333334444", name: "/node-01" }),
      ]),
    ).toBe(true);
  });

  it("does not match unrelated short identities", () => {
    expect(
      isContainerAttachedToPod(makeContainer(), [
        makePodContainer({ id: "abcdef", name: "other" }),
      ]),
    ).toBe(false);
  });
});

describe("PodsPage", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    useContainerStore.setState({
      containers: [],
      loading: false,
      error: null,
      _fetchAbortController: null,
      _transitionalRefreshTimer: null,
    });
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("creates a pod with native network options", async () => {
    invokeMock
      .mockImplementation((command: string, args?: Record<string, unknown>) => {
        if (command === "pod_list") {
          return Promise.resolve([]);
        }
        if (command === "container_list") {
          return Promise.resolve([]);
        }
        if (command === "pod_create") {
          return Promise.resolve({
            id: `pod-${args?.name}`,
            name: args?.name,
            driver: args?.driver,
            createdAt: "2026-06-14T00:00:00Z",
            containers: [],
          });
        }
        return Promise.resolve(null);
      });

    render(<PodsPage />);

    const input = await screen.findByPlaceholderText("pod-name");
    fireEvent.change(input, { target: { value: "api-stack" } });
    fireEvent.change(screen.getByPlaceholderText("driver"), { target: { value: "macvlan" } });
    fireEvent.click(screen.getByText("Internal"));
    fireEvent.click(screen.getByText("IPv6"));
    fireEvent.click(screen.getByRole("button", { name: /create/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("pod_create", {
        name: "api-stack",
        driver: "macvlan",
        internal: true,
        enableIpv6: true,
      });
    });
  });

  it("starts the Engine when pod listing finds auto-start disabled", async () => {
    let podListCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "pod_list") {
        podListCalls += 1;
        if (podListCalls === 1) {
          return Promise.reject(
            new Error("Implicit runtime start disabled by CRATEBAY_DISABLE_RUNTIME_AUTO_START"),
          );
        }
        return Promise.resolve([
          {
            id: "pod-after-start",
            name: "pod-after-start",
            driver: "bridge",
            createdAt: "2026-06-14T00:00:00Z",
            containers: [],
          },
        ]);
      }
      if (command === "container_list") {
        return Promise.resolve([]);
      }
      if (command === "runtime_start") {
        return Promise.resolve("ok");
      }
      return Promise.resolve(null);
    });

    render(<PodsPage />);

    expect(await screen.findByText("Engine is offline")).toBeInTheDocument();
    expect(screen.queryByText(/CRATEBAY_DISABLE_RUNTIME_AUTO_START/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Start Engine" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("runtime_start");
    });
    expect(await screen.findByText("pod-after-start")).toBeInTheDocument();
  });

  it("deletes a pod without force by default", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "pod_list") {
        return Promise.resolve([
          {
            id: "pod-api-stack",
            name: "api-stack",
            driver: "bridge",
            createdAt: "2026-06-14T00:00:00Z",
            containers: [],
          },
        ]);
      }
      if (command === "container_list") {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    render(<PodsPage />);

    await screen.findByText("api-stack");
    fireEvent.click(screen.getByTitle("Delete Pod"));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("pod_delete", {
        name: "api-stack",
        force: false,
      });
    });
  });

  it("passes force when deleting a pod with the force checkbox enabled", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "pod_list") {
        return Promise.resolve([
          {
            id: "pod-api-stack",
            name: "api-stack",
            driver: "bridge",
            createdAt: "2026-06-14T00:00:00Z",
            containers: [],
          },
        ]);
      }
      if (command === "container_list") {
        return Promise.resolve([]);
      }
      return Promise.resolve(null);
    });

    render(<PodsPage />);

    await screen.findByText("api-stack");
    fireEvent.click(screen.getByTitle("Delete Pod"));
    fireEvent.click(screen.getByRole("checkbox", { name: "Disconnect containers and delete" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("pod_delete", {
        name: "api-stack",
        force: true,
      });
    });
  });

  it("shows the native pod removal error inside the delete dialog", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "pod_list") {
        return Promise.resolve([
          {
            id: "pod-api-stack",
            name: "api-stack",
            driver: "bridge",
            createdAt: "2026-06-14T00:00:00Z",
            containers: [],
          },
        ]);
      }
      if (command === "container_list") {
        return Promise.resolve([]);
      }
      if (command === "pod_delete") {
        return Promise.reject(new Error("pod is in use by CrateBay containers"));
      }
      return Promise.resolve(null);
    });

    render(<PodsPage />);

    await screen.findByText("api-stack");
    fireEvent.click(screen.getByTitle("Delete Pod"));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("pod is in use by CrateBay containers")).toBeInTheDocument();
  });
});

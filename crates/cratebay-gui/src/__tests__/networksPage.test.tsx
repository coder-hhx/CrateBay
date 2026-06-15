import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { NetworksPage } from "@/pages/NetworksPage";
import { useSettingsStore } from "@/stores/settingsStore";

const invokeMock = vi.fn();

vi.mock("@/lib/tauri", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function network(
  name = "workspace-net",
  overrides: Partial<ReturnType<typeof baseNetwork>> = {},
) {
  return {
    ...baseNetwork(name),
    ...overrides,
  };
}

function baseNetwork(name = "workspace-net") {
  return {
    id: `net-${name}`,
    name,
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
  };
}

describe("NetworksPage", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("renders native Engine networks", async () => {
    invokeMock.mockResolvedValueOnce([network()]);

    render(<NetworksPage />);

    expect(await screen.findByText("workspace-net")).toBeInTheDocument();
    expect(screen.getByText("bridge")).toBeInTheDocument();
    expect(screen.getAllByText("local").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Attachable")).toBeInTheDocument();
  });

  it("creates a network and refreshes the list", async () => {
    invokeMock
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce(network("sandbox-net", { driver: "macvlan", internal: true }))
      .mockResolvedValueOnce([network("sandbox-net", { driver: "macvlan", internal: true })]);

    render(<NetworksPage />);

    const input = await screen.findByPlaceholderText("network-name");
    fireEvent.change(input, { target: { value: "sandbox-net" } });
    fireEvent.change(screen.getByPlaceholderText("driver"), { target: { value: "macvlan" } });
    fireEvent.click(screen.getByText("Internal"));
    fireEvent.click(screen.getByText("IPv6"));
    fireEvent.click(screen.getByRole("button", { name: /create/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("network_create", {
        name: "sandbox-net",
        driver: "macvlan",
        internal: true,
        enableIpv6: true,
      });
    });
    expect(await screen.findByText("sandbox-net")).toBeInTheDocument();
    expect(screen.getByText("macvlan")).toBeInTheDocument();
    expect(screen.getAllByText("Internal").length).toBeGreaterThanOrEqual(1);
  });

  it("opens inspect details from native network inspect", async () => {
    invokeMock.mockResolvedValueOnce([network()]).mockResolvedValueOnce(network());

    render(<NetworksPage />);

    await screen.findByText("workspace-net");
    fireEvent.click(screen.getByTitle("Inspect Network"));

    expect(await screen.findByText("Managed by")).toBeInTheDocument();
    expect(screen.getByText("cratebay-engine")).toBeInTheDocument();
    expect(screen.getByText(/endpoint-abc123/)).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("network_inspect", { id: "net-workspace-net" });
  });

  it("passes force when deleting a network from the confirmation dialog", async () => {
    invokeMock
      .mockResolvedValueOnce([network()])
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([]);

    render(<NetworksPage />);

    await screen.findByText("workspace-net");
    fireEvent.click(screen.getByTitle("Delete Network"));
    fireEvent.click(screen.getByText("Detach containers and delete"));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("network_delete", {
        id: "net-workspace-net",
        force: true,
      });
    });
  });

  it("shows the native network removal error inside the delete dialog", async () => {
    invokeMock
      .mockResolvedValueOnce([network()])
      .mockRejectedValueOnce(new Error("network is in use by CrateBay containers"));

    render(<NetworksPage />);

    await screen.findByText("workspace-net");
    fireEvent.click(screen.getByTitle("Delete Network"));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("network is in use by CrateBay containers")).toBeInTheDocument();
  });

  it("starts the Engine when network listing finds auto-start disabled", async () => {
    let networkListCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "network_list") {
        networkListCalls += 1;
        if (networkListCalls === 1) {
          return Promise.reject(
            new Error("Implicit runtime start disabled by CRATEBAY_DISABLE_RUNTIME_AUTO_START"),
          );
        }
        return Promise.resolve([network("net-after-start")]);
      }
      if (command === "runtime_start") {
        return Promise.resolve("ok");
      }
      return Promise.resolve(null);
    });

    render(<NetworksPage />);

    expect(await screen.findByText("Engine is offline")).toBeInTheDocument();
    expect(screen.queryByText(/CRATEBAY_DISABLE_RUNTIME_AUTO_START/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Start Engine" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("runtime_start");
    });
    expect(await screen.findByText("net-after-start")).toBeInTheDocument();
  });
});

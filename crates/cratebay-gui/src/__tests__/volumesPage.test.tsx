import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { VolumesPage } from "@/pages/VolumesPage";
import { useSettingsStore } from "@/stores/settingsStore";

const invokeMock = vi.fn();

vi.mock("@/lib/tauri", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function volume(name = "workspace-cache", driver = "local") {
  return {
    name,
    driver,
    mountpoint: `/var/lib/cratebay-engine/volumes/${name}/_data`,
    createdAt: "2026-06-14T00:00:00Z",
    scope: "local",
    labels: { "com.cratebay.volume": "true" },
    options: {},
    managedBy: "cratebay-engine",
  };
}

describe("VolumesPage", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("renders native Engine volumes", async () => {
    invokeMock.mockResolvedValueOnce([volume()]);

    render(<VolumesPage />);

    expect(await screen.findByText("workspace-cache")).toBeInTheDocument();
    expect(screen.getAllByText("local").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/cratebay-engine\/volumes\/workspace-cache/)).toBeInTheDocument();
  });

  it("creates a volume and refreshes the list", async () => {
    invokeMock
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce(volume("cache-a", "nfs"))
      .mockResolvedValueOnce([volume("cache-a", "nfs")]);

    render(<VolumesPage />);

    const input = await screen.findByPlaceholderText("volume-name");
    fireEvent.change(input, { target: { value: "cache-a" } });
    fireEvent.change(screen.getByPlaceholderText("driver"), { target: { value: "nfs" } });
    fireEvent.click(screen.getByRole("button", { name: /create/i }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("volume_create", {
        name: "cache-a",
        driver: "nfs",
      });
    });
    expect(await screen.findByText("cache-a")).toBeInTheDocument();
    expect(screen.getByText("nfs")).toBeInTheDocument();
  });

  it("opens inspect details from native volume inspect", async () => {
    invokeMock.mockResolvedValueOnce([volume()]).mockResolvedValueOnce(volume());

    render(<VolumesPage />);

    await screen.findByText("workspace-cache");
    fireEvent.click(screen.getByTitle("Inspect Volume"));

    expect(await screen.findByText("Managed by")).toBeInTheDocument();
    expect(screen.getByText("cratebay-engine")).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith("volume_inspect", { name: "workspace-cache" });
  });

  it("passes force when deleting a volume from the confirmation dialog", async () => {
    invokeMock
      .mockResolvedValueOnce([volume()])
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([]);

    render(<VolumesPage />);

    await screen.findByText("workspace-cache");
    fireEvent.click(screen.getByTitle("Delete Volume"));
    fireEvent.click(screen.getByText("Force removal"));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("volume_delete", {
        name: "workspace-cache",
        force: true,
      });
    });
  });

  it("shows the native volume removal error inside the delete dialog", async () => {
    invokeMock
      .mockResolvedValueOnce([volume()])
      .mockRejectedValueOnce(new Error("volume is in use by CrateBay containers"));

    render(<VolumesPage />);

    await screen.findByText("workspace-cache");
    fireEvent.click(screen.getByTitle("Delete Volume"));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("volume is in use by CrateBay containers")).toBeInTheDocument();
  });

  it("starts the Engine when volume listing finds auto-start disabled", async () => {
    let volumeListCalls = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "volume_list") {
        volumeListCalls += 1;
        if (volumeListCalls === 1) {
          return Promise.reject(
            new Error("Implicit runtime start disabled by CRATEBAY_DISABLE_RUNTIME_AUTO_START"),
          );
        }
        return Promise.resolve([volume("cache-after-start")]);
      }
      if (command === "runtime_start") {
        return Promise.resolve("ok");
      }
      return Promise.resolve(null);
    });

    render(<VolumesPage />);

    expect(await screen.findByText("Engine is offline")).toBeInTheDocument();
    expect(screen.queryByText(/CRATEBAY_DISABLE_RUNTIME_AUTO_START/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Start Engine" }));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("runtime_start");
    });
    expect(await screen.findByText("cache-after-start")).toBeInTheDocument();
  });
});

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

import { ContainerLogs } from "@/components/container/ContainerLogs";
import { useSettingsStore } from "@/stores/settingsStore";

describe("ContainerLogs", () => {
  beforeEach(() => {
    resetTauriMocks();
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("loads logs with the default tail and timestamps enabled", async () => {
    mockInvoke.mockResolvedValue([
      {
        stream: "stdout",
        message: "ready",
        timestamp: "2026-06-03T00:00:00Z",
      },
    ]);

    render(<ContainerLogs containerId="c-1" />);

    await screen.findByText("ready");
    expect(mockInvoke).toHaveBeenCalledWith("container_logs", {
      id: "c-1",
      options: { tail: 200, timestamps: true },
    });
  });

  it("refreshes logs with caller-selected tail and timestamp options", async () => {
    mockInvoke.mockResolvedValue([
      {
        stream: "stderr",
        message: "warn",
        timestamp: null,
      },
    ]);

    render(<ContainerLogs containerId="c-1" />);
    await screen.findByText("warn");
    mockInvoke.mockClear();

    fireEvent.change(screen.getByLabelText("Tail"), {
      target: { value: "50" },
    });
    fireEvent.click(screen.getByLabelText("Timestamps"));
    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("container_logs", {
        id: "c-1",
        options: { tail: 50, timestamps: false },
      });
    });
  });
});

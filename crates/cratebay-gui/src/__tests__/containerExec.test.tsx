import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
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

import { ContainerExec } from "@/components/container/ContainerExec";
import { useSettingsStore } from "@/stores/settingsStore";

describe("ContainerExec", () => {
  beforeEach(() => {
    resetTauriMocks();
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("executes a shell command in the selected container and renders output", async () => {
    mockInvoke.mockImplementation((command, args) => {
      if (command === "container_exec") {
        expect(args).toEqual({
          id: "c-1",
          cmd: ["sh", "-lc", "printf ready"],
          working_dir: "/workspace",
          timeout: null,
          max_output_bytes: 1048576,
        });
        return Promise.resolve({
          exitCode: 0,
          stdout: "ready\n",
          stderr: "",
          stdoutTruncated: false,
          stderrTruncated: false,
          timedOut: false,
        });
      }
      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    render(<ContainerExec containerId="c-1" />);

    fireEvent.change(screen.getByLabelText("Command"), {
      target: { value: "printf ready" },
    });
    fireEvent.change(screen.getByLabelText("Working directory"), {
      target: { value: "/workspace" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Execute command" }));

    await screen.findByText("ready");
    expect(screen.getByText("Exit 0")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("container_exec", {
      id: "c-1",
      cmd: ["sh", "-lc", "printf ready"],
      working_dir: "/workspace",
      timeout: null,
      max_output_bytes: 1048576,
    });
  });

  it("passes timeout and output limits to the native exec command", async () => {
    mockInvoke.mockImplementation((command, args) => {
      if (command === "container_exec") {
        expect(args).toEqual({
          id: "c-1",
          cmd: ["sh", "-lc", "printf limited"],
          working_dir: null,
          timeout: 7,
          max_output_bytes: 2048,
        });
        return Promise.resolve({
          exitCode: 0,
          stdout: "limited\n",
          stderr: "",
          stdoutTruncated: false,
          stderrTruncated: false,
          timedOut: false,
        });
      }
      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    render(<ContainerExec containerId="c-1" />);

    fireEvent.change(screen.getByLabelText("Command"), {
      target: { value: "printf limited" },
    });
    fireEvent.change(screen.getByLabelText("Timeout (sec)"), {
      target: { value: "7" },
    });
    fireEvent.change(screen.getByLabelText("Max output bytes"), {
      target: { value: "2048" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Execute command" }));

    await screen.findByText("limited");
    expect(mockInvoke).toHaveBeenCalledWith("container_exec", {
      id: "c-1",
      cmd: ["sh", "-lc", "printf limited"],
      working_dir: null,
      timeout: 7,
      max_output_bytes: 2048,
    });
  });

  it("does not invoke exec when the container is not running", async () => {
    render(<ContainerExec containerId="c-1" enabled={false} />);

    expect(screen.getByText("Start the container to execute commands.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Execute command" })).not.toBeInTheDocument();

    await waitFor(() => expect(mockInvoke).not.toHaveBeenCalled());
  });
});

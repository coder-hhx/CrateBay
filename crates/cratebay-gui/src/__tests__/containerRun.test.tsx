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

import { ContainerRun } from "@/components/container/ContainerRun";
import { useContainerStore } from "@/stores/containerStore";
import { useSettingsStore } from "@/stores/settingsStore";

describe("ContainerRun", () => {
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
    });
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("runs a one-shot container and renders the collected output", async () => {
    mockInvoke.mockImplementation((command, args) => {
      if (command === "image_list") {
        return Promise.resolve([
          {
            id: "img-1",
            repoTags: ["alpine:latest"],
            size: 1024,
            sizeBytes: 1024,
            sizeHuman: "1 KB",
            created: 1,
          },
        ]);
      }
      if (command === "pod_list") {
        return Promise.resolve([
          {
            id: "pod-1",
            name: "sandbox",
            driver: "bridge",
            createdAt: null,
            labels: {},
            containers: [],
          },
        ]);
      }
      if (command === "network_list") {
        return Promise.resolve([
          {
            id: "net-workspace",
            name: "workspace-net",
            driver: "bridge",
            scope: "local",
            internal: false,
            attachable: true,
            labels: {},
            containers: {},
            managedBy: "cratebay-engine",
          },
        ]);
      }
      if (command === "volume_list") {
        return Promise.resolve([
          {
            name: "workspace-cache",
            driver: "local",
            mountpoint: "/var/lib/cratebay-engine/volumes/workspace-cache/_data",
            createdAt: null,
            scope: "local",
            labels: {},
            options: {},
            managedBy: "cratebay-engine",
          },
        ]);
      }
      if (command === "container_run") {
        expect(args).toEqual({
          request: {
            image: "alpine:latest",
            command: ["sh", "-lc", "echo ready"],
            pod: "sandbox",
            pull: true,
            remove: true,
            timeoutSecs: 120,
            maxOutputBytes: 200000,
            registryMirrors: ["docker.1ms.run", "docker.xuanyuan.me", "dockerhub.icu"],
          },
        });
        return Promise.resolve({
          id: "run-1",
          name: "cratebay-run-test",
          image: "alpine:latest",
          exitCode: 0,
          stdout: "ready\n",
          stderr: "",
          stdoutTruncated: false,
          stderrTruncated: false,
          timedOut: false,
        });
      }
      if (command === "container_list") {
        return Promise.resolve([]);
      }
      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    render(<ContainerRun />);

    fireEvent.click(screen.getByRole("button", { name: "Run" }));
    fireEvent.change(await screen.findByPlaceholderText("alpine:latest"), {
      target: { value: "alpine:latest" },
    });
    fireEvent.change(screen.getByLabelText("Command"), {
      target: { value: "echo ready" },
    });
    await waitFor(() => expect(screen.getByText("No pod")).toBeInTheDocument());
    fireEvent.click(screen.getByText("No pod"));
    await waitFor(() => expect(screen.getByText("sandbox")).toBeInTheDocument());
    fireEvent.click(screen.getByText("sandbox"));
    fireEvent.click(screen.getByRole("button", { name: "Run Container" }));

    await screen.findByText("ready");
    expect(screen.getByText("Exit 0")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("container_run", expect.any(Object));
  });

  it(
    "submits native run options from the advanced controls",
    async () => {
    mockInvoke.mockImplementation((command, args) => {
      if (command === "image_list") {
        return Promise.resolve([
          {
            id: "img-1",
            repoTags: ["alpine:latest"],
            size: 1024,
            sizeBytes: 1024,
            sizeHuman: "1 KB",
            created: 1,
          },
        ]);
      }
      if (command === "pod_list") {
        return Promise.resolve([]);
      }
      if (command === "network_list") {
        return Promise.resolve([
          {
            id: "net-workspace",
            name: "workspace-net",
            driver: "bridge",
            scope: "local",
            internal: false,
            attachable: true,
            labels: {},
            containers: {},
            managedBy: "cratebay-engine",
          },
        ]);
      }
      if (command === "volume_list") {
        return Promise.resolve([
          {
            name: "workspace-cache",
            driver: "local",
            mountpoint: "/var/lib/cratebay-engine/volumes/workspace-cache/_data",
            createdAt: null,
            scope: "local",
            labels: {},
            options: {},
            managedBy: "cratebay-engine",
          },
        ]);
      }
      if (command === "container_run") {
        expect(args).toEqual({
          request: {
            name: "sandbox-run",
            image: "alpine:latest",
            entrypoint: "/bin/sh",
            command: ["sh", "-lc", "node -v"],
            env: ["NODE_ENV=test"],
            ports: [
              {
                hostPort: 5000,
                containerPort: 5000,
                protocol: "sctp",
              },
            ],
            volumes: [
              {
                hostPath: "workspace-cache",
                containerPath: "/cache",
                readOnly: true,
              },
            ],
            cpuCores: 2,
            memoryMb: 1024,
            workingDir: "/workspace",
            pod: undefined,
            network: "workspace-net",
            user: "1000:1000",
            readOnlyRootfs: true,
            pull: false,
            remove: false,
            timeoutSecs: 45,
            maxOutputBytes: 4096,
          },
        });
        return Promise.resolve({
          id: "run-2",
          name: "sandbox-run",
          image: "alpine:latest",
          exitCode: 0,
          stdout: "v24\n",
          stderr: "",
          stdoutTruncated: false,
          stderrTruncated: false,
          timedOut: false,
        });
      }
      if (command === "container_list") {
        return Promise.resolve([]);
      }
      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    render(<ContainerRun />);

    fireEvent.click(screen.getByRole("button", { name: "Run" }));
    fireEvent.change(screen.getByLabelText("Name (optional)"), {
      target: { value: "sandbox-run" },
    });
    fireEvent.change(screen.getByLabelText("Entrypoint"), {
      target: { value: "/bin/sh" },
    });
    fireEvent.change(await screen.findByPlaceholderText("alpine:latest"), {
      target: { value: "alpine:latest" },
    });
    fireEvent.change(screen.getByLabelText("Command"), {
      target: { value: "node -v" },
    });
    fireEvent.change(screen.getByLabelText("Working directory"), {
      target: { value: "/workspace" },
    });
    fireEvent.click(screen.getByText("Default"));
    fireEvent.click(await screen.findByText("workspace-net"));
    fireEvent.change(screen.getByLabelText("User"), {
      target: { value: "1000:1000" },
    });
    fireEvent.change(screen.getByLabelText("Environment variable"), {
      target: { value: "NODE_ENV=test" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add environment variable" }));
    fireEvent.change(screen.getByLabelText("Publish port"), {
      target: { value: "5000:5000/sctp" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add port" }));
    fireEvent.click(screen.getByText("Select volume"));
    fireEvent.click(await screen.findByText("workspace-cache"));
    fireEvent.change(screen.getByPlaceholderText("Mount path"), {
      target: { value: "/cache" },
    });
    fireEvent.click(screen.getByText("Read-only mount"));
    fireEvent.click(screen.getByRole("button", { name: "Add existing volume" }));
    fireEvent.change(screen.getByLabelText("CPU Cores"), {
      target: { value: "2" },
    });
    fireEvent.change(screen.getByLabelText("Memory (MB)"), {
      target: { value: "1024" },
    });
    fireEvent.change(screen.getByLabelText("Timeout (sec)"), {
      target: { value: "45" },
    });
    fireEvent.change(screen.getByLabelText("Max output bytes"), {
      target: { value: "4096" },
    });
    fireEvent.click(screen.getByText("Pull image when missing"));
    fireEvent.click(screen.getByText("Read-only root"));
    fireEvent.click(screen.getByText("Keep container after exit"));
    fireEvent.click(screen.getByRole("button", { name: "Run Container" }));

    await screen.findByText("v24");
    expect(mockInvoke).toHaveBeenCalledWith("container_run", expect.any(Object));
    },
    15_000,
  );
});

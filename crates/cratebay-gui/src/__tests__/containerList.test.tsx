import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { mockInvoke, resetTauriMocks } from "@/__mocks__/tauriMock";

// Mock Tauri before importing components that use stores
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

import { ContainerList } from "@/components/container/ContainerList";
import { useContainerStore } from "@/stores/containerStore";
import { useSettingsStore } from "@/stores/settingsStore";
import type { ContainerInfo } from "@/types/container";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
const makeContainer = (overrides: Partial<ContainerInfo> = {}): ContainerInfo => ({
  id: "c-1",
  shortId: "c-1abc",
  name: "node-01",
  image: "node:20-slim",
  status: "running",
  state: "running",
  createdAt: new Date().toISOString(),
  cpuCores: 2,
  memoryMb: 2048,
  ports: [],
  labels: {
    "com.cratebay.template_id": "node-dev",
  },
  ...overrides,
});

function resetStore() {
  useContainerStore.setState({
    containers: [],
    loading: false,
    error: null,
    selectedContainerId: null,
    templates: [],
    filter: { status: "all", search: "", templateId: null },
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
describe("ContainerList", () => {
  beforeEach(() => {
    resetStore();
    resetTauriMocks();
    useSettingsStore.setState((state) => ({
      settings: {
        ...state.settings,
        language: "en",
      },
    }));
  });

  it("shows loading state", () => {
    useContainerStore.setState({ loading: true });
    render(<ContainerList />);

    expect(screen.getByText("Loading containers...")).toBeInTheDocument();
  });

  it("shows empty state when no containers", () => {
    render(<ContainerList />);

    expect(
      screen.getByText("No containers match the current filters. Create one to get started."),
    ).toBeInTheDocument();
  });

  it("renders container rows with name and image", () => {
    useContainerStore.setState({
      containers: [makeContainer()],
    });
    render(<ContainerList />);

    expect(screen.getByText("node-01")).toBeInTheDocument();
    expect(screen.getByText("node:20-slim")).toBeInTheDocument();
  });

  it("renders container name and shortId", () => {
    useContainerStore.setState({
      containers: [makeContainer({ name: "my-node", shortId: "abc123" })],
    });
    render(<ContainerList />);

    expect(screen.getByText("my-node")).toBeInTheDocument();
    expect(screen.getByText("abc123")).toBeInTheDocument();
  });

  it("renders container image", () => {
    useContainerStore.setState({
      containers: [makeContainer({ image: "python:3.12-slim" })],
    });
    render(<ContainerList />);

    expect(screen.getByText("python:3.12-slim")).toBeInTheDocument();
  });

  it("renders status label for running containers", () => {
    useContainerStore.setState({
      containers: [makeContainer({ status: "running" })],
    });
    render(<ContainerList />);

    expect(screen.getByText("Running")).toBeInTheDocument();
  });

  it("renders status label for stopped containers", () => {
    useContainerStore.setState({
      containers: [makeContainer({ status: "stopped" })],
    });
    render(<ContainerList />);

    expect(screen.getByText("Stopped")).toBeInTheDocument();
  });

  it("renders CPU and memory info", () => {
    useContainerStore.setState({
      containers: [makeContainer({ cpuCores: 4, memoryMb: 4096 })],
    });
    render(<ContainerList />);

    expect(screen.getByText("4 CPU")).toBeInTheDocument();
    expect(screen.getByText("4096 MB")).toBeInTheDocument();
  });

  it("renders multiple containers", () => {
    useContainerStore.setState({
      containers: [
        makeContainer({ id: "c-1", name: "node-01" }),
        makeContainer({ id: "c-2", name: "py-dev", image: "python:3.12-slim" }),
        makeContainer({ id: "c-3", name: "rust-box", image: "rust:1.75-slim", status: "stopped" }),
      ],
    });
    render(<ContainerList />);

    expect(screen.getByText("node-01")).toBeInTheDocument();
    expect(screen.getByText("py-dev")).toBeInTheDocument();
    expect(screen.getByText("rust-box")).toBeInTheDocument();
  });

  it("shows stop button for running containers", () => {
    useContainerStore.setState({
      containers: [makeContainer({ status: "running" })],
    });
    render(<ContainerList />);

    expect(screen.getByTitle("Stop")).toBeInTheDocument();
    expect(screen.queryByTitle("Start")).not.toBeInTheDocument();
  });

  it("shows start button for stopped containers", () => {
    useContainerStore.setState({
      containers: [makeContainer({ status: "stopped" })],
    });
    render(<ContainerList />);

    expect(screen.getByTitle("Start")).toBeInTheDocument();
    expect(screen.queryByTitle("Stop")).not.toBeInTheDocument();
  });

  it("always shows delete button", () => {
    useContainerStore.setState({
      containers: [makeContainer()],
    });
    render(<ContainerList />);

    expect(screen.getByTitle("Delete")).toBeInTheDocument();
  });

  it("clicking a row selects the container", () => {
    useContainerStore.setState({
      containers: [makeContainer({ id: "c-42" })],
    });
    render(<ContainerList />);

    const cards = screen.getAllByTestId("container-card");
    fireEvent.click(cards[0]);
    expect(useContainerStore.getState().selectedContainerId).toBe("c-42");
  });

  it("respects filter — only shows filtered containers", () => {
    useContainerStore.setState({
      containers: [
        makeContainer({ id: "c-1", name: "node-01", image: "node:20-slim", status: "running" }),
        makeContainer({ id: "c-2", name: "py-dev", image: "python:3.12-slim", status: "stopped" }),
      ],
      filter: { status: "running", search: "", templateId: null },
    });
    render(<ContainerList />);

    expect(screen.getByText("node-01")).toBeInTheDocument();
    expect(screen.queryByText("py-dev")).not.toBeInTheDocument();
  });
});

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, act } from "@testing-library/react";
import { useAppStore } from "@/stores/appStore";
import { setupTauriMocks, mockInvokeResponses, mockListen, resetTauriMocks } from "@/__mocks__/tauriMock";

setupTauriMocks();

vi.mock("@/pages/ContainersPage", () => ({
  ContainersPage: () => <div data-testid="page-containers">ContainersPage</div>,
}));
vi.mock("@/pages/SettingsPage", () => ({
  SettingsPage: () => <div data-testid="page-settings">SettingsPage</div>,
}));
vi.mock("@/pages/ImagesPage", () => ({
  ImagesPage: () => <div data-testid="page-images">ImagesPage</div>,
}));

import App from "../App";

describe("runtime:health handling", () => {
  let now = 1_700_000_000_000;

  beforeEach(() => {
    now = 1_700_000_000_000;
    vi.spyOn(Date, "now").mockImplementation(() => now);
    resetTauriMocks();
    useAppStore.setState({
      currentPage: "images",
      sidebarOpen: true,
      sidebarWidth: 260,
      engineConnected: true,
      dockerConnected: true,
      runtimeStatus: "running",
      builtinRuntimeReady: true,
      theme: "dark",
    });
    mockInvokeResponses({
      engine_status: {
        connected: true,
        engine_source: "builtin",
        source: "builtin",
        version: "25.0.0",
        api_version: "1.44",
        os: "linux",
        arch: "arm64",
        socket_path: "/tmp/cratebay/engine.sock",
      },
      runtime_status: {
        state: "ready",
        platform: "macos-vz",
        cpu_cores: 2,
        memory_mb: 2048,
        disk_gb: 20,
        engine_responsive: true,
        compatibility_responsive: true,
        docker_responsive: true,
        uptime_seconds: null,
        resource_usage: null,
      },
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("does not downgrade from running on transient starting health", async () => {
    let runtimeHealthHandler: ((payload: unknown) => void) | null = null;
    mockListen.mockImplementation(
      ((event: string, handler: (payload: unknown) => void) => {
        if (event === "runtime:health") {
          runtimeHealthHandler = handler;
        }
        return Promise.resolve(() => {});
      }) as any,
    );

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    await act(async () => {
      runtimeHealthHandler?.({
        runtime_state: "Starting",
        docker_responsive: false,
        docker_version: null,
        uptime_seconds: null,
        last_check: new Date().toISOString(),
      });
    });

    expect(useAppStore.getState().runtimeStatus).toBe("running");
    expect(useAppStore.getState().engineConnected).toBe(true);
    expect(useAppStore.getState().dockerConnected).toBe(true);
  });

  it("downgrades after grace window if health keeps failing", async () => {
    let runtimeHealthHandler: ((payload: unknown) => void) | null = null;
    mockListen.mockImplementation(
      ((event: string, handler: (payload: unknown) => void) => {
        if (event === "runtime:health") {
          runtimeHealthHandler = handler;
        }
        return Promise.resolve(() => {});
      }) as any,
    );

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    await act(async () => {
      runtimeHealthHandler?.({
        runtime_state: "Ready",
        engine_responsive: true,
        compatibility_responsive: true,
        docker_responsive: true,
        docker_version: "25.0.0",
        uptime_seconds: 10,
        last_check: new Date().toISOString(),
      });
    });

    now += 95_000;

    await act(async () => {
      runtimeHealthHandler?.({
        runtime_state: "Starting",
        docker_responsive: false,
        docker_version: null,
        uptime_seconds: null,
        last_check: new Date().toISOString(),
      });
    });

    expect(useAppStore.getState().runtimeStatus).toBe("starting");
    expect(useAppStore.getState().engineConnected).toBe(false);
    expect(useAppStore.getState().dockerConnected).toBe(false);
  });

  it("updates to running when health reports ready", async () => {
    let runtimeHealthHandler: ((payload: unknown) => void) | null = null;
    mockListen.mockImplementation(
      ((event: string, handler: (payload: unknown) => void) => {
        if (event === "runtime:health") {
          runtimeHealthHandler = handler;
        }
        return Promise.resolve(() => {});
      }) as any,
    );

    useAppStore.setState({
      runtimeStatus: "starting",
      engineConnected: false,
      dockerConnected: false,
    });

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    await act(async () => {
      runtimeHealthHandler?.({
        runtime_state: "Ready",
        engine_responsive: true,
        compatibility_responsive: true,
        docker_responsive: true,
        docker_version: "25.0.0",
        uptime_seconds: 10,
        last_check: new Date().toISOString(),
      });
    });

    expect(useAppStore.getState().runtimeStatus).toBe("running");
    expect(useAppStore.getState().engineConnected).toBe(true);
    expect(useAppStore.getState().dockerConnected).toBe(true);
  });

  it("accepts native health events without legacy compatibility aliases", async () => {
    let runtimeHealthHandler: ((payload: unknown) => void) | null = null;
    mockListen.mockImplementation(
      ((event: string, handler: (payload: unknown) => void) => {
        if (event === "runtime:health") {
          runtimeHealthHandler = handler;
        }
        return Promise.resolve(() => {});
      }) as any,
    );

    useAppStore.setState({
      runtimeStatus: "starting",
      engineConnected: false,
      dockerConnected: false,
      builtinRuntimeReady: false,
    });

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    await act(async () => {
      runtimeHealthHandler?.({
        runtime_state: "Ready",
        engine_responsive: true,
        engine_source: "builtin",
        uptime_seconds: 10,
        last_check: new Date().toISOString(),
      });
    });

    expect(useAppStore.getState().runtimeStatus).toBe("running");
    expect(useAppStore.getState().engineConnected).toBe(true);
    expect(useAppStore.getState().dockerConnected).toBe(true);
    expect(useAppStore.getState().builtinRuntimeReady).toBe(true);
  });

  it("accepts camelCase runtime health event fields", async () => {
    let runtimeHealthHandler: ((payload: unknown) => void) | null = null;
    mockListen.mockImplementation(
      ((event: string, handler: (payload: unknown) => void) => {
        if (event === "runtime:health") {
          runtimeHealthHandler = handler;
        }
        return Promise.resolve(() => {});
      }) as any,
    );

    useAppStore.setState({
      runtimeStatus: "starting",
      engineConnected: false,
      dockerConnected: false,
      builtinRuntimeReady: false,
    });

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    await act(async () => {
      runtimeHealthHandler?.({
        runtimeState: "Ready",
        engineResponsive: true,
        engineSource: "builtin",
        compatibilityResponsive: false,
        uptimeSeconds: 10,
        lastCheck: new Date().toISOString(),
      });
    });

    expect(useAppStore.getState().runtimeStatus).toBe("running");
    expect(useAppStore.getState().engineConnected).toBe(true);
    expect(useAppStore.getState().dockerConnected).toBe(true);
    expect(useAppStore.getState().builtinRuntimeReady).toBe(true);
  });

  it("does not promote compatibility source to built-in runtime readiness", async () => {
    let runtimeHealthHandler: ((payload: unknown) => void) | null = null;
    mockListen.mockImplementation(
      ((event: string, handler: (payload: unknown) => void) => {
        if (event === "runtime:health") {
          runtimeHealthHandler = handler;
        }
        return Promise.resolve(() => {});
      }) as any,
    );

    useAppStore.setState({
      runtimeStatus: "starting",
      engineConnected: false,
      dockerConnected: false,
      builtinRuntimeReady: false,
    });

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    await act(async () => {
      runtimeHealthHandler?.({
        runtime_state: "Ready",
        engine_responsive: true,
        compatibility_responsive: true,
        docker_responsive: true,
        docker_source: "builtin",
        docker_version: "25.0.0",
        uptime_seconds: 10,
        last_check: new Date().toISOString(),
      });
    });

    expect(useAppStore.getState().runtimeStatus).toBe("running");
    expect(useAppStore.getState().engineConnected).toBe(true);
    expect(useAppStore.getState().dockerConnected).toBe(true);
    expect(useAppStore.getState().builtinRuntimeReady).toBe(false);
  });

  it("keeps native engine disconnected when health only reports compatibility", async () => {
    let runtimeHealthHandler: ((payload: unknown) => void) | null = null;
    mockListen.mockImplementation(
      ((event: string, handler: (payload: unknown) => void) => {
        if (event === "runtime:health") {
          runtimeHealthHandler = handler;
        }
        return Promise.resolve(() => {});
      }) as any,
    );

    useAppStore.setState({
      runtimeStatus: "starting",
      engineConnected: false,
      dockerConnected: false,
    });

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    await act(async () => {
      runtimeHealthHandler?.({
        runtime_state: "Starting",
        engine_responsive: false,
        compatibility_responsive: true,
        docker_responsive: true,
        docker_version: "25.0.0",
        uptime_seconds: 10,
        last_check: new Date().toISOString(),
      });
    });

    expect(useAppStore.getState().runtimeStatus).toBe("starting");
    expect(useAppStore.getState().engineConnected).toBe(false);
    expect(useAppStore.getState().dockerConnected).toBe(true);
  });

  it("does not use compatibility-only connectivity for downgrade grace", async () => {
    let runtimeHealthHandler: ((payload: unknown) => void) | null = null;
    mockListen.mockImplementation(
      ((event: string, handler: (payload: unknown) => void) => {
        if (event === "runtime:health") {
          runtimeHealthHandler = handler;
        }
        return Promise.resolve(() => {});
      }) as any,
    );

    useAppStore.setState({
      runtimeStatus: "running",
      engineConnected: false,
      dockerConnected: true,
    });

    mockInvokeResponses({
      engine_status: {
        connected: true,
        version: "25.0.0",
        api_version: "1.44",
        os: "linux",
        arch: "arm64",
        socket_path: "/tmp/cratebay/engine.sock",
      },
      runtime_status: {
        state: "starting",
        engine_responsive: false,
        compatibility_responsive: true,
        docker_responsive: true,
      },
    });

    render(<App />);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    await act(async () => {
      runtimeHealthHandler?.({
        runtime_state: "Starting",
        engine_responsive: false,
        compatibility_responsive: true,
        docker_responsive: true,
        docker_version: "25.0.0",
        uptime_seconds: 10,
        last_check: new Date().toISOString(),
      });
    });

    expect(useAppStore.getState().runtimeStatus).toBe("starting");
    expect(useAppStore.getState().engineConnected).toBe(false);
    expect(useAppStore.getState().dockerConnected).toBe(true);
  });
});

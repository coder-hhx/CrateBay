import { beforeEach, describe, expect, it } from "vitest";
import { useAppStore } from "@/stores/appStore";
import {
  deriveRuntimeStoreState,
  isBuiltinDockerSource,
  mapRuntimeState,
  syncRuntimeStoreState,
  type DockerStatusResponse,
  type RuntimeStatusResponse,
} from "@/lib/runtimeStatus";

function runtimeStatus(
  patch: Partial<RuntimeStatusResponse> = {},
): RuntimeStatusResponse {
  return {
    state: "ready",
    platform: "macos-vz",
    cpu_cores: 2,
    memory_mb: 2048,
    disk_gb: 20,
    docker_responsive: true,
    uptime_seconds: 120,
    ...patch,
  };
}

function dockerStatus(patch: Partial<DockerStatusResponse> = {}): DockerStatusResponse {
  return {
    connected: true,
    source: "builtin",
    ...patch,
  };
}

describe("runtimeStatus helpers", () => {
  beforeEach(() => {
    useAppStore.setState({
      dockerConnected: false,
      runtimeStatus: "stopped",
      builtinRuntimeReady: false,
    });
  });

  it.each([
    ["ready", "running"],
    ["Ready", "running"],
    ["starting", "starting"],
    ["provisioning", "starting"],
    ["error: boot failed", "error"],
    ["stopped", "stopped"],
    ["none", "stopped"],
  ] as const)("maps backend runtime state %s to %s", (backend, expected) => {
    expect(mapRuntimeState(backend)).toBe(expected);
  });

  it("maps serde error object state to error", () => {
    expect(mapRuntimeState({ Error: "boot failed" })).toBe("error");
  });

  it("treats a responsive Docker endpoint as a ready built-in runtime", () => {
    expect(
      deriveRuntimeStoreState(
        dockerStatus({ connected: true }),
        runtimeStatus({ state: "starting", docker_responsive: false }),
      ),
    ).toEqual({
      runtimeStatus: "running",
      dockerConnected: true,
      builtinRuntimeReady: true,
    });
  });

  it("treats runtime Docker responsiveness as ready even if docker_status is stale", () => {
    expect(
      deriveRuntimeStoreState(
        dockerStatus({ connected: false }),
        runtimeStatus({ state: "ready", docker_responsive: true }),
      ),
    ).toEqual({
      runtimeStatus: "running",
      dockerConnected: true,
      builtinRuntimeReady: true,
    });
  });

  it("does not mark builtin runtime ready for an explicit Docker host", () => {
    expect(
      deriveRuntimeStoreState(
        dockerStatus({
          connected: true,
          source: "tcp://127.0.0.1:2375",
        }),
        runtimeStatus({ state: "starting", docker_responsive: false }),
      ),
    ).toEqual({
      runtimeStatus: "running",
      dockerConnected: true,
      builtinRuntimeReady: false,
    });
  });

  it("keeps a non-responsive runtime state visible", () => {
    expect(
      deriveRuntimeStoreState(
        dockerStatus({ connected: false }),
        runtimeStatus({ state: "provisioning", docker_responsive: false }),
      ),
    ).toEqual({
      runtimeStatus: "starting",
      dockerConnected: false,
      builtinRuntimeReady: false,
    });
  });

  it("falls back to stopped when diagnostics are unavailable", () => {
    expect(deriveRuntimeStoreState(null, null)).toEqual({
      runtimeStatus: "stopped",
      dockerConnected: false,
      builtinRuntimeReady: false,
    });
  });

  it("syncs the shared app store", () => {
    syncRuntimeStoreState(
      dockerStatus({ connected: false }),
      runtimeStatus({ state: "error: missing image", docker_responsive: false }),
    );

    expect(useAppStore.getState()).toMatchObject({
      runtimeStatus: "error",
      dockerConnected: false,
      builtinRuntimeReady: false,
    });
  });

  it("detects builtin docker sources", () => {
    expect(isBuiltinDockerSource("builtin")).toBe(true);
    expect(isBuiltinDockerSource("built-in")).toBe(true);
    expect(isBuiltinDockerSource("runtime")).toBe(true);
    expect(isBuiltinDockerSource("tcp://127.0.0.1:2375")).toBe(false);
  });
});

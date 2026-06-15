import { beforeEach, describe, expect, it } from "vitest";
import { useAppStore } from "@/stores/appStore";
import {
  deriveRuntimeStoreState,
  engineEndpointApiVersion,
  engineEndpointNativeReady,
  engineEndpointSocketPath,
  isBuiltinEngineSource,
  isBuiltinDockerSource,
  mapRuntimeState,
  normalizeRuntimeResourceUsage,
  engineEndpointSource,
  engineEndpointVersion,
  runtimeBackendRuntime,
  runtimeCpuCores,
  runtimeDiskGb,
  runtimeCompatibilityResponsive,
  runtimeEngineCompatible,
  runtimeDockerCompatible,
  runtimeEngineApi,
  runtimeEngineKind,
  runtimeEngineName,
  runtimeEngineResponsive,
  runtimeCompatibilitySource,
  runtimeHealthState,
  runtimeStatusSource,
  runtimeMemoryMb,
  runtimeNetworkStack,
  runtimeOciRuntime,
  runtimeUptimeSeconds,
  syncRuntimeStoreState,
  type EngineEndpointStatusResponse,
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
    engine_responsive: true,
    compatibility_responsive: true,
    docker_responsive: true,
    uptime_seconds: 120,
    ...patch,
  };
}

function engineStatus(
  patch: Partial<EngineEndpointStatusResponse> = {},
): EngineEndpointStatusResponse {
  return {
    connected: true,
    version: "cratebay-containerd",
    api_version: "cratebay.engine.v1",
    engine_source: "builtin",
    source: "builtin",
    ...patch,
  };
}

describe("runtimeStatus helpers", () => {
  beforeEach(() => {
    useAppStore.setState({
      engineConnected: false,
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

  it("treats a native endpoint contract as a ready built-in runtime", () => {
    expect(
      deriveRuntimeStoreState(
        engineStatus({ connected: true }),
        runtimeStatus({
          state: "starting",
          engine_responsive: false,
          compatibility_responsive: false,
          docker_responsive: false,
        }),
      ),
    ).toEqual({
      runtimeStatus: "running",
      engineConnected: true,
      dockerConnected: true,
      builtinRuntimeReady: true,
    });
  });

  it("treats runtime engine responsiveness as ready even if endpoint diagnostics are stale", () => {
    expect(
      deriveRuntimeStoreState(
        engineStatus({ connected: false }),
        runtimeStatus({ state: "ready", engine_responsive: true }),
      ),
    ).toEqual({
      runtimeStatus: "running",
      engineConnected: true,
      dockerConnected: true,
      builtinRuntimeReady: true,
    });
  });

  it("does not treat compatibility-only runtime responsiveness as native Engine readiness", () => {
    expect(runtimeEngineResponsive(runtimeStatus({ engine_responsive: false }))).toBe(false);
    expect(
      runtimeCompatibilityResponsive(
        runtimeStatus({
          engine_responsive: false,
          compatibility_responsive: true,
          docker_responsive: true,
        }),
      ),
    ).toBe(true);
    expect(
      deriveRuntimeStoreState(
        engineStatus({
          version: "25.0.0",
          api_version: "1.44",
        }),
        runtimeStatus({
          state: "starting",
          engine_responsive: false,
          compatibility_responsive: true,
          docker_responsive: true,
        }),
      ),
    ).toEqual({
      runtimeStatus: "starting",
      engineConnected: false,
      dockerConnected: true,
      builtinRuntimeReady: false,
    });
  });

  it("detects native endpoint readiness only from the strict CrateBay Engine contract", () => {
    expect(engineEndpointNativeReady(engineStatus())).toBe(true);
    expect(
      engineEndpointNativeReady(engineStatus({ version: "25.0.0", api_version: "1.44" })),
    ).toBe(false);
    expect(
      engineEndpointNativeReady(
        engineStatus({ source: "tcp://127.0.0.1:2375", engine_source: undefined }),
      ),
    ).toBe(false);
  });

  it("reads endpoint fields from real Tauri camelCase payloads", () => {
    const endpoint = engineStatus({
      api_version: undefined,
      apiVersion: "cratebay.engine.v1",
      socket_path: undefined,
      socketPath: "/tmp/cratebay/engine.sock",
      engine_source: undefined,
      engineSource: "builtin",
    });

    expect(engineEndpointVersion(endpoint)).toBe("cratebay-containerd");
    expect(engineEndpointApiVersion(endpoint)).toBe("cratebay.engine.v1");
    expect(engineEndpointSocketPath(endpoint)).toBe("/tmp/cratebay/engine.sock");
    expect(engineEndpointSource(endpoint)).toBe("builtin");
    expect(engineEndpointNativeReady(endpoint)).toBe(true);
  });

  it("prefers native engine responsiveness over compatibility endpoint staleness", () => {
    expect(
      deriveRuntimeStoreState(
        engineStatus({ connected: false }),
        runtimeStatus({
          state: "starting",
          engine_responsive: true,
          compatibility_responsive: false,
          docker_responsive: false,
        }),
      ),
    ).toEqual({
      runtimeStatus: "running",
      engineConnected: true,
      dockerConnected: false,
      builtinRuntimeReady: true,
    });
  });

  it("does not mark builtin runtime ready for an explicit compatibility host", () => {
    expect(
      deriveRuntimeStoreState(
        engineStatus({
          connected: true,
          version: "25.0.0",
          api_version: "1.44",
          source: "tcp://127.0.0.1:2375",
          engine_source: undefined,
        }),
        runtimeStatus({
          state: "starting",
          engine_responsive: false,
          compatibility_responsive: false,
          docker_responsive: false,
        }),
      ),
    ).toEqual({
      runtimeStatus: "starting",
      engineConnected: false,
      dockerConnected: true,
      builtinRuntimeReady: false,
    });
  });

  it("uses engine_source as the primary endpoint source field", () => {
    const endpoint = engineStatus({
      source: undefined,
      engine_source: "builtin",
    });

    expect(engineEndpointSource(endpoint)).toBe("builtin");
    expect(
      deriveRuntimeStoreState(
        endpoint,
        runtimeStatus({
          state: "starting",
          engine_responsive: false,
          compatibility_responsive: false,
          docker_responsive: false,
        }),
      ),
    ).toEqual({
      runtimeStatus: "running",
      engineConnected: true,
      dockerConnected: true,
      builtinRuntimeReady: true,
    });
  });

  it("uses runtime engine_source when endpoint diagnostics are unavailable", () => {
    const runtime = runtimeStatus({
      state: "ready",
      engine_source: "builtin",
      docker_source: undefined,
    });

    expect(runtimeStatusSource(runtime)).toBe("builtin");
    expect(deriveRuntimeStoreState(null, runtime)).toEqual({
      runtimeStatus: "running",
      engineConnected: true,
      dockerConnected: true,
      builtinRuntimeReady: true,
    });
  });

  it("keeps runtime engine and compatibility sources separate", () => {
    const runtime = runtimeStatus({
      state: "ready",
      engine_responsive: false,
      compatibility_responsive: true,
      docker_responsive: true,
      engine_source: undefined,
      docker_source: "builtin",
    });

    expect(runtimeStatusSource(runtime)).toBe(null);
    expect(runtimeCompatibilitySource(runtime)).toBe("builtin");
    expect(deriveRuntimeStoreState(null, runtime)).toEqual({
      runtimeStatus: "running",
      engineConnected: false,
      dockerConnected: true,
      builtinRuntimeReady: false,
    });
  });

  it("reads runtime health state from either field casing", () => {
    expect(runtimeHealthState({ runtime_state: "Ready", uptime_seconds: null, last_check: "" }))
      .toBe("Ready");
    expect(
      runtimeHealthState({
        runtime_state: "Starting",
        runtimeState: "Ready",
        uptime_seconds: null,
        last_check: "",
      }),
    ).toBe("Ready");
    expect(runtimeHealthState({ runtimeState: "Ready", uptimeSeconds: 1, lastCheck: "" }))
      .toBe("Ready");
    expect(runtimeHealthState(null)).toBe("none");
  });

  it("keeps a non-responsive runtime state visible", () => {
    expect(
      deriveRuntimeStoreState(
        engineStatus({ connected: false }),
        runtimeStatus({
          state: "provisioning",
          engine_responsive: false,
          compatibility_responsive: false,
          docker_responsive: false,
        }),
      ),
    ).toEqual({
      runtimeStatus: "starting",
      engineConnected: false,
      dockerConnected: false,
      builtinRuntimeReady: false,
    });
  });

  it("falls back to stopped when diagnostics are unavailable", () => {
    expect(deriveRuntimeStoreState(null, null)).toEqual({
      runtimeStatus: "stopped",
      engineConnected: false,
      dockerConnected: false,
      builtinRuntimeReady: false,
    });
  });

  it("syncs the shared app store", () => {
    syncRuntimeStoreState(
      engineStatus({ connected: false }),
      runtimeStatus({
        state: "error: missing image",
        engine_responsive: false,
        compatibility_responsive: false,
        docker_responsive: false,
      }),
    );

    expect(useAppStore.getState()).toMatchObject({
      runtimeStatus: "error",
      engineConnected: false,
      dockerConnected: false,
      builtinRuntimeReady: false,
    });
  });

  it("detects builtin engine sources", () => {
    expect(isBuiltinEngineSource("builtin")).toBe(true);
    expect(isBuiltinEngineSource("built-in")).toBe(true);
    expect(isBuiltinEngineSource("runtime")).toBe(true);
    expect(isBuiltinEngineSource("tcp://127.0.0.1:2375")).toBe(false);
    expect(isBuiltinDockerSource("runtime")).toBe(true);
  });

  it("normalizes snake_case runtime resource usage", () => {
    expect(
      normalizeRuntimeResourceUsage(
        runtimeStatus({
          resource_usage: {
            cpu_percent: 18.5,
            memory_used_mb: 768,
            memory_total_mb: 2048,
            disk_used_gb: 6.5,
            disk_total_gb: 20,
            container_count: 3,
          },
        }),
      ),
    ).toEqual({
      cpuPercent: 18.5,
      memoryUsedMb: 768,
      memoryTotalMb: 2048,
      diskUsedGb: 6.5,
      diskTotalGb: 20,
      containerCount: 3,
    });
  });

  it("normalizes camelCase runtime resource usage", () => {
    expect(
      normalizeRuntimeResourceUsage(
        runtimeStatus({
          resourceUsage: {
            cpuPercent: 12,
            memoryUsedMb: 512,
            memoryTotalMb: 4096,
            diskUsedGb: 7,
            diskTotalGb: 32,
            containerCount: 4,
          },
        }),
      ),
    ).toEqual({
      cpuPercent: 12,
      memoryUsedMb: 512,
      memoryTotalMb: 4096,
      diskUsedGb: 7,
      diskTotalGb: 32,
      containerCount: 4,
    });
  });

  it("reads runtime sizing and uptime from either field casing", () => {
    expect(runtimeCpuCores(runtimeStatus({ cpu_cores: 6 }))).toBe(6);
    expect(runtimeMemoryMb(runtimeStatus({ memory_mb: 8192 }))).toBe(8192);
    expect(runtimeDiskGb(runtimeStatus({ disk_gb: 64 }))).toBe(64);
    expect(runtimeUptimeSeconds(runtimeStatus({ uptime_seconds: 3600 }))).toBe(3600);

    expect(
      runtimeCpuCores(runtimeStatus({ cpu_cores: undefined, cpuCores: 8 })),
    ).toBe(8);
    expect(
      runtimeMemoryMb(runtimeStatus({ memory_mb: undefined, memoryMb: 16384 })),
    ).toBe(16384);
    expect(runtimeDiskGb(runtimeStatus({ disk_gb: undefined, diskGb: 128 }))).toBe(128);
    expect(
      runtimeUptimeSeconds(
        runtimeStatus({ uptime_seconds: undefined, uptimeSeconds: 7200 }),
      ),
    ).toBe(7200);
  });

  it("reads engine metadata from either field casing", () => {
    expect(
      runtimeBackendRuntime(
        runtimeStatus({ engine: { backend_runtime: "containerd-shim" } }),
      ),
    ).toBe("containerd-shim");
    expect(
      runtimeOciRuntime(runtimeStatus({ engine: { oci_runtime: "youki" } })),
    ).toBe("youki");
    expect(
      runtimeNetworkStack(runtimeStatus({ engine: { network_stack: "CNI bridge" } })),
    ).toBe("CNI bridge");
    expect(
      runtimeEngineCompatible(
        runtimeStatus({ engine: { docker_compatible: false } }),
      ),
    ).toBe(false);

    expect(
      runtimeBackendRuntime(
        runtimeStatus({ engine: { backendRuntime: "containerd" } }),
      ),
    ).toBe("containerd");
    expect(runtimeOciRuntime(runtimeStatus({ engine: { ociRuntime: "runc" } }))).toBe(
      "runc",
    );
    expect(
      runtimeNetworkStack(runtimeStatus({ engine: { networkStack: "CNI" } })),
    ).toBe("CNI");
    expect(
      runtimeEngineCompatible(
        runtimeStatus({ engine: { dockerCompatible: true } }),
      ),
    ).toBe(true);
    expect(runtimeDockerCompatible(runtimeStatus({ engine: { dockerCompatible: true } }))).toBe(
      true,
    );
  });

  it("uses stable engine metadata fallbacks when optional fields are missing", () => {
    expect(runtimeEngineName(runtimeStatus({ engine: {} }))).toBe("CrateBay Engine");
    expect(runtimeEngineKind(runtimeStatus({ engine: {} }))).toBe("cratebay-containerd");
    expect(runtimeEngineApi(runtimeStatus({ engine: {} }))).toBe("cratebay.engine.v1");
    expect(runtimeBackendRuntime(runtimeStatus({ engine: {} }))).toBe("containerd");
    expect(runtimeOciRuntime(runtimeStatus({ engine: {} }))).toBe("runc");
    expect(runtimeNetworkStack(runtimeStatus({ engine: {} }))).toBe("CNI");
    expect(runtimeEngineCompatible(runtimeStatus({ engine: {} }))).toBe(true);
  });
});

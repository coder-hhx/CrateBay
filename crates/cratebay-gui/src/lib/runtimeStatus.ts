import { useAppStore } from "@/stores/appStore";

export type RuntimeStatus = "starting" | "running" | "stopped" | "error";

export interface DockerStatusResponse {
  connected: boolean;
  version?: string | null;
  api_version?: string | null;
  os?: string | null;
  arch?: string | null;
  source?: string | null;
  socket_path?: string | null;
}

export interface RuntimeStatusResponse {
  state: string;
  platform: string;
  cpu_cores: number;
  memory_mb: number;
  disk_gb: number;
  docker_responsive: boolean;
  uptime_seconds: number | null;
  resource_usage?: unknown;
}

export interface RuntimeHealthPayload {
  runtime_state: string | Record<string, string>;
  docker_responsive: boolean;
  docker_version: string | null;
  uptime_seconds: number | null;
  last_check: string;
  docker_source: string | null;
}

export interface RuntimeStoreState {
  runtimeStatus: RuntimeStatus;
  dockerConnected: boolean;
  builtinRuntimeReady: boolean;
}

export function isBuiltinDockerSource(source: string | null | undefined): boolean {
  if (source === null || source === undefined) return false;
  const normalized = source.trim().toLowerCase();
  return normalized === "builtin" || normalized === "built-in" || normalized === "runtime";
}

export function mapRuntimeState(state: string | Record<string, string>): RuntimeStatus {
  if (typeof state === "object" && state !== null) {
    if ("Error" in state) return "error";
    return "stopped";
  }
  const normalized = state.toLowerCase();
  if (normalized === "ready") return "running";
  if (normalized === "starting" || normalized === "provisioning") return "starting";
  if (normalized.startsWith("error")) return "error";
  return "stopped";
}

export function deriveRuntimeStoreState(
  dockerStatus: DockerStatusResponse | null | undefined,
  runtimeStatus: RuntimeStatusResponse | null | undefined,
): RuntimeStoreState {
  const dockerConnected = dockerStatus?.connected ?? false;
  const runtimeResponsive = runtimeStatus?.docker_responsive ?? false;
  const builtinRuntimeReady =
    runtimeResponsive || (dockerConnected && isBuiltinDockerSource(dockerStatus?.source));

  if (dockerConnected || runtimeResponsive) {
    return {
      runtimeStatus: "running",
      dockerConnected: true,
      builtinRuntimeReady,
    };
  }

  return {
    runtimeStatus: runtimeStatus ? mapRuntimeState(runtimeStatus.state) : "stopped",
    dockerConnected: false,
    builtinRuntimeReady: false,
  };
}

export function syncRuntimeStoreState(
  dockerStatus: DockerStatusResponse | null | undefined,
  runtimeStatus: RuntimeStatusResponse | null | undefined,
) {
  useAppStore.setState(deriveRuntimeStoreState(dockerStatus, runtimeStatus));
}

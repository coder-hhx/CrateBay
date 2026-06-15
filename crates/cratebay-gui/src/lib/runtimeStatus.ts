import { useAppStore } from "@/stores/appStore";

export type RuntimeStatus = "starting" | "running" | "stopped" | "error";

export interface EngineEndpointStatusResponse {
  connected: boolean;
  version?: string | null;
  api_version?: string | null;
  apiVersion?: string | null;
  os?: string | null;
  arch?: string | null;
  engine_source?: string | null;
  engineSource?: string | null;
  source?: string | null;
  socket_path?: string | null;
  socketPath?: string | null;
}

// Compatibility alias for the existing Tauri command payload. The backend
// still exposes docker_* field names for older clients, but the value describes
// CrateBay Engine's Docker-compatible endpoint, not a Docker runtime.
export type DockerStatusResponse = EngineEndpointStatusResponse;

export interface RuntimeStatusResponse {
  state: string;
  platform: string;
  cpu_cores?: number;
  cpuCores?: number;
  memory_mb?: number;
  memoryMb?: number;
  disk_gb?: number;
  diskGb?: number;
  engine_responsive?: boolean;
  engineResponsive?: boolean;
  compatibility_responsive?: boolean;
  compatibilityResponsive?: boolean;
  compatibility_version?: string | null;
  compatibilityVersion?: string | null;
  engine_source?: string | null;
  engineSource?: string | null;
  docker_source?: string | null;
  dockerSource?: string | null;
  docker_responsive?: boolean;
  dockerResponsive?: boolean;
  engine?: {
    name?: string | null;
    kind?: string | null;
    api?: string | null;
    backendRuntime?: string;
    backend_runtime?: string;
    ociRuntime?: string;
    oci_runtime?: string;
    networkStack?: string;
    network_stack?: string;
    dockerCompatible?: boolean;
    docker_compatible?: boolean;
  };
  uptime_seconds?: number | null;
  uptimeSeconds?: number | null;
  resource_usage?: unknown;
  resourceUsage?: unknown;
}

export interface RuntimeHealthPayload {
  runtime_state?: string | Record<string, string>;
  runtimeState?: string | Record<string, string>;
  engine_responsive?: boolean;
  engineResponsive?: boolean;
  compatibility_responsive?: boolean;
  compatibilityResponsive?: boolean;
  compatibility_version?: string | null;
  compatibilityVersion?: string | null;
  compatibility_source?: string | null;
  compatibilitySource?: string | null;
  docker_responsive?: boolean;
  dockerResponsive?: boolean;
  docker_version?: string | null;
  dockerVersion?: string | null;
  uptime_seconds?: number | null;
  uptimeSeconds?: number | null;
  last_check?: string;
  lastCheck?: string;
  engine_source?: string | null;
  engineSource?: string | null;
  docker_source?: string | null;
  dockerSource?: string | null;
  engine?: {
    name: string;
    kind: string;
    api: string;
    backendRuntime?: string;
    ociRuntime?: string;
    networkStack?: string;
    dockerCompatible: boolean;
  };
}

export type RuntimeResourceUsage = {
  cpuPercent: number;
  memoryUsedMb: number;
  memoryTotalMb: number;
  diskUsedGb: number;
  diskTotalGb: number;
  containerCount?: number;
};

type RuntimeEngineMetadata = NonNullable<RuntimeStatusResponse["engine"]>;

function asRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function optionalNumber(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function optionalString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined;
}

function optionalBoolean(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

function numberField(
  value: unknown,
  camelCase: string,
  snakeCase: string,
): number | undefined {
  const record = asRecord(value);
  return optionalNumber(record[camelCase]) ?? optionalNumber(record[snakeCase]);
}

function stringField(
  value: unknown,
  camelCase: string,
  snakeCase: string,
  fallback: string,
): string {
  const record = asRecord(value);
  return optionalString(record[camelCase]) ?? optionalString(record[snakeCase]) ?? fallback;
}

function booleanField(
  value: unknown,
  camelCase: string,
  snakeCase: string,
): boolean | undefined {
  const record = asRecord(value);
  return optionalBoolean(record[camelCase]) ?? optionalBoolean(record[snakeCase]);
}

export function runtimeEngineResponsive(
  runtimeStatus: RuntimeStatusResponse | RuntimeHealthPayload | null | undefined,
): boolean {
  const record = asRecord(runtimeStatus);
  return Boolean(record.engineResponsive ?? record.engine_responsive ?? false);
}

export function runtimeCompatibilityResponsive(
  runtimeStatus: RuntimeStatusResponse | RuntimeHealthPayload | null | undefined,
): boolean {
  const record = asRecord(runtimeStatus);
  return Boolean(
    record.compatibilityResponsive ??
      record.compatibility_responsive ??
      record.dockerResponsive ??
      record.docker_responsive ??
      false,
  );
}

export function engineEndpointNativeReady(
  endpointStatus: EngineEndpointStatusResponse | null | undefined,
): boolean {
  if (!endpointStatus?.connected) return false;
  if (!isBuiltinEngineSource(engineEndpointSource(endpointStatus))) return false;
  const api = engineEndpointApiVersion(endpointStatus);
  const kind = engineEndpointVersion(endpointStatus);
  return api === "cratebay.engine.v1" && kind === "cratebay-containerd";
}

export function engineEndpointVersion(
  endpointStatus: EngineEndpointStatusResponse | null | undefined,
): string | null {
  return optionalString(endpointStatus?.version) ?? null;
}

export function engineEndpointApiVersion(
  endpointStatus: EngineEndpointStatusResponse | null | undefined,
): string | null {
  return (
    optionalString(endpointStatus?.apiVersion) ??
    optionalString(endpointStatus?.api_version) ??
    null
  );
}

export function engineEndpointSocketPath(
  endpointStatus: EngineEndpointStatusResponse | null | undefined,
): string | null {
  return (
    optionalString(endpointStatus?.socketPath) ??
    optionalString(endpointStatus?.socket_path) ??
    null
  );
}

export function runtimeCpuCores(runtimeStatus: RuntimeStatusResponse | null | undefined): number {
  return numberField(runtimeStatus, "cpuCores", "cpu_cores") ?? 0;
}

export function runtimeMemoryMb(runtimeStatus: RuntimeStatusResponse | null | undefined): number {
  return numberField(runtimeStatus, "memoryMb", "memory_mb") ?? 0;
}

export function runtimeDiskGb(runtimeStatus: RuntimeStatusResponse | null | undefined): number {
  return numberField(runtimeStatus, "diskGb", "disk_gb") ?? 0;
}

export function runtimeUptimeSeconds(
  runtimeStatus: RuntimeStatusResponse | null | undefined,
): number | null {
  return numberField(runtimeStatus, "uptimeSeconds", "uptime_seconds") ?? null;
}

export function runtimeEngineName(runtimeStatus: RuntimeStatusResponse | null | undefined): string {
  return stringField(runtimeStatus?.engine, "name", "name", "CrateBay Engine");
}

export function runtimeEngineKind(runtimeStatus: RuntimeStatusResponse | null | undefined): string {
  return stringField(runtimeStatus?.engine, "kind", "kind", "cratebay-containerd");
}

export function runtimeEngineApi(runtimeStatus: RuntimeStatusResponse | null | undefined): string {
  return stringField(runtimeStatus?.engine, "api", "api", "cratebay.engine.v1");
}

export function runtimeBackendRuntime(
  runtimeStatus: RuntimeStatusResponse | null | undefined,
): string {
  return stringField(runtimeStatus?.engine, "backendRuntime", "backend_runtime", "containerd");
}

export function runtimeOciRuntime(runtimeStatus: RuntimeStatusResponse | null | undefined): string {
  return stringField(runtimeStatus?.engine, "ociRuntime", "oci_runtime", "runc");
}

export function runtimeNetworkStack(
  runtimeStatus: RuntimeStatusResponse | null | undefined,
): string {
  return stringField(runtimeStatus?.engine, "networkStack", "network_stack", "CNI");
}

export function runtimeEngineCompatible(
  runtimeStatus: RuntimeStatusResponse | null | undefined,
): boolean {
  const engine = runtimeStatus?.engine as RuntimeEngineMetadata | undefined;
  return booleanField(engine, "dockerCompatible", "docker_compatible") ?? true;
}

export const runtimeDockerCompatible = runtimeEngineCompatible;

export function normalizeRuntimeResourceUsage(
  runtimeStatus: RuntimeStatusResponse | null | undefined,
): RuntimeResourceUsage | null {
  const source = asRecord(
    asRecord(runtimeStatus).resourceUsage ?? asRecord(runtimeStatus).resource_usage,
  );
  if (Object.keys(source).length === 0) return null;
  return {
    cpuPercent: numberField(source, "cpuPercent", "cpu_percent") ?? 0,
    memoryUsedMb: numberField(source, "memoryUsedMb", "memory_used_mb") ?? 0,
    memoryTotalMb: numberField(source, "memoryTotalMb", "memory_total_mb") ?? 0,
    diskUsedGb: numberField(source, "diskUsedGb", "disk_used_gb") ?? 0,
    diskTotalGb: numberField(source, "diskTotalGb", "disk_total_gb") ?? 0,
    containerCount: numberField(source, "containerCount", "container_count"),
  };
}

export interface RuntimeStoreState {
  runtimeStatus: RuntimeStatus;
  engineConnected: boolean;
  dockerConnected: boolean;
  builtinRuntimeReady: boolean;
}

export function isBuiltinEngineSource(source: string | null | undefined): boolean {
  if (source === null || source === undefined) return false;
  const normalized = source.trim().toLowerCase();
  return normalized === "builtin" || normalized === "built-in" || normalized === "runtime";
}

export const isBuiltinDockerSource = isBuiltinEngineSource;

export function engineEndpointSource(
  endpointStatus: EngineEndpointStatusResponse | null | undefined,
): string | null {
  return (
    optionalString(endpointStatus?.engineSource) ??
    optionalString(endpointStatus?.engine_source) ??
    optionalString(endpointStatus?.source) ??
    null
  );
}

export function runtimeHealthSource(
  payload: RuntimeHealthPayload | null | undefined,
): string | null {
  return (
    optionalString(payload?.engineSource) ??
    optionalString(payload?.engine_source) ??
    null
  );
}

export function runtimeHealthState(
  payload: RuntimeHealthPayload | null | undefined,
): string | Record<string, string> {
  return payload?.runtimeState ?? payload?.runtime_state ?? "none";
}

export function runtimeStatusSource(
  runtimeStatus: RuntimeStatusResponse | null | undefined,
): string | null {
  return (
    optionalString(runtimeStatus?.engineSource) ??
    optionalString(runtimeStatus?.engine_source) ??
    null
  );
}

export function runtimeCompatibilitySource(
  runtimeStatus: RuntimeStatusResponse | RuntimeHealthPayload | null | undefined,
): string | null {
  const record = asRecord(runtimeStatus);
  return (
    optionalString(record.compatibilitySource) ??
    optionalString(record.compatibility_source) ??
    optionalString(record.dockerSource) ??
    optionalString(record.docker_source) ??
    null
  );
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
  endpointStatus: EngineEndpointStatusResponse | null | undefined,
  runtimeStatus: RuntimeStatusResponse | null | undefined,
): RuntimeStoreState {
  const endpointConnected = endpointStatus?.connected ?? false;
  const nativeResponsive =
    runtimeEngineResponsive(runtimeStatus) || engineEndpointNativeReady(endpointStatus);
  const compatibilityResponsive =
    runtimeCompatibilityResponsive(runtimeStatus) || endpointConnected;
  const builtinRuntimeReady = nativeResponsive;

  if (nativeResponsive || compatibilityResponsive) {
    return {
      runtimeStatus: nativeResponsive
        ? "running"
        : runtimeStatus
          ? mapRuntimeState(runtimeStatus.state)
          : "stopped",
      engineConnected: nativeResponsive,
      dockerConnected: compatibilityResponsive,
      builtinRuntimeReady,
    };
  }

  return {
    runtimeStatus: runtimeStatus ? mapRuntimeState(runtimeStatus.state) : "stopped",
    engineConnected: false,
    dockerConnected: false,
    builtinRuntimeReady: false,
  };
}

export function syncRuntimeStoreState(
  endpointStatus: EngineEndpointStatusResponse | null | undefined,
  runtimeStatus: RuntimeStatusResponse | null | undefined,
) {
  useAppStore.setState(deriveRuntimeStoreState(endpointStatus, runtimeStatus));
}

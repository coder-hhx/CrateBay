import { useCallback, useEffect, useState } from "react";
import { Activity, Box, Boxes, Cpu, Database, HardDrive, Layers, Loader2, MemoryStick, Network, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";
import { EngineOfflineCallout } from "@/components/common/EngineOfflineCallout";
import { invoke } from "@/lib/tauri";
import {
  normalizeRuntimeResourceUsage,
  runtimeBackendRuntime,
  runtimeDiskGb,
  engineEndpointApiVersion,
  engineEndpointNativeReady,
  engineEndpointSocketPath,
  engineEndpointVersion,
  runtimeEngineCompatible,
  runtimeEngineApi,
  runtimeEngineKind,
  runtimeEngineName,
  runtimeEngineResponsive,
  runtimeMemoryMb,
  runtimeNetworkStack,
  runtimeOciRuntime,
  runtimeUptimeSeconds,
  type EngineEndpointStatusResponse,
  type RuntimeStatusResponse,
} from "@/lib/runtimeStatus";
import { useI18n } from "@/lib/i18n";
import type { ContainerInfo } from "@/types/container";
import type { LocalImageInfo } from "@/types/image";
import type { NetworkInfo } from "@/types/network";
import type { PodInfo } from "@/types/pod";
import type { VolumeInfo } from "@/types/volume";

type DashboardSnapshot = {
  engineEndpoint: EngineEndpointStatusResponse;
  runtime: RuntimeStatusResponse;
  containers: ContainerInfo[];
  images: LocalImageInfo[];
  pods: PodInfo[];
  volumes: VolumeInfo[];
  networks: NetworkInfo[];
};

export function DashboardPage() {
  const { t } = useI18n();
  const [snapshot, setSnapshot] = useState<DashboardSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [engineEndpoint, runtime] = await Promise.all([
        invoke<EngineEndpointStatusResponse>("engine_status"),
        invoke<RuntimeStatusResponse>("runtime_status"),
      ]);

      const engineOnline = runtimeEngineResponsive(runtime) || engineEndpointNativeReady(engineEndpoint);
      const [containers, images, pods, volumes, networks] = await Promise.all([
        loadResource<ContainerInfo[]>("container_list", [], engineOnline),
        loadResource<LocalImageInfo[]>("image_list", [], engineOnline),
        loadResource<PodInfo[]>("pod_list", [], engineOnline),
        loadResource<VolumeInfo[]>("volume_list", [], engineOnline),
        loadResource<NetworkInfo[]>("network_list", [], engineOnline),
      ]);

      setSnapshot({ engineEndpoint, runtime, containers, images, pods, volumes, networks });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const interval = window.setInterval(() => void refresh(), 5000);
    return () => window.clearInterval(interval);
  }, [refresh]);

  const handleStartEngine = useCallback(async () => {
    setStarting(true);
    setError(null);
    try {
      await invoke("runtime_start");
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setStarting(false);
    }
  }, [refresh]);

  const runtime = snapshot?.runtime;
  const usage = normalizeRuntimeResourceUsage(runtime);
  const runningCount = snapshot?.containers.filter((container) => container.status === "running").length ?? 0;
  const endpointNativeReady = engineEndpointNativeReady(snapshot?.engineEndpoint);
  const endpointVersion = engineEndpointVersion(snapshot?.engineEndpoint);
  const endpointApiVersion = engineEndpointApiVersion(snapshot?.engineEndpoint);
  const engineOnline = runtimeEngineResponsive(runtime) || endpointNativeReady;
  const engineLabel = runtimeEngineName(runtime);
  const backendRuntime = runtimeBackendRuntime(runtime);
  const ociRuntime = runtimeOciRuntime(runtime);
  const networkStack = runtimeNetworkStack(runtime);
  const engineKind = runtimeEngineKind(runtime);
  const engineApi = runtimeEngineApi(runtime);
  const memoryMb = runtimeMemoryMb(runtime);
  const diskGb = runtimeDiskGb(runtime);
  const engineHint = runtime?.engine
    ? `${backendRuntime} / ${ociRuntime} · ${engineKind}`
    : endpointNativeReady
      ? `native API ${endpointApiVersion} · ${endpointVersion}`
      : endpointVersion
        ? `compatibility API ${endpointVersion}`
      : runtime?.platform;
  const runtimeHint = engineHint ? `${engineLabel} · ${engineHint}` : engineLabel;

  return (
    <div className="flex h-full flex-col overflow-auto" data-testid="dashboard-page">
      <header className="flex items-center gap-3 border-b border-border px-5 py-3">
        <div>
          <h1 className="text-sm font-semibold">{t("dashboard", "title")}</h1>
          <p className="text-xs text-muted-foreground">{t("dashboard", "subtitle")}</p>
        </div>
        <Button className="ml-auto" variant="ghost" size="sm" onClick={() => void refresh()} disabled={loading}>
          {loading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="h-3.5 w-3.5" />}
          {t("common", "refresh")}
        </Button>
      </header>

      <div className="space-y-4 px-5 py-4">
        {error ? (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        ) : null}

        {!loading && !engineOnline ? (
          <EngineOfflineCallout starting={starting} onStart={() => void handleStartEngine()} />
        ) : null}

        <section className="grid gap-2 sm:grid-cols-2 xl:grid-cols-6">
          <Metric icon={Box} label={t("dashboard", "containers")} value={snapshot?.containers.length ?? 0} hint={`${runningCount} ${t("dashboard", "running")}`} />
          <Metric icon={Layers} label={t("dashboard", "images")} value={snapshot?.images.length ?? 0} />
          <Metric icon={Boxes} label={t("dashboard", "pods")} value={snapshot?.pods.length ?? 0} />
          <Metric icon={Database} label={t("dashboard", "volumes")} value={snapshot?.volumes.length ?? 0} />
          <Metric icon={Network} label={t("dashboard", "networks")} value={snapshot?.networks.length ?? 0} />
          <Metric
            icon={Activity}
            label={t("dashboard", "runtime")}
            value={engineOnline ? t("dashboard", "online") : t("dashboard", "offline")}
            hint={engineOnline ? runtimeHint : engineHint}
          />
        </section>

        <section className="rounded-md border border-border bg-card">
          <SectionHeading title={t("dashboard", "systemMonitoring")} subtitle={t("dashboard", "systemMonitoringDesc")} />
          <div className="grid divide-y divide-border lg:grid-cols-3 lg:divide-x lg:divide-y-0">
            <UsagePanel icon={Cpu} label="CPU" used={usage?.cpuPercent ?? 0} total={100} suffix="%" />
            <UsagePanel icon={MemoryStick} label={t("dashboard", "memory")} used={usage?.memoryUsedMb ?? 0} total={usage?.memoryTotalMb ?? memoryMb} suffix=" MB" />
            <UsagePanel icon={HardDrive} label={t("dashboard", "disk")} used={usage?.diskUsedGb ?? 0} total={usage?.diskTotalGb ?? diskGb} suffix=" GB" />
          </div>
        </section>

        <section className="rounded-md border border-border bg-card">
          <SectionHeading title={t("dashboard", "runtimeDetails")} subtitle={t("dashboard", "runtimeDetailsDesc")} />
          <div className="grid gap-x-8 gap-y-2 border-t border-border px-4 py-3 text-xs sm:grid-cols-2 lg:grid-cols-4">
            <Detail label={t("dashboard", "platform")} value={runtime?.platform ?? "-"} />
            <Detail label={t("dashboard", "engine")} value={engineLabel} />
            <Detail label={t("dashboard", "backendRuntime")} value={backendRuntime} />
            <Detail label={t("dashboard", "ociRuntime")} value={ociRuntime} />
            <Detail label={t("dashboard", "networkStack")} value={networkStack} />
            <Detail label={t("dashboard", "nativeApi")} value={engineApi} />
            <Detail
              label={t("dashboard", "runtimeContainers")}
              value={
                usage?.containerCount === undefined ? "-" : String(usage.containerCount)
              }
            />
            <Detail
              label={t("dashboard", "compatibility")}
              value={
                runtimeEngineCompatible(runtime)
                  ? t("dashboard", "dockerCompatibilityEndpoint")
                  : t("dashboard", "nativeOnly")
              }
            />
            <Detail label={t("dashboard", "endpoint")} value={engineEndpointSocketPath(snapshot?.engineEndpoint) ?? "-"} mono />
            <Detail label={t("dashboard", "uptime")} value={formatUptime(runtimeUptimeSeconds(runtime))} />
          </div>
        </section>
      </div>
    </div>
  );
}

function Metric({ icon: Icon, label, value, hint }: { icon: typeof Box; label: string; value: string | number; hint?: string }) {
  return (
    <div className="rounded-md border border-border bg-card px-3 py-2.5">
      <div className="flex items-center gap-2 text-xs text-muted-foreground"><Icon className="h-3.5 w-3.5" />{label}</div>
      <div className="mt-1.5 text-xl font-semibold tabular-nums">{value}</div>
      {hint ? <div className="mt-0.5 truncate text-xs text-muted-foreground">{hint}</div> : null}
    </div>
  );
}

function UsagePanel({ icon: Icon, label, used, total, suffix }: { icon: typeof Cpu; label: string; used: number; total: number; suffix: string }) {
  const percent = total > 0 ? Math.min(100, Math.max(0, (used / total) * 100)) : 0;
  return (
    <div className="px-4 py-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 text-xs font-medium"><Icon className="h-3.5 w-3.5 text-muted-foreground" />{label}</div>
        <span className="font-mono text-xs text-muted-foreground">{formatNumber(percent)}%</span>
      </div>
      <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-muted"><div className="h-full rounded-full bg-primary" style={{ width: `${percent}%` }} /></div>
      <div className="mt-2 font-mono text-xs text-muted-foreground">{formatNumber(used)} / {formatNumber(total)}{suffix}</div>
    </div>
  );
}

function SectionHeading({ title, subtitle }: { title: string; subtitle: string }) {
  return <div className="px-4 py-3"><h2 className="text-sm font-semibold">{title}</h2><p className="mt-0.5 text-xs text-muted-foreground">{subtitle}</p></div>;
}

function Detail({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return <div><div className="text-muted-foreground">{label}</div><div className={`mt-0.5 truncate ${mono ? "font-mono" : ""}`} title={value}>{value}</div></div>;
}

function formatNumber(value: number) {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

function formatUptime(seconds?: number | null) {
  if (!seconds) return "-";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return `${hours}h ${minutes}m`;
}

async function loadResource<T>(command: string, fallback: T, strict: boolean): Promise<T> {
  if (!strict) {
    return fallback;
  }
  try {
    return await invoke<T>(command);
  } catch (error) {
    throw error;
  }
}

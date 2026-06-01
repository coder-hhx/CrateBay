import * as React from "react";

import { invoke } from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui/button";

export type ContainerStats = {
  id: string;
  name: string;
  readAt: string;
  cpuPercent: number; // may exceed 100 when using multiple cores
  cpuCoresUsed: number;
  memoryUsedMb: number;
  memoryLimitMb: number;
  memoryPercent: number;
};

export async function fetchContainerStats(containerId: string): Promise<ContainerStats> {
  return invoke<ContainerStats>("container_stats", { id: containerId });
}

export function ContainerMonitoring({
  containerId,
  cpuCores,
  memoryMb,
  enabled = true,
}: {
  containerId: string;
  cpuCores?: number;
  memoryMb?: number;
  enabled?: boolean;
}) {
  const { t } = useI18n();
  const [stats, setStats] = React.useState<ContainerStats | null>(null);
  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const inFlightRef = React.useRef(false);

  const refresh = React.useCallback(async () => {
    if (inFlightRef.current) return;
    inFlightRef.current = true;
    setLoading(true);
    setError(null);
    try {
      const next = await fetchContainerStats(containerId);
      setStats(next);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setStats(null);
      setError(message);
    } finally {
      setLoading(false);
      inFlightRef.current = false;
    }
  }, [containerId]);

  React.useEffect(() => {
    if (!enabled) return;
    void refresh();
    const id = window.setInterval(() => void refresh(), 2000);
    return () => window.clearInterval(id);
  }, [enabled, refresh]);

  const cpuPercentOfLimit =
    cpuCores !== undefined && cpuCores > 0 && stats
      ? (stats.cpuCoresUsed / cpuCores) * 100
      : stats?.cpuPercent ?? 0;

  const memPercentOfLimit =
    memoryMb !== undefined && memoryMb > 0 && stats
      ? (stats.memoryUsedMb / memoryMb) * 100
      : stats?.memoryPercent ?? 0;

  return (
    <div className="rounded-lg border border-border bg-card p-3" data-testid="container-monitoring">
      <div className="flex items-center justify-end">
        <Button variant="ghost" size="xs" onClick={() => void refresh()} disabled={loading}>
          {t("common", "refresh")}
        </Button>
      </div>

      {!enabled ? (
        <div className="mt-2 text-sm text-muted-foreground">
          {t("containers", "containerNotRunning")}
        </div>
      ) : stats ? (
        <div className="mt-2 grid grid-cols-2 gap-x-6 gap-y-3 text-sm">
          <div>
            <div className="text-[10px] uppercase tracking-wider text-muted-foreground">CPU</div>
            <div className="font-mono">
              {cpuPercentOfLimit.toFixed(1)}%{" "}
              {cpuCores !== undefined && cpuCores > 0
                ? `(${stats.cpuCoresUsed.toFixed(2)} / ${cpuCores} cores)`
                : `(${stats.cpuCoresUsed.toFixed(2)} cores)`}
            </div>
          </div>
          <div>
            <div className="text-[10px] uppercase tracking-wider text-muted-foreground">MEM</div>
            <div className="font-mono">
              {stats.memoryUsedMb.toFixed(0)} /{" "}
              {memoryMb !== undefined && memoryMb > 0
                ? `${memoryMb}`
                : stats.memoryLimitMb.toFixed(0)}{" "}
              MB ({memPercentOfLimit.toFixed(1)}%)
            </div>
          </div>
          <div className="col-span-2 text-[10px] uppercase tracking-wider text-muted-foreground">
            {new Date(stats.readAt).toLocaleTimeString("zh-CN", { hour12: false })}
          </div>
        </div>
      ) : (
        <div className="mt-2 text-sm text-muted-foreground">
          {error ? `${t("common", "error")}: ${error}` : t("common", "noResults")}
        </div>
      )}
    </div>
  );
}

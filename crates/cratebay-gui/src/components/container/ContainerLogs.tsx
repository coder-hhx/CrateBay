import * as React from "react";

import { invoke } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";

type LogEntry = {
  stream: "stdout" | "stderr" | string;
  message: string;
  timestamp: string | null;
};

export function ContainerLogs({ containerId, tail = 200 }: { containerId: string; tail?: number }) {
  const { t } = useI18n();
  const [tailLines, setTailLines] = React.useState(tail);
  const [timestamps, setTimestamps] = React.useState(true);
  const [entries, setEntries] = React.useState<LogEntry[]>([]);
  const [loading, setLoading] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const loadLogs = React.useCallback(async (tailValue: number, includeTimestamps: boolean) => {
    const normalizedTail = Number.isFinite(tailValue) ? Math.max(0, Math.floor(tailValue)) : 0;
    setLoading(true);
    setError(null);
    try {
      const logs = await invoke<LogEntry[]>("container_logs", {
        id: containerId,
        options: { tail: normalizedTail > 0 ? normalizedTail : undefined, timestamps: includeTimestamps },
      });
      setEntries(Array.isArray(logs) ? logs : []);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
    } finally {
      setLoading(false);
    }
  }, [containerId]);

  const refresh = React.useCallback(async () => {
    await loadLogs(tailLines, timestamps);
  }, [loadLogs, tailLines, timestamps]);

  React.useEffect(() => {
    setTailLines(tail);
    setTimestamps(true);
    void loadLogs(tail, true);
  }, [loadLogs, tail]);

  return (
    <div
      className="overflow-hidden rounded-lg border border-border bg-zinc-950"
      data-testid="container-logs"
    >
      <div className="flex flex-wrap items-center justify-end gap-2 border-b border-border/60 px-3 py-2">
        <label className="flex items-center gap-1.5 text-xs text-zinc-400">
          <span>{t("containers", "containerLogsTail")}</span>
          <Input
            type="number"
            min={0}
            value={tailLines}
            onChange={(event) => setTailLines(Number(event.target.value) || 0)}
            disabled={loading}
            className="h-7 w-20 border-zinc-700 bg-zinc-900 px-2 py-1 font-mono text-xs text-zinc-100"
          />
        </label>
        <label className="flex h-7 items-center gap-1.5 rounded border border-zinc-700 px-2 text-xs text-zinc-400">
          <Checkbox
            checked={timestamps}
            onCheckedChange={(checked) => setTimestamps(checked === true)}
            disabled={loading}
          />
          <span>{t("containers", "containerLogsTimestamps")}</span>
        </label>
        <Button
          variant="ghost"
          size="xs"
          onClick={() => void refresh()}
          disabled={loading}
        >
          {t("common", "refresh")}
        </Button>
      </div>

      <ScrollArea className="max-h-64 p-3 font-mono text-xs leading-5">
        {loading ? (
          <div className="text-zinc-500">{t("common", "loading")}</div>
        ) : error ? (
          <div className="text-red-400">
            {t("common", "error")}: {error}
          </div>
        ) : entries.length === 0 ? (
          <div className="text-zinc-500">{t("common", "noResults")}</div>
        ) : (
          <div className="space-y-0.5">
            {entries.map((entry, idx) => (
              <div key={idx} className="flex gap-2">
                <span className="shrink-0 text-zinc-500">
                  {entry.timestamp ? formatLogTimestamp(entry.timestamp) : "—"}
                </span>
                <span
                  className={cn(
                    "shrink-0 font-semibold",
                    entry.stream === "stderr" ? "text-red-400" : "text-cyan-400",
                  )}
                >
                  [{entry.stream}]
                </span>
                <span className="min-w-0 whitespace-pre-wrap text-zinc-200">
                  {entry.message}
                </span>
              </div>
            ))}
          </div>
        )}
      </ScrollArea>
    </div>
  );
}

function formatLogTimestamp(ts: string): string {
  try {
    const date = new Date(ts);
    if (Number.isNaN(date.getTime())) return ts;
    return date.toLocaleTimeString("zh-CN", { hour12: false });
  } catch {
    return ts;
  }
}

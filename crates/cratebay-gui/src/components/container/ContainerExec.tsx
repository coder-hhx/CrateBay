import * as React from "react";
import { Loader2, Play } from "lucide-react";

import { invoke } from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import type { ExecResult } from "@/types/container";

const DEFAULT_MAX_OUTPUT_BYTES = 1_048_576;

export function ContainerExec({
  containerId,
  enabled = true,
}: {
  containerId: string;
  enabled?: boolean;
}) {
  const { t } = useI18n();
  const [command, setCommand] = React.useState("pwd");
  const [workingDir, setWorkingDir] = React.useState("");
  const [timeoutSecs, setTimeoutSecs] = React.useState(0);
  const [maxOutputBytes, setMaxOutputBytes] = React.useState(DEFAULT_MAX_OUTPUT_BYTES);
  const [running, setRunning] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [result, setResult] = React.useState<ExecResult | null>(null);

  const canExecute = enabled && command.trim().length > 0 && !running;

  const execute = React.useCallback(async () => {
    const nextCommand = command.trim();
    if (!enabled || nextCommand.length === 0) return;

    setRunning(true);
    setError(null);
    setResult(null);
    try {
      const output = await invoke<ExecResult>("container_exec", {
        id: containerId,
        cmd: ["sh", "-lc", nextCommand],
        working_dir: workingDir.trim() || null,
        timeout: timeoutSecs > 0 ? Math.floor(timeoutSecs) : null,
        max_output_bytes: Math.max(0, Math.floor(maxOutputBytes)),
      });
      setResult(output);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setRunning(false);
    }
  }, [command, containerId, enabled, maxOutputBytes, timeoutSecs, workingDir]);

  return (
    <div className="space-y-3" data-testid="container-exec">
      {!enabled ? (
        <div className="rounded-md border border-border bg-muted/30 p-3 text-xs text-muted-foreground">
          {t("containers", "terminalUnavailable")}
        </div>
      ) : (
        <>
          <div className="space-y-2">
            <Label htmlFor="container-exec-command">{t("containers", "runCommand")}</Label>
            <Textarea
              id="container-exec-command"
              value={command}
              onChange={(event) => setCommand(event.target.value)}
              className="min-h-20 resize-y font-mono text-xs"
              spellCheck={false}
            />
          </div>
          <div className="grid gap-2 sm:grid-cols-[1fr_9rem_10rem_auto] sm:items-end">
            <div className="space-y-2">
              <Label htmlFor="container-exec-working-dir">
                {t("containers", "workingDir")}
              </Label>
              <Input
                id="container-exec-working-dir"
                value={workingDir}
                onChange={(event) => setWorkingDir(event.target.value)}
                placeholder="/workspace"
                className="font-mono text-xs"
                autoComplete="off"
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="container-exec-timeout">{t("containers", "timeoutSecs")}</Label>
              <Input
                id="container-exec-timeout"
                type="number"
                min={0}
                max={3600}
                value={timeoutSecs}
                onChange={(event) => setTimeoutSecs(Number(event.target.value) || 0)}
                disabled={running}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="container-exec-max-output">{t("containers", "maxOutputBytes")}</Label>
              <Input
                id="container-exec-max-output"
                type="number"
                min={0}
                value={maxOutputBytes}
                onChange={(event) => setMaxOutputBytes(Number(event.target.value) || 0)}
                disabled={running}
              />
            </div>
            <Button onClick={() => void execute()} disabled={!canExecute}>
              {running ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Play className="h-3.5 w-3.5" />
              )}
              {t("containers", "executeCommand")}
            </Button>
          </div>

          {error !== null && (
            <div className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive">
              {t("common", "error")}: {error}
            </div>
          )}

          {result !== null && (
            <div className="space-y-2">
              <div
                className={cn(
                  "inline-flex items-center rounded px-2 py-1 text-xs font-medium",
                  result.exitCode === 0 && !result.timedOut
                    ? "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
                    : "bg-amber-500/10 text-amber-600 dark:text-amber-400",
                )}
              >
                {result.timedOut
                  ? t("containers", "timedOut")
                  : `${t("containers", "exitCode")} ${result.exitCode}`}
              </div>
              {(result.stdoutTruncated || result.stderrTruncated) && (
                <div className="text-xs text-amber-600 dark:text-amber-400">
                  {t("containers", "outputTruncated")}
                </div>
              )}
              <ExecOutput title="stdout" value={result.stdout} empty={t("common", "noResults")} />
              <ExecOutput title="stderr" value={result.stderr} empty={t("common", "noResults")} />
            </div>
          )}
        </>
      )}
    </div>
  );
}

function ExecOutput({
  title,
  value,
  empty,
}: {
  title: string;
  value: string;
  empty: string;
}) {
  return (
    <div className="overflow-hidden rounded-md border border-border bg-zinc-950">
      <div className="border-b border-border/60 px-3 py-1.5 font-mono text-[10px] uppercase tracking-wider text-zinc-500">
        {title}
      </div>
      <pre className="max-h-48 overflow-auto whitespace-pre-wrap p-3 font-mono text-xs leading-5 text-zinc-200">
        {value.length > 0 ? value : empty}
      </pre>
    </div>
  );
}

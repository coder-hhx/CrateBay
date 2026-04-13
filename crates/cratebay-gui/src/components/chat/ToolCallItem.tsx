import { useState } from "react";
import { useI18n } from "@/lib/i18n";
import type { ToolCallInfo } from "@/types/chat";
import { Terminal, Wrench, ChevronRight } from "lucide-react";

function safeStringify(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function previewText(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text;
  return text.slice(0, maxLen) + "\n…(truncated)";
}

interface ToolCallItemProps {
  toolCall: ToolCallInfo;
}

/**
 * A single tool call card with compact header and collapsible detail.
 *
 * - Status dot: running=yellow pulse, success=green, error=red, pending=gray
 * - Bash/shell commands show the command inline in the header
 * - Other tools show the tool name
 * - Collapsible via native <details> element
 */
export function ToolCallItem({ toolCall }: ToolCallItemProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);

  const isBash =
    toolCall.toolName === "Bash" ||
    toolCall.toolName === "bash" ||
    toolCall.toolName === "shell";

  const bashCmd =
    isBash && typeof toolCall.parameters?.command === "string"
      ? toolCall.parameters.command.trim()
      : "";
  const firstLine = bashCmd ? bashCmd.split("\n")[0] : "";

  // Status dot color
  const dotClass =
    toolCall.status === "running"
      ? "bg-yellow-400 animate-pulse"
      : toolCall.status === "success"
        ? "bg-emerald-500"
        : toolCall.status === "error"
          ? "bg-red-500"
          : "bg-zinc-400";

  // Status label
  const statusLabel =
    toolCall.status === "running"
      ? t("chat", "toolExecuting")
      : toolCall.status === "success"
        ? t("chat", "toolCompleted")
        : toolCall.status === "error"
          ? t("chat", "toolFailed")
          : t("chat", "toolPreparing");

  // Status text color
  const statusClass =
    toolCall.status === "running"
      ? "text-yellow-500"
      : toolCall.status === "success"
        ? "text-emerald-500"
        : toolCall.status === "error"
          ? "text-red-500"
          : "text-muted-foreground";

  // Duration
  const duration =
    toolCall.startedAt && toolCall.completedAt
      ? (() => {
          const ms =
            new Date(toolCall.completedAt).getTime() -
            new Date(toolCall.startedAt).getTime();
          return ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`;
        })()
      : null;

  return (
    <details
      className="group/tool overflow-hidden rounded-lg border border-border/30 transition-colors hover:border-border/60"
      onToggle={(e) =>
        setOpen((e.currentTarget as HTMLDetailsElement).open)
      }
    >
      <summary className="flex cursor-pointer select-none items-center gap-2 px-3 py-1.5">
        {/* Status dot */}
        <span
          className={`inline-block h-2 w-2 shrink-0 rounded-full ${dotClass} transition-all`}
        />

        {isBash ? (
          /* Bash: terminal icon + inline command */
          <div className="flex min-w-0 flex-1 items-center gap-1.5">
            <Terminal className="h-3 w-3 shrink-0 text-muted-foreground/60" />
            {firstLine ? (
              <span className="min-w-0 truncate font-mono text-[12px] text-foreground/80">
                <span className="mr-0.5 text-muted-foreground/50">$</span>
                {firstLine.length > 56 ? (
                  <>
                    {firstLine.slice(0, 56)}
                    <span className="text-muted-foreground">&hellip;</span>
                  </>
                ) : (
                  firstLine
                )}
                {bashCmd.includes("\n") && (
                  <span className="ml-1 text-[11px] text-muted-foreground/50">
                    +{bashCmd.split("\n").length - 1} lines
                  </span>
                )}
              </span>
            ) : (
              <span className="text-[13px] font-medium text-muted-foreground">
                Bash
              </span>
            )}
          </div>
        ) : (
          /* Other tools: wrench icon + tool name + param summary */
          <div className="flex min-w-0 flex-1 items-center gap-1.5">
            <Wrench className="h-3 w-3 shrink-0 text-muted-foreground/60" />
            <span className="text-[13px] font-medium text-muted-foreground">
              {toolCall.toolLabel || toolCall.toolName}
            </span>
            {!isBash && toolCall.parameters && Object.keys(toolCall.parameters).length > 0 && (
              <span className="truncate text-[11px] text-muted-foreground/50">
                ({Object.keys(toolCall.parameters).join(", ")})
              </span>
            )}
          </div>
        )}

        {/* Duration */}
        {duration !== null && (
          <span className="shrink-0 text-[11px] text-muted-foreground">
            {duration}
          </span>
        )}

        {/* Status label */}
        <span
          className={`ml-auto shrink-0 text-[11px] font-medium ${statusClass}`}
        >
          {statusLabel}
        </span>

        {/* Chevron */}
        <ChevronRight className="h-3 w-3 shrink-0 text-muted-foreground/50 transition-transform group-open/tool:rotate-90" />
      </summary>

      {open && (
        <div className="space-y-2.5 border-t border-border/30 px-3 py-2.5">
          {/* Parameters / Command */}
          {isBash && bashCmd ? (
            <div>
              <div className="mb-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground/60">
                Command
              </div>
              <pre className="max-h-44 overflow-auto whitespace-pre rounded-md bg-zinc-950/60 p-2.5 text-[12px] leading-relaxed text-emerald-300/90 dark:bg-zinc-900/80">
                <span className="mr-1 select-none text-emerald-500/50">$</span>
                {bashCmd}
              </pre>
            </div>
          ) : (
            <div>
              <div className="mb-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground/60">
                {t("common", "parameters")}
              </div>
              <pre className="max-h-44 overflow-auto whitespace-pre rounded-md bg-muted/40 p-2.5 text-[12px] leading-relaxed">
                {safeStringify(toolCall.parameters)}
              </pre>
            </div>
          )}

          {/* Result */}
          {toolCall.status === "success" && toolCall.result !== undefined && (
            <div>
              <div className="mb-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground/60">
                {t("common", "result")}
              </div>
              <pre className="max-h-64 overflow-auto whitespace-pre-wrap rounded-md bg-muted/40 p-2.5 text-[12px] leading-relaxed">
                {previewText(
                  typeof toolCall.result === "string"
                    ? toolCall.result
                    : safeStringify(toolCall.result),
                  6000,
                )}
              </pre>
            </div>
          )}

          {/* Error */}
          {toolCall.status === "error" && toolCall.error !== undefined && (
            <div>
              <div className="mb-1.5 text-[11px] font-medium uppercase tracking-wider text-destructive/60">
                {t("common", "error")}
              </div>
              <pre className="max-h-36 overflow-auto whitespace-pre-wrap rounded-md bg-destructive/10 p-2.5 text-[12px] leading-relaxed text-destructive">
                {toolCall.error}
              </pre>
            </div>
          )}
        </div>
      )}
    </details>
  );
}

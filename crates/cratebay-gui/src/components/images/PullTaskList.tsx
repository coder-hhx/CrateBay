import { useState, useRef, useEffect } from "react";
import { usePullStore } from "@/stores/pullStore";
import { useI18n } from "@/lib/i18n";
import type { PullTask } from "@/stores/pullStore";
import { Button } from "@/components/ui/button";
import { X, CheckCircle2, XCircle, Download, Loader2, ChevronDown } from "lucide-react";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const val = bytes / Math.pow(1024, i);
  return `${val.toFixed(i > 1 ? 1 : 0)} ${units[i]}`;
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec <= 0) return "";
  return `${formatBytes(bytesPerSec)}/s`;
}

export function PullTaskList() {
  const { t } = useI18n();
  const tasks = usePullStore((s) => s.tasks);
  const removeTask = usePullStore((s) => s.removeTask);
  const clearCompleted = usePullStore((s) => s.clearCompleted);
  const [expanded, setExpanded] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);

  // Auto-expand when new task starts or a task completes
  const prevCountRef = useRef(tasks.length);
  const prevCompletedRef = useRef(0);
  useEffect(() => {
    const completed = tasks.filter((t) => t.complete).length;
    if (tasks.length > prevCountRef.current || completed > prevCompletedRef.current) {
      setExpanded(true);
    }
    prevCountRef.current = tasks.length;
    prevCompletedRef.current = completed;
  }, [tasks]);

  // Click outside to close
  useEffect(() => {
    if (!expanded) return;
    const handler = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        setExpanded(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [expanded]);

  if (tasks.length === 0) return null;

  const activeTasks = tasks.filter((t) => !t.complete);
  const completedCount = tasks.filter((t) => t.complete).length;
  const hasError = tasks.some((t) => t.complete && t.error !== null);

  return (
    <div className="relative" ref={panelRef}>
      {/* Trigger badge */}
      <button
        aria-label={t("images", "pullTasks")}
        onClick={() => setExpanded((v) => !v)}
        className="inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium transition-colors focus:outline-none bg-primary/10 text-primary hover:bg-primary/20"
      >
        {activeTasks.length > 0 ? (
          <Loader2 className="h-3 w-3 animate-spin" />
        ) : hasError ? (
          <XCircle className="h-3 w-3 text-destructive" />
        ) : (
          <CheckCircle2 className="h-3 w-3 text-green-500" />
        )}
        <Download className="h-3 w-3" />
        <span>{tasks.length}</span>
        <ChevronDown className={`h-3 w-3 transition-transform ${expanded ? "rotate-180" : ""}`} />
      </button>

      {/* Dropdown panel */}
      {expanded && (
        <div className="absolute right-0 top-full z-50 mt-1 w-80 rounded-lg border border-border bg-card shadow-lg">
          {/* Header */}
          <div className="flex items-center justify-between px-3 py-2 border-b border-border">
            <span className="text-xs font-medium text-foreground">
              {formatPullTaskSummary(
                t("images", "pullTasks"),
                activeTasks.length,
                completedCount,
                t("images", "pullTasksActive"),
                t("images", "pullTasksCompleted"),
              )}
            </span>
            {completedCount > 0 && (
              <Button
                variant="ghost"
                size="sm"
                className="h-5 px-1.5 text-[10px] text-muted-foreground"
                onClick={clearCompleted}
              >
                {t("images", "pullTasksClear")}
              </Button>
            )}
          </div>

          {/* Task list */}
          <div className="max-h-60 overflow-y-auto divide-y divide-border">
            {tasks.map((task) => (
              <PullTaskRow key={task.id} task={task} onRemove={() => removeTask(task.id)} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function PullTaskRow({ task, onRemove }: { task: PullTask; onRemove: () => void }) {
  const { t } = useI18n();
  if (task.complete) {
    return (
      <div className="flex items-center justify-between gap-2 px-3 py-1.5">
        <div className="flex items-center gap-1.5 min-w-0 flex-1">
          {task.error !== null ? (
            <XCircle className="h-3 w-3 flex-shrink-0 text-destructive" />
          ) : (
            <CheckCircle2 className="h-3 w-3 flex-shrink-0 text-green-500" />
          )}
          <span className="truncate text-xs text-foreground">{task.image}</span>
          <span className={`text-[10px] flex-shrink-0 ${task.error !== null ? "text-destructive" : "text-green-600"}`}>
            {task.error !== null ? t("images", "pullTasksFailed") : t("images", "pullTasksDone")}
          </span>
        </div>
        <button
          onClick={onRemove}
          className="flex-shrink-0 rounded p-0.5 text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
        >
          <X className="h-3 w-3" />
        </button>
      </div>
    );
  }

  const speedStr = formatSpeed(task.speed);
  const downloadedStr = task.currentBytes > 0 ? formatBytes(task.currentBytes) : "";

  return (
    <div className="px-3 py-2 space-y-1">
      <div className="flex items-center gap-1.5 min-w-0">
        <Loader2 className="h-3 w-3 flex-shrink-0 animate-spin text-primary" />
        <span className="truncate text-xs font-medium text-foreground">{task.image}</span>
      </div>

      {/* Indeterminate progress bar */}
      <div className="h-1 w-full overflow-hidden rounded-full bg-muted">
        <div className="h-full w-1/3 rounded-full bg-primary animate-indeterminate" />
      </div>

      {/* Stats: downloaded size + speed */}
      <div className="flex items-center justify-between text-[10px] text-muted-foreground">
        <span className="truncate">
          {downloadedStr || t("images", "pullTasksPreparing")}
        </span>
        {speedStr && <span className="flex-shrink-0 tabular-nums ml-2">{speedStr}</span>}
      </div>
    </div>
  );
}

function formatPullTaskSummary(
  label: string,
  active: number,
  completed: number,
  activeLabel: string,
  completedLabel: string,
): string {
  const completedPart = completed > 0 ? ` / ${completed} ${completedLabel}` : "";
  return `${label} (${active} ${activeLabel}${completedPart})`;
}

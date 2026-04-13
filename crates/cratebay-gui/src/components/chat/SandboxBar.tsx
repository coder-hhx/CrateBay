import { Box } from "lucide-react";

interface SandboxBarProps {
  sandboxId: string;
  language?: string;
  status: "running" | "stopped" | "unknown";
}

/**
 * Compact bar showing the sandbox bound to the current chat session.
 * Placed above the message list.
 */
export function SandboxBar({ sandboxId, language, status }: SandboxBarProps) {
  const shortId = sandboxId.length > 12 ? sandboxId.slice(0, 12) : sandboxId;

  const statusColor =
    status === "running"
      ? "bg-emerald-500"
      : status === "stopped"
        ? "bg-zinc-400"
        : "bg-yellow-400";

  const statusLabel =
    status === "running"
      ? "Running"
      : status === "stopped"
        ? "Stopped"
        : "Unknown";

  return (
    <div className="flex items-center gap-3 border-b border-border/40 bg-muted/20 px-4 py-1.5 text-[12px] text-muted-foreground">
      <Box className="h-3.5 w-3.5 text-primary/60" />

      <span className="font-medium">Sandbox</span>

      <span className="font-mono text-foreground/70">{shortId}</span>

      {language && (
        <span className="rounded-full bg-primary/10 px-2 py-0.5 text-[11px] font-medium text-primary">
          {language}
        </span>
      )}

      <div className="flex items-center gap-1.5">
        <span className={`inline-block h-1.5 w-1.5 rounded-full ${statusColor}`} />
        <span>{statusLabel}</span>
      </div>
    </div>
  );
}

import { useState } from "react";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";
import type { ContainerInfo } from "@/types/container";
import { useContainerStore } from "@/stores/containerStore";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ContainerDeleteDialog } from "@/components/container/ContainerDeleteDialog";
import {
  Play,
  Square,
  Trash2,
  Terminal,
  ScrollText,
} from "lucide-react";

interface ContainerCardProps {
  container: ContainerInfo;
}

/**
 * Single container card for grid view.
 * Entire card is clickable to open detail panel.
 */
export function ContainerCard({ container }: ContainerCardProps) {
  const { t } = useI18n();
  const startContainer = useContainerStore((s) => s.startContainer);
  const stopContainer = useContainerStore((s) => s.stopContainer);
  const selectContainer = useContainerStore((s) => s.selectContainer);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const isRunning = container.status === "running" || container.status === "paused";
  const isStopped =
    container.status === "stopped" ||
    container.status === "exited";
  const isCreating =
    container.status === "creating" ||
    container.status === "created" ||
    container.status === "restarting" ||
    container.status === "removing";
  const isError = container.status === "dead";

  const handleCardClick = () => {
    selectContainer(container.id);
  };

  /** Wrap action handlers to prevent card click */
  const stop = (e: React.MouseEvent) => { e.stopPropagation(); void stopContainer(container.id); };
  const start = (e: React.MouseEvent) => { e.stopPropagation(); void startContainer(container.id); };
  const remove = (e: React.MouseEvent) => {
    e.stopPropagation();
    setDeleteOpen(true);
  };
  const openDetail = (e: React.MouseEvent) => { e.stopPropagation(); selectContainer(container.id); };

  return (
    <>
      <div
        onClick={isCreating ? undefined : handleCardClick}
        data-testid="container-card"
        className={cn(
          "group rounded-md border border-border bg-card p-3 transition-colors",
          isRunning && "cursor-pointer border-l-2 border-l-emerald-400 hover:border-primary/40",
          isStopped && "cursor-pointer border-l-2 border-l-zinc-500 hover:border-primary/30",
          isCreating && "animate-pulse border-l-2 border-l-yellow-400",
          isError && "cursor-pointer border-l-2 border-l-red-400 hover:border-destructive/40",
        )}
      >
      {/* Header: name + status badge */}
      <div className="mb-2 flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-sm font-semibold text-foreground">
            {container.name}
          </h3>
          <p className="mt-0.5 truncate font-mono text-xs text-muted-foreground">
            {container.image}
          </p>
        </div>
        <StatusBadge status={container.status} />
      </div>

      {/* Port tags */}
      {container.ports.length > 0 && (
        <div className="mb-3 flex flex-wrap gap-1">
          {container.ports.map((port) => (
            <span
              key={`${port.hostPort}-${port.containerPort}`}
              className="inline-flex items-center rounded-md border border-border bg-muted/50 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
            >
              {port.hostPort}:{port.containerPort}
            </span>
          ))}
        </div>
      )}

      {/* Specs (limits) */}
      {(container.cpuCores !== undefined || container.memoryMb !== undefined) && (
        <div className="mb-3 flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs text-muted-foreground">
          {container.cpuCores !== undefined && (
            <span>CPU {container.cpuCores} cores</span>
          )}
          {container.memoryMb !== undefined && (
            <span>MEM {container.memoryMb} MB</span>
          )}
        </div>
      )}

      {/* Actions — negative margins extend border to card edge,
           pl-2 (16-2=14px effective) so button icon (with its own pl) aligns with text above */}
      <div className="-mx-3 -mb-3 flex items-center gap-0.5 border-t border-border/60 px-1.5 py-1.5">
        {isRunning ? (
          <>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 gap-1 px-2 text-xs"
              onClick={stop}
              data-testid="container-stop"
            >
              <Square className="h-3.5 w-3.5" />
              {t("containers", "stop")}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 gap-1 px-2 text-xs"
              onClick={openDetail}
            >
              <Terminal className="h-3.5 w-3.5" />
              {t("containers", "terminal")}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 gap-1 px-2 text-xs"
              onClick={openDetail}
            >
              <ScrollText className="h-3.5 w-3.5" />
              {t("containers", "logs")}
            </Button>
          </>
        ) : isStopped ? (
          <Button
            variant="ghost"
            size="sm"
            className="h-7 gap-1 px-2 text-xs"
            onClick={start}
            data-testid="container-start"
          >
            <Play className="h-3.5 w-3.5" />
            {t("containers", "start")}
          </Button>
        ) : isCreating ? (
          <Button variant="ghost" size="sm" className="h-7 px-2 text-xs" disabled>
            {container.shortId.startsWith("creating") ? `${t("containers", "creating")}...` : container.shortId}
          </Button>
        ) : null}

        <Button
          variant="ghost"
          size="sm"
          className="ml-auto h-7 px-2 text-xs text-destructive hover:text-destructive"
          onClick={remove}
          data-testid="container-delete"
        >
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>
      </div>
      <ContainerDeleteDialog
        container={container}
        open={deleteOpen}
        onOpenChange={setDeleteOpen}
      />
    </>
  );
}

function StatusBadge({ status }: { status: ContainerInfo["status"] }) {
  const { t } = useI18n();

  // Map all Docker statuses to display variants
  const getVariant = (s: string) => {
    switch (s) {
      case "running":
        return {
          label: t("containers", "running"),
          dotClass: "bg-emerald-400",
          textClass: "text-emerald-500",
        };
      case "exited":
      case "stopped":
        return {
          label: t("containers", "stopped"),
          dotClass: "bg-zinc-400",
          textClass: "text-muted-foreground",
        };
      case "creating":
      case "created":
        return {
          label: t("containers", "creating"),
          dotClass: "bg-yellow-400 animate-pulse",
          textClass: "text-yellow-500",
        };
      case "paused":
        return {
          label: t("containers", "paused"),
          dotClass: "bg-amber-400",
          textClass: "text-amber-500",
        };
      case "restarting":
        return {
          label: t("containers", "restarting"),
          dotClass: "bg-cyan-400 animate-pulse",
          textClass: "text-cyan-500",
        };
      case "dead":
        return {
          label: t("containers", "dead"),
          dotClass: "bg-red-400",
          textClass: "text-red-500",
        };
      default:
        return {
          label: s,
          dotClass: "bg-red-400",
          textClass: "text-red-500",
        };
    }
  };

  const variant = getVariant(status);

  return (
    <Badge
      variant="outline"
      className={cn(
        "flex items-center gap-1.5 border-transparent bg-transparent px-0 text-[10px] font-medium",
        variant.textClass,
      )}
    >
      <span className={cn("inline-block h-1.5 w-1.5 rounded-full", variant.dotClass)} />
      {variant.label}
    </Badge>
  );
}

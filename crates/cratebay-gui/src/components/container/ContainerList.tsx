import { useContainerStore, type ContainerInfo } from "@/stores/containerStore";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Play, Square, Trash2, Box } from "lucide-react";

export function ContainerList() {
  const { t } = useI18n();
  const filteredContainers = useContainerStore((s) => s.filteredContainers);
  const loading = useContainerStore((s) => s.loading);
  const containers = filteredContainers();

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
        {t("containers", "loadingContainers")}
      </div>
    );
  }

  if (containers.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-center text-muted-foreground">
        <Box className="mb-3 h-12 w-12 opacity-20" />
        <h3 className="text-sm font-medium">{t("containers", "noContainers")}</h3>
        <p className="mt-1 text-xs">{t("containers", "noContainersHint")}</p>
      </div>
    );
  }

  return (
    <div className="space-y-0" data-testid="container-list">
      {containers.map((container) => (
        <ContainerRow key={container.id} container={container} />
      ))}
    </div>
  );
}

function ContainerRow({ container }: { container: ContainerInfo }) {
  const { t } = useI18n();
  const startContainer = useContainerStore((s) => s.startContainer);
  const stopContainer = useContainerStore((s) => s.stopContainer);
  const deleteContainer = useContainerStore((s) => s.deleteContainer);
  const selectContainer = useContainerStore((s) => s.selectContainer);
  const isRunning = container.status === "running" || container.status === "paused";

  const statusDot = getStatusDot(container.status, {
    running: t("containers", "running"),
    paused: t("containers", "paused"),
    creating: t("containers", "creating"),
    restarting: t("containers", "restarting"),
    stopped: t("containers", "stopped"),
    dead: t("containers", "dead"),
  });

  const remove = (event: React.MouseEvent) => {
    event.stopPropagation();
    if (!window.confirm(t("containers", "confirmDelete").replace("{name}", container.name))) return;
    void deleteContainer(container.id);
  };

  return (
    <div
      onClick={() => selectContainer(container.id)}
      data-testid="container-card"
      className="group flex cursor-pointer items-center gap-4 border-b border-border/50 px-4 py-3 transition-colors last:border-b-0 hover:bg-muted/50"
    >
      {/* Status dot */}
      <div className="flex-shrink-0">
        <span className={cn("inline-block h-2.5 w-2.5 rounded-full", statusDot.dotClass)} />
      </div>

      {/* Name + image */}
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="truncate text-sm font-medium text-foreground">
            {container.name}
          </span>
          <span className="hidden text-[11px] text-muted-foreground sm:inline">
            {container.shortId}
          </span>
        </div>
        <div className="mt-0.5 flex items-center gap-3 text-xs text-muted-foreground">
          <span className="truncate">{container.image}</span>
          {container.ports.length > 0 && (
            <>
              <span className="text-border">·</span>
              <span className="font-mono">
                {container.ports.map((p) => `${p.hostPort}:${p.containerPort}`).join(", ")}
              </span>
            </>
          )}
        </div>
      </div>

      {/* Status label */}
      <div className="hidden flex-shrink-0 sm:block">
        <span className={cn("text-xs font-medium", statusDot.textClass)}>
          {statusDot.label}
        </span>
      </div>

      {/* Specs */}
      <div className="hidden flex-shrink-0 text-right text-xs text-muted-foreground lg:block">
        {container.cpuCores !== undefined && (
          <span>{container.cpuCores} CPU</span>
        )}
        {container.cpuCores !== undefined && container.memoryMb !== undefined && (
          <span className="mx-1.5 text-border">·</span>
        )}
        {container.memoryMb !== undefined && (
          <span>{container.memoryMb} MB</span>
        )}
      </div>

      {/* Actions — always visible */}
      <div className="flex flex-shrink-0 items-center gap-0.5">
        {isRunning ? (
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={(e) => { e.stopPropagation(); void stopContainer(container.id); }}
            data-testid="container-stop"
            title={t("containers", "stop")}
          >
            <Square className="h-3.5 w-3.5" />
          </Button>
        ) : (
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={(e) => { e.stopPropagation(); void startContainer(container.id); }}
            data-testid="container-start"
            title={t("containers", "start")}
          >
            <Play className="h-3.5 w-3.5" />
          </Button>
        )}
        <Button
          variant="ghost"
          size="icon-xs"
          onClick={remove}
          data-testid="container-delete"
          title={t("containers", "delete")}
          className="text-destructive hover:text-destructive"
        >
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}

function getStatusDot(status: ContainerInfo["status"], labels: Record<string, string>) {
  switch (status) {
    case "running":
      return {
        label: labels.running,
        dotClass: "bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.6)]",
        textClass: "text-emerald-500",
      };
    case "paused":
      return {
        label: labels.paused,
        dotClass: "bg-amber-400",
        textClass: "text-amber-500",
      };
    case "creating":
    case "created":
      return {
        label: labels.creating,
        dotClass: "bg-yellow-400 animate-pulse",
        textClass: "text-yellow-500",
      };
    case "restarting":
      return {
        label: labels.restarting,
        dotClass: "bg-blue-400 animate-pulse",
        textClass: "text-blue-500",
      };
    case "exited":
    case "stopped":
      return {
        label: labels.stopped,
        dotClass: "bg-zinc-400",
        textClass: "text-muted-foreground",
      };
    case "dead":
      return {
        label: labels.dead,
        dotClass: "bg-red-400",
        textClass: "text-red-500",
      };
    default:
      return {
        label: status,
        dotClass: "bg-zinc-400",
        textClass: "text-muted-foreground",
      };
  }
}

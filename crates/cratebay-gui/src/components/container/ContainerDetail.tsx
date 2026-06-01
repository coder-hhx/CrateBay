import { useState, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@/lib/tauri";
import { useContainerStore, type ContainerInfo } from "@/stores/containerStore";
import { useI18n } from "@/lib/i18n";
import { useAppStore } from "@/stores/appStore";
import { ContainerLogs } from "@/components/container/ContainerLogs";
import { ContainerMonitoring } from "@/components/container/ContainerMonitoring";
import { TerminalView } from "@/components/container/TerminalView";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { Play, Square, Trash2, Copy, Check, X, PackagePlus, Loader2 } from "lucide-react";

/**
 * Container detail panel — fixed-positioned overlay on the right side.
 * Uses React Portal to render outside ContainersPage's overflow-hidden,
 * positioned relative to the <main> content area.
 */
const PANEL_WIDTH = 400;

type ContainerInspectDetail = {
  info: ContainerInfo;
  networkSettings: unknown;
  mounts: unknown[];
  state: {
    status: string;
    running: boolean;
    startedAt: string | null;
    finishedAt: string | null;
    exitCode: number | null;
    error: string | null;
    pid: number | null;
  };
};

export function ContainerDetail() {
  const selectedContainerId = useContainerStore((s) => s.selectedContainerId);
  const containers = useContainerStore((s) => s.containers);
  const selectContainer = useContainerStore((s) => s.selectContainer);
  const [portalTarget, setPortalTarget] = useState<HTMLElement | null>(null);

  const container = containers.find((c) => c.id === selectedContainerId) ?? null;
  const isOpen = container !== null;

  // Find the <main> element as portal mount point
  useEffect(() => {
    const main = document.querySelector("main");
    if (main) {
      // Ensure main is a positioning context
      if (getComputedStyle(main).position === "static") {
        main.style.position = "relative";
      }
      setPortalTarget(main);
    }
  }, []);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") selectContainer(null);
    },
    [selectContainer],
  );

  useEffect(() => {
    if (isOpen) {
      document.addEventListener("keydown", handleKeyDown);
      return () => document.removeEventListener("keydown", handleKeyDown);
    }
  }, [isOpen, handleKeyDown]);

  if (!portalTarget) return null;

  return createPortal(
    <>
      {/* Click-away backdrop — transparent */}
      {isOpen && (
        <div
          className="absolute inset-0 z-40"
          onClick={() => selectContainer(null)}
        />
      )}

      {/* Panel — slides in from the right */}
      <div
        data-testid="container-detail"
        className={cn(
          "absolute bottom-0 right-0 top-0 z-50 flex flex-col border-l border-border bg-card shadow-2xl transition-transform duration-300 ease-in-out",
          isOpen ? "translate-x-0" : "translate-x-full",
        )}
        style={{ width: `${PANEL_WIDTH}px` }}
      >
        {container !== null && (
          <DetailHeader container={container} onClose={() => selectContainer(null)} />
        )}
        <div className="flex-1 overflow-y-auto">
          {container !== null && <DetailContent container={container} />}
        </div>
      </div>
    </>,
    portalTarget,
  );
}

function DetailHeader({
  container,
  onClose,
}: {
  container: ContainerInfo;
  onClose: () => void;
}) {
  const { startContainer, stopContainer, deleteContainer, selectContainer, fetchImages } = useContainerStore();
  const { t } = useI18n();
  const { addNotification } = useAppStore();
  const isRunning = container.status === "running" || container.status === "paused";
  const [operating, setOperating] = useState(false);
  const [packDialogOpen, setPackDialogOpen] = useState(false);
  const [packImage, setPackImage] = useState(defaultPackImageName(container.name));
  const [packing, setPacking] = useState(false);

  useEffect(() => {
    setPackImage(defaultPackImageName(container.name));
  }, [container.name]);

  const handleAction = async (action: () => Promise<void>, successMsg: string, errorTitle: string) => {
    setOperating(true);
    try {
      await action();
      addNotification({ type: "success", title: successMsg, message: container.name, dismissable: true });
    } catch (error) {
      addNotification({
        type: "error",
        title: errorTitle,
        message: error instanceof Error ? error.message : String(error),
        dismissable: true,
      });
    } finally {
      setOperating(false);
    }
  };

  const handleDelete = async () => {
    if (!confirm(t("containers", "confirmDelete").replace("{name}", container.name))) return;
    setOperating(true);
    try {
      await deleteContainer(container.id);
      selectContainer(null);
      addNotification({ type: "success", title: t("containers", "deleteSuccess"), message: container.name, dismissable: true });
    } catch (error) {
      addNotification({
        type: "error",
        title: t("containers", "deleteFailed"),
        message: error instanceof Error ? error.message : String(error),
        dismissable: true,
      });
    } finally {
      setOperating(false);
    }
  };

  const handlePack = async () => {
    const image = packImage.trim();
    if (image.length === 0) return;

    setPacking(true);
    try {
      await invoke<string>("image_pack_container", {
        container: container.id,
        image,
      });
      await fetchImages();
      addNotification({
        type: "success",
        title: t("containers", "packImageSuccess"),
        message: image,
        dismissable: true,
      });
      setPackDialogOpen(false);
    } catch (error) {
      addNotification({
        type: "error",
        title: t("containers", "packImageFailed"),
        message: error instanceof Error ? error.message : String(error),
        dismissable: true,
      });
    } finally {
      setPacking(false);
    }
  };

  return (
    <>
      <div className="flex items-center gap-2 border-b border-border px-4 py-3">
        <h2 className="flex-1 truncate text-sm font-semibold text-foreground">
          {container.name}
        </h2>

        {isRunning ? (
          <button
            onClick={() => void handleAction(() => stopContainer(container.id), t("containers", "stopSuccess"), t("containers", "stopFailed"))}
            disabled={operating}
            className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-40"
            title="停止"
          >
            <Square className="h-3.5 w-3.5" />
          </button>
        ) : (
          <button
            onClick={() => void handleAction(() => startContainer(container.id), t("containers", "startSuccess"), t("containers", "startFailed"))}
            disabled={operating}
            className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-emerald-600 disabled:opacity-40"
            title="启动"
          >
            <Play className="h-3.5 w-3.5" />
          </button>
        )}

        <button
          onClick={() => setPackDialogOpen(true)}
          disabled={operating || packing}
          className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-40"
          title={t("containers", "packImage")}
        >
          <PackagePlus className="h-3.5 w-3.5" />
        </button>

        <button
          onClick={() => void handleDelete()}
          disabled={operating}
          className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:opacity-40"
          title="删除"
        >
          <Trash2 className="h-3.5 w-3.5" />
        </button>

        <div className="mx-1 h-4 w-px bg-border" />
        <button
          onClick={onClose}
          className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      <Dialog open={packDialogOpen} onOpenChange={setPackDialogOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{t("containers", "packImage")}</DialogTitle>
            <DialogDescription>{t("containers", "packImageDesc")}</DialogDescription>
          </DialogHeader>
          <div className="flex flex-col gap-2 py-2">
            <Label htmlFor="pack-image-name">{t("containers", "imageName")}</Label>
            <Input
              id="pack-image-name"
              value={packImage}
              onChange={(event) => setPackImage(event.target.value)}
              placeholder="cratebay/my-container:latest"
              className="font-mono text-xs"
              autoComplete="off"
            />
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setPackDialogOpen(false)} disabled={packing}>
              {t("common", "cancel")}
            </Button>
            <Button onClick={() => void handlePack()} disabled={packing || packImage.trim().length === 0}>
              {packing ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <PackagePlus className="h-3.5 w-3.5" />}
              {t("containers", "packImage")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

function DetailContent({ container }: { container: ContainerInfo }) {
  const { t } = useI18n();
  const isRunning = container.status === "running" || container.status === "paused";
  const [inspectDetail, setInspectDetail] = useState<ContainerInspectDetail | null>(null);
  const [inspectError, setInspectError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    setInspectDetail(null);
    setInspectError(null);

    void invoke<ContainerInspectDetail>("container_inspect", { id: container.id })
      .then((detail) => {
        if (!cancelled) setInspectDetail(detail);
      })
      .catch((error) => {
        if (!cancelled) {
          setInspectError(error instanceof Error ? error.message : String(error));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [container.id]);

  const runtimeState = inspectDetail?.state;
  const networkAddresses = collectNetworkAddresses(inspectDetail?.networkSettings);
  const mountTargets = collectMountTargets(inspectDetail?.mounts);

  return (
    <div className="flex flex-col gap-5 p-4">
      {/* Status badge */}
      <StatusBadge status={container.status} />

      {/* Overview */}
      <section>
        <SectionTitle>{t("containers", "overview") ?? "概览"}</SectionTitle>
        <div className="space-y-3">
          <CopyableField label="ID" value={container.shortId} />
          <DetailField label={t("containers", "image")}>{container.image}</DetailField>
          <DetailField label={t("containers", "created")}>
            {formatRelativeTime(container.createdAt)}
          </DetailField>
          <DetailField label={t("containers", "template")}>{container.labels?.["com.cratebay.template_id"] || "—"}</DetailField>
        </div>
      </section>

      {/* Specs — plain text, no cards */}
      <section>
        <SectionTitle>规格</SectionTitle>
        <div className="grid grid-cols-2 gap-x-6 gap-y-2">
          <DetailField label={t("containers", "cpuCores")}>
            {container.cpuCores !== undefined ? `${container.cpuCores} 核心` : "—"}
          </DetailField>
          <DetailField label={t("containers", "memoryMb")}>
            {container.memoryMb !== undefined ? `${container.memoryMb} MB` : "—"}
          </DetailField>
        </div>
      </section>

      <section>
        <SectionTitle>{t("containers", "resources")}</SectionTitle>
        <ContainerMonitoring
          containerId={container.id}
          cpuCores={container.cpuCores}
          memoryMb={container.memoryMb}
          enabled={isRunning}
        />
      </section>

      <section data-testid="container-inspect">
        <SectionTitle>{t("containers", "runtimeDetails")}</SectionTitle>
        {inspectError !== null ? (
          <div className="rounded-lg border border-border bg-muted/30 p-3 text-xs text-muted-foreground">
            {t("containers", "inspectUnavailable")}: {inspectError}
          </div>
        ) : inspectDetail === null ? (
          <div className="text-xs text-muted-foreground">{t("common", "loading")}</div>
        ) : (
          <div className="grid grid-cols-2 gap-x-6 gap-y-2">
            <DetailField label={t("containers", "state")}>
              {runtimeState?.status || container.state || "—"}
            </DetailField>
            <DetailField label={t("containers", "pid")}>
              {runtimeState?.pid !== null && runtimeState?.pid !== undefined
                ? String(runtimeState.pid)
                : "—"}
            </DetailField>
            <DetailField label={t("containers", "network")}>
              {networkAddresses.length > 0 ? networkAddresses.join(", ") : "—"}
            </DetailField>
            <DetailField label={t("containers", "mounts")}>
              {mountTargets.length > 0 ? mountTargets.join(", ") : t("containers", "noMounts")}
            </DetailField>
          </div>
        )}
      </section>

      {/* Ports */}
      {container.ports.length > 0 && (
        <section>
          <SectionTitle>{t("containers", "ports") ?? "端口映射"}</SectionTitle>
          <div className="space-y-1.5">
            {container.ports.map((port) => (
              <div
                key={`${port.hostPort}-${port.containerPort}`}
                className="flex items-center justify-between rounded-md bg-muted/40 px-3 py-1.5"
              >
                <div className="flex items-center gap-2">
                  <span className="font-mono text-xs font-medium text-foreground">
                    {port.hostPort}:{port.containerPort}
                  </span>
                  <span className={cn(
                    "rounded px-1 py-0.5 text-[10px] font-medium uppercase",
                    port.protocol === "tcp"
                      ? "bg-blue-500/10 text-blue-600 dark:text-blue-400"
                      : "bg-purple-500/10 text-purple-600 dark:text-purple-400"
                  )}>
                    {port.protocol}
                  </span>
                </div>
                <CopyButton value={`${port.hostPort}:${port.containerPort}`} />
              </div>
            ))}
          </div>
        </section>
      )}

      {/* Logs and terminal */}
      <section>
        <SectionTitle>{t("containers", "operations")}</SectionTitle>
        <Tabs defaultValue="logs" className="gap-3">
          <TabsList className="w-full">
            <TabsTrigger value="logs" className="text-xs">
              {t("containers", "logs")}
            </TabsTrigger>
            <TabsTrigger value="terminal" className="text-xs">
              {t("containers", "terminal")}
            </TabsTrigger>
          </TabsList>
          <TabsContent value="logs">
            <ContainerLogs containerId={container.id} tail={200} />
          </TabsContent>
          <TabsContent value="terminal">
            {isRunning ? (
              <TerminalView containerId={container.id} />
            ) : (
              <div className="rounded-lg border border-border bg-muted/30 p-3 text-xs text-muted-foreground">
                {t("containers", "terminalUnavailable")}
              </div>
            )}
          </TabsContent>
        </Tabs>
      </section>
    </div>
  );
}

/* ─── Helper Components ─── */

function defaultPackImageName(containerName: string): string {
  const name = containerName
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "");

  return `cratebay/${name || "container"}:latest`;
}

function collectNetworkAddresses(networkSettings: unknown): string[] {
  const root = toRecord(networkSettings);
  const networks = toRecord(root?.Networks ?? root?.networks);
  if (!networks) return [];

  return Object.values(networks)
    .flatMap((network) => {
      const record = toRecord(network);
      if (!record) return [];
      return [
        stringValue(record.IPAddress ?? record.ipAddress),
        stringValue(record.GlobalIPv6Address ?? record.globalIPv6Address),
      ];
    })
    .filter((address): address is string => address !== null && address.length > 0);
}

function collectMountTargets(mounts: unknown[] | undefined): string[] {
  if (!Array.isArray(mounts)) return [];

  return mounts
    .map((mount) => {
      const record = toRecord(mount);
      return stringValue(record?.Destination ?? record?.destination);
    })
    .filter((target): target is string => target !== null && target.length > 0);
}

function toRecord(value: unknown): Record<string, unknown> | null {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function stringValue(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
      {children}
    </h3>
  );
}

function DetailField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-[10px] uppercase tracking-wider text-muted-foreground">{label}</span>
      <span className="truncate text-sm text-foreground">{children}</span>
    </div>
  );
}

function CopyButton({ value }: { value: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // ignore
    }
  };

  return (
    <button
      onClick={handleCopy}
      className="flex h-6 w-6 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
      title="复制"
    >
      {copied ? (
        <Check className="h-3 w-3 text-emerald-500" />
      ) : (
        <Copy className="h-3 w-3" />
      )}
    </button>
  );
}

function CopyableField({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-2">
      <div className="flex flex-col gap-0.5 flex-1 min-w-0">
        <span className="text-[10px] uppercase tracking-wider text-muted-foreground">{label}</span>
        <span className="truncate font-mono text-sm text-foreground">{value}</span>
      </div>
      <CopyButton value={value} />
    </div>
  );
}

function StatusBadge({ status }: { status: ContainerInfo["status"] }) {
  const { t } = useI18n();
  type DisplayStatus = "running" | "stopped" | "creating" | "error";

  const displayStatus: DisplayStatus = (() => {
    switch (status) {
      case "running":
      case "paused":
        return "running";
      case "creating":
      case "restarting":
      case "removing":
        return "creating";
      case "dead":
        return "error";
      case "stopped":
      case "created":
      case "exited":
        return "stopped";
      default:
        return "error";
    }
  })();

  const variants: Record<DisplayStatus, { labelKey: DisplayStatus; dotClass: string; badgeClass: string }> = {
    running: {
      labelKey: "running",
      dotClass: "bg-emerald-400",
      badgeClass: "border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
    },
    stopped: {
      labelKey: "stopped",
      dotClass: "bg-zinc-400",
      badgeClass: "border-zinc-400/30 bg-zinc-400/10 text-zinc-500",
    },
    creating: {
      labelKey: "creating",
      dotClass: "bg-yellow-400 animate-pulse",
      badgeClass: "border-yellow-500/30 bg-yellow-500/10 text-yellow-600 dark:text-yellow-400",
    },
    error: {
      labelKey: "error",
      dotClass: "bg-red-400",
      badgeClass: "border-red-500/30 bg-red-500/10 text-red-600 dark:text-red-400",
    },
  };

  const variant = variants[displayStatus];

  return (
    <Badge
      variant="outline"
      className={cn(
        "inline-flex w-fit items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium",
        variant.badgeClass,
      )}
    >
      <span className={cn("inline-block h-2 w-2 rounded-full", variant.dotClass)} />
      {t("containers", variant.labelKey)}
    </Badge>
  );
}

function formatRelativeTime(isoString: string): string {
  try {
    const date = new Date(isoString);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMin = Math.floor(diffMs / 60000);
    const diffHour = Math.floor(diffMs / 3600000);
    const diffDay = Math.floor(diffMs / 86400000);

    if (diffMin < 1) return "刚刚";
    if (diffMin < 60) return `${diffMin} 分钟前`;
    if (diffHour < 24) return `${diffHour} 小时前`;
    if (diffDay < 7) return `${diffDay} 天前`;
    return date.toLocaleDateString("zh-CN", { month: "short", day: "numeric" });
  } catch {
    return "—";
  }
}

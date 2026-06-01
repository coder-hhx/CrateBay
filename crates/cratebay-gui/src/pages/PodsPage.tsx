import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";
import { useContainerStore } from "@/stores/containerStore";
import type { ContainerInfo } from "@/types/container";
import type { PodContainerInfo, PodInfo } from "@/types/pod";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Boxes,
  Eye,
  Link2,
  Loader2,
  Plus,
  RefreshCw,
  Trash2,
  Unlink,
} from "lucide-react";

export function PodsPage() {
  const { t } = useI18n();
  const containers = useContainerStore((state) => state.containers);
  const fetchContainers = useContainerStore((state) => state.fetchContainers);
  const [pods, setPods] = useState<PodInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [newName, setNewName] = useState("");
  const [creating, setCreating] = useState(false);
  const [busyPod, setBusyPod] = useState<string | null>(null);
  const [containerInputs, setContainerInputs] = useState<Record<string, string>>({});
  const [deleteTarget, setDeleteTarget] = useState<PodInfo | null>(null);
  const [forceDelete, setForceDelete] = useState(true);
  const [inspectTarget, setInspectTarget] = useState<PodInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const podCount = useMemo(() => pods.length, [pods]);

  const fetchPods = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<PodInfo[]>("pod_list");
      setPods(result);
    } catch (err) {
      setPods([]);
      setError(formatError(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchPods();
    void fetchContainers();
  }, [fetchContainers, fetchPods]);

  const handleCreate = useCallback(async () => {
    const name = newName.trim();
    if (name.length === 0) return;
    setCreating(true);
    setError(null);
    try {
      await invoke<PodInfo>("pod_create", { name });
      setNewName("");
      await fetchPods();
    } catch (err) {
      setError(formatError(err));
    } finally {
      setCreating(false);
    }
  }, [fetchPods, newName]);

  const handleAddContainer = useCallback(async (podName: string) => {
    const container = containerInputs[podName]?.trim();
    if (!container) return;
    setBusyPod(podName);
    setError(null);
    try {
      await invoke("pod_add_container", { name: podName, container });
      setContainerInputs((prev) => ({ ...prev, [podName]: "" }));
      await fetchPods();
    } catch (err) {
      setError(formatError(err));
    } finally {
      setBusyPod(null);
    }
  }, [containerInputs, fetchPods]);

  const handleRemoveContainer = useCallback(async (podName: string, container: string) => {
    setBusyPod(podName);
    setError(null);
    try {
      await invoke("pod_remove_container", { name: podName, container, force: true });
      await fetchPods();
    } catch (err) {
      setError(formatError(err));
    } finally {
      setBusyPod(null);
    }
  }, [fetchPods]);

  const handleDelete = useCallback(async () => {
    if (deleteTarget === null) return;
    setBusyPod(deleteTarget.name);
    setError(null);
    try {
      await invoke("pod_delete", { name: deleteTarget.name, force: forceDelete });
      setDeleteTarget(null);
      await fetchPods();
    } catch (err) {
      setError(formatError(err));
    } finally {
      setBusyPod(null);
    }
  }, [deleteTarget, fetchPods, forceDelete]);

  return (
    <div className="flex h-full flex-col" data-testid="pods-page">
      <div className="flex items-center gap-3 border-b border-border px-6 py-2.5">
        <div className="flex min-w-0 flex-col">
          <h1 className="text-sm font-semibold text-foreground">{t("pods", "title")}</h1>
          <p className="text-xs text-muted-foreground">{t("pods", "subtitle")}</p>
        </div>
        <Badge variant="outline" className="ml-1 text-[10px]">
          {podCount} {t("pods", "podCount")}
        </Badge>
        <div className="ml-auto flex items-center gap-2">
          <Input
            value={newName}
            onChange={(event) => setNewName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void handleCreate();
              }
            }}
            placeholder={t("pods", "namePlaceholder")}
            className="h-8 w-48 text-xs"
          />
          <Button size="sm" onClick={() => void handleCreate()} disabled={creating || newName.trim().length === 0}>
            {creating ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Plus className="h-3.5 w-3.5" />}
            {t("pods", "create")}
          </Button>
          <Button variant="ghost" size="sm" onClick={() => void fetchPods()} disabled={loading}>
            <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
            {t("common", "refresh")}
          </Button>
        </div>
      </div>

      {error !== null && (
        <div className="mx-6 mt-4 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      )}

      <div className="flex-1 overflow-auto px-6 py-4">
        {loading ? (
          <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            {t("pods", "loadingPods")}
          </div>
        ) : pods.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center text-muted-foreground">
            <Boxes className="mb-3 h-12 w-12 opacity-20" />
            <h3 className="text-sm font-medium">{t("pods", "noPods")}</h3>
            <p className="mt-1 text-xs">{t("pods", "noPodsHint")}</p>
            <p className="mt-3 rounded-md bg-muted px-2 py-1 font-mono text-[11px]">{t("pods", "commandHint")}</p>
          </div>
        ) : (
          <div className="space-y-3">
            {pods.map((pod) => (
              <PodRow
                key={pod.id || pod.name}
                pod={pod}
                busy={busyPod === pod.name}
                availableContainers={containers}
                containerValue={containerInputs[pod.name] ?? ""}
                onContainerValueChange={(value) =>
                  setContainerInputs((prev) => ({ ...prev, [pod.name]: value }))
                }
                onAddContainer={() => void handleAddContainer(pod.name)}
                onRemoveContainer={(container) => void handleRemoveContainer(pod.name, container)}
                onInspect={() => setInspectTarget(pod)}
                onDelete={() => {
                  setForceDelete(true);
                  setDeleteTarget(pod);
                }}
              />
            ))}
          </div>
        )}
      </div>

      <PodInspectDialog pod={inspectTarget} onClose={() => setInspectTarget(null)} />

      <Dialog
        open={deleteTarget !== null}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
      >
        <DialogContent className="sm:max-w-[420px]">
          <DialogHeader>
            <DialogTitle>{t("pods", "deletePod")}</DialogTitle>
            <DialogDescription>{t("pods", "confirmDelete")}</DialogDescription>
          </DialogHeader>
          {deleteTarget !== null && (
            <div className="space-y-3">
              <div className="rounded-md border bg-muted px-2 py-1 font-mono text-xs">
                {deleteTarget.name}
              </div>
              <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <Checkbox checked={forceDelete} onCheckedChange={(checked) => setForceDelete(checked === true)} />
                {t("pods", "forceDelete")}
              </label>
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteTarget(null)}>
              {t("common", "cancel")}
            </Button>
            <Button variant="destructive" onClick={() => void handleDelete()} disabled={busyPod !== null}>
              {busyPod !== null ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Trash2 className="h-3.5 w-3.5" />}
              {t("common", "delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function PodRow({
  pod,
  busy,
  availableContainers,
  containerValue,
  onContainerValueChange,
  onAddContainer,
  onRemoveContainer,
  onInspect,
  onDelete,
}: {
  pod: PodInfo;
  busy: boolean;
  availableContainers: ContainerInfo[];
  containerValue: string;
  onContainerValueChange: (value: string) => void;
  onAddContainer: () => void;
  onRemoveContainer: (container: string) => void;
  onInspect: () => void;
  onDelete: () => void;
}) {
  const { t } = useI18n();
  const shortId = pod.id.length > 12 ? pod.id.slice(0, 12) : pod.id || "-";
  const containerOptions = availableContainers.filter((container) =>
    !isContainerAttachedToPod(container, pod.containers),
  );
  const selectedContainer = availableContainers.find((container) =>
    container.id === containerValue
    || container.shortId === containerValue
    || container.name === containerValue,
  );
  const selectedValue = selectedContainer?.id ?? "__manual";

  return (
    <div className="rounded-lg border border-border bg-card px-4 py-3">
      <div className="flex items-start gap-3">
        <div className="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary">
          <Boxes className="h-[18px] w-[18px]" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="truncate font-mono text-sm font-semibold text-foreground">{pod.name}</span>
            <Badge variant="outline" className="text-[10px]">{pod.driver || "bridge"}</Badge>
            <Badge variant="secondary" className="text-[10px]">
              {pod.containers.length} {t("pods", "containerCount")}
            </Badge>
          </div>
          <div className="mt-0.5 flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
            <span className="font-mono">{shortId}</span>
            {pod.createdAt && <span>{pod.createdAt}</span>}
          </div>
        </div>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onInspect} title={t("pods", "inspectPod")}>
            <Eye className="h-3.5 w-3.5" />
          </Button>
          <Button variant="ghost" size="icon" className="h-7 w-7 text-destructive hover:text-destructive" onClick={onDelete} title={t("pods", "deletePod")}>
            <Trash2 className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>

      <div className="mt-3 grid gap-2 md:grid-cols-[minmax(180px,260px)_minmax(180px,1fr)_auto]">
        <div className="flex flex-col gap-1.5">
          <Label className="text-[11px] text-muted-foreground">{t("pods", "selectContainer")}</Label>
          <Select
            value={selectedValue}
            onValueChange={(value) => {
              if (value === "__manual") {
                onContainerValueChange("");
                return;
              }
              const container = availableContainers.find((item) => item.id === value);
              if (container) onContainerValueChange(container.name || container.id);
            }}
          >
            <SelectTrigger aria-label={t("pods", "selectContainer")} className="h-8 w-full text-xs">
              <SelectValue placeholder={t("pods", "selectContainer")} />
            </SelectTrigger>
            <SelectContent className="w-full">
              <SelectItem value="__manual">{t("pods", "manualContainer")}</SelectItem>
              {containerOptions.map((container) => (
                <SelectItem key={container.id} value={container.id}>
                  {formatContainerOption(container)}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className="flex flex-col gap-1.5">
          <Label htmlFor={`pod-container-${pod.name}`} className="text-[11px] text-muted-foreground">
            {t("pods", "manualContainer")}
          </Label>
          <Input
            id={`pod-container-${pod.name}`}
            value={containerValue}
            onChange={(event) => onContainerValueChange(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                onAddContainer();
              }
            }}
            placeholder={t("pods", "containerPlaceholder")}
            className="h-8 text-xs"
          />
        </div>
        <Button
          variant="outline"
          size="sm"
          className="self-end"
          onClick={onAddContainer}
          disabled={busy || containerValue.trim().length === 0}
        >
          {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Link2 className="h-3.5 w-3.5" />}
          {t("pods", "addContainer")}
        </Button>
      </div>

      {pod.containers.length > 0 && (
        <div className="mt-3 flex flex-wrap gap-2">
          {pod.containers.map((container) => (
            <span
              key={container.id}
              className="inline-flex max-w-full items-center gap-1 rounded-md border bg-muted px-2 py-1 text-xs"
            >
              <span className="truncate font-mono">{container.name || container.id.slice(0, 12)}</span>
              <span className="text-muted-foreground">{container.ipv4Address ?? container.ipv6Address ?? ""}</span>
              <button
                type="button"
                className="rounded p-0.5 text-muted-foreground hover:bg-background hover:text-destructive"
                onClick={() => onRemoveContainer(container.id)}
                title={t("pods", "removeContainer")}
                disabled={busy}
              >
                <Unlink className="h-3 w-3" />
              </button>
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

function formatContainerOption(container: ContainerInfo): string {
  const shortId = container.shortId || container.id.slice(0, 12);
  return `${container.name} · ${shortId}`;
}

export function isContainerAttachedToPod(
  container: ContainerInfo,
  attachedContainers: PodContainerInfo[],
): boolean {
  const containerIds = [container.id, container.shortId]
    .map(normalizeContainerIdentity)
    .filter(Boolean);
  const containerName = normalizeContainerName(container.name);

  return attachedContainers.some((attached) => {
    const attachedIds = [attached.id]
      .map(normalizeContainerIdentity)
      .filter(Boolean);
    const attachedName = normalizeContainerName(attached.name);

    const idMatches = containerIds.some((containerId) =>
      attachedIds.some((attachedId) => containerIdentityMatches(containerId, attachedId)),
    );
    const nameMatches =
      containerName.length > 0 &&
      attachedName.length > 0 &&
      containerName === attachedName;

    return idMatches || nameMatches;
  });
}

function normalizeContainerIdentity(value: string | undefined): string {
  return (value ?? "").trim().replace(/^sha256:/, "");
}

function normalizeContainerName(value: string | undefined): string {
  return (value ?? "").trim().replace(/^\/+/, "");
}

function containerIdentityMatches(left: string, right: string): boolean {
  if (left.length === 0 || right.length === 0) return false;
  if (left === right) return true;

  const minLength = Math.min(left.length, right.length);
  if (minLength < 12) return false;

  return left.startsWith(right) || right.startsWith(left);
}

function PodInspectDialog({ pod, onClose }: { pod: PodInfo | null; onClose: () => void }) {
  const { t } = useI18n();

  return (
    <Dialog open={pod !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-[640px]">
        <DialogHeader>
          <DialogTitle>{t("pods", "inspectPod")}</DialogTitle>
          <DialogDescription className="font-mono text-xs break-all">
            {pod?.id ?? ""}
          </DialogDescription>
        </DialogHeader>
        {pod !== null && (
          <ScrollArea className="max-h-[52vh] pr-2">
            <div className="grid grid-cols-[140px_1fr] gap-x-4 gap-y-2 text-sm">
              <span className="text-muted-foreground">{t("common", "name")}</span>
              <span className="font-mono">{pod.name}</span>
              <span className="text-muted-foreground">{t("pods", "driver")}</span>
              <span>{pod.driver}</span>
              <span className="text-muted-foreground">{t("pods", "created")}</span>
              <span>{pod.createdAt ?? "-"}</span>
              <span className="text-muted-foreground">{t("pods", "containers")}</span>
              <div className="space-y-1">
                {pod.containers.length === 0 ? (
                  <span className="text-muted-foreground">-</span>
                ) : (
                  pod.containers.map((container) => (
                    <div key={container.id} className="rounded-md border bg-muted px-2 py-1 text-xs">
                      <div className="font-mono text-foreground">{container.name || container.id.slice(0, 12)}</div>
                      <div className="text-muted-foreground">{container.id}</div>
                      {(container.ipv4Address || container.ipv6Address) && (
                        <div className="text-muted-foreground">{container.ipv4Address ?? container.ipv6Address}</div>
                      )}
                    </div>
                  ))
                )}
              </div>
            </div>
          </ScrollArea>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("common", "close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function formatError(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return "Operation failed";
}

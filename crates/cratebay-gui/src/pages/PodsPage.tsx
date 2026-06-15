import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";
import { useContainerStore } from "@/stores/containerStore";
import type { ContainerInfo } from "@/types/container";
import type { PodContainerInfo, PodInfo } from "@/types/pod";
import { Button } from "@/components/ui/button";
import { EngineOfflineCallout } from "@/components/common/EngineOfflineCallout";
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
import { formatTauriError, isImplicitRuntimeStartDisabled } from "@/lib/runtimeOffline";
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
  const [newDriver, setNewDriver] = useState("bridge");
  const [newInternal, setNewInternal] = useState(false);
  const [newEnableIpv6, setNewEnableIpv6] = useState(false);
  const [creating, setCreating] = useState(false);
  const [busyPod, setBusyPod] = useState<string | null>(null);
  const [containerInputs, setContainerInputs] = useState<Record<string, string>>({});
  const [deleteTarget, setDeleteTarget] = useState<PodInfo | null>(null);
  const [forceDelete, setForceDelete] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [inspectTarget, setInspectTarget] = useState<PodInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [engineOffline, setEngineOffline] = useState(false);
  const [startingEngine, setStartingEngine] = useState(false);

  const podCount = useMemo(() => pods.length, [pods]);
  const nameValidationError = useMemo(() => validatePodName(newName, t), [newName, t]);

  const fetchPods = useCallback(async () => {
    setLoading(true);
    setError(null);
    setEngineOffline(false);
    try {
      const result = await invoke<PodInfo[]>("pod_list");
      setPods(result);
    } catch (err) {
      setPods([]);
      if (isImplicitRuntimeStartDisabled(err)) {
        setEngineOffline(true);
      } else {
        setError(formatTauriError(err, t("common", "operationFailed")));
      }
    } finally {
      setLoading(false);
    }
  }, [t]);

  useEffect(() => {
    void fetchPods();
    void fetchContainers();
  }, [fetchContainers, fetchPods]);

  const handleCreate = useCallback(async () => {
    const name = newName.trim();
    if (name.length === 0) return;
    const driver = newDriver.trim() || "bridge";
    setCreating(true);
    setError(null);
    try {
      await invoke<PodInfo>("pod_create", {
        name,
        driver,
        internal: newInternal,
        enableIpv6: newEnableIpv6,
      });
      setNewName("");
      setNewDriver("bridge");
      setNewInternal(false);
      setNewEnableIpv6(false);
      await fetchPods();
      await fetchContainers();
    } catch (err) {
      setError(formatTauriError(err, t("common", "operationFailed")));
    } finally {
      setCreating(false);
    }
  }, [fetchContainers, fetchPods, newDriver, newEnableIpv6, newInternal, newName, t]);

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
      setError(formatTauriError(err, t("common", "operationFailed")));
    } finally {
      setBusyPod(null);
    }
  }, [containerInputs, fetchPods, t]);

  const handleRemoveContainer = useCallback(async (podName: string, container: string) => {
    setBusyPod(podName);
    setError(null);
    try {
      await invoke("pod_remove_container", { name: podName, container, force: true });
      await fetchPods();
    } catch (err) {
      setError(formatTauriError(err, t("common", "operationFailed")));
    } finally {
      setBusyPod(null);
    }
  }, [fetchPods, t]);

  const handleDelete = useCallback(async () => {
    if (deleteTarget === null) return;
    setBusyPod(deleteTarget.name);
    setError(null);
    setDeleteError(null);
    try {
      await invoke("pod_delete", { name: deleteTarget.name, force: forceDelete });
      setDeleteTarget(null);
      setForceDelete(false);
      await fetchPods();
    } catch (err) {
      const message = formatTauriError(err, t("common", "operationFailed"));
      setDeleteError(message);
      setError(message);
    } finally {
      setBusyPod(null);
    }
  }, [deleteTarget, fetchPods, forceDelete, t]);

  const handleStartEngine = useCallback(async () => {
    setStartingEngine(true);
    setError(null);
    try {
      await invoke("runtime_start");
      await fetchPods();
      await fetchContainers();
    } catch (err) {
      setError(formatTauriError(err, t("common", "operationFailed")));
    } finally {
      setStartingEngine(false);
    }
  }, [fetchContainers, fetchPods, t]);

  return (
    <div className="flex h-full flex-col" data-testid="pods-page">
      <div className="flex flex-col gap-2 border-b border-border px-6 py-2.5 xl:flex-row xl:items-center">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <div className="min-w-0">
            <h1 className="text-sm font-semibold text-foreground">{t("pods", "title")}</h1>
            <p className="text-xs text-muted-foreground">{t("pods", "subtitle")}</p>
          </div>
          <Badge variant="outline" className="text-[10px]">
            {podCount} {t("pods", "podCount")}
          </Badge>
        </div>
        <div className="flex flex-wrap items-center gap-2 xl:ml-auto xl:justify-end">
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
            aria-invalid={nameValidationError !== null}
            className={cn(
              "h-8 w-full text-xs sm:w-48",
              nameValidationError !== null && "border-destructive/60 focus-visible:ring-destructive/30",
            )}
          />
          <Input
            value={newDriver}
            onChange={(event) => setNewDriver(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void handleCreate();
              }
            }}
            placeholder={t("pods", "driverPlaceholder")}
            className="h-8 w-full font-mono text-xs sm:w-28"
          />
          <label className="flex h-8 items-center gap-1.5 rounded-md border border-border px-2 text-xs text-muted-foreground">
            <Checkbox
              checked={newInternal}
              onCheckedChange={(checked) => setNewInternal(checked === true)}
            />
            {t("pods", "internal")}
          </label>
          <label className="flex h-8 items-center gap-1.5 rounded-md border border-border px-2 text-xs text-muted-foreground">
            <Checkbox
              checked={newEnableIpv6}
              onCheckedChange={(checked) => setNewEnableIpv6(checked === true)}
            />
            IPv6
          </label>
          <Button
            size="sm"
            onClick={() => void handleCreate()}
            disabled={creating || nameValidationError !== null}
            title={nameValidationError ?? undefined}
          >
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
      {engineOffline && !loading && (
        <div className="mx-6 mt-4">
          <EngineOfflineCallout starting={startingEngine} onStart={() => void handleStartEngine()} />
        </div>
      )}
      {newName.trim().length > 0 && nameValidationError !== null && (
        <div className="mx-6 mt-4 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {nameValidationError}
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
            <Boxes className="mb-2 h-8 w-8 opacity-40" />
            <h3 className="text-sm font-medium">{t("pods", "noPods")}</h3>
            <p className="mt-1 text-xs">{t("pods", "noPodsHint")}</p>
            <p className="mt-3 rounded-md bg-muted px-2 py-1 font-mono text-[11px]">{t("pods", "commandHint")}</p>
          </div>
        ) : (
          <div className="space-y-2">
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
                  setForceDelete(false);
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
          if (!open) {
            setDeleteTarget(null);
            setForceDelete(false);
            setDeleteError(null);
          }
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
              {deleteError !== null && (
                <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
                  {deleteError}
                </div>
              )}
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
    <div className="rounded-md border border-border bg-card px-3 py-2.5">
      <div className="flex items-start gap-2.5">
        <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground">
          <Boxes className="h-4 w-4" />
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

      <div className="mt-2.5 grid gap-2 md:grid-cols-[minmax(180px,260px)_minmax(180px,1fr)_auto]">
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
        <div className="mt-2.5 flex flex-wrap gap-1.5">
          {pod.containers.map((container) => (
            <span
              key={container.id}
              className="inline-flex max-w-full items-center gap-1 rounded border border-border bg-muted/60 px-2 py-1 text-xs"
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

function validatePodName(name: string, t: (namespace: string, key: string) => string): string | null {
  const trimmed = name.trim();
  if (trimmed.length === 0) return t("pods", "nameRequired");
  if (trimmed.length > 63) return t("pods", "nameTooLong");
  if (!/^[a-zA-Z0-9][a-zA-Z0-9_.-]*$/.test(trimmed)) {
    return t("pods", "nameInvalid");
  }
  return null;
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

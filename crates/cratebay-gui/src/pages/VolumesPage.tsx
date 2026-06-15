import { useCallback, useEffect, useMemo, useState } from "react";
import { Database, Eye, Loader2, Plus, RefreshCw, Trash2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { EngineOfflineCallout } from "@/components/common/EngineOfflineCallout";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { formatTauriError, isImplicitRuntimeStartDisabled } from "@/lib/runtimeOffline";
import { invoke } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";
import type { VolumeInfo } from "@/types/volume";

export function VolumesPage() {
  const { t } = useI18n();
  const [volumes, setVolumes] = useState<VolumeInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [busyVolume, setBusyVolume] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [newDriver, setNewDriver] = useState("local");
  const [inspectTarget, setInspectTarget] = useState<VolumeInfo | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<VolumeInfo | null>(null);
  const [forceDelete, setForceDelete] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [engineOffline, setEngineOffline] = useState(false);
  const [startingEngine, setStartingEngine] = useState(false);
  const nameValidationError = useMemo(() => validateVolumeName(newName, t), [newName, t]);

  const fetchVolumes = useCallback(async () => {
    setLoading(true);
    setError(null);
    setEngineOffline(false);
    try {
      const result = await invoke<VolumeInfo[]>("volume_list");
      setVolumes(result);
    } catch (err) {
      setVolumes([]);
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
    void fetchVolumes();
  }, [fetchVolumes]);

  const handleCreate = useCallback(async () => {
    const name = newName.trim();
    if (name.length === 0 || nameValidationError !== null) return;
    const driver = newDriver.trim() || "local";
    setCreating(true);
    setError(null);
    try {
      await invoke<VolumeInfo>("volume_create", { name, driver });
      setNewName("");
      setNewDriver("local");
      await fetchVolumes();
    } catch (err) {
      setError(formatTauriError(err, t("common", "operationFailed")));
    } finally {
      setCreating(false);
    }
  }, [fetchVolumes, nameValidationError, newDriver, newName, t]);

  const handleInspect = useCallback(async (volume: VolumeInfo) => {
    setBusyVolume(volume.name);
    setError(null);
    try {
      const result = await invoke<VolumeInfo>("volume_inspect", { name: volume.name });
      setInspectTarget(result);
    } catch (err) {
      setError(formatTauriError(err, t("common", "operationFailed")));
    } finally {
      setBusyVolume(null);
    }
  }, [t]);

  const handleDelete = useCallback(async () => {
    if (deleteTarget === null) return;
    setBusyVolume(deleteTarget.name);
    setError(null);
    setDeleteError(null);
    try {
      await invoke("volume_delete", { name: deleteTarget.name, force: forceDelete });
      setDeleteTarget(null);
      setForceDelete(false);
      await fetchVolumes();
    } catch (err) {
      const message = formatTauriError(err, t("common", "operationFailed"));
      setDeleteError(message);
      setError(message);
    } finally {
      setBusyVolume(null);
    }
  }, [deleteTarget, fetchVolumes, forceDelete, t]);

  const handleStartEngine = useCallback(async () => {
    setStartingEngine(true);
    setError(null);
    try {
      await invoke("runtime_start");
      await fetchVolumes();
    } catch (err) {
      setError(formatTauriError(err, t("common", "operationFailed")));
    } finally {
      setStartingEngine(false);
    }
  }, [fetchVolumes, t]);

  return (
    <div className="flex h-full flex-col" data-testid="volumes-page">
      <div className="flex flex-col gap-2 border-b border-border px-6 py-2.5 xl:flex-row xl:items-center">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <div className="min-w-0">
            <h1 className="text-sm font-semibold text-foreground">{t("volumes", "title")}</h1>
            <p className="text-xs text-muted-foreground">{t("volumes", "subtitle")}</p>
          </div>
          <Badge variant="outline" className="text-[10px]">
            {volumes.length} {t("volumes", "volumeCount")}
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
            placeholder={t("volumes", "namePlaceholder")}
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
            placeholder={t("volumes", "driverPlaceholder")}
            className="h-8 w-full font-mono text-xs sm:w-28"
          />
          <Button
            size="sm"
            onClick={() => void handleCreate()}
            disabled={creating || nameValidationError !== null}
            title={nameValidationError ?? undefined}
          >
            {creating ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Plus className="h-3.5 w-3.5" />}
            {t("volumes", "create")}
          </Button>
          <Button variant="ghost" size="sm" onClick={() => void fetchVolumes()} disabled={loading}>
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
            {t("volumes", "loadingVolumes")}
          </div>
        ) : volumes.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center text-muted-foreground">
            <Database className="mb-2 h-8 w-8 opacity-40" />
            <h3 className="text-sm font-medium">{t("volumes", "noVolumes")}</h3>
            <p className="mt-1 text-xs">{t("volumes", "noVolumesHint")}</p>
            <p className="mt-3 rounded-md bg-muted px-2 py-1 font-mono text-[11px]">{t("volumes", "commandHint")}</p>
          </div>
        ) : (
          <div className="overflow-x-auto rounded-md border border-border bg-card">
            <div className="grid min-w-[680px] grid-cols-[minmax(180px,1.2fr)_120px_minmax(220px,2fr)_96px] border-b border-border px-3 py-2 text-[11px] font-medium uppercase text-muted-foreground">
              <span>{t("common", "name")}</span>
              <span>{t("volumes", "driver")}</span>
              <span>{t("volumes", "mountpoint")}</span>
              <span className="text-right">{t("common", "actions")}</span>
            </div>
            {volumes.map((volume) => (
              <VolumeRow
                key={volume.name}
                volume={volume}
                busy={busyVolume === volume.name}
                onInspect={() => void handleInspect(volume)}
                onDelete={() => setDeleteTarget(volume)}
              />
            ))}
          </div>
        )}
      </div>

      <VolumeInspectDialog volume={inspectTarget} onClose={() => setInspectTarget(null)} />

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
            <DialogTitle>{t("volumes", "deleteVolume")}</DialogTitle>
            <DialogDescription>{t("volumes", "confirmDelete")}</DialogDescription>
          </DialogHeader>
          {deleteTarget !== null && (
            <div className="space-y-3">
              <div className="rounded-md border bg-muted px-2 py-1 font-mono text-xs">
                {deleteTarget.name}
              </div>
              <label className="flex items-center gap-2 rounded-md border border-border px-3 py-2 text-xs text-muted-foreground">
                <Checkbox
                  checked={forceDelete}
                  onCheckedChange={(checked) => setForceDelete(checked === true)}
                />
                {t("volumes", "forceDelete")}
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
            <Button variant="destructive" onClick={() => void handleDelete()} disabled={busyVolume !== null}>
              {busyVolume !== null ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Trash2 className="h-3.5 w-3.5" />}
              {t("common", "delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function VolumeRow({
  volume,
  busy,
  onInspect,
  onDelete,
}: {
  volume: VolumeInfo;
  busy: boolean;
  onInspect: () => void;
  onDelete: () => void;
}) {
  const { t } = useI18n();

  return (
    <div className="grid min-w-[680px] grid-cols-[minmax(180px,1.2fr)_120px_minmax(220px,2fr)_96px] items-center border-b border-border px-3 py-2.5 text-xs last:border-b-0">
      <div className="min-w-0">
        <div className="truncate font-mono text-sm font-semibold text-foreground">{volume.name}</div>
        <div className="mt-0.5 flex items-center gap-2 text-[11px] text-muted-foreground">
          <span>{volume.scope || "local"}</span>
          {volume.createdAt ? <span>{volume.createdAt}</span> : null}
        </div>
      </div>
      <Badge variant="outline" className="w-fit text-[10px]">
        {volume.driver || "local"}
      </Badge>
      <div className="truncate font-mono text-xs text-muted-foreground" title={volume.mountpoint}>
        {volume.mountpoint || "-"}
      </div>
      <div className="flex justify-end gap-1">
        <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onInspect} disabled={busy} title={t("volumes", "inspectVolume")}>
          {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Eye className="h-3.5 w-3.5" />}
        </Button>
        <Button variant="ghost" size="icon" className="h-7 w-7 text-destructive hover:text-destructive" onClick={onDelete} title={t("volumes", "deleteVolume")}>
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}

function VolumeInspectDialog({
  volume,
  onClose,
}: {
  volume: VolumeInfo | null;
  onClose: () => void;
}) {
  const { t } = useI18n();
  if (volume === null) return null;

  return (
    <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="sm:max-w-[560px]">
        <DialogHeader>
          <DialogTitle>{t("volumes", "inspectVolume")}</DialogTitle>
          <DialogDescription>{volume.name}</DialogDescription>
        </DialogHeader>
        <ScrollArea className="max-h-[440px] pr-3">
          <div className="grid gap-2 text-xs">
            <Detail label={t("common", "name")} value={volume.name} mono />
            <Detail label={t("volumes", "driver")} value={volume.driver || "local"} />
            <Detail label={t("volumes", "scope")} value={volume.scope || "local"} />
            <Detail label={t("volumes", "mountpoint")} value={volume.mountpoint || "-"} mono />
            <Detail label={t("volumes", "managedBy")} value={volume.managedBy || "cratebay"} />
            <JsonBlock title={t("volumes", "labels")} value={volume.labels} />
            <JsonBlock title={t("volumes", "options")} value={volume.options} />
          </div>
        </ScrollArea>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            {t("common", "close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Detail({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="grid grid-cols-[120px_minmax(0,1fr)] gap-3 border-b border-border py-2 last:border-b-0">
      <span className="text-muted-foreground">{label}</span>
      <span className={cn("truncate", mono && "font-mono")} title={value}>
        {value}
      </span>
    </div>
  );
}

function JsonBlock({ title, value }: { title: string; value: unknown }) {
  return (
    <div className="border-b border-border py-2 last:border-b-0">
      <div className="mb-1 text-muted-foreground">{title}</div>
      <pre className="max-h-40 overflow-auto rounded-md bg-muted px-2 py-1 font-mono text-[11px]">
        {JSON.stringify(value ?? {}, null, 2)}
      </pre>
    </div>
  );
}

function validateVolumeName(value: string, t: (namespace: string, key: string) => string): string | null {
  const name = value.trim();
  if (name.length === 0) return t("volumes", "nameRequired");
  if (name.length > 63) return t("volumes", "nameTooLong");
  if (!/^[a-zA-Z0-9][a-zA-Z0-9_.-]*$/.test(name)) return t("volumes", "nameInvalid");
  return null;
}

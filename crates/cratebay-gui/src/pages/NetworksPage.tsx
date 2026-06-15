import { useCallback, useEffect, useMemo, useState } from "react";
import { Eye, Loader2, Network, Plus, RefreshCw, Trash2 } from "lucide-react";

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
import type { NetworkInfo } from "@/types/network";

export function NetworksPage() {
  const { t } = useI18n();
  const [networks, setNetworks] = useState<NetworkInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [busyNetwork, setBusyNetwork] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [newDriver, setNewDriver] = useState("bridge");
  const [newInternal, setNewInternal] = useState(false);
  const [newEnableIpv6, setNewEnableIpv6] = useState(false);
  const [inspectTarget, setInspectTarget] = useState<NetworkInfo | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<NetworkInfo | null>(null);
  const [forceDelete, setForceDelete] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [engineOffline, setEngineOffline] = useState(false);
  const [startingEngine, setStartingEngine] = useState(false);
  const nameValidationError = useMemo(() => validateNetworkName(newName, t), [newName, t]);

  const fetchNetworks = useCallback(async () => {
    setLoading(true);
    setError(null);
    setEngineOffline(false);
    try {
      const result = await invoke<NetworkInfo[]>("network_list");
      setNetworks(result);
    } catch (err) {
      setNetworks([]);
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
    void fetchNetworks();
  }, [fetchNetworks]);

  const handleCreate = useCallback(async () => {
    const name = newName.trim();
    if (name.length === 0 || nameValidationError !== null) return;
    const driver = newDriver.trim() || "bridge";
    setCreating(true);
    setError(null);
    try {
      await invoke<NetworkInfo>("network_create", {
        name,
        driver,
        internal: newInternal,
        enableIpv6: newEnableIpv6,
      });
      setNewName("");
      setNewDriver("bridge");
      setNewInternal(false);
      setNewEnableIpv6(false);
      await fetchNetworks();
    } catch (err) {
      setError(formatTauriError(err, t("common", "operationFailed")));
    } finally {
      setCreating(false);
    }
  }, [fetchNetworks, nameValidationError, newDriver, newEnableIpv6, newInternal, newName, t]);

  const handleInspect = useCallback(async (network: NetworkInfo) => {
    setBusyNetwork(network.id || network.name);
    setError(null);
    try {
      const result = await invoke<NetworkInfo>("network_inspect", { id: network.id || network.name });
      setInspectTarget(result);
    } catch (err) {
      setError(formatTauriError(err, t("common", "operationFailed")));
    } finally {
      setBusyNetwork(null);
    }
  }, [t]);

  const handleDelete = useCallback(async () => {
    if (deleteTarget === null) return;
    const id = deleteTarget.id || deleteTarget.name;
    setBusyNetwork(id);
    setError(null);
    setDeleteError(null);
    try {
      await invoke("network_delete", { id, force: forceDelete });
      setDeleteTarget(null);
      setForceDelete(false);
      await fetchNetworks();
    } catch (err) {
      const message = formatTauriError(err, t("common", "operationFailed"));
      setDeleteError(message);
      setError(message);
    } finally {
      setBusyNetwork(null);
    }
  }, [deleteTarget, fetchNetworks, forceDelete, t]);

  const handleStartEngine = useCallback(async () => {
    setStartingEngine(true);
    setError(null);
    try {
      await invoke("runtime_start");
      await fetchNetworks();
    } catch (err) {
      setError(formatTauriError(err, t("common", "operationFailed")));
    } finally {
      setStartingEngine(false);
    }
  }, [fetchNetworks, t]);

  return (
    <div className="flex h-full flex-col" data-testid="networks-page">
      <div className="flex flex-col gap-2 border-b border-border px-6 py-2.5 xl:flex-row xl:items-center">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <div className="min-w-0">
            <h1 className="text-sm font-semibold text-foreground">{t("networks", "title")}</h1>
            <p className="text-xs text-muted-foreground">{t("networks", "subtitle")}</p>
          </div>
          <Badge variant="outline" className="text-[10px]">
            {networks.length} {t("networks", "networkCount")}
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
            placeholder={t("networks", "namePlaceholder")}
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
            placeholder={t("networks", "driverPlaceholder")}
            className="h-8 w-full font-mono text-xs sm:w-28"
          />
          <label className="flex h-8 items-center gap-1.5 rounded-md border border-border px-2 text-xs text-muted-foreground">
            <Checkbox
              checked={newInternal}
              onCheckedChange={(checked) => setNewInternal(checked === true)}
            />
            {t("networks", "internal")}
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
            {t("networks", "create")}
          </Button>
          <Button variant="ghost" size="sm" onClick={() => void fetchNetworks()} disabled={loading}>
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
            {t("networks", "loadingNetworks")}
          </div>
        ) : networks.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 text-center text-muted-foreground">
            <Network className="mb-2 h-8 w-8 opacity-40" />
            <h3 className="text-sm font-medium">{t("networks", "noNetworks")}</h3>
            <p className="mt-1 text-xs">{t("networks", "noNetworksHint")}</p>
            <p className="mt-3 rounded-md bg-muted px-2 py-1 font-mono text-[11px]">{t("networks", "commandHint")}</p>
          </div>
        ) : (
          <div className="overflow-x-auto rounded-md border border-border bg-card">
            <div className="grid min-w-[660px] grid-cols-[minmax(180px,1.2fr)_120px_100px_100px_96px] border-b border-border px-3 py-2 text-[11px] font-medium uppercase text-muted-foreground">
              <span>{t("common", "name")}</span>
              <span>{t("networks", "driver")}</span>
              <span>{t("networks", "scope")}</span>
              <span>{t("networks", "containers")}</span>
              <span className="text-right">{t("common", "actions")}</span>
            </div>
            {networks.map((network) => (
              <NetworkRow
                key={network.id || network.name}
                network={network}
                busy={busyNetwork === (network.id || network.name)}
                onInspect={() => void handleInspect(network)}
                onDelete={() => setDeleteTarget(network)}
              />
            ))}
          </div>
        )}
      </div>

      <NetworkInspectDialog network={inspectTarget} onClose={() => setInspectTarget(null)} />

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
            <DialogTitle>{t("networks", "deleteNetwork")}</DialogTitle>
            <DialogDescription>{t("networks", "confirmDelete")}</DialogDescription>
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
                {t("networks", "forceDelete")}
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
            <Button variant="destructive" onClick={() => void handleDelete()} disabled={busyNetwork !== null}>
              {busyNetwork !== null ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Trash2 className="h-3.5 w-3.5" />}
              {t("common", "delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function NetworkRow({
  network,
  busy,
  onInspect,
  onDelete,
}: {
  network: NetworkInfo;
  busy: boolean;
  onInspect: () => void;
  onDelete: () => void;
}) {
  const { t } = useI18n();
  const shortId = network.id.length > 12 ? network.id.slice(0, 12) : network.id || "-";
  const containerCount = Object.keys(network.containers ?? {}).length;

  return (
    <div className="grid min-w-[660px] grid-cols-[minmax(180px,1.2fr)_120px_100px_100px_96px] items-center border-b border-border px-3 py-2.5 text-xs last:border-b-0">
      <div className="min-w-0">
        <div className="truncate font-mono text-sm font-semibold text-foreground">{network.name}</div>
        <div className="mt-0.5 flex items-center gap-2 text-[11px] text-muted-foreground">
          <span className="font-mono">{shortId}</span>
          {network.internal ? <span>{t("networks", "internal")}</span> : null}
          {network.attachable ? <span>{t("networks", "attachable")}</span> : null}
        </div>
      </div>
      <Badge variant="outline" className="w-fit text-[10px]">
        {network.driver || "bridge"}
      </Badge>
      <span className="text-muted-foreground">{network.scope || "local"}</span>
      <span className="font-mono text-muted-foreground">{containerCount}</span>
      <div className="flex justify-end gap-1">
        <Button variant="ghost" size="icon" className="h-7 w-7" onClick={onInspect} disabled={busy} title={t("networks", "inspectNetwork")}>
          {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Eye className="h-3.5 w-3.5" />}
        </Button>
        <Button variant="ghost" size="icon" className="h-7 w-7 text-destructive hover:text-destructive" onClick={onDelete} title={t("networks", "deleteNetwork")}>
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}

function NetworkInspectDialog({
  network,
  onClose,
}: {
  network: NetworkInfo | null;
  onClose: () => void;
}) {
  const { t } = useI18n();
  if (network === null) return null;

  return (
    <Dialog open onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent className="sm:max-w-[560px]">
        <DialogHeader>
          <DialogTitle>{t("networks", "inspectNetwork")}</DialogTitle>
          <DialogDescription>{network.name}</DialogDescription>
        </DialogHeader>
        <ScrollArea className="max-h-[440px] pr-3">
          <div className="grid gap-2 text-xs">
            <Detail label={t("common", "name")} value={network.name} mono />
            <Detail label="ID" value={network.id || "-"} mono />
            <Detail label={t("networks", "driver")} value={network.driver || "bridge"} />
            <Detail label={t("networks", "scope")} value={network.scope || "local"} />
            <Detail label={t("networks", "managedBy")} value={network.managedBy || "cratebay"} />
            <JsonBlock title={t("networks", "containers")} value={network.containers} />
            <JsonBlock title={t("networks", "labels")} value={network.labels} />
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

function validateNetworkName(value: string, t: (namespace: string, key: string) => string): string | null {
  const name = value.trim();
  if (name.length === 0) return t("networks", "nameRequired");
  if (name.length > 63) return t("networks", "nameTooLong");
  if (!/^[a-zA-Z0-9][a-zA-Z0-9_.-]*$/.test(name)) return t("networks", "nameInvalid");
  return null;
}

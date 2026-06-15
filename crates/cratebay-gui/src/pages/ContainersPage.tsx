import { useCallback, useEffect, useMemo, useState } from "react";
import { useContainerStore } from "@/stores/containerStore";
import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/lib/i18n";
import { invoke } from "@/lib/tauri";
import { formatTauriError, isImplicitRuntimeStartDisabled } from "@/lib/runtimeOffline";
import { EngineOfflineCallout } from "@/components/common/EngineOfflineCallout";
import { ContainerCard } from "@/components/container/ContainerCard";
import { ContainerList } from "@/components/container/ContainerList";
import { ContainerDetail } from "@/components/container/ContainerDetail";
import { ContainerCreate } from "@/components/container/ContainerCreate";
import { ContainerRun } from "@/components/container/ContainerRun";
import { PodsPage } from "@/pages/PodsPage";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { RefreshCw, Search, Box, LayoutGrid, List } from "lucide-react";
import type { ContainerFilter, ContainerInfo } from "@/types/container";

type FilterStatus = ContainerFilter["status"];
type ViewMode = "grid" | "table";
type ContainersSection = "containers" | "pods";

type FilterLabelKey = "all" | "running" | "stopped" | "creating";
const FILTER_LABEL_KEYS: Record<FilterStatus, FilterLabelKey> = {
  all: "all",
  running: "running",
  stopped: "stopped",
  creating: "creating",
};

function applyStatusFilter(list: ContainerInfo[], filter: ContainerFilter): ContainerInfo[] {
  const statusFilter = filter.status as FilterStatus;
  return list.filter((c) => {
    if (statusFilter !== "all") {
      if (statusFilter === "stopped") {
        if (c.status !== "stopped" && c.status !== "exited" && c.status !== "dead") return false;
      } else if (statusFilter === "creating") {
        if (c.status !== "creating" && c.status !== "created") return false;
      } else if (c.status !== statusFilter) {
        return false;
      }
    }
    if (filter.templateId !== null && c.labels?.["com.cratebay.template_id"] !== filter.templateId) return false;
    if (
      filter.search.length > 0 &&
      !c.name.toLowerCase().includes(filter.search.toLowerCase()) &&
      !c.image.toLowerCase().includes(filter.search.toLowerCase())
    ) {
      return false;
    }
    return true;
  });
}

export function ContainersPage() {
  const { t } = useI18n();
  const fetchContainers = useContainerStore((s) => s.fetchContainers);
  const fetchTemplates = useContainerStore((s) => s.fetchTemplates);
  const containers = useContainerStore((s) => s.containers);
  const filter = useContainerStore((s) => s.filter);
  const setFilter = useContainerStore((s) => s.setFilter);
  const loading = useContainerStore((s) => s.loading);
  const error = useContainerStore((s) => s.error);
  const [viewMode, setViewMode] = useState<ViewMode>("grid");
  const [section, setSection] = useState<ContainersSection>("containers");
  const [startingEngine, setStartingEngine] = useState(false);
  const [startError, setStartError] = useState<string | null>(null);

  // Read builtin runtime ready state from appStore
  const builtinRuntimeReady = useAppStore((s) => s.builtinRuntimeReady);
  const engineConnected = useAppStore((s) => s.engineConnected);

  useEffect(() => {
    void fetchContainers();
    void fetchTemplates();
  }, [fetchContainers, fetchTemplates, t]);

  // Apply filters on all containers
  const filteredContainers = useMemo(
    () => applyStatusFilter(containers, filter),
    [containers, filter],
  );

  // Counts for filter chips
  const counts = useMemo(
    () => ({
      all: containers.length,
      running: containers.filter((c) => c.status === "running").length,
      stopped: containers.filter(
        (c) => c.status === "stopped" || c.status === "exited" || c.status === "dead",
      ).length,
      creating: containers.filter(
        (c) => c.status === "creating" || c.status === "created",
      ).length,
    }),
    [containers],
  );

  const currentChipFilter: FilterStatus =
    (filter.status as FilterStatus) in FILTER_LABEL_KEYS
      ? (filter.status as FilterStatus)
      : "all";

  const handleChipClick = (f: FilterStatus) => {
    setFilter({ status: f });
  };

  const engineOffline = error !== null && isImplicitRuntimeStartDisabled(error);
  const visibleError = engineOffline ? startError : (error ?? startError);

  const handleStartEngine = useCallback(async () => {
    setStartingEngine(true);
    setStartError(null);
    try {
      await invoke("runtime_start");
      await fetchContainers();
      await fetchTemplates();
    } catch (err) {
      setStartError(formatTauriError(err, t("common", "operationFailed")));
    } finally {
      setStartingEngine(false);
    }
  }, [fetchContainers, fetchTemplates]);

  return (
    <div className="relative flex h-full flex-col overflow-hidden">
      <div className="flex items-center gap-3 border-b border-border px-6 py-2.5">
        <div
          className="flex items-center gap-0.5 rounded-md border border-border p-0.5"
          data-testid="containers-section-tabs"
        >
          {(["containers", "pods"] as ContainersSection[]).map((item) => (
            <button
              key={item}
              type="button"
              data-testid={`containers-tab-${item}`}
              onClick={() => setSection(item)}
              aria-pressed={section === item}
              className={cn(
                "rounded px-3 py-1.5 text-xs font-medium transition-colors focus:outline-none",
                section === item
                  ? "bg-muted text-foreground"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {t("nav", item)}
            </button>
          ))}
        </div>
        <p className="text-xs text-muted-foreground">
          {section === "containers" ? t("containers", "title") : t("pods", "subtitle")}
        </p>
      </div>

      {section === "pods" ? (
        <div className="min-h-0 flex-1 overflow-hidden">
          <PodsPage />
        </div>
      ) : (
        <>
          {/* Error alerts */}
          {visibleError !== null && (
            <div className="mx-6 mt-2 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
              {visibleError}
            </div>
          )}
          {engineOffline && (
            <div className="mx-6 mt-2">
              <EngineOfflineCallout starting={startingEngine} onStart={() => void handleStartEngine()} />
            </div>
          )}
          {!builtinRuntimeReady && engineConnected && (
            <div className="mx-6 mt-2 rounded-md border border-blue-500/30 bg-blue-500/10 px-3 py-2 text-xs text-blue-600 dark:text-blue-400">
              {t("containers", "builtinRuntimeNotReady")}
            </div>
          )}

          {/* Unified toolbar */}
          <div className="flex items-center gap-3 border-b border-border px-6 py-2.5">
            <div className="relative w-56">
              <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                data-testid="search-input"
                value={filter.search}
                onChange={(e) => setFilter({ search: e.target.value })}
                placeholder={t("containers", "searchPlaceholder")}
                className="h-8 pl-8 text-xs"
              />
            </div>
            <div className="h-4 w-px bg-border" />
            <div className="flex items-center gap-1" data-testid="status-filter">
              {(["all", "running", "stopped", "creating"] as FilterStatus[]).map((f) => (
                <button
                  key={f}
                  onClick={() => handleChipClick(f)}
                  className={cn(
                    "inline-flex h-8 min-w-[76px] items-center justify-center gap-1 whitespace-nowrap rounded-md px-2 text-xs font-medium transition-colors focus:outline-none",
                    currentChipFilter === f
                      ? "bg-accent text-foreground"
                      : "text-muted-foreground hover:bg-accent/70 hover:text-foreground",
                  )}
                >
                  <span>{t("containers", FILTER_LABEL_KEYS[f])}</span>
                  <span className="font-mono text-[11px] text-muted-foreground">{counts[f]}</span>
                </button>
              ))}
            </div>
            <div className="ml-auto flex items-center gap-2">
              <div className="flex items-center gap-0.5 rounded-md border border-border p-0.5">
                <button
                  onClick={() => setViewMode("grid")}
                  className={cn(
                    "rounded p-1.5 transition-colors focus:outline-none",
                    viewMode === "grid"
                      ? "bg-muted text-foreground"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                  aria-label={t("containers", "gridView")}
                >
                  <LayoutGrid className="h-3.5 w-3.5" />
                </button>
                <button
                  onClick={() => setViewMode("table")}
                  className={cn(
                    "rounded p-1.5 transition-colors focus:outline-none",
                    viewMode === "table"
                      ? "bg-muted text-foreground"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                  aria-label={t("containers", "tableView")}
                >
                  <List className="h-3.5 w-3.5" />
                </button>
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => void fetchContainers()}
                disabled={loading}
              >
                <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
                {t("common", "refresh")}
              </Button>
              <ContainerRun />
              <div data-testid="create-container">
                <ContainerCreate />
              </div>
            </div>
          </div>

          {/* Container content area */}
          <div className="flex-1 overflow-auto px-6 py-4" data-testid="container-list">
            {viewMode === "table" ? (
              <ContainerList />
            ) : loading && filteredContainers.length === 0 ? (
              <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
                {t("containers", "loadingContainers")}
              </div>
            ) : filteredContainers.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-16 text-center text-muted-foreground">
                <Box className="mb-3 h-12 w-12 opacity-20" />
                <h3 className="text-sm font-medium">{t("containers", "noContainers")}</h3>
                <p className="mt-1 text-xs">
                  {t("containers", "noContainersHint")}
                </p>
              </div>
            ) : (
              <div className="grid grid-cols-[repeat(auto-fill,minmax(340px,1fr))] gap-4">
                {filteredContainers.map((c) => (
                  <ContainerCard key={c.id} container={c} />
                ))}
              </div>
            )}
          </div>
          {/* Detail panel — absolute overlay on the right */}
          <ContainerDetail />
        </>
      )}
    </div>
  );
}

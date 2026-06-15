/**
 * ImagesPage — OCI image management page.
 *
 * Features:
 * - Local images tab: list, inspect, remove local images
 * - Search tab: search registries, pull images
 * - Inline image details via Dialog
 *
 * Uses Tauri commands: image_list, image_search, image_pull,
 * image_remove, image_inspect, image_tag.
 *
 * @see api-spec.md for Tauri command signatures
 */

import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { invoke } from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { usePullStore } from "@/stores/pullStore";
import { PullTaskList } from "@/components/images/PullTaskList";
import { EngineOfflineCallout } from "@/components/common/EngineOfflineCallout";
import { formatTauriError, isImplicitRuntimeStartDisabled } from "@/lib/runtimeOffline";
import type {
  LocalImageInfo,
  ImageSearchResult,
  ImageInspectInfo,
  BundleImageLoadResult,
} from "@/types/image";
import type { ContainerInfo } from "@/types/container";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Layers,
  Search,
  Trash2,
  Download,
  Eye,
  RefreshCw,
  Loader2,
  HardDrive,
  Globe,
  Star,
  ArrowDownToLine,
  PackageOpen,
  PackagePlus,
  Upload,
  Tag,
  CheckCircle2,
  AlertCircle,
} from "lucide-react";

type ImageOperationFeedback =
  | {
      kind: "export";
      status: "success";
      message: string;
      imageCount: number;
      bytes: number;
      path: string;
      at: Date;
    }
  | {
      kind: "import";
      status: "success";
      message: string;
      imageCount: number;
      path: string;
      at: Date;
    }
  | {
      kind: "preload";
      status: "success" | "warning";
      message: string;
      loaded: number;
      skipped: number;
      failed: number;
      failedDetails: string;
      at: Date;
    }
  | {
      kind: "tag";
      status: "success";
      message: string;
      source: string;
      target: string;
      at: Date;
    }
  | {
      kind: "pack";
      status: "success";
      message: string;
      container: string;
      image: string;
      at: Date;
    }
  | {
      kind: "error";
      status: "error";
      message: string;
      at: Date;
    };

export function ImagesPage() {
  const { t } = useI18n();
  const [activeTab, setActiveTab] = useState<"local" | "search">("local");
  const refreshLocalRef = useRef<(() => void) | null>(null);
  const [toolbarRight, setToolbarRight] = useState<React.ReactNode>(null);

  // Refresh local images when a pull task completes.
  const tasks = usePullStore((s) => s.tasks);
  const prevCompletedRef = useRef(0);
  useEffect(() => {
    const completedCount = tasks.filter((t) => t.complete && t.error === null).length;
    if (completedCount > prevCompletedRef.current) {
      refreshLocalRef.current?.();
    }
    prevCompletedRef.current = completedCount;
  }, [tasks]);

  return (
    <div className="flex h-full flex-col" data-testid="images-page">
      {/* Unified toolbar */}
      <div className="flex items-center gap-3 overflow-x-auto border-b border-border px-6 py-2.5">
        <div className="flex items-center gap-0.5 rounded-md border border-border p-0.5">
          <button
            onClick={() => setActiveTab("local")}
            data-testid="images-tab-local"
            className={cn(
              "inline-flex h-8 items-center gap-1.5 whitespace-nowrap rounded px-2.5 text-xs font-medium transition-colors focus:outline-none",
              activeTab === "local"
                ? "bg-accent text-foreground"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <HardDrive className="h-3.5 w-3.5" />
            {t("images", "localImages")}
          </button>
          <button
            onClick={() => setActiveTab("search")}
            data-testid="images-tab-search"
            className={cn(
              "inline-flex h-8 items-center gap-1.5 whitespace-nowrap rounded px-2.5 text-xs font-medium transition-colors focus:outline-none",
              activeTab === "search"
                ? "bg-accent text-foreground"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <Globe className="h-3.5 w-3.5" />
            {t("images", "searchImages")}
          </button>
        </div>
        <div className="h-4 w-px bg-border" />
        {/* Dynamic right-side controls injected by active tab */}
        <div className="flex min-w-max flex-1 items-center gap-2">
          {toolbarRight}
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto">
        {activeTab === "local" ? (
          <LocalImagesTab onRefreshRef={refreshLocalRef} onToolbar={setToolbarRight} />
        ) : (
          <SearchImagesTab onToolbar={setToolbarRight} />
        )}
      </div>
    </div>
  );
}

/* ========== Local Images Tab ========== */

function imageArchiveFilters(t: ReturnType<typeof useI18n>["t"]) {
  return [
    { name: t("images", "imageArchiveFilter"), extensions: ["tar", "tar.gz", "tgz"] },
    { name: t("images", "allFilesFilter"), extensions: ["*"] },
  ];
}

function LocalImagesTab({ onRefreshRef, onToolbar }: { onRefreshRef: React.MutableRefObject<(() => void) | null>; onToolbar: (node: React.ReactNode) => void }) {
  const { t } = useI18n();
  const [images, setImages] = useState<LocalImageInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState("");
  const [inspectInfo, setInspectInfo] = useState<ImageInspectInfo | null>(null);
  const [inspectLoading, setInspectLoading] = useState(false);
  const [removeConfirm, setRemoveConfirm] = useState<string | null>(null);
  const [forceRemove, setForceRemove] = useState(false);
  const [removing, setRemoving] = useState(false);

  // Batch selection state
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [batchRemoveConfirm, setBatchRemoveConfirm] = useState(false);
  const [batchForceRemove, setBatchForceRemove] = useState(false);
  const [batchRemoving, setBatchRemoving] = useState(false);
  const [batchProgress, setBatchProgress] = useState({ done: 0, total: 0, failed: 0 });
  const [exportDialogOpen, setExportDialogOpen] = useState(false);
  const [exportPath, setExportPath] = useState("cratebay-images.tar");
  const [exporting, setExporting] = useState(false);
  const [importDialogOpen, setImportDialogOpen] = useState(false);
  const [importPath, setImportPath] = useState("");
  const [importing, setImporting] = useState(false);
  const [preloadingBundled, setPreloadingBundled] = useState(false);
  const [operationFeedback, setOperationFeedback] = useState<ImageOperationFeedback | null>(null);
  const [tagTarget, setTagTarget] = useState<LocalImageInfo | null>(null);
  const [tagValue, setTagValue] = useState("");
  const [tagging, setTagging] = useState(false);
  const [packDialogOpen, setPackDialogOpen] = useState(false);
  const [containers, setContainers] = useState<ContainerInfo[]>([]);
  const [containersLoading, setContainersLoading] = useState(false);
  const [selectedContainerId, setSelectedContainerId] = useState("");
  const [packImage, setPackImage] = useState("cratebay/packed:latest");
  const [packing, setPacking] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [engineOffline, setEngineOffline] = useState(false);
  const [startingEngine, setStartingEngine] = useState(false);

  const handleChooseExportPath = useCallback(async () => {
    setOperationFeedback(null);
    try {
      const selected = await save({
        title: t("images", "chooseExportPath"),
        defaultPath: exportPath,
        filters: imageArchiveFilters(t),
        canCreateDirectories: true,
      });
      if (selected !== null) {
        setExportPath(selected);
      }
    } catch (err) {
      setOperationFeedback({
        kind: "error",
        status: "error",
        message: formatDialogError(err, t("images", "filePickerUnavailable")),
        at: new Date(),
      });
    }
  }, [exportPath, t]);

  const handleChooseImportPath = useCallback(async () => {
    setOperationFeedback(null);
    try {
      const selected = await open({
        title: t("images", "chooseImportPath"),
        multiple: false,
        filters: imageArchiveFilters(t),
      });
      if (typeof selected === "string") {
        setImportPath(selected);
      }
    } catch (err) {
      setOperationFeedback({
        kind: "error",
        status: "error",
        message: formatDialogError(err, t("images", "filePickerUnavailable")),
        at: new Date(),
      });
    }
  }, [t]);

  const fetchImages = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    setEngineOffline(false);
    try {
      const result = await Promise.race([
        invoke<LocalImageInfo[]>("image_list"),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error(t("images", "imageListTimeout"))), 8000),
        ),
      ]);
      setImages(result);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.warn("[ImagesPage] fetchImages failed:", message);
      setImages([]);
      if (isImplicitRuntimeStartDisabled(err)) {
        setEngineOffline(true);
      } else {
        setLoadError(formatTauriError(err, t("common", "operationFailed")));
      }
    } finally {
      setLoading(false);
    }
  }, [t]);

  const fetchContainers = useCallback(async () => {
    setContainersLoading(true);
    try {
      const result = await invoke<ContainerInfo[] | null>("container_list");
      const list = Array.isArray(result) ? result : [];
      setContainers(list);
      setSelectedContainerId((current) => {
        if (current.length > 0 && list.some((container) => container.id === current)) {
          return current;
        }
        return list[0]?.id ?? "";
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.warn("[ImagesPage] fetchContainers failed:", message);
      setContainers([]);
      setSelectedContainerId("");
    } finally {
      setContainersLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchImages();
    void fetchContainers();
  }, [fetchImages, fetchContainers]);

  const handleStartEngine = useCallback(async () => {
    setStartingEngine(true);
    setLoadError(null);
    setOperationFeedback(null);
    try {
      await invoke("runtime_start");
      await fetchImages();
      await fetchContainers();
    } catch (err) {
      setLoadError(formatTauriError(err, t("common", "operationFailed")));
    } finally {
      setStartingEngine(false);
    }
  }, [fetchContainers, fetchImages, t]);

  // Expose the active tab refresh action to the parent toolbar.
  useEffect(() => {
    onRefreshRef.current = () => void fetchImages();
    return () => {
      onRefreshRef.current = null;
    };
  }, [fetchImages, onRefreshRef]);

  const filteredImages = useMemo(() => {
    if (filter.length === 0) return images;
    const q = filter.toLowerCase();
    return images.filter(
      (img) =>
        img.repoTags.some((tag) => tag.toLowerCase().includes(q)) ||
        img.id.toLowerCase().includes(q),
    );
  }, [images, filter]);

  const handleInspect = useCallback(async (id: string) => {
    setInspectLoading(true);
    try {
      const info = await invoke<ImageInspectInfo>("image_inspect", { id });
      setInspectInfo(info);
    } catch {
      // Engine not available — cannot inspect
      setInspectInfo(null);
    } finally {
      setInspectLoading(false);
    }
  }, []);

  const handleRemove = useCallback(async (id: string) => {
    setRemoving(true);
    setOperationFeedback(null);
    try {
      await invoke("image_remove", { id, force: forceRemove });
    } catch (err) {
      setOperationFeedback({
        kind: "error",
        status: "error",
        message: formatOperationError(err, t("common", "operationFailed")),
        at: new Date(),
      });
      setRemoving(false);
      return;
    }
    // Refresh the full list from CrateBay Engine to get accurate state
    await fetchImages();
    setRemoving(false);
    setRemoveConfirm(null);
    setForceRemove(false);
  }, [fetchImages, forceRemove]);

  // Clear selection when images list changes (after refresh)
  useEffect(() => {
    setSelectedIds((prev) => {
      const imageIdSet = new Set(images.map((i) => i.id));
      const next = new Set([...prev].filter((id) => imageIdSet.has(id)));
      return next.size === prev.size ? prev : next;
    });
  }, [images]);

  const toggleSelect = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const toggleSelectAll = useCallback(() => {
    const userImages = filteredImages.filter((i) => !isSystemImage(i));
    const userIds = userImages.map((i) => i.id);
    const allSelected = userIds.length > 0 && userIds.every((id) => selectedIds.has(id));
    if (allSelected) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(userIds));
    }
  }, [filteredImages, selectedIds]);

  const handleBatchRemove = useCallback(async () => {
    const ids = [...selectedIds];
    if (ids.length === 0) return;
    setBatchRemoving(true);
    setOperationFeedback(null);
    setBatchProgress({ done: 0, total: ids.length, failed: 0 });
    let failed = 0;
    let lastError = "";
    for (let i = 0; i < ids.length; i++) {
      try {
        await invoke("image_remove", { id: ids[i], force: batchForceRemove });
      } catch (err) {
        failed++;
        lastError = formatOperationError(err, t("common", "operationFailed"));
      }
      setBatchProgress({ done: i + 1, total: ids.length, failed });
    }
    await fetchImages();
    setSelectedIds(new Set());
    setBatchRemoving(false);
    setBatchRemoveConfirm(false);
    setBatchForceRemove(false);
    setBatchProgress({ done: 0, total: 0, failed: 0 });
    if (failed > 0) {
      setOperationFeedback({
        kind: "error",
        status: "error",
        message:
          lastError.length > 0
            ? `${t("images", "batchFailed").replace("{count}", String(failed))}: ${lastError}`
            : t("images", "batchFailed").replace("{count}", String(failed)),
        at: new Date(),
      });
    }
  }, [selectedIds, fetchImages, batchForceRemove, t]);

  const selectedImageRefs = useMemo(() => {
    return images
      .filter((image) => selectedIds.has(image.id))
      .map((image) => image.repoTags[0] ?? image.id)
      .filter((reference) => reference.length > 0);
  }, [images, selectedIds]);

  const handleExport = useCallback(async () => {
    const output = exportPath.trim();
    if (output.length === 0 || selectedImageRefs.length === 0) return;
    setExporting(true);
    setOperationFeedback(null);
    try {
      const bytes = await invoke<number>("image_export", {
        images: selectedImageRefs,
        output,
      });
      const message = t("images", "exportSuccess")
        .replace("{count}", String(selectedImageRefs.length))
        .replace("{bytes}", String(bytes));
      setOperationFeedback({
        kind: "export",
        status: "success",
        message,
        imageCount: selectedImageRefs.length,
        bytes,
        path: output,
        at: new Date(),
      });
      setExportDialogOpen(false);
    } catch (err) {
      setOperationFeedback({
        kind: "error",
        status: "error",
        message: err instanceof Error ? err.message : String(err),
        at: new Date(),
      });
    } finally {
      setExporting(false);
    }
  }, [exportPath, selectedImageRefs, t]);

  const handleImport = useCallback(async () => {
    const input = importPath.trim();
    if (input.length === 0) return;
    setImporting(true);
    setOperationFeedback(null);
    try {
      const loaded = await invoke<string[]>("image_import", { input });
      const message = t("images", "importSuccess").replace("{count}", String(loaded.length));
      setOperationFeedback({
        kind: "import",
        status: "success",
        message,
        imageCount: loaded.length,
        path: input,
        at: new Date(),
      });
      setImportDialogOpen(false);
      await fetchImages();
    } catch (err) {
      setOperationFeedback({
        kind: "error",
        status: "error",
        message: err instanceof Error ? err.message : String(err),
        at: new Date(),
      });
    } finally {
      setImporting(false);
    }
  }, [fetchImages, importPath, t]);

  const handlePreloadBundled = useCallback(async () => {
    setPreloadingBundled(true);
    setOperationFeedback(null);
    try {
      const results = await invoke<BundleImageLoadResult[]>("image_preload_bundled");
      const loaded = results.filter((result) => result.loaded).length;
      const skipped = results.filter((result) => result.skipped).length;
      const failed = results.filter((result) => !result.loaded && !result.skipped).length;
      const summary = t("images", "preloadSummary")
        .replace("{loaded}", String(loaded))
        .replace("{skipped}", String(skipped))
        .replace("{failed}", String(failed));
      const failedDetails = results
        .filter((result) => !result.loaded && !result.skipped)
        .map((result) => `${result.imageName}: ${result.message}`)
        .join("; ");
      setOperationFeedback({
        kind: "preload",
        status: failed > 0 ? "warning" : "success",
        message: failedDetails.length > 0 ? `${summary} ${failedDetails}` : summary,
        loaded,
        skipped,
        failed,
        failedDetails,
        at: new Date(),
      });
      await fetchImages();
    } catch (err) {
      setOperationFeedback({
        kind: "error",
        status: "error",
        message: err instanceof Error ? err.message : String(err),
        at: new Date(),
      });
    } finally {
      setPreloadingBundled(false);
    }
  }, [fetchImages, t]);

  const openTagDialog = useCallback((image: LocalImageInfo) => {
    setTagTarget(image);
    setTagValue(suggestImageTag(image));
    setOperationFeedback(null);
  }, []);

  const handleTag = useCallback(async () => {
    if (tagTarget === null) return;
    const source = primaryImageReference(tagTarget);
    const target = tagValue.trim();
    if (source.length === 0 || target.length === 0) return;

    setTagging(true);
    setOperationFeedback(null);
    try {
      await invoke("image_tag", { source, target });
      const message = t("images", "tagSuccess")
        .replace("{source}", source)
        .replace("{target}", target);
      setOperationFeedback({
        kind: "tag",
        status: "success",
        message,
        source,
        target,
        at: new Date(),
      });
      setTagTarget(null);
      setTagValue("");
      await fetchImages();
    } catch (err) {
      setOperationFeedback({
        kind: "error",
        status: "error",
        message: err instanceof Error ? err.message : String(err),
        at: new Date(),
      });
    } finally {
      setTagging(false);
    }
  }, [fetchImages, tagTarget, tagValue, t]);

  const openPackDialog = useCallback(() => {
    const selectedContainer = containers.find((container) => container.id === selectedContainerId) ?? containers[0];
    if (selectedContainer !== undefined) {
      setSelectedContainerId(selectedContainer.id);
      setPackImage(suggestPackedImageTag(selectedContainer));
    } else {
      setPackImage("cratebay/packed:latest");
    }
    setOperationFeedback(null);
    setPackDialogOpen(true);
    void fetchContainers();
  }, [containers, fetchContainers, selectedContainerId]);

  const handlePackContainer = useCallback(async () => {
    const container = selectedContainerId.trim();
    const image = packImage.trim();
    if (container.length === 0 || image.length === 0) return;

    setPacking(true);
    setOperationFeedback(null);
    try {
      const imageRef = await invoke<string>("image_pack_container", {
        container,
        image,
      });
      const selectedContainer = containers.find((item) => item.id === container);
      const containerLabel = selectedContainer?.name ?? container;
      const packedImage = imageRef.length > 0 ? imageRef : image;
      const message = t("images", "packSuccess")
        .replace("{container}", containerLabel)
        .replace("{image}", packedImage);
      setOperationFeedback({
        kind: "pack",
        status: "success",
        message,
        container: containerLabel,
        image: packedImage,
        at: new Date(),
      });
      setPackDialogOpen(false);
      await fetchImages();
    } catch (err) {
      setOperationFeedback({
        kind: "error",
        status: "error",
        message: err instanceof Error ? err.message : String(err),
        at: new Date(),
      });
    } finally {
      setPacking(false);
    }
  }, [containers, fetchImages, packImage, selectedContainerId, t]);

  const allVisibleSelected = (() => {
    const userImages = filteredImages.filter((i) => !isSystemImage(i));
    return userImages.length > 0 && userImages.every((i) => selectedIds.has(i.id));
  })();
  const selectedPackContainer = containers.find((container) => container.id === selectedContainerId);

  // Inject toolbar controls into parent
  useEffect(() => {
    onToolbar(
      <>
        <div className="relative w-56">
          <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder={t("images", "filterPlaceholder")}
            data-testid="image-filter-input"
            className="h-8 pl-8 text-xs"
          />
        </div>
        <div className="ml-auto flex items-center gap-2">
          <PullTaskList />
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void fetchImages()}
            disabled={loading}
            data-testid="image-refresh"
          >
            <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
            {t("common", "refresh")}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => void handlePreloadBundled()}
            disabled={preloadingBundled || engineOffline}
            data-testid="image-preload-bundled"
          >
            {preloadingBundled ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Download className="h-3.5 w-3.5" />}
            {t("images", "preloadBundled")}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setImportDialogOpen(true)}
            disabled={engineOffline}
            data-testid="image-import-open"
          >
            <Upload className="h-3.5 w-3.5" />
            {t("images", "importImages")}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={openPackDialog}
            disabled={containersLoading || engineOffline}
            data-testid="image-pack-open"
          >
            {containersLoading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <PackagePlus className="h-3.5 w-3.5" />}
            {t("images", "packFromContainer")}
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={selectedImageRefs.length === 0 || engineOffline}
            onClick={() => setExportDialogOpen(true)}
            data-testid="image-export-open"
          >
            <PackageOpen className="h-3.5 w-3.5" />
            {t("images", "exportImages")}
          </Button>
          <Button
            variant="destructive"
            size="sm"
            disabled={selectedIds.size === 0 || engineOffline}
            onClick={() => setBatchRemoveConfirm(true)}
            data-testid="image-batch-remove-open"
          >
            <Trash2 className="h-3.5 w-3.5" />
            {t("images", "batchRemove")}
            {selectedIds.size > 0 && (
              <Badge variant="secondary" className="ml-1 h-4 min-w-4 px-1 text-[10px]">
                {selectedIds.size}
              </Badge>
            )}
          </Button>
        </div>
      </>
    );
    return () => onToolbar(null);
  }, [filter, loading, selectedIds, selectedImageRefs.length, t, fetchImages, handlePreloadBundled, preloadingBundled, containersLoading, openPackDialog, onToolbar, engineOffline]);

  return (
    <div className="px-6 py-4">
      {/* Image count with select all */}
      {operationFeedback !== null && (
        <ImageOperationFeedbackBanner feedback={operationFeedback} />
      )}
      {loadError !== null && (
        <div className="mb-3 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {loadError}
        </div>
      )}
      {engineOffline && !loading && (
        <div className="mb-3">
          <EngineOfflineCallout starting={startingEngine} onStart={() => void handleStartEngine()} />
        </div>
      )}

      <div className="mb-3 flex items-center gap-2">
        <Checkbox
          checked={allVisibleSelected}
          onCheckedChange={() => toggleSelectAll()}
          disabled={filteredImages.length === 0}
        />
        <p className="text-xs text-muted-foreground">
          {filteredImages.length} {t("images", "imageCount")}
          {selectedIds.size > 0 && (
            <span className="ml-1.5 text-foreground font-medium">
              ({t("images", "selectedCount").replace("{count}", String(selectedIds.size))})
            </span>
          )}
        </p>
      </div>

      {/* Image list */}
      {loading ? (
        <div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
          <Loader2 className="mr-2 h-4 w-4 animate-spin" />
          {t("images", "loadingImages")}
        </div>
      ) : filteredImages.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-16 text-center text-muted-foreground">
          <Layers className="mb-3 h-12 w-12 opacity-20" />
          <h3 className="text-sm font-medium">{t("images", "noImages")}</h3>
          <p className="mt-1 text-xs">{t("images", "noImagesHint")}</p>
        </div>
      ) : (
        <div className="space-y-4">
          {/* System images */}
          {filteredImages.some((i) => isSystemImage(i)) && (
            <div>
              <h3 className="mb-2 text-xs font-medium text-muted-foreground uppercase tracking-wider">
                {t("images", "systemImages")}
              </h3>
              <div className="space-y-2">
                {filteredImages.filter((i) => isSystemImage(i)).map((img) => (
                  <LocalImageRow
                    key={img.id}
                    image={img}
                    selected={false}
                    onToggleSelect={() => {}}
                    onInspect={() => void handleInspect(img.id)}
                    onTag={() => openTagDialog(img)}
                    onRemove={() => {}}
                  />
                ))}
              </div>
            </div>
          )}

          {/* User images */}
          {filteredImages.some((i) => !isSystemImage(i)) && (
            <div>
              <h3 className="mb-2 text-xs font-medium text-muted-foreground uppercase tracking-wider">
                {t("images", "userImages")}
              </h3>
              <div className="space-y-2">
                {filteredImages.filter((i) => !isSystemImage(i)).map((img) => (
                  <LocalImageRow
                    key={img.id}
                    image={img}
                    selected={selectedIds.has(img.id)}
                    onToggleSelect={() => toggleSelect(img.id)}
                    onInspect={() => void handleInspect(img.id)}
                    onTag={() => openTagDialog(img)}
                    onRemove={() => {
                      setForceRemove(false);
                      setRemoveConfirm(img.id);
                    }}
                  />
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* Inspect Dialog */}
      <Dialog
        open={inspectInfo !== null}
        onOpenChange={(open) => {
          if (!open) setInspectInfo(null);
        }}
      >
        <DialogContent className="sm:max-w-[640px]">
          <DialogHeader>
            <DialogTitle>{t("images", "inspectImage")}</DialogTitle>
            <DialogDescription className="font-mono text-xs break-all">
              {inspectInfo?.id ?? ""}
            </DialogDescription>
          </DialogHeader>

          {inspectLoading ? (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
            </div>
          ) : inspectInfo !== null ? (
            <ScrollArea className="max-h-[50vh] pr-2">
              <div className="grid grid-cols-[140px_1fr] gap-x-4 gap-y-2 text-sm">
                <span className="text-muted-foreground">{t("images", "imageId")}</span>
                <span className="font-mono text-xs break-all">{inspectInfo.id}</span>

                <span className="text-muted-foreground">{t("images", "repoTags")}</span>
                <span>{inspectInfo.repoTags.length > 0 ? inspectInfo.repoTags.join(", ") : "-"}</span>

                <span className="text-muted-foreground">{t("images", "imageSize")}</span>
                <span>{(inspectInfo.sizeBytes / (1024 * 1024)).toFixed(1)} MB</span>

                <span className="text-muted-foreground">{t("images", "imageCreated")}</span>
                <span>{inspectInfo.created}</span>

                <span className="text-muted-foreground">{t("images", "architecture")}</span>
                <span>{inspectInfo.architecture}</span>

                <span className="text-muted-foreground">{t("images", "operatingSystem")}</span>
                <span>{inspectInfo.os}</span>

                <span className="text-muted-foreground">{t("images", "imageBuilderVersion")}</span>
                <span>{inspectInfo.dockerVersion}</span>

                <span className="text-muted-foreground">{t("images", "layers")}</span>
                <span>{inspectInfo.layers}</span>
              </div>
            </ScrollArea>
          ) : null}

          <DialogFooter>
            <Button variant="outline" onClick={() => setInspectInfo(null)}>
              {t("common", "close")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Tag Dialog */}
      <Dialog
        open={tagTarget !== null}
        onOpenChange={(open) => {
          if (!open && !tagging) {
            setTagTarget(null);
            setTagValue("");
          }
        }}
      >
        <DialogContent className="sm:max-w-[460px]">
          <DialogHeader>
            <DialogTitle>{t("images", "tagImage")}</DialogTitle>
            <DialogDescription>{t("images", "tagImageDesc")}</DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-1.5">
              <div className="text-xs font-medium text-muted-foreground">{t("images", "sourceImage")}</div>
              <div className="rounded-md border bg-muted px-2 py-1 font-mono text-xs break-all">
                {tagTarget !== null ? primaryImageReference(tagTarget) : ""}
              </div>
            </div>
            <div className="space-y-1.5">
              <div className="text-xs font-medium text-muted-foreground">{t("images", "targetTag")}</div>
              <Input
                value={tagValue}
                onChange={(event) => setTagValue(event.target.value)}
                placeholder="repo/name:tag"
                className="font-mono text-xs"
                autoComplete="off"
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              disabled={tagging}
              onClick={() => {
                setTagTarget(null);
                setTagValue("");
              }}
            >
              {t("common", "cancel")}
            </Button>
            <Button onClick={() => void handleTag()} disabled={tagging || tagValue.trim().length === 0}>
              {tagging ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Tag className="h-3.5 w-3.5" />}
              {t("images", "tagImage")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Remove Confirm Dialog */}
      <Dialog
        open={removeConfirm !== null}
        onOpenChange={(open) => {
          if (!open) {
            setRemoveConfirm(null);
            setForceRemove(false);
          }
        }}
      >
        <DialogContent className="sm:max-w-[400px]">
          <DialogHeader>
            <DialogTitle>{t("images", "removeImage")}</DialogTitle>
            <DialogDescription>{t("images", "confirmRemove")}</DialogDescription>
          </DialogHeader>
          {removeConfirm !== null && (
            <div className="space-y-3">
              <div className="rounded-md border bg-muted px-2 py-1 text-xs font-mono text-foreground break-all">
                {images.find((i) => i.id === removeConfirm)?.repoTags.join(", ") ?? removeConfirm}
              </div>
              <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <Checkbox
                  checked={forceRemove}
                  onCheckedChange={(checked) => setForceRemove(checked === true)}
                />
                {t("images", "forceRemove")}
              </label>
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setRemoveConfirm(null)}>
              {t("common", "cancel")}
            </Button>
            <Button
              variant="destructive"
              disabled={removing}
              onClick={() => {
                if (removeConfirm !== null) void handleRemove(removeConfirm);
              }}
            >
              {removing ? `${t("common", "loading")}` : t("common", "delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Batch Remove Confirm Dialog */}
      <Dialog
        open={batchRemoveConfirm}
        onOpenChange={(open) => {
          if (!open && !batchRemoving) {
            setBatchRemoveConfirm(false);
            setBatchForceRemove(false);
          }
        }}
      >
        <DialogContent className="sm:max-w-[440px]">
          <DialogHeader>
            <DialogTitle>{t("images", "batchRemove")}</DialogTitle>
            <DialogDescription>
              {t("images", "confirmBatchRemove").replace("{count}", String(selectedIds.size))}
            </DialogDescription>
          </DialogHeader>
          {batchRemoving ? (
            <div className="space-y-2">
              <div className="flex items-center justify-between text-xs text-muted-foreground">
                <span>{t("images", "batchProgress").replace("{done}", String(batchProgress.done)).replace("{total}", String(batchProgress.total))}</span>
                {batchProgress.failed > 0 && (
                  <span className="text-destructive">
                    {t("images", "batchFailed").replace("{count}", String(batchProgress.failed))}
                  </span>
                )}
              </div>
              <div className="h-2 w-full overflow-hidden rounded-full bg-muted">
                <div
                  className="h-full rounded-full bg-primary transition-all"
                  style={{ width: `${batchProgress.total > 0 ? (batchProgress.done / batchProgress.total) * 100 : 0}%` }}
                />
              </div>
            </div>
          ) : (
            <div className="space-y-3">
              <ScrollArea className="max-h-[200px]">
                <div className="space-y-1">
                  {images
                    .filter((i) => selectedIds.has(i.id))
                    .map((img) => (
                      <div key={img.id} className="rounded-md border bg-muted px-2 py-1 text-xs font-mono text-foreground break-all">
                        {img.repoTags[0] ?? img.id.slice(7, 19)}
                      </div>
                    ))}
                </div>
              </ScrollArea>
              <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <Checkbox
                  checked={batchForceRemove}
                  onCheckedChange={(checked) => setBatchForceRemove(checked === true)}
                />
                {t("images", "forceRemove")}
              </label>
            </div>
          )}
          <DialogFooter>
            <Button
              variant="outline"
              disabled={batchRemoving}
              onClick={() => setBatchRemoveConfirm(false)}
            >
              {t("common", "cancel")}
            </Button>
            <Button
              variant="destructive"
              disabled={batchRemoving}
              onClick={() => void handleBatchRemove()}
            >
              {batchRemoving
                ? `${batchProgress.done}/${batchProgress.total}`
                : t("images", "confirmBatchRemoveBtn").replace("{count}", String(selectedIds.size))}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Export Dialog */}
      <Dialog open={exportDialogOpen} onOpenChange={setExportDialogOpen}>
        <DialogContent className="sm:max-w-[460px]">
          <DialogHeader>
            <DialogTitle>{t("images", "exportImages")}</DialogTitle>
            <DialogDescription>
              {t("images", "exportSelected").replace("{count}", String(selectedImageRefs.length))}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <div className="flex gap-2">
              <Input
                value={exportPath}
                onChange={(event) => setExportPath(event.target.value)}
                placeholder={t("images", "exportPath")}
                className="font-mono text-xs"
              />
              <Button variant="outline" onClick={() => void handleChooseExportPath()}>
                {t("images", "browse")}
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">{t("images", "archivePathHint")}</p>
            <ScrollArea className="max-h-[160px]">
              <div className="space-y-1">
                {selectedImageRefs.map((reference) => (
                  <div key={reference} className="rounded-md border bg-muted px-2 py-1 font-mono text-xs">
                    {reference}
                  </div>
                ))}
              </div>
            </ScrollArea>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setExportDialogOpen(false)}>
              {t("common", "cancel")}
            </Button>
            <Button onClick={() => void handleExport()} disabled={exporting || exportPath.trim().length === 0}>
              {exporting ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <PackageOpen className="h-3.5 w-3.5" />}
              {t("images", "exportImages")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Import Dialog */}
      <Dialog open={importDialogOpen} onOpenChange={setImportDialogOpen}>
        <DialogContent className="sm:max-w-[440px]">
          <DialogHeader>
            <DialogTitle>{t("images", "importImages")}</DialogTitle>
            <DialogDescription>{t("images", "archivePathHint")}</DialogDescription>
          </DialogHeader>
          <div className="flex gap-2">
            <Input
              value={importPath}
              onChange={(event) => setImportPath(event.target.value)}
              placeholder={t("images", "importPath")}
              className="font-mono text-xs"
            />
            <Button variant="outline" onClick={() => void handleChooseImportPath()}>
              {t("images", "browse")}
            </Button>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setImportDialogOpen(false)}>
              {t("common", "cancel")}
            </Button>
            <Button onClick={() => void handleImport()} disabled={importing || importPath.trim().length === 0}>
              {importing ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Upload className="h-3.5 w-3.5" />}
              {t("images", "importArchive")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Pack Container Dialog */}
      <Dialog
        open={packDialogOpen}
        onOpenChange={(open) => {
          if (!open && !packing) setPackDialogOpen(false);
        }}
      >
        <DialogContent className="sm:max-w-[500px]">
          <DialogHeader>
            <DialogTitle>{t("images", "packFromContainer")}</DialogTitle>
            <DialogDescription>{t("images", "packFromContainerDesc")}</DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            {containers.length === 0 ? (
              <div className="rounded-md border border-border bg-muted px-3 py-2 text-xs text-muted-foreground">
                {t("images", "packNoContainers")}
              </div>
            ) : (
              <>
                <div className="space-y-1.5">
                  <div className="text-xs font-medium text-muted-foreground">{t("images", "sourceContainer")}</div>
                  <Select
                    value={selectedContainerId}
                    onValueChange={(value) => {
                      setSelectedContainerId(value);
                      const container = containers.find((item) => item.id === value);
                      if (container !== undefined) {
                        setPackImage(suggestPackedImageTag(container));
                      }
                    }}
                  >
                    <SelectTrigger
                      aria-label={t("images", "sourceContainer")}
                      className="h-9 w-full text-xs"
                      disabled={packing}
                      data-testid="image-pack-container-select"
                    >
                      <SelectValue placeholder={t("images", "sourceContainer")} />
                    </SelectTrigger>
                    <SelectContent className="w-full">
                      {containers.map((container) => (
                        <SelectItem key={container.id} value={container.id}>
                          {formatContainerOption(container)}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
                <div className="space-y-1.5">
                  <div className="text-xs font-medium text-muted-foreground">{t("images", "packImageName")}</div>
                  <Input
                    value={packImage}
                    onChange={(event) => setPackImage(event.target.value)}
                    placeholder="repo/name:tag"
                    className="font-mono text-xs"
                    disabled={packing}
                    autoComplete="off"
                    data-testid="image-pack-name-input"
                  />
                </div>
                {selectedPackContainer !== undefined && (
                  <div className="grid grid-cols-[90px_1fr] gap-x-3 gap-y-1 rounded-md border bg-muted px-3 py-2 text-xs">
                    <span className="text-muted-foreground">{t("images", "sourceContainer")}</span>
                    <span className="min-w-0 truncate">{formatContainerOption(selectedPackContainer)}</span>
                    <span className="text-muted-foreground">{t("containers", "image")}</span>
                    <span className="min-w-0 truncate font-mono">{selectedPackContainer.image}</span>
                    <span className="text-muted-foreground">{t("containers", "status")}</span>
                    <span className="min-w-0 truncate">{selectedPackContainer.status}</span>
                  </div>
                )}
              </>
            )}
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              disabled={packing}
              onClick={() => setPackDialogOpen(false)}
            >
              {t("common", "cancel")}
            </Button>
            <Button
              onClick={() => void handlePackContainer()}
              disabled={packing || selectedContainerId.trim().length === 0 || packImage.trim().length === 0}
              data-testid="image-pack-submit"
            >
              {packing ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <PackagePlus className="h-3.5 w-3.5" />}
              {t("images", "packFromContainer")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

/** Check whether an image is bundled with CrateBay. */
function isSystemImage(image: LocalImageInfo): boolean {
  return image.repoTags.some((t) => t.startsWith("cratebay-"));
}

function primaryImageReference(image: LocalImageInfo): string {
  return image.repoTags.find((tag) => tag && tag !== "<none>:<none>") ?? image.id;
}

function suggestImageTag(image: LocalImageInfo): string {
  const reference = primaryImageReference(image);
  if (reference.startsWith("sha256:")) {
    return "cratebay/image:latest";
  }

  const slashIndex = reference.lastIndexOf("/");
  const colonIndex = reference.lastIndexOf(":");
  const hasTag = colonIndex > slashIndex;
  if (!hasTag) {
    return `${reference}:copy`;
  }

  const repo = reference.slice(0, colonIndex);
  const tag = reference.slice(colonIndex + 1);
  return `${repo}:${tag}-copy`;
}

function suggestPackedImageTag(container: ContainerInfo): string {
  const base = sanitizeImageTagPart(container.name || container.shortId || container.id);
  return `cratebay/${base}:snapshot`;
}

function sanitizeImageTagPart(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "") || "container";
}

function formatContainerOption(container: ContainerInfo): string {
  return `${container.name} (${container.status})`;
}

function formatDialogError(err: unknown, fallback: string): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return fallback;
}

function formatOperationError(err: unknown, fallback: string): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  if (err === null || err === undefined) return fallback;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

function ImageOperationFeedbackBanner({ feedback }: { feedback: ImageOperationFeedback }) {
  const { t } = useI18n();
  const isError = feedback.status === "error";
  const isWarning = feedback.status === "warning";
  const Icon = isError || isWarning ? AlertCircle : CheckCircle2;
  const colorClass = isError
    ? "border-destructive/30 bg-destructive/10 text-destructive"
    : isWarning
      ? "border-amber-500/30 bg-amber-500/10 text-amber-600 dark:text-amber-400"
      : "border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400";

  return (
    <div
      className={cn("mb-3 rounded-md border px-3 py-2 text-xs", colorClass)}
      data-testid="image-operation-feedback"
    >
      <div className="flex items-center gap-2 font-medium">
        <Icon className="h-3.5 w-3.5" />
        <span>{feedback.message}</span>
      </div>
      <div className="mt-1 grid gap-1 text-muted-foreground sm:grid-cols-3">
        {imageOperationMetrics(feedback, t).map((metric) => (
          <span key={metric.label} className="min-w-0 truncate" title={metric.value}>
            {metric.label}: {metric.value}
          </span>
        ))}
      </div>
    </div>
  );
}

function imageOperationMetrics(
  feedback: ImageOperationFeedback,
  t: ReturnType<typeof useI18n>["t"],
): Array<{ label: string; value: string }> {
  const completedAt = {
    label: t("images", "operationCompletedAt"),
    value: formatClockTime(feedback.at),
  };

  if (feedback.kind === "export") {
    return [
      { label: t("images", "operationImages"), value: String(feedback.imageCount) },
      { label: t("images", "operationBytes"), value: formatBytes(feedback.bytes) },
      { label: t("images", "operationPath"), value: feedback.path },
      completedAt,
    ];
  }

  if (feedback.kind === "import") {
    return [
      { label: t("images", "operationImages"), value: String(feedback.imageCount) },
      { label: t("images", "operationPath"), value: feedback.path },
      completedAt,
    ];
  }

  if (feedback.kind === "preload") {
    return [
      { label: t("images", "operationLoaded"), value: String(feedback.loaded) },
      { label: t("images", "operationSkipped"), value: String(feedback.skipped) },
      { label: t("images", "operationFailed"), value: String(feedback.failed) },
      completedAt,
    ];
  }

  if (feedback.kind === "tag") {
    return [
      { label: t("images", "sourceImage"), value: feedback.source },
      { label: t("images", "targetTag"), value: feedback.target },
      completedAt,
    ];
  }

  if (feedback.kind === "pack") {
    return [
      { label: t("images", "operationContainer"), value: feedback.container },
      { label: t("images", "targetTag"), value: feedback.image },
      completedAt,
    ];
  }

  return [completedAt];
}

function LocalImageRow({
  image,
  selected,
  onToggleSelect,
  onInspect,
  onTag,
  onRemove,
}: {
  image: LocalImageInfo;
  selected: boolean;
  onToggleSelect: () => void;
  onInspect: () => void;
  onTag: () => void;
  onRemove: () => void;
}) {
  const { t, locale } = useI18n();
  const mainTag = image.repoTags[0] ?? "<none>";
  const isBuiltin = isSystemImage(image);
  const additionalTags = image.repoTags.length > 1 ? image.repoTags.length - 1 : 0;
  const createdDate = new Date(image.created * 1000);

  return (
    <div
      className={cn(
        "flex items-center gap-3 rounded-md border bg-card px-3 py-2.5 transition-colors hover:border-primary/30",
        selected ? "border-primary/40 bg-primary/5" : "border-border",
      )}
      data-testid="image-row"
    >
      {/* Checkbox */}
      <Checkbox
        checked={selected}
        onCheckedChange={onToggleSelect}
        className="flex-shrink-0"
        disabled={isBuiltin}
      />

      {/* Icon */}
      <div className="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary">
        <Layers className="h-[18px] w-[18px]" />
      </div>

      {/* Info */}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate font-mono text-sm font-semibold text-foreground">
            {mainTag}
          </span>
          {additionalTags > 0 && (
            <Badge variant="outline" className="text-[10px]">
              +{additionalTags} {t("images", "tags")}
            </Badge>
          )}
          {isBuiltin && (
            <Badge variant="secondary" className="text-[10px]">
              {t("images", "systemBadge")}
            </Badge>
          )}
        </div>
        <div className="mt-0.5 flex items-center gap-3 text-xs text-muted-foreground">
          <span>{image.sizeHuman}</span>
          <span>
            {formatRelativeTime(createdDate, locale, {
              withinHour: t("images", "withinHour"),
              hoursAgo: t("images", "hoursAgo"),
              daysAgo: t("images", "daysAgo"),
            })}
          </span>
          <span className="font-mono">{image.id.slice(7, 19)}</span>
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-1">
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={onInspect}
          title={t("images", "inspectImage")}
          data-testid="image-inspect-action"
        >
          <Eye className="h-3.5 w-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={onTag}
          title={t("images", "tagImage")}
          data-testid="image-tag-action"
        >
          <Tag className="h-3.5 w-3.5" />
        </Button>
        {!isBuiltin && (
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-destructive hover:text-destructive"
            onClick={onRemove}
            title={t("images", "removeImage")}
            data-testid="image-remove-action"
          >
            <Trash2 className="h-3.5 w-3.5" />
          </Button>
        )}
      </div>
    </div>
  );
}

/* ========== Search Images Tab ========== */

function isLikelyProxyOrNetworkBlocked(message: string): boolean {
  const normalized = message.toLowerCase();
  if (
    normalized.includes("proxyconnect") ||
    normalized.includes("tls handshake") ||
    normalized.includes("temporary failure in name resolution") ||
    normalized.includes("network is unreachable") ||
    normalized.includes("no route to host")
  ) {
    return true;
  }

  const timedOut =
    normalized.includes("timeout") ||
    normalized.includes("timed out") ||
    normalized.includes("i/o timeout");
  const registryRelated =
    normalized.includes("registry") ||
    normalized.includes("docker.io") ||
    normalized.includes("index.docker.io") ||
    normalized.includes("hub.docker.com");

  return timedOut && registryRelated;
}

function SearchImagesTab({ onToolbar }: { onToolbar: (node: React.ReactNode) => void }) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<ImageSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const startPull = usePullStore((s) => s.startPull);

  const handleSearch = useCallback(async () => {
    if (query.trim().length === 0) return;
    setSearching(true);
    setSearchError(null);
    try {
      const data = await Promise.race([
        invoke<ImageSearchResult[]>("image_search", { query: query.trim() }),
        new Promise<ImageSearchResult[]>((_, reject) =>
          window.setTimeout(() => reject(new Error(t("images", "searchTimeout"))), 15000),
        ),
      ]);
      setResults(data);
    } catch (err) {
      setResults([]);
      const message = err instanceof Error ? err.message : String(err);
      if (isLikelyProxyOrNetworkBlocked(message)) {
        setSearchError(`${t("images", "searchProxyHint")} (${message})`);
      } else if (message.length > 0 && message !== "[object Object]") {
        setSearchError(message);
      } else {
        setSearchError(t("images", "searchError"));
      }
    } finally {
      setSearching(false);
    }
  }, [query, t]);

  const handlePull = useCallback((image: string) => {
    void startPull(image);
  }, [startPull]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") {
        e.preventDefault();
        void handleSearch();
      }
    },
    [handleSearch],
  );

  // Inject toolbar controls into parent
  useEffect(() => {
    onToolbar(
      <>
        <div className="relative w-56">
          <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t("images", "searchPlaceholder")}
            data-testid="image-search-input"
            className="h-8 pl-8 text-xs"
          />
        </div>
        <div className="ml-auto flex items-center gap-2">
          <PullTaskList />
          <Button
            size="sm"
            onClick={() => void handleSearch()}
            disabled={searching || query.trim().length === 0}
            data-testid="image-search-submit"
          >
            {searching ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Search className="h-3.5 w-3.5" />
            )}
            {t("common", "search")}
          </Button>
        </div>
      </>
    );
    return () => onToolbar(null);
  }, [query, searching, t, handleSearch, handleKeyDown, onToolbar]);

  return (
    <div className="px-6 py-4">

      {/* Search error */}
      {searchError !== null && (
        <div className="mb-4 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {searchError}
        </div>
      )}

      {/* Results */}
      {results.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-16 text-center text-muted-foreground">
          <Globe className="mb-3 h-12 w-12 opacity-20" />
          <h3 className="text-sm font-medium">{t("images", "searchHint")}</h3>
          <p className="mt-1 text-xs">Docker Hub</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 lg:grid-cols-2 xl:grid-cols-3">
          {results.map((result, idx) => (
            <SearchResultCard
              key={`${result.source}-${result.reference}-${idx}`}
              result={result}
              onPull={() => handlePull(result.reference)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function SearchResultCard({
  result,
  onPull,
}: {
  result: ImageSearchResult;
  onPull: () => void;
}) {
  const { t } = useI18n();

  return (
    <div
      className="flex flex-col justify-between rounded-md border border-border bg-card p-3 transition-colors hover:border-primary/30"
      data-testid="image-search-result"
    >
      <div>
        <div className="flex items-center gap-2">
          <Badge
            variant="outline"
            className="text-[10px]"
          >
            {result.source}
          </Badge>
          {result.official && (
            <Badge className="border-primary/15 bg-primary/10 text-primary text-[10px]">
              {t("images", "official")}
            </Badge>
          )}
        </div>
        <span className="mt-1 block truncate text-sm font-semibold text-foreground">
          {result.reference}
        </span>
        {result.description.length > 0 && (
          <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
            {result.description}
          </p>
        )}
      </div>

      <div className="mt-3 flex items-center justify-between gap-2">
        <div className="flex items-center gap-3 text-xs text-muted-foreground">
          <span className="inline-flex items-center gap-1">
            <Star className="h-3.5 w-3.5" />
            {result.stars ?? "-"}
          </span>
          <span className="inline-flex items-center gap-1">
            <ArrowDownToLine className="h-3.5 w-3.5" />
            {formatPulls(result.pulls)}
          </span>
        </div>
        <Button
          size="sm"
          variant="outline"
          className="h-7 gap-1 px-2 text-xs"
          onClick={onPull}
          data-testid="image-search-pull"
        >
          <Download className="h-3 w-3" />
          {t("images", "pull")}
        </Button>
      </div>
    </div>
  );
}

/* ========== Helpers ========== */

function formatPulls(pulls?: number): string {
  if (pulls === undefined || pulls === null) return "-";
  if (pulls >= 1_000_000_000) return `${(pulls / 1_000_000_000).toFixed(1)}B`;
  if (pulls >= 1_000_000) return `${(pulls / 1_000_000).toFixed(1)}M`;
  if (pulls >= 1_000) return `${(pulls / 1_000).toFixed(1)}K`;
  return String(pulls);
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }
  return `${value >= 10 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
}

function formatClockTime(value: Date): string {
  return value.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function formatRelativeTime(
  date: Date,
  locale: string,
  labels: {
    withinHour: string;
    hoursAgo: string;
    daysAgo: string;
  },
): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffHour = Math.floor(diffMs / 3600000);
  const diffDay = Math.floor(diffMs / 86400000);

  if (diffHour < 1) return labels.withinHour;
  if (diffHour < 24) return labels.hoursAgo.replace("{count}", String(diffHour));
  if (diffDay < 30) return labels.daysAgo.replace("{count}", String(diffDay));
  return date.toLocaleDateString(locale);
}

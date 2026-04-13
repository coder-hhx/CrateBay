/**
 * ImagesPage — Docker image management page.
 *
 * Features:
 * - Local images tab: list, inspect, remove local images
 * - Search tab: search Docker Hub, pull images
 * - Inline image details via Dialog
 *
 * Uses Tauri commands: image_list, image_search, image_pull,
 * image_remove, image_inspect, image_tag.
 *
 * @see api-spec.md for Tauri command signatures
 */

import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { invoke } from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { usePullStore } from "@/stores/pullStore";
import { PullTaskList } from "@/components/images/PullTaskList";
import type {
  LocalImageInfo,
  ImageSearchResult,
  ImageInspectInfo,
} from "@/types/image";
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
} from "lucide-react";

export function ImagesPage() {
  const { t } = useI18n();
  const [activeTab, setActiveTab] = useState<"local" | "search">("local");
  const refreshLocalRef = useRef<(() => void) | null>(null);
  const [toolbarRight, setToolbarRight] = useState<React.ReactNode>(null);

  // 当有拉取任务完成时，自动刷新本地镜像列表
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
    <div className="flex h-full flex-col">
      {/* Unified toolbar */}
      <div className="flex items-center gap-3 border-b border-border px-6 py-2.5">
        {/* Tab pills */}
        <button
          onClick={() => setActiveTab("local")}
          className={cn(
            "inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium transition-colors focus:outline-none",
            activeTab === "local"
              ? "bg-primary/10 text-primary"
              : "text-muted-foreground hover:bg-muted hover:text-foreground",
          )}
        >
          <HardDrive className="h-3 w-3" />
          {t("images", "localImages")}
        </button>
        <button
          onClick={() => setActiveTab("search")}
          className={cn(
            "inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium transition-colors focus:outline-none",
            activeTab === "search"
              ? "bg-primary/10 text-primary"
              : "text-muted-foreground hover:bg-muted hover:text-foreground",
          )}
        >
          <Globe className="h-3 w-3" />
          {t("images", "searchImages")}
        </button>
        <div className="h-4 w-px bg-border" />
        {/* Dynamic right-side controls injected by active tab */}
        <div className="flex flex-1 items-center gap-2">
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

function LocalImagesTab({ onRefreshRef, onToolbar }: { onRefreshRef: React.MutableRefObject<(() => void) | null>; onToolbar: (node: React.ReactNode) => void }) {
  const { t } = useI18n();
  const [images, setImages] = useState<LocalImageInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState("");
  const [inspectInfo, setInspectInfo] = useState<ImageInspectInfo | null>(null);
  const [inspectLoading, setInspectLoading] = useState(false);
  const [removeConfirm, setRemoveConfirm] = useState<string | null>(null);
  const [removing, setRemoving] = useState(false);

  // Batch selection state
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [batchRemoveConfirm, setBatchRemoveConfirm] = useState(false);
  const [batchRemoving, setBatchRemoving] = useState(false);
  const [batchProgress, setBatchProgress] = useState({ done: 0, total: 0, failed: 0 });

  const fetchImages = useCallback(async () => {
    setLoading(true);
    try {
      const result = await Promise.race([
        invoke<LocalImageInfo[]>("image_list"),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error("镜像列表加载超时")), 8000),
        ),
      ]);
      setImages(result);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.warn("[ImagesPage] fetchImages failed:", message);
      // Docker not available or timeout — show empty list
      setImages([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchImages();
  }, [fetchImages]);

  // 暴露刷新方法给父组件
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
      // Docker not available — cannot inspect
      setInspectInfo(null);
    } finally {
      setInspectLoading(false);
    }
  }, []);

  const handleRemove = useCallback(async (id: string) => {
    setRemoving(true);
    try {
      await invoke("image_remove", { id, force: true });
    } catch {
      // Docker not available — removal failed
      setRemoving(false);
      setRemoveConfirm(null);
      return;
    }
    // Refresh the full list from Docker to get accurate state
    await fetchImages();
    setRemoving(false);
    setRemoveConfirm(null);
  }, [fetchImages]);

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
    setBatchProgress({ done: 0, total: ids.length, failed: 0 });
    let failed = 0;
    for (let i = 0; i < ids.length; i++) {
      try {
        await invoke("image_remove", { id: ids[i], force: true });
      } catch {
        failed++;
      }
      setBatchProgress({ done: i + 1, total: ids.length, failed });
    }
    await fetchImages();
    setSelectedIds(new Set());
    setBatchRemoving(false);
    setBatchRemoveConfirm(false);
    setBatchProgress({ done: 0, total: 0, failed: 0 });
  }, [selectedIds, fetchImages]);

  const allVisibleSelected = (() => {
    const userImages = filteredImages.filter((i) => !isSystemImage(i));
    return userImages.length > 0 && userImages.every((i) => selectedIds.has(i.id));
  })();

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
          >
            <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
            {t("common", "refresh")}
          </Button>
          <Button
            variant="destructive"
            size="sm"
            disabled={selectedIds.size === 0}
            onClick={() => setBatchRemoveConfirm(true)}
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
  }, [filter, loading, selectedIds, t, fetchImages, onToolbar]);

  return (
    <div className="px-6 py-4">
      {/* Image count with select all */}
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
                {t("images", "sandboxImages")}
              </h3>
              <div className="space-y-2">
                {filteredImages.filter((i) => isSystemImage(i)).map((img) => (
                  <LocalImageRow
                    key={img.id}
                    image={img}
                    selected={false}
                    onToggleSelect={() => {}}
                    onInspect={() => void handleInspect(img.id)}
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
                    onRemove={() => setRemoveConfirm(img.id)}
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

                <span className="text-muted-foreground">OS</span>
                <span>{inspectInfo.os}</span>

                <span className="text-muted-foreground">Docker Version</span>
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

      {/* Remove Confirm Dialog */}
      <Dialog
        open={removeConfirm !== null}
        onOpenChange={(open) => {
          if (!open) setRemoveConfirm(null);
        }}
      >
        <DialogContent className="sm:max-w-[400px]">
          <DialogHeader>
            <DialogTitle>{t("images", "removeImage")}</DialogTitle>
            <DialogDescription>{t("images", "confirmRemove")}</DialogDescription>
          </DialogHeader>
          {removeConfirm !== null && (
            <div className="rounded-md border bg-muted px-2 py-1 text-xs font-mono text-foreground break-all">
              {images.find((i) => i.id === removeConfirm)?.repoTags.join(", ") ?? removeConfirm}
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
          if (!open && !batchRemoving) setBatchRemoveConfirm(false);
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
    </div>
  );
}

/** Check whether an image is a CrateBay built-in sandbox image. */
function isSystemImage(image: LocalImageInfo): boolean {
  return image.repoTags.some((t) => t.startsWith("cratebay-"));
}

function LocalImageRow({
  image,
  selected,
  onToggleSelect,
  onInspect,
  onRemove,
}: {
  image: LocalImageInfo;
  selected: boolean;
  onToggleSelect: () => void;
  onInspect: () => void;
  onRemove: () => void;
}) {
  const { t } = useI18n();
  const mainTag = image.repoTags[0] ?? "<none>";
  const isBuiltin = isSystemImage(image);
  const additionalTags = image.repoTags.length > 1 ? image.repoTags.length - 1 : 0;
  const createdDate = new Date(image.created * 1000);

  return (
    <div className={cn(
      "flex items-center gap-3 rounded-lg border bg-card px-4 py-3 transition-colors hover:border-primary/30",
      selected ? "border-primary/40 bg-primary/5" : "border-border",
    )}>
      {/* Checkbox */}
      <Checkbox
        checked={selected}
        onCheckedChange={onToggleSelect}
        className="flex-shrink-0"
        disabled={isBuiltin}
      />

      {/* Icon */}
      <div className="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
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
          <span>{formatRelativeTime(createdDate)}</span>
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
        >
          <Eye className="h-3.5 w-3.5" />
        </Button>
        {!isBuiltin && (
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-destructive hover:text-destructive"
            onClick={onRemove}
            title={t("images", "removeImage")}
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
          window.setTimeout(() => reject(new Error("Image search timeout (15s)")), 15000),
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
  }, [query]);

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
            className="h-8 pl-8 text-xs"
          />
        </div>
        <div className="ml-auto flex items-center gap-2">
          <PullTaskList />
          <Button
            size="sm"
            onClick={() => void handleSearch()}
            disabled={searching || query.trim().length === 0}
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
          <p className="mt-1 text-xs">Docker Hub &middot; Quay.io &middot; GitHub Container Registry</p>
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
    <div className="flex flex-col justify-between rounded-xl border border-border bg-card p-4 transition-all hover:border-primary/30">
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

function formatRelativeTime(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffHour = Math.floor(diffMs / 3600000);
  const diffDay = Math.floor(diffMs / 86400000);

  if (diffHour < 1) return "1 小时内";
  if (diffHour < 24) return `${diffHour} 小时前`;
  if (diffDay < 30) return `${diffDay} 天前`;
  return date.toLocaleDateString("zh-CN");
}

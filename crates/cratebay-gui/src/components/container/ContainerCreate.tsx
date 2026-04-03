import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useContainerStore, type ContainerCreateRequest } from "@/stores/containerStore";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Plus, ChevronDown, Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";


/**
 * Format bytes to human-readable size string.
 */
function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function ContainerCreate() {
  const { t } = useI18n();
  const createContainer = useContainerStore((s) => s.createContainer);
  const images = useContainerStore((s) => s.images);
  const imagesLoading = useContainerStore((s) => s.imagesLoading);
  const fetchImages = useContainerStore((s) => s.fetchImages);

  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [image, setImage] = useState("");
  const [imageDropdownOpen, setImageDropdownOpen] = useState(false);
  const [cpuCores, setCpuCores] = useState(2);
  const [memoryMb, setMemoryMb] = useState(2048);

  const imageInputRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Fetch images when dialog opens
  useEffect(() => {
    if (open) {
      fetchImages();
    }
  }, [open, fetchImages]);

  // Close dropdown when clicking outside
  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node) &&
        imageInputRef.current &&
        !imageInputRef.current.contains(event.target as Node)
      ) {
        setImageDropdownOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Build deduplicated image list: one entry per image, prefer non-cratebay tags
  const imageOptions = useMemo(() => {
    const result: { tag: string; sizeBytes: number }[] = [];
    for (const img of images) {
      const validTags = img.repoTags.filter(
        (tag) => tag && tag !== "<none>:<none>",
      );
      if (validTags.length === 0) continue;
      // Prefer the original upstream tag over cratebay-* alias
      const preferred = validTags.find((t) => !t.startsWith("cratebay-")) ?? validTags[0];
      result.push({ tag: preferred, sizeBytes: img.sizeBytes });
    }
    const query = image.trim().toLowerCase();
    if (query.length === 0) return result;
    return result.filter((item) => item.tag.toLowerCase().includes(query));
  }, [images, image]);

  const resetForm = useCallback(() => {
    setName("");
    setImage("");
    setImageDropdownOpen(false);
    setCpuCores(2);
    setMemoryMb(2048);
  }, []);

  const canCreate = image.trim().length > 0;

  const handleCreate = useCallback(() => {
    if (!canCreate) return;
    const trimmedName = name.trim();
    const req: ContainerCreateRequest = {
      name: trimmedName || `cratebay-${Date.now().toString(36)}`,
      image: image.trim(),
      cpuCores,
      memoryMb,
      autoStart: true,
    };
    // Fire-and-forget: close dialog immediately, store handles optimistic update
    void createContainer(req);
    resetForm();
    setOpen(false);
  }, [canCreate, name, image, cpuCores, memoryMb, createContainer, resetForm]);

  return (
    <Dialog open={open} onOpenChange={(v) => { setOpen(v); if (!v) resetForm(); }}>
      <DialogTrigger asChild>
        <Button size="sm">
          <Plus className="h-4 w-4" />
          {t("containers", "create")}
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("containers", "create")}</DialogTitle>
          <DialogDescription>
            {t("containers", "createDesc")}
          </DialogDescription>
        </DialogHeader>

        <div className="flex max-h-[60vh] flex-col gap-4 overflow-y-auto py-4">
          {/* Name */}
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="container-name">{t("containers", "nameOptional")}</Label>
            <Input
              id="container-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="my-container"
            />
          </div>

          {/* Image — searchable dropdown with local images */}
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="container-image">{t("containers", "selectImage")}</Label>
            <div className="relative">
              <Input
                ref={imageInputRef}
                id="container-image"
                value={image}
                onChange={(e) => {
                  setImage(e.target.value);
                  setImageDropdownOpen(true);
                }}
                onFocus={() => setImageDropdownOpen(true)}
                placeholder={t("containers", "selectImage")}
                className="pr-8"
                autoComplete="off"
              />
              <button
                type="button"
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                onClick={() => {
                  setImageDropdownOpen(!imageDropdownOpen);
                  imageInputRef.current?.focus();
                }}
              >
                {imagesLoading ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <ChevronDown className="h-4 w-4" />
                )}
              </button>

              {/* Dropdown list */}
              {imageDropdownOpen && imageOptions.length > 0 && (
                <div
                  ref={dropdownRef}
                  className="mt-1 max-h-40 overflow-y-auto rounded-md border border-border bg-popover shadow-md"
                >
                  {imageOptions.map((item) => (
                    <button
                      key={item.tag}
                      type="button"
                      className={cn(
                        "flex w-full items-center justify-between px-3 py-2 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground",
                        image === item.tag && "bg-accent/50",
                      )}
                      onClick={() => {
                        setImage(item.tag);
                        setImageDropdownOpen(false);
                      }}
                    >
                      <span className="truncate font-mono text-xs">{item.tag}</span>
                      <span className="ml-2 flex-shrink-0 text-xs text-muted-foreground">
                        {formatSize(item.sizeBytes)}
                      </span>
                    </button>
                  ))}
                </div>
              )}

              {/* Empty state */}
              {imageDropdownOpen && imageOptions.length === 0 && !imagesLoading && image.trim().length > 0 && (
                <div
                  ref={dropdownRef}
                  className="mt-1 rounded-md border border-border bg-popover p-3 shadow-md"
                >
                  <p className="text-xs text-muted-foreground">
                    本地无匹配镜像，创建时将自动拉取 <span className="font-mono font-medium text-foreground">{image.trim()}</span>
                  </p>
                </div>
              )}
            </div>
          </div>

          {/* Resources */}
          <div className="grid grid-cols-2 gap-4">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="cpu">{t("containers", "cpuCores")}</Label>
              <Input
                id="cpu"
                type="number"
                min={1}
                max={16}
                value={cpuCores}
                onChange={(e) => setCpuCores(Number(e.target.value) || 2)}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="memory">{t("containers", "memoryMb")}</Label>
              <Input
                id="memory"
                type="number"
                min={256}
                max={65536}
                step={256}
                value={memoryMb}
                onChange={(e) => setMemoryMb(Number(e.target.value) || 2048)}
              />
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)}>
            {t("common", "cancel")}
          </Button>
          <Button onClick={handleCreate} disabled={!canCreate}>
            {t("containers", "create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

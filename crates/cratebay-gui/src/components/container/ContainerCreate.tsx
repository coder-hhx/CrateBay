import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@/lib/tauri";
import { useContainerStore, type ContainerCreateRequest } from "@/stores/containerStore";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Plus, ChevronDown, Loader2, X } from "lucide-react";
import { cn } from "@/lib/utils";
import type { PodInfo } from "@/types/pod";
import type { PortMapping, VolumeMount } from "@/types/container";

type NetworkMode = "" | "bridge" | "none" | "host";

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function parsePortMapping(input: string): PortMapping {
  const spec = input.trim();
  if (!spec) throw new Error("empty");

  const slashIndex = spec.lastIndexOf("/");
  const portPart = slashIndex >= 0 ? spec.slice(0, slashIndex) : spec;
  const protocol = (slashIndex >= 0 ? spec.slice(slashIndex + 1) : "tcp").toLowerCase();
  if (protocol !== "tcp" && protocol !== "udp") {
    throw new Error("protocol");
  }

  const parts = portPart.split(":");
  if (parts.length !== 1 && parts.length !== 2) {
    throw new Error("format");
  }
  const containerPort = parsePort(parts[parts.length - 1]);
  const hostPort = parts.length === 2 ? parsePort(parts[0]) : containerPort;

  return { hostPort, containerPort, protocol };
}

function parsePort(value: string): number {
  const port = Number(value.trim());
  if (!Number.isInteger(port) || port < 1 || port > 65535) {
    throw new Error("port");
  }
  return port;
}

function formatPortMapping(port: PortMapping): string {
  const prefix =
    port.hostPort === port.containerPort
      ? `${port.containerPort}`
      : `${port.hostPort}:${port.containerPort}`;
  return `${prefix}/${port.protocol}`;
}

function parseVolumeMount(input: string): VolumeMount {
  const spec = input.trim();
  if (!spec) throw new Error("empty");

  const parts = spec.split(":");
  if (parts.length !== 2 && parts.length !== 3) {
    throw new Error("format");
  }
  const [hostPath, containerPath, mode] = parts.map((part) => part.trim());
  if (!hostPath || !containerPath || !containerPath.startsWith("/")) {
    throw new Error("path");
  }
  if (mode && mode !== "ro" && mode !== "rw") {
    throw new Error("mode");
  }

  return {
    hostPath,
    containerPath,
    readOnly: mode === "ro" ? true : mode === "rw" ? false : undefined,
  };
}

function formatVolumeMount(volume: VolumeMount): string {
  const mode = volume.readOnly ? ":ro" : "";
  return `${volume.hostPath}:${volume.containerPath}${mode}`;
}

function parseEnvVar(input: string): string {
  const spec = input.trim();
  if (!spec) throw new Error("empty");

  const separatorIndex = spec.indexOf("=");
  if (separatorIndex <= 0) {
    throw new Error("format");
  }

  const key = spec.slice(0, separatorIndex).trim();
  const value = spec.slice(separatorIndex + 1).trim();
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) {
    throw new Error("key");
  }

  return `${key}=${value}`;
}

function envKey(envVar: string): string {
  return envVar.split("=", 1)[0];
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
  const [entrypoint, setEntrypoint] = useState("");
  const [command, setCommand] = useState("");
  const [workingDir, setWorkingDir] = useState("");
  const [pod, setPod] = useState("");
  const [network, setNetwork] = useState<NetworkMode>("");
  const [user, setUser] = useState("");
  const [readOnlyRootfs, setReadOnlyRootfs] = useState(false);
  const [pods, setPods] = useState<PodInfo[]>([]);
  const [podsLoading, setPodsLoading] = useState(false);
  const [portInput, setPortInput] = useState("");
  const [ports, setPorts] = useState<PortMapping[]>([]);
  const [volumeInput, setVolumeInput] = useState("");
  const [volumes, setVolumes] = useState<VolumeMount[]>([]);
  const [envInput, setEnvInput] = useState("");
  const [envVars, setEnvVars] = useState<string[]>([]);
  const [cpuCores, setCpuCores] = useState(2);
  const [memoryMb, setMemoryMb] = useState(2048);
  const [formError, setFormError] = useState<string | null>(null);

  const imageInputRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    fetchImages();

    let active = true;
    setPodsLoading(true);
    invoke<PodInfo[]>("pod_list")
      .then((result) => {
        if (active) setPods(result);
      })
      .catch(() => {
        if (active) setPods([]);
      })
      .finally(() => {
        if (active) setPodsLoading(false);
      });

    return () => {
      active = false;
    };
  }, [open, fetchImages]);

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

  const imageOptions = useMemo(() => {
    const result: { tag: string; sizeBytes: number }[] = [];
    for (const img of images) {
      const validTags = img.repoTags.filter(
        (tag) => tag && tag !== "<none>:<none>",
      );
      if (validTags.length === 0) continue;
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
    setEntrypoint("");
    setCommand("");
    setWorkingDir("");
    setPod("");
    setNetwork("");
    setUser("");
    setReadOnlyRootfs(false);
    setPortInput("");
    setPorts([]);
    setVolumeInput("");
    setVolumes([]);
    setEnvInput("");
    setEnvVars([]);
    setCpuCores(2);
    setMemoryMb(2048);
    setFormError(null);
  }, []);

  const canCreate = image.trim().length > 0;
  const networkLabel =
    network === "bridge"
      ? t("containers", "networkBridge")
      : network === "none"
        ? t("containers", "networkNone")
        : network === "host"
          ? t("containers", "networkHost")
          : t("containers", "defaultNetwork");

  const addPort = useCallback(() => {
    try {
      const parsed = parsePortMapping(portInput);
      setPorts((current) => [...current, parsed]);
      setPortInput("");
      setFormError(null);
    } catch {
      setFormError(t("containers", "invalidPort"));
    }
  }, [portInput, t]);

  const addVolume = useCallback(() => {
    try {
      const parsed = parseVolumeMount(volumeInput);
      setVolumes((current) => [...current, parsed]);
      setVolumeInput("");
      setFormError(null);
    } catch {
      setFormError(t("containers", "invalidVolume"));
    }
  }, [volumeInput, t]);

  const addEnvVar = useCallback(() => {
    try {
      const parsed = parseEnvVar(envInput);
      const key = envKey(parsed);
      setEnvVars((current) => [
        ...current.filter((item) => envKey(item) !== key),
        parsed,
      ]);
      setEnvInput("");
      setFormError(null);
    } catch {
      setFormError(t("containers", "invalidEnv"));
    }
  }, [envInput, t]);

  const handleCreate = useCallback(() => {
    if (!canCreate) return;

    let nextPorts = ports;
    let nextVolumes = volumes;
    let nextEnvVars = envVars;
    try {
      if (envInput.trim()) {
        const parsed = parseEnvVar(envInput);
        const key = envKey(parsed);
        nextEnvVars = [
          ...nextEnvVars.filter((item) => envKey(item) !== key),
          parsed,
        ];
      }
    } catch {
      setFormError(t("containers", "invalidEnv"));
      return;
    }

    try {
      if (portInput.trim()) {
        nextPorts = [...nextPorts, parsePortMapping(portInput)];
      }
      if (volumeInput.trim()) {
        nextVolumes = [...nextVolumes, parseVolumeMount(volumeInput)];
      }
    } catch {
      setFormError(
        portInput.trim() ? t("containers", "invalidPort") : t("containers", "invalidVolume"),
      );
      return;
    }

    const trimmedName = name.trim();
    const req: ContainerCreateRequest = {
      name: trimmedName || `cratebay-${Date.now().toString(36)}`,
      image: image.trim(),
      entrypoint: entrypoint.trim() || undefined,
      command: command.trim() || undefined,
      env: nextEnvVars.length > 0 ? nextEnvVars : undefined,
      workingDir: workingDir.trim() || undefined,
      pod: pod || undefined,
      network: network || undefined,
      user: user.trim() || undefined,
      readOnlyRootfs: readOnlyRootfs || undefined,
      ports: nextPorts.length > 0 ? nextPorts : undefined,
      volumes: nextVolumes.length > 0 ? nextVolumes : undefined,
      cpuCores,
      memoryMb,
      autoStart: true,
    };
    void createContainer(req);
    resetForm();
    setOpen(false);
  }, [
    canCreate,
    ports,
    volumes,
    envVars,
    portInput,
    volumeInput,
    envInput,
    name,
    image,
    entrypoint,
    command,
    workingDir,
    pod,
    network,
    user,
    readOnlyRootfs,
    cpuCores,
    memoryMb,
    createContainer,
    resetForm,
    t,
  ]);

  return (
    <Dialog open={open} onOpenChange={(v) => { setOpen(v); if (!v) resetForm(); }}>
      <DialogTrigger asChild>
        <Button size="sm">
          <Plus className="h-4 w-4" />
          {t("containers", "create")}
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{t("containers", "create")}</DialogTitle>
          <DialogDescription>
            {t("containers", "createDesc")}
          </DialogDescription>
        </DialogHeader>

        <div className="flex max-h-[68vh] flex-col gap-4 overflow-y-auto py-4">
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="container-name">{t("containers", "nameOptional")}</Label>
            <Input
              id="container-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="my-container"
            />
          </div>

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

              {imageDropdownOpen && imageOptions.length === 0 && !imagesLoading && image.trim().length > 0 && (
                <div
                  ref={dropdownRef}
                  className="mt-1 rounded-md border border-border bg-popover p-3 shadow-md"
                >
                  <p className="text-xs text-muted-foreground">
                    {t("containers", "missingImagePrefix")} <span className="font-mono font-medium text-foreground">{image.trim()}</span>
                  </p>
                </div>
              )}
            </div>
          </div>

          <div className="grid gap-4 sm:grid-cols-3">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="entrypoint">{t("containers", "entrypoint")}</Label>
              <Input
                id="entrypoint"
                value={entrypoint}
                onChange={(e) => setEntrypoint(e.target.value)}
                placeholder="/bin/sh"
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="command">{t("containers", "command")}</Label>
              <Input
                id="command"
                value={command}
                onChange={(e) => setCommand(e.target.value)}
                placeholder="sleep infinity"
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="working-dir">{t("containers", "workingDir")}</Label>
              <Input
                id="working-dir"
                value={workingDir}
                onChange={(e) => setWorkingDir(e.target.value)}
                placeholder="/workspace"
              />
            </div>
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
            <div className="flex flex-col gap-1.5">
              <Label>{t("containers", "pod")}</Label>
              <Select
                value={pod || "__none"}
                onValueChange={(value) => {
                  const nextPod = value === "__none" ? "" : value;
                  setPod(nextPod);
                  if (nextPod) setNetwork("");
                }}
              >
                <SelectTrigger className="w-full">
                  <SelectValue>
                    {pod || (podsLoading ? t("common", "loading") : t("containers", "noPod"))}
                  </SelectValue>
                </SelectTrigger>
                <SelectContent className="w-full">
                  <SelectItem value="__none">{t("containers", "noPod")}</SelectItem>
                  {pods.map((item) => (
                    <SelectItem key={item.id} value={item.name}>
                      {item.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
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

          <div className="grid gap-4 sm:grid-cols-[1fr_1fr_auto]">
            <div className="flex flex-col gap-1.5">
              <Label>{t("containers", "network")}</Label>
              <Select
                value={network || "__default"}
                onValueChange={(value) => {
                  const nextNetwork = value === "__default" ? "" : (value as NetworkMode);
                  setNetwork(nextNetwork);
                  if (nextNetwork) setPod("");
                }}
              >
                <SelectTrigger className="w-full">
                  <SelectValue>{networkLabel}</SelectValue>
                </SelectTrigger>
                <SelectContent className="w-full">
                  <SelectItem value="__default">{t("containers", "defaultNetwork")}</SelectItem>
                  <SelectItem value="bridge">{t("containers", "networkBridge")}</SelectItem>
                  <SelectItem value="none">{t("containers", "networkNone")}</SelectItem>
                  <SelectItem value="host">{t("containers", "networkHost")}</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="container-user">{t("containers", "user")}</Label>
              <Input
                id="container-user"
                value={user}
                onChange={(e) => setUser(e.target.value)}
                placeholder="1000:1000"
              />
            </div>
            <label className="flex min-h-9 items-center gap-2 self-end rounded-md border border-border px-3 py-2 text-sm">
              <Checkbox
                checked={readOnlyRootfs}
                onCheckedChange={(checked) => setReadOnlyRootfs(checked === true)}
              />
              <span>{t("containers", "readOnlyRootfs")}</span>
            </label>
          </div>

          <div className="flex flex-col gap-2">
            <Label htmlFor="env-var">{t("containers", "envVar")}</Label>
            <div className="flex gap-2">
              <Input
                id="env-var"
                value={envInput}
                onChange={(e) => setEnvInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    addEnvVar();
                  }
                }}
                placeholder="NODE_ENV=production"
              />
              <Button type="button" variant="outline" size="icon" onClick={addEnvVar} aria-label={t("containers", "addEnv")}>
                <Plus className="h-4 w-4" />
              </Button>
            </div>
            {envVars.length > 0 && (
              <div className="flex flex-wrap gap-2">
                {envVars.map((item) => (
                  <button
                    key={envKey(item)}
                    type="button"
                    className="inline-flex max-w-full items-center gap-1 rounded-md border border-border px-2 py-1 font-mono text-xs"
                    onClick={() => setEnvVars((current) => current.filter((envVar) => envVar !== item))}
                  >
                    <span className="truncate">{item}</span>
                    <X className="h-3 w-3 text-muted-foreground" />
                  </button>
                ))}
              </div>
            )}
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
            <div className="flex flex-col gap-2">
              <Label htmlFor="port-mapping">{t("containers", "publishPort")}</Label>
              <div className="flex gap-2">
                <Input
                  id="port-mapping"
                  value={portInput}
                  onChange={(e) => setPortInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      addPort();
                    }
                  }}
                  placeholder="8080:80/tcp"
                />
                <Button type="button" variant="outline" size="icon" onClick={addPort} aria-label={t("containers", "addPort")}>
                  <Plus className="h-4 w-4" />
                </Button>
              </div>
              {ports.length > 0 && (
                <div className="flex flex-wrap gap-2">
                  {ports.map((item, index) => (
                    <button
                      key={`${item.hostPort}-${item.containerPort}-${item.protocol}-${index}`}
                      type="button"
                      className="inline-flex max-w-full items-center gap-1 rounded-md border border-border px-2 py-1 font-mono text-xs"
                      onClick={() => setPorts((current) => current.filter((_, i) => i !== index))}
                    >
                      <span className="truncate">{formatPortMapping(item)}</span>
                      <X className="h-3 w-3 text-muted-foreground" />
                    </button>
                  ))}
                </div>
              )}
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="volume-mount">{t("containers", "volumeMount")}</Label>
              <div className="flex gap-2">
                <Input
                  id="volume-mount"
                  value={volumeInput}
                  onChange={(e) => setVolumeInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      addVolume();
                    }
                  }}
                  placeholder="/host:/container:ro"
                />
                <Button type="button" variant="outline" size="icon" onClick={addVolume} aria-label={t("containers", "addVolume")}>
                  <Plus className="h-4 w-4" />
                </Button>
              </div>
              {volumes.length > 0 && (
                <div className="flex flex-wrap gap-2">
                  {volumes.map((item, index) => (
                    <button
                      key={`${item.hostPath}-${item.containerPath}-${index}`}
                      type="button"
                      className="inline-flex max-w-full items-center gap-1 rounded-md border border-border px-2 py-1 font-mono text-xs"
                      onClick={() => setVolumes((current) => current.filter((_, i) => i !== index))}
                    >
                      <span className="truncate">{formatVolumeMount(item)}</span>
                      <X className="h-3 w-3 text-muted-foreground" />
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>

          {formError && (
            <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {formError}
            </p>
          )}
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

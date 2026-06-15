import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@/lib/tauri";
import { useContainerStore } from "@/stores/containerStore";
import { useSettingsStore } from "@/stores/settingsStore";
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
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";
import {
  envKey,
  formatPortMapping,
  formatVolumeMount,
  isBuiltInNetworkMode,
  parseEnvVar,
  parsePortMapping,
  parseVolumeMount,
} from "@/lib/containerForm";
import { ChevronDown, Loader2, Play, Plus, X, Zap } from "lucide-react";
import type { ContainerRunRequest, ContainerRunResult, PortMapping, VolumeMount } from "@/types/container";
import type { PodInfo } from "@/types/pod";
import type { NetworkInfo } from "@/types/network";
import type { VolumeInfo } from "@/types/volume";

type NetworkMode = string;

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function ContainerRun() {
  const { t } = useI18n();
  const images = useContainerStore((s) => s.images);
  const imagesLoading = useContainerStore((s) => s.imagesLoading);
  const fetchImages = useContainerStore((s) => s.fetchImages);
  const fetchContainers = useContainerStore((s) => s.fetchContainers);
  const registryMirrors = useSettingsStore((s) => s.settings.registryMirrors);

  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [image, setImage] = useState("");
  const [entrypoint, setEntrypoint] = useState("");
  const [command, setCommand] = useState("echo hello from CrateBay");
  const [workingDir, setWorkingDir] = useState("");
  const [pod, setPod] = useState("");
  const [network, setNetwork] = useState<NetworkMode>("");
  const [user, setUser] = useState("");
  const [readOnlyRootfs, setReadOnlyRootfs] = useState(false);
  const [pullImage, setPullImage] = useState(true);
  const [timeoutSecs, setTimeoutSecs] = useState(120);
  const [maxOutputBytes, setMaxOutputBytes] = useState(200000);
  const [cpuCores, setCpuCores] = useState("");
  const [memoryMb, setMemoryMb] = useState("");
  const [envInput, setEnvInput] = useState("");
  const [envVars, setEnvVars] = useState<string[]>([]);
  const [portInput, setPortInput] = useState("");
  const [ports, setPorts] = useState<PortMapping[]>([]);
  const [volumeInput, setVolumeInput] = useState("");
  const [volumes, setVolumes] = useState<VolumeMount[]>([]);
  const [availableVolumes, setAvailableVolumes] = useState<VolumeInfo[]>([]);
  const [volumesLoading, setVolumesLoading] = useState(false);
  const [selectedVolume, setSelectedVolume] = useState("");
  const [selectedVolumePath, setSelectedVolumePath] = useState("/workspace");
  const [selectedVolumeReadOnly, setSelectedVolumeReadOnly] = useState(false);
  const [keepContainer, setKeepContainer] = useState(false);
  const [pods, setPods] = useState<PodInfo[]>([]);
  const [podsLoading, setPodsLoading] = useState(false);
  const [networks, setNetworks] = useState<NetworkInfo[]>([]);
  const [networksLoading, setNetworksLoading] = useState(false);
  const [running, setRunning] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [result, setResult] = useState<ContainerRunResult | null>(null);
  const [imageDropdownOpen, setImageDropdownOpen] = useState(false);

  const imageInputRef = useRef<HTMLInputElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    void fetchImages();

    let active = true;
    setPodsLoading(true);
    invoke<PodInfo[]>("pod_list")
      .then((items) => {
        if (active) setPods(items);
      })
      .catch(() => {
        if (active) setPods([]);
      })
      .finally(() => {
        if (active) setPodsLoading(false);
      });

    setNetworksLoading(true);
    invoke<NetworkInfo[]>("network_list")
      .then((items) => {
        if (active) setNetworks(items);
      })
      .catch(() => {
        if (active) setNetworks([]);
      })
      .finally(() => {
        if (active) setNetworksLoading(false);
      });

    setVolumesLoading(true);
    invoke<VolumeInfo[]>("volume_list")
      .then((items) => {
        if (active) setAvailableVolumes(items);
      })
      .catch(() => {
        if (active) setAvailableVolumes([]);
      })
      .finally(() => {
        if (active) setVolumesLoading(false);
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
      const tags = img.repoTags.filter((tag) => tag && tag !== "<none>:<none>");
      if (tags.length === 0) continue;
      result.push({ tag: tags.find((tag) => !tag.startsWith("cratebay-")) ?? tags[0], sizeBytes: img.sizeBytes });
    }
    const query = image.trim().toLowerCase();
    return query.length === 0
      ? result
      : result.filter((item) => item.tag.toLowerCase().includes(query));
  }, [images, image]);

  const canRun = image.trim().length > 0 && command.trim().length > 0 && !running;
  const networkLabel =
    network === "bridge"
      ? t("containers", "networkBridge")
      : network === "none"
        ? t("containers", "networkNone")
        : network === "host"
          ? t("containers", "networkHost")
          : network || t("containers", "defaultNetwork");
  const customNetworks = networks.filter((item) => !isBuiltInNetworkMode(item.name));

  const resetState = useCallback(() => {
    setName("");
    setImage("");
    setEntrypoint("");
    setCommand("echo hello from CrateBay");
    setWorkingDir("");
    setPod("");
    setNetwork("");
    setUser("");
    setReadOnlyRootfs(false);
    setPullImage(true);
    setTimeoutSecs(120);
    setMaxOutputBytes(200000);
    setCpuCores("");
    setMemoryMb("");
    setEnvInput("");
    setEnvVars([]);
    setPortInput("");
    setPorts([]);
    setVolumeInput("");
    setVolumes([]);
    setSelectedVolume("");
    setSelectedVolumePath("/workspace");
    setSelectedVolumeReadOnly(false);
    setKeepContainer(false);
    setFormError(null);
    setResult(null);
    setImageDropdownOpen(false);
  }, []);

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

  const addExistingVolume = useCallback(() => {
    const containerPath = selectedVolumePath.trim();
    if (!selectedVolume || !containerPath.startsWith("/")) {
      setFormError(t("containers", "invalidVolume"));
      return;
    }
    setVolumes((current) => [
      ...current,
      {
        hostPath: selectedVolume,
        containerPath,
        readOnly: selectedVolumeReadOnly || undefined,
      },
    ]);
    setSelectedVolumePath("/workspace");
    setSelectedVolumeReadOnly(false);
    setFormError(null);
  }, [selectedVolume, selectedVolumePath, selectedVolumeReadOnly, t]);

  const handleRun = useCallback(async () => {
    if (!canRun) return;
    const timeout = Number.isFinite(timeoutSecs) ? Math.max(0, Math.floor(timeoutSecs)) : 0;
    const outputLimit = Number.isFinite(maxOutputBytes) ? Math.max(0, Math.floor(maxOutputBytes)) : 0;
    const cpu = cpuCores.trim() ? Math.floor(Number(cpuCores)) : undefined;
    const memory = memoryMb.trim() ? Math.floor(Number(memoryMb)) : undefined;
    if (cpu !== undefined && (!Number.isFinite(cpu) || cpu < 1 || cpu > 16)) {
      setFormError(t("containers", "invalidCpu"));
      return;
    }
    if (memory !== undefined && (!Number.isFinite(memory) || memory < 256 || memory > 65536)) {
      setFormError(t("containers", "invalidMemory"));
      return;
    }

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

    let nextPorts = ports;
    let nextVolumes = volumes;
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

    const request: ContainerRunRequest = {
      ...(name.trim() ? { name: name.trim() } : {}),
      image: image.trim(),
      ...(entrypoint.trim() ? { entrypoint: entrypoint.trim() } : {}),
      command: ["sh", "-lc", command.trim()],
      ...(nextEnvVars.length > 0 ? { env: nextEnvVars } : {}),
      ...(nextPorts.length > 0 ? { ports: nextPorts } : {}),
      ...(nextVolumes.length > 0 ? { volumes: nextVolumes } : {}),
      ...(cpu !== undefined ? { cpuCores: cpu } : {}),
      ...(memory !== undefined ? { memoryMb: memory } : {}),
      ...(workingDir.trim() ? { workingDir: workingDir.trim() } : {}),
      pod: pod || undefined,
      ...(network ? { network } : {}),
      ...(user.trim() ? { user: user.trim() } : {}),
      ...(readOnlyRootfs ? { readOnlyRootfs } : {}),
      pull: pullImage,
      remove: !keepContainer,
      timeoutSecs: timeout > 0 ? timeout : undefined,
      maxOutputBytes: outputLimit > 0 ? outputLimit : undefined,
      registryMirrors: pullImage && registryMirrors.length > 0 ? registryMirrors : undefined,
    };

    setRunning(true);
    setFormError(null);
    setResult(null);
    try {
      const output = await invoke<ContainerRunResult>("container_run", { request });
      setResult(output);
      void fetchContainers();
    } catch (error) {
      setFormError(error instanceof Error ? error.message : String(error));
    } finally {
      setRunning(false);
    }
  }, [
    canRun,
    command,
    cpuCores,
    entrypoint,
    envInput,
    envVars,
    fetchContainers,
    image,
    keepContainer,
    maxOutputBytes,
    memoryMb,
    name,
    network,
    portInput,
    ports,
    pod,
    pullImage,
    readOnlyRootfs,
    registryMirrors,
    t,
    timeoutSecs,
    user,
    volumeInput,
    volumes,
    workingDir,
  ]);

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => { setOpen(nextOpen); if (!nextOpen) resetState(); }}>
      <DialogTrigger asChild>
        <Button size="sm" variant="outline" data-testid="container-run">
          <Zap className="h-4 w-4" />
          {t("containers", "run")}
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{t("containers", "run")}</DialogTitle>
          <DialogDescription>{t("containers", "runDesc")}</DialogDescription>
        </DialogHeader>

        <div className="flex max-h-[70vh] flex-col gap-4 overflow-y-auto py-4">
          <div className="grid gap-4 sm:grid-cols-[200px_1fr]">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="run-name">{t("containers", "nameOptional")}</Label>
              <Input
                id="run-name"
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="sandbox-run"
                disabled={running}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="run-entrypoint">{t("containers", "entrypoint")}</Label>
              <Input
                id="run-entrypoint"
                value={entrypoint}
                onChange={(event) => setEntrypoint(event.target.value)}
                placeholder="/bin/sh"
                disabled={running}
              />
            </div>
          </div>

          <div className="grid gap-4 sm:grid-cols-[1fr_180px]">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="run-image">{t("containers", "selectImage")}</Label>
              <div className="relative">
                <Input
                  ref={imageInputRef}
                  id="run-image"
                  value={image}
                  onChange={(event) => {
                    setImage(event.target.value);
                    setImageDropdownOpen(true);
                  }}
                  onFocus={() => setImageDropdownOpen(true)}
                  placeholder="alpine:latest"
                  className="pr-8"
                  autoComplete="off"
                  disabled={running}
                />
                <button
                  type="button"
                  className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground disabled:opacity-50"
                  onClick={() => {
                    setImageDropdownOpen(!imageDropdownOpen);
                    imageInputRef.current?.focus();
                  }}
                  disabled={running}
                  aria-label={t("containers", "selectImage")}
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
                    className="absolute z-50 mt-1 max-h-44 w-full overflow-y-auto rounded-md border border-border bg-popover shadow-md"
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
              </div>
            </div>

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
                <SelectTrigger className="w-full" disabled={running}>
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
          </div>

          <div className="flex flex-col gap-1.5">
            <Label htmlFor="run-command">{t("containers", "runCommand")}</Label>
            <Textarea
              id="run-command"
              value={command}
              onChange={(event) => setCommand(event.target.value)}
              className="min-h-20 font-mono text-sm"
              placeholder="uname -a"
              disabled={running}
            />
          </div>

          <div className="grid gap-4 sm:grid-cols-3">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="run-working-dir">{t("containers", "workingDir")}</Label>
              <Input
                id="run-working-dir"
                value={workingDir}
                onChange={(event) => setWorkingDir(event.target.value)}
                placeholder="/workspace"
                disabled={running}
              />
            </div>
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
                <SelectTrigger className="w-full" disabled={running}>
                  <SelectValue>{networkLabel}</SelectValue>
                </SelectTrigger>
                <SelectContent className="w-full">
                  <SelectItem value="__default">{t("containers", "defaultNetwork")}</SelectItem>
                  <SelectItem value="bridge">{t("containers", "networkBridge")}</SelectItem>
                  <SelectItem value="none">{t("containers", "networkNone")}</SelectItem>
                  <SelectItem value="host">{t("containers", "networkHost")}</SelectItem>
                  {customNetworks.map((item) => (
                    <SelectItem key={item.id || item.name} value={item.name}>
                      {item.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {networksLoading && (
                <span className="text-xs text-muted-foreground">{t("common", "loading")}</span>
              )}
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="run-user">{t("containers", "user")}</Label>
              <Input
                id="run-user"
                value={user}
                onChange={(event) => setUser(event.target.value)}
                placeholder="1000:1000"
                disabled={running}
              />
            </div>
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
            <div className="flex flex-col gap-2">
              <Label htmlFor="run-env-var">{t("containers", "envVar")}</Label>
              <div className="flex gap-2">
                <Input
                  id="run-env-var"
                  value={envInput}
                  onChange={(event) => setEnvInput(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      addEnvVar();
                    }
                  }}
                  placeholder="NODE_ENV=production"
                  disabled={running}
                />
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  onClick={addEnvVar}
                  disabled={running}
                  aria-label={t("containers", "addEnv")}
                >
                  <Plus className="h-4 w-4" />
                </Button>
              </div>
              {envVars.length > 0 && (
                <div className="flex flex-wrap gap-2">
                  {envVars.map((item) => (
                    <button
                      key={envKey(item)}
                      type="button"
                      className="inline-flex max-w-full items-center gap-1 rounded-md border border-border px-2 py-1 font-mono text-xs disabled:opacity-50"
                      onClick={() => setEnvVars((current) => current.filter((envVar) => envVar !== item))}
                      disabled={running}
                    >
                      <span className="truncate">{item}</span>
                      <X className="h-3 w-3 text-muted-foreground" />
                    </button>
                  ))}
                </div>
              )}
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="run-port-mapping">{t("containers", "publishPort")}</Label>
              <div className="flex gap-2">
                <Input
                  id="run-port-mapping"
                  value={portInput}
                  onChange={(event) => setPortInput(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      addPort();
                    }
                  }}
                  placeholder="8080:80/tcp"
                  disabled={running}
                />
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  onClick={addPort}
                  disabled={running}
                  aria-label={t("containers", "addPort")}
                >
                  <Plus className="h-4 w-4" />
                </Button>
              </div>
              {ports.length > 0 && (
                <div className="flex flex-wrap gap-2">
                  {ports.map((item, index) => (
                    <button
                      key={`${item.hostPort}-${item.containerPort}-${item.protocol}-${index}`}
                      type="button"
                      className="inline-flex max-w-full items-center gap-1 rounded-md border border-border px-2 py-1 font-mono text-xs disabled:opacity-50"
                      onClick={() => setPorts((current) => current.filter((_, i) => i !== index))}
                      disabled={running}
                    >
                      <span className="truncate">{formatPortMapping(item)}</span>
                      <X className="h-3 w-3 text-muted-foreground" />
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>

          <div className="grid gap-4 sm:grid-cols-1">
            <div className="flex flex-col gap-2">
              <Label htmlFor="run-volume-mount">{t("containers", "volumeMount")}</Label>
              <div className="grid gap-2 sm:grid-cols-[1fr_1fr_auto_auto]">
                <Select
                  value={selectedVolume || "__none"}
                  onValueChange={(value) => setSelectedVolume(value === "__none" ? "" : value)}
                >
                  <SelectTrigger className="w-full" disabled={running}>
                    <SelectValue>
                      {selectedVolume || (volumesLoading ? t("common", "loading") : t("containers", "selectVolume"))}
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent className="w-full">
                    <SelectItem value="__none">{t("containers", "selectVolume")}</SelectItem>
                    {availableVolumes.map((item) => (
                      <SelectItem key={item.name} value={item.name}>
                        {item.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Input
                  value={selectedVolumePath}
                  onChange={(event) => setSelectedVolumePath(event.target.value)}
                  placeholder={t("containers", "mountPath")}
                  disabled={running}
                />
                <label className="flex min-h-9 items-center gap-2 rounded-md border border-border px-3 py-2 text-sm">
                  <Checkbox
                    checked={selectedVolumeReadOnly}
                    onCheckedChange={(checked) => setSelectedVolumeReadOnly(checked === true)}
                    disabled={running}
                  />
                  <span>{t("containers", "readOnlyMount")}</span>
                </label>
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  onClick={addExistingVolume}
                  disabled={running || !selectedVolume}
                  aria-label={t("containers", "addExistingVolume")}
                >
                  <Plus className="h-4 w-4" />
                </Button>
              </div>
              <div className="flex gap-2">
                <Input
                  id="run-volume-mount"
                  value={volumeInput}
                  onChange={(event) => setVolumeInput(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      addVolume();
                    }
                  }}
                  placeholder="/host:/container:ro"
                  disabled={running}
                />
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  onClick={addVolume}
                  disabled={running}
                  aria-label={t("containers", "addVolume")}
                >
                  <Plus className="h-4 w-4" />
                </Button>
              </div>
              {volumes.length > 0 && (
                <div className="flex flex-wrap gap-2">
                  {volumes.map((item, index) => (
                    <button
                      key={`${item.hostPath}-${item.containerPath}-${index}`}
                      type="button"
                      className="inline-flex max-w-full items-center gap-1 rounded-md border border-border px-2 py-1 font-mono text-xs disabled:opacity-50"
                      onClick={() => setVolumes((current) => current.filter((_, i) => i !== index))}
                      disabled={running}
                    >
                      <span className="truncate">{formatVolumeMount(item)}</span>
                      <X className="h-3 w-3 text-muted-foreground" />
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>

          <div className="grid gap-4 sm:grid-cols-4">
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="run-cpu">{t("containers", "cpuCores")}</Label>
              <Input
                id="run-cpu"
                type="number"
                min={1}
                max={16}
                value={cpuCores}
                onChange={(event) => setCpuCores(event.target.value)}
                placeholder="2"
                disabled={running}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="run-memory">{t("containers", "memoryMb")}</Label>
              <Input
                id="run-memory"
                type="number"
                min={256}
                max={65536}
                step={256}
                value={memoryMb}
                onChange={(event) => setMemoryMb(event.target.value)}
                placeholder="2048"
                disabled={running}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="run-timeout">{t("containers", "timeoutSecs")}</Label>
              <Input
                id="run-timeout"
                type="number"
                min={0}
                max={3600}
                value={timeoutSecs}
                onChange={(event) => setTimeoutSecs(Number(event.target.value) || 0)}
                disabled={running}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="run-max-output">{t("containers", "maxOutputBytes")}</Label>
              <Input
                id="run-max-output"
                type="number"
                min={0}
                value={maxOutputBytes}
                onChange={(event) => setMaxOutputBytes(Number(event.target.value) || 0)}
                disabled={running}
              />
            </div>
          </div>

          <div className="grid gap-3 sm:grid-cols-3">
            <label className="flex min-h-9 items-center gap-2 rounded-md border border-border px-3 py-2 text-sm">
              <Checkbox
                checked={pullImage}
                onCheckedChange={(checked) => setPullImage(checked === true)}
                disabled={running}
              />
              <span>{t("containers", "pullImage")}</span>
            </label>
            <label className="flex min-h-9 items-center gap-2 rounded-md border border-border px-3 py-2 text-sm">
              <Checkbox
                checked={readOnlyRootfs}
                onCheckedChange={(checked) => setReadOnlyRootfs(checked === true)}
                disabled={running}
              />
              <span>{t("containers", "readOnlyRootfs")}</span>
            </label>
            <label className="flex min-h-9 items-center gap-2 self-end rounded-md border border-border px-3 py-2 text-sm">
              <Checkbox
                checked={keepContainer}
                onCheckedChange={(checked) => setKeepContainer(checked === true)}
                disabled={running}
              />
              <span>{t("containers", "keepContainer")}</span>
            </label>
          </div>

          {formError && (
            <p className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive">
              {formError}
            </p>
          )}

          {result && (
            <div className="rounded-md border border-border">
              <div className="flex flex-wrap items-center gap-2 border-b border-border px-3 py-2 text-xs">
                <span className="font-mono text-muted-foreground">{result.name}</span>
                <span className={cn(
                  "rounded-full px-2 py-0.5 font-medium",
                  result.exitCode === 0 && !result.timedOut
                    ? "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
                    : "bg-destructive/10 text-destructive",
                )}>
                  {result.timedOut
                    ? t("containers", "timedOut")
                    : `${t("containers", "exitCode")} ${result.exitCode}`}
                </span>
                {(result.stdoutTruncated || result.stderrTruncated) && (
                  <span className="rounded-full bg-amber-500/10 px-2 py-0.5 font-medium text-amber-600 dark:text-amber-400">
                    {t("containers", "outputTruncated")}
                  </span>
                )}
              </div>
              <div className="grid gap-0 md:grid-cols-2">
                <RunOutput title="stdout" value={result.stdout} empty={t("common", "noResults")} />
                <RunOutput title="stderr" value={result.stderr} empty={t("common", "noResults")} />
              </div>
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)} disabled={running}>
            {t("common", "close")}
          </Button>
          <Button onClick={() => void handleRun()} disabled={!canRun}>
            {running ? <Loader2 className="h-4 w-4 animate-spin" /> : <Play className="h-4 w-4" />}
            {t("containers", "runContainer")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function RunOutput({ title, value, empty }: { title: string; value: string; empty: string }) {
  return (
    <div className="min-w-0 border-b border-border md:border-b-0 md:border-r last:border-r-0">
      <div className="border-b border-border px-3 py-1.5 font-mono text-xs text-muted-foreground">
        {title}
      </div>
      <pre className="max-h-52 min-h-24 overflow-auto whitespace-pre-wrap break-words bg-muted/30 px-3 py-2 font-mono text-xs">
        {value || empty}
      </pre>
    </div>
  );
}

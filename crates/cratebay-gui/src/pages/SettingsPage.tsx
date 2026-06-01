import { useCallback, useEffect, useState } from "react";
import { invoke } from "@/lib/tauri";
import {
  syncRuntimeStoreState,
  type DockerStatusResponse,
  type RuntimeStatusResponse,
} from "@/lib/runtimeStatus";
import { useSettingsStore } from "@/stores/settingsStore";
import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/lib/i18n";
import { DEFAULT_REGISTRY_MIRRORS } from "@/types/settings";
import { APP_VERSION } from "@/lib/constants";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  ExternalLink,
  Loader2,
  Package,
  Play,
  RefreshCw,
  RotateCcw,
  Square,
  Plus,
  X,
} from "lucide-react";

export function SettingsPage() {
  const { t } = useI18n();

  return (
    <div className="flex h-full flex-col overflow-auto p-6">
      <Tabs defaultValue="general" className="flex-1">
        <TabsList className="mb-4">
          <TabsTrigger value="general" data-testid="settings-tab-general">
            {t("settings", "general")}
          </TabsTrigger>
          <TabsTrigger value="runtime" data-testid="settings-tab-runtime">
            {t("settings", "runtime")}
          </TabsTrigger>
          <TabsTrigger value="about" data-testid="settings-tab-about">
            {t("settings", "about")}
          </TabsTrigger>
        </TabsList>

        <TabsContent value="general" className="mt-4">
          <GeneralTab />
        </TabsContent>

        <TabsContent value="runtime" className="mt-4">
          <RuntimeTab />
        </TabsContent>

        <TabsContent value="about" className="mt-4">
          <AboutTab />
        </TabsContent>
      </Tabs>
    </div>
  );
}

function SettingRow({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between border-b border-border py-3">
      <div className="flex flex-col gap-0.5">
        <span className="text-sm font-medium">{label}</span>
        {description && (
          <span className="text-xs text-muted-foreground">{description}</span>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function GeneralTab() {
  const { t } = useI18n();
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="flex max-w-2xl flex-col">
      <SettingRow label={t("settings", "language")} description={t("settings", "languageDesc")}>
        <Select
          value={settings.language}
          onValueChange={(v) => void updateSettings({ language: v as "en" | "zh-CN" })}
        >
          <SelectTrigger className="w-48">
            <SelectValue>
              {settings.language === "zh-CN"
                ? t("settings", "simplifiedChinese")
                : t("settings", "english")}
            </SelectValue>
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="en">{t("settings", "english")}</SelectItem>
            <SelectItem value="zh-CN">{t("settings", "simplifiedChinese")}</SelectItem>
          </SelectContent>
        </Select>
      </SettingRow>

      <SettingRow label={t("settings", "theme")} description={t("settings", "themeDesc")}>
        <Select
          value={settings.theme}
          onValueChange={(v) =>
            void updateSettings({ theme: v as "dark" | "light" | "system" })
          }
        >
          <SelectTrigger className="w-48">
            <SelectValue>
              {settings.theme === "dark"
                ? t("settings", "themeDark")
                : settings.theme === "light"
                  ? t("settings", "themeLight")
                  : t("settings", "themeSystem")}
            </SelectValue>
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="dark">{t("settings", "themeDark")}</SelectItem>
            <SelectItem value="light">{t("settings", "themeLight")}</SelectItem>
            <SelectItem value="system">{t("settings", "themeSystem")}</SelectItem>
          </SelectContent>
        </Select>
      </SettingRow>
    </div>
  );
}

function RuntimeStatusDot({
  status,
}: {
  status: "running" | "starting" | "error" | "disconnected";
}) {
  const colorClass =
    status === "running"
      ? "bg-green-500"
      : status === "starting"
        ? "bg-yellow-500 animate-pulse"
        : status === "error"
          ? "bg-red-500"
          : "bg-muted-foreground";

  return <span className={`inline-block h-2 w-2 rounded-full ${colorClass}`} />;
}

function RuntimeTab() {
  const { t } = useI18n();
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  const runtimeStatus = useAppStore((s) => s.runtimeStatus);
  const dockerConnected = useAppStore((s) => s.dockerConnected);
  const runtimeLoading = useAppStore((s) => s.runtimeLoading);
  const setRuntimeLoading = useAppStore((s) => s.setRuntimeLoading);
  const addNotification = useAppStore((s) => s.addNotification);
  const [proxyInput, setProxyInput] = useState(settings.runtimeHttpProxy);
  const [dockerStatusInfo, setDockerStatusInfo] = useState<DockerStatusResponse | null>(null);
  const [runtimeStatusInfo, setRuntimeStatusInfo] = useState<RuntimeStatusResponse | null>(null);
  const [diagnosticsLoading, setDiagnosticsLoading] = useState(false);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);

  useEffect(() => {
    setProxyInput(settings.runtimeHttpProxy);
  }, [settings.runtimeHttpProxy]);

  const loadDiagnostics = useCallback(async () => {
    setDiagnosticsLoading(true);
    setDiagnosticsError(null);
    try {
      const [dockerStatus, runtimeInfo] = await Promise.all([
        invoke<DockerStatusResponse | null>("docker_status"),
        invoke<RuntimeStatusResponse | null>("runtime_status"),
      ]);
      setDockerStatusInfo(dockerStatus ?? null);
      setRuntimeStatusInfo(runtimeInfo ?? null);
      syncRuntimeStoreState(dockerStatus, runtimeInfo);
    } catch (error) {
      setDockerStatusInfo(null);
      setRuntimeStatusInfo(null);
      setDiagnosticsError(formatError(error));
    } finally {
      setDiagnosticsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadDiagnostics();
  }, [loadDiagnostics]);

  const handleRuntimeStart = async () => {
    try {
      setRuntimeLoading(true);
      await invoke("runtime_start");
      await loadDiagnostics();
      addNotification({
        type: "success",
        title: t("settings", "runtimeStarting"),
        dismissable: true,
      });
    } catch (error) {
      addNotification({
        type: "error",
        title: t("common", "error"),
        message: error instanceof Error ? error.message : String(error),
        dismissable: true,
      });
    } finally {
      setRuntimeLoading(false);
    }
  };

  const handleRuntimeStop = async () => {
    try {
      setRuntimeLoading(true);
      await invoke("runtime_stop");
      await loadDiagnostics();
      addNotification({
        type: "success",
        title: t("settings", "runtimeStopped"),
        dismissable: true,
      });
    } catch (error) {
      addNotification({
        type: "error",
        title: t("common", "error"),
        message: error instanceof Error ? error.message : String(error),
        dismissable: true,
      });
    } finally {
      setRuntimeLoading(false);
    }
  };

  const handleRuntimeRestart = async () => {
    try {
      setRuntimeLoading(true);
      if (runtimeStatus !== "stopped") {
        await invoke("runtime_stop");
      }
      await invoke("runtime_start");
      await loadDiagnostics();
      addNotification({
        type: "success",
        title: t("settings", "runtimeRestart"),
        message: t("settings", "runtimeProxyRestartHint"),
        dismissable: true,
      });
    } catch (error) {
      addNotification({
        type: "error",
        title: t("common", "error"),
        message: error instanceof Error ? error.message : String(error),
        dismissable: true,
      });
    } finally {
      setRuntimeLoading(false);
    }
  };

  const handleSaveRuntimeProxy = async () => {
    try {
      await updateSettings({ runtimeHttpProxy: proxyInput.trim() });
      addNotification({
        type: "success",
        title: t("settings", "runtimeProxySaveSuccess"),
        message: t("settings", "runtimeProxyRestartHint"),
        dismissable: true,
      });
    } catch (error) {
      addNotification({
        type: "error",
        title: t("common", "error"),
        message: error instanceof Error ? error.message : String(error),
        dismissable: true,
      });
    }
  };

  const runtimeProxyDirty = proxyInput.trim() !== settings.runtimeHttpProxy;
  const displayStatus = dockerConnected
    ? "running"
    : runtimeStatus === "starting" || runtimeStatus === "error"
      ? runtimeStatus
      : "disconnected";

  return (
    <div className="flex max-w-2xl flex-col">
      <SettingRow
        label={t("settings", "containerEngine")}
        description={t("settings", "containerEngineDesc")}
      >
        <div className="flex items-center gap-2">
          <RuntimeStatusDot status={displayStatus} />
          <span
            className={`text-sm font-medium ${
              dockerConnected
                ? "text-green-500"
                : runtimeStatus === "error"
                  ? "text-red-500"
                  : "text-muted-foreground"
            }`}
          >
            {dockerConnected
              ? t("settings", "dockerSourceBuiltin")
              : runtimeStatus === "starting"
                ? t("settings", "runtimeStarting")
                : runtimeStatus === "error"
                  ? t("settings", "runtimeError")
                  : t("common", "disconnected")}
          </span>
        </div>
      </SettingRow>

      <SettingRow
        label={t("settings", "runtimeControl")}
        description={t("settings", "runtimeControlDesc")}
      >
        <div className="flex gap-2">
          <Button
            onClick={() => void handleRuntimeStart()}
            disabled={runtimeLoading || runtimeStatus === "running"}
            size="sm"
            variant={runtimeStatus === "running" ? "outline" : "default"}
            className="gap-1.5"
          >
            <Play size={14} />
            {t("common", "start")}
          </Button>
          <Button
            onClick={() => void handleRuntimeStop()}
            disabled={runtimeLoading || runtimeStatus === "stopped"}
            size="sm"
            variant={runtimeStatus === "stopped" ? "outline" : "destructive"}
            className="gap-1.5"
          >
            <Square size={14} />
            {t("common", "stop")}
          </Button>
          <Button
            onClick={() => void handleRuntimeRestart()}
            disabled={runtimeLoading}
            size="sm"
            variant="outline"
            className="gap-1.5"
          >
            <RotateCcw size={14} />
            {t("settings", "runtimeRestart")}
          </Button>
        </div>
      </SettingRow>

      <RuntimeDiagnostics
        dockerStatus={dockerStatusInfo}
        runtimeInfo={runtimeStatusInfo}
        loading={diagnosticsLoading}
        error={diagnosticsError}
        onRefresh={loadDiagnostics}
      />

      <SettingRow
        label={t("settings", "runtimeHttpProxy")}
        description={t("settings", "runtimeHttpProxyDesc")}
      >
        <Input
          value={proxyInput}
          onChange={(e) => setProxyInput(e.target.value)}
          placeholder="127.0.0.1:7890"
          className="w-64 font-mono text-xs"
        />
      </SettingRow>

      <div className="flex items-center justify-between border-b border-border py-3">
        <p className="text-xs text-muted-foreground">{t("settings", "runtimeProxyRestartHint")}</p>
        <Button
          size="sm"
          variant="outline"
          onClick={() => void handleSaveRuntimeProxy()}
          disabled={!runtimeProxyDirty}
          className="gap-1.5"
        >
          {t("settings", "runtimeProxySave")}
        </Button>
      </div>

      <div className="mt-6 border-t border-border pt-4">
        <RegistryMirrorsSection />
      </div>
    </div>
  );
}

function RuntimeDiagnostics({
  dockerStatus,
  runtimeInfo,
  loading,
  error,
  onRefresh,
}: {
  dockerStatus: DockerStatusResponse | null;
  runtimeInfo: RuntimeStatusResponse | null;
  loading: boolean;
  error: string | null;
  onRefresh: () => Promise<void>;
}) {
  const { t } = useI18n();

  return (
    <div
      className="mt-6 border-t border-border pt-4"
      data-testid="runtime-diagnostics"
    >
      <div className="mb-3 flex items-center justify-between gap-3">
        <div>
          <h3 className="text-sm font-medium text-foreground">
            {t("settings", "runtimeDiagnostics")}
          </h3>
          <p className="text-xs text-muted-foreground">
            {t("settings", "runtimeDiagnosticsDesc")}
          </p>
        </div>
        <Button
          size="sm"
          variant="ghost"
          className="gap-1.5 text-xs"
          onClick={() => void onRefresh()}
          disabled={loading}
        >
          {loading ? (
            <Loader2 size={12} className="animate-spin" />
          ) : (
            <RefreshCw size={12} />
          )}
          {t("settings", "refreshDiagnostics")}
        </Button>
      </div>

      {error !== null && (
        <div className="mb-3 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      )}

      <div className="grid gap-3 md:grid-cols-2">
        <div className="rounded-md border border-border bg-muted/20 px-3 py-2">
          <div className="text-xs font-medium text-foreground">
            {t("settings", "dockerDiagnostics")}
          </div>
          <div className="mt-2">
            <DiagnosticsRow
              label={t("common", "status")}
              value={
                dockerStatus === null
                  ? "—"
                  : dockerStatus.connected
                    ? t("common", "connected")
                    : t("common", "disconnected")
              }
            />
            <DiagnosticsRow
              label={t("settings", "dockerVersion")}
              value={valueOrDash(dockerStatus?.version)}
            />
            <DiagnosticsRow
              label={t("settings", "dockerApiVersion")}
              value={valueOrDash(dockerStatus?.api_version)}
            />
            <DiagnosticsRow
              label={t("settings", "dockerOsArch")}
              value={formatOsArch(dockerStatus?.os, dockerStatus?.arch)}
            />
            <DiagnosticsRow
              label={t("settings", "dockerSource")}
              value={formatDockerSource(dockerStatus?.source, t("settings", "dockerSourceBuiltin"))}
            />
            <DiagnosticsRow
              label={t("settings", "dockerSocketPath")}
              value={valueOrDash(dockerStatus?.socket_path)}
              monospace
            />
          </div>
        </div>

        <div className="rounded-md border border-border bg-muted/20 px-3 py-2">
          <div className="text-xs font-medium text-foreground">
            {t("settings", "dockerSourceBuiltin")}
          </div>
          <div className="mt-2">
            <DiagnosticsRow
              label={t("common", "status")}
              value={valueOrDash(runtimeInfo?.state)}
            />
            <DiagnosticsRow
              label={t("settings", "runtimePlatform")}
              value={valueOrDash(runtimeInfo?.platform)}
            />
            <DiagnosticsRow
              label={t("settings", "runtimeCpuCores")}
              value={runtimeInfo?.cpu_cores ?? "—"}
            />
            <DiagnosticsRow
              label={t("settings", "runtimeMemoryMb")}
              value={runtimeInfo?.memory_mb !== undefined && runtimeInfo?.memory_mb !== null ? `${runtimeInfo.memory_mb} MB` : "—"}
            />
            <DiagnosticsRow
              label={t("settings", "runtimeDiskGb")}
              value={runtimeInfo?.disk_gb !== undefined && runtimeInfo?.disk_gb !== null ? `${runtimeInfo.disk_gb} GB` : "—"}
            />
            <DiagnosticsRow
              label={t("settings", "runtimeDockerResponsive")}
              value={
                runtimeInfo?.docker_responsive === undefined || runtimeInfo?.docker_responsive === null
                  ? "—"
                  : runtimeInfo.docker_responsive
                    ? t("common", "connected")
                    : t("common", "disconnected")
              }
            />
            <DiagnosticsRow
              label={t("settings", "runtimeUptime")}
              value={formatUptime(runtimeInfo?.uptime_seconds)}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

function DiagnosticsRow({
  label,
  value,
  monospace = false,
}: {
  label: string;
  value: React.ReactNode;
  monospace?: boolean;
}) {
  return (
    <div className="grid grid-cols-[112px_minmax(0,1fr)] gap-2 border-t border-border/60 py-1.5 first:border-t-0">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span
        className={`min-w-0 break-all text-xs text-foreground ${
          monospace ? "font-mono" : ""
        }`}
      >
        {value}
      </span>
    </div>
  );
}

function valueOrDash(value: string | null | undefined): string {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : "—";
}

function formatOsArch(os: string | null | undefined, arch: string | null | undefined): string {
  const parts = [valueOrDash(os), valueOrDash(arch)].filter((part) => part !== "—");
  return parts.length > 0 ? parts.join(" / ") : "—";
}

function formatDockerSource(source: string | null | undefined, builtinLabel: string): string {
  const value = valueOrDash(source);
  if (value === "—") return value;
  return /^(builtin|built-in|runtime)$/i.test(value) ? builtinLabel : value;
}

function formatUptime(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds)) {
    return "—";
  }
  if (seconds < 60) return `${seconds}s`;

  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = seconds % 60;
  if (minutes < 60) return `${minutes}m ${remainingSeconds}s`;

  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}h ${remainingMinutes}m`;
}

function formatError(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return String(error);
}

function RegistryMirrorsSection() {
  const { t } = useI18n();
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  const [newMirror, setNewMirror] = useState("");
  const mirrors = settings.registryMirrors;

  const addMirror = () => {
    const trimmed = newMirror.trim().replace(/^https?:\/\//, "").replace(/\/+$/, "");
    if (!trimmed || mirrors.includes(trimmed)) {
      setNewMirror("");
      return;
    }
    void updateSettings({ registryMirrors: [...mirrors, trimmed] });
    setNewMirror("");
  };

  const removeMirror = (index: number) => {
    void updateSettings({ registryMirrors: mirrors.filter((_, i) => i !== index) });
  };

  const resetToDefaults = () => {
    void updateSettings({ registryMirrors: [...DEFAULT_REGISTRY_MIRRORS] });
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-foreground">
            {t("settings", "registryMirrors")}
          </h3>
          <p className="text-xs text-muted-foreground">
            {t("settings", "registryMirrorsDesc")}
          </p>
        </div>
        <Button
          size="sm"
          variant="ghost"
          className="gap-1.5 text-xs"
          onClick={resetToDefaults}
        >
          <RotateCcw size={12} />
          {t("settings", "restoreDefaults")}
        </Button>
      </div>

      <div className="flex flex-col gap-1.5">
        {mirrors.length === 0 ? (
          <div className="rounded-md border border-dashed border-border py-4 text-center text-xs text-muted-foreground">
            {t("settings", "registryMirrorsEmpty")}
          </div>
        ) : (
          mirrors.map((mirror, index) => (
            <div
              key={`${mirror}-${index}`}
              className="group flex items-center justify-between rounded-md border border-border bg-muted/30 px-3 py-2"
            >
              <div className="flex items-center gap-2">
                <span className="w-5 font-mono text-xs text-muted-foreground">
                  {index + 1}.
                </span>
                <span className="font-mono text-sm text-foreground">{mirror}</span>
              </div>
              <button
                type="button"
                onClick={() => removeMirror(index)}
                className="text-muted-foreground opacity-0 transition-all hover:text-destructive group-hover:opacity-100"
              >
                <X size={14} />
              </button>
            </div>
          ))
        )}
      </div>

      <div className="flex gap-2">
        <Input
          value={newMirror}
          onChange={(e) => setNewMirror(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              addMirror();
            }
          }}
          placeholder={t("settings", "registryMirrorPlaceholder")}
          className="flex-1 font-mono text-sm"
        />
        <Button
          size="sm"
          variant="outline"
          onClick={addMirror}
          disabled={!newMirror.trim()}
          className="gap-1.5"
        >
          <Plus size={14} />
          {t("common", "add")}
        </Button>
      </div>

      <p className="text-xs text-muted-foreground">
        {t("settings", "registryMirrorsHint")}
      </p>
    </div>
  );
}

function AboutTab() {
  const { t } = useI18n();

  return (
    <div className="flex max-w-2xl flex-col">
      <div className="flex items-center gap-4 border-b border-border pb-6">
        <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-primary/10">
          <Package size={24} className="text-primary" />
        </div>
        <div>
          <h2 className="text-xl font-bold text-foreground">CrateBay</h2>
          <p className="text-sm text-muted-foreground">
            {t("settings", "aboutSubtitle")}
          </p>
        </div>
      </div>

      <SettingRow label={t("common", "version")}>
        <span className="font-mono text-sm text-muted-foreground">v{APP_VERSION}</span>
      </SettingRow>

      <SettingRow label={t("settings", "builtWith")}>
        <span className="text-sm text-muted-foreground">
          Tauri v2 + React + TypeScript
        </span>
      </SettingRow>

      <SettingRow label={t("settings", "license")}>
        <span className="text-sm text-muted-foreground">MIT License</span>
      </SettingRow>

      <div className="flex gap-3 pt-6">
        <a
          href="https://github.com/nicepkg/CrateBay"
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-1.5 rounded-md bg-muted px-3 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
        >
          <ExternalLink size={14} />
          {t("settings", "github")}
        </a>
      </div>
    </div>
  );
}

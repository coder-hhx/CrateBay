import { useCallback, useEffect, useState } from "react";
import { invoke } from "@/lib/tauri";
import {
  engineEndpointApiVersion,
  engineEndpointSocketPath,
  engineEndpointVersion,
  engineEndpointSource,
  normalizeRuntimeResourceUsage,
  runtimeBackendRuntime,
  runtimeCpuCores,
  runtimeDiskGb,
  runtimeEngineName,
  runtimeEngineResponsive,
  runtimeMemoryMb,
  runtimeNetworkStack,
  runtimeOciRuntime,
  runtimeUptimeSeconds,
  syncRuntimeStoreState,
  type EngineEndpointStatusResponse,
  type RuntimeStatusResponse,
} from "@/lib/runtimeStatus";
import { useSettingsStore } from "@/stores/settingsStore";
import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/lib/i18n";
import {
  DEFAULT_REGISTRY_MIRRORS,
  DEFAULT_RUNTIME_HTTP_PROXY,
  DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST,
  DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT,
  DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE,
  DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST,
} from "@/types/settings";
import { APP_VERSION } from "@/lib/constants";
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
  AlertTriangle,
  CheckCircle2,
  Download,
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

type AppUpdateCheckResult = {
  configured: boolean;
  available: boolean;
  currentVersion: string;
  version?: string | null;
  date?: string | null;
  body?: string | null;
  channel: "stable" | "prerelease";
  releaseTag?: string | null;
  releaseName?: string | null;
  releaseUrl?: string | null;
  repository: string;
  message?: string | null;
};

type UpdateState =
  | { status: "checking"; result?: AppUpdateCheckResult }
  | { status: "ready"; result: AppUpdateCheckResult }
  | { status: "installing"; result: AppUpdateCheckResult }
  | { status: "installed"; result: AppUpdateCheckResult }
  | { status: "restarting"; result: AppUpdateCheckResult }
  | { status: "error"; result?: AppUpdateCheckResult; message: string };

type EngineMaintenanceSnapshot = {
  contract: Record<string, unknown> | null;
  substrate: Record<string, unknown> | null;
  storageGc: Record<string, unknown> | null;
  shimTasks: Record<string, unknown> | null;
};

type RuntimeDiagnosticsSnapshot = {
  ok: boolean;
  runtime: RuntimeStatusResponse;
  engineContract: DiagnosticSectionPayload;
  substrate: DiagnosticSectionPayload;
  storageGc: DiagnosticSectionPayload;
  shimTasks: DiagnosticSectionPayload;
  generatedAtUnix: number;
};

type DiagnosticSectionPayload = {
  ok: boolean;
  value?: Record<string, unknown> | null;
  error?: string | null;
};

type EngineMaintenanceResult =
  | {
      type: "refresh";
      candidateCount: number;
      reclaimableBytes: number;
      shimTaskCount: number;
      at: Date;
    }
  | {
      type: "storage-gc";
      candidateCount: number;
      reclaimableBytes: number;
      at: Date;
    }
  | {
      type: "shim-reap";
      id: string;
      reclaimableBytes: number;
      remainingTasks: number;
      at: Date;
    };

type RuntimeOperationResult = {
  action: "start" | "stop" | "restart" | "provision" | "proxy";
  status: "success" | "error";
  title: string;
  message: string;
  runtimeState: string;
  endpoint: string;
  at: Date;
};

const DEFAULT_RUNTIME_PROXY_SETTINGS = {
  runtimeHttpProxy: DEFAULT_RUNTIME_HTTP_PROXY,
  runtimeHttpProxyBridge: DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE,
  runtimeHttpProxyBindHost: DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST,
  runtimeHttpProxyBindPort: DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT,
  runtimeHttpProxyGuestHost: DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST,
};

export function SettingsPage() {
  const { t } = useI18n();

  return (
    <div className="flex h-full flex-col overflow-auto" data-testid="settings-page">
      <header className="border-b border-border px-6 py-4">
        <h1 className="text-sm font-semibold">{t("settings", "title")}</h1>
        <p className="mt-1 text-xs text-muted-foreground">{t("settings", "subtitle")}</p>
      </header>

      <div className="grid gap-x-6 px-6 py-5 xl:grid-cols-[minmax(0,0.9fr)_minmax(420px,1.1fr)]">
        <div>
          <SettingsPanel title={t("settings", "general")} dataTestId="settings-section-general">
          <GeneralTab />
          </SettingsPanel>
          <SettingsPanel title={t("settings", "updates")} dataTestId="settings-section-updates">
            <UpdatesPanel />
          </SettingsPanel>
          <SettingsPanel title={t("settings", "about")} dataTestId="settings-section-about">
            <AboutTab />
          </SettingsPanel>
        </div>

        <div>
          <SettingsPanel title={t("settings", "runtime")} dataTestId="settings-section-runtime">
          <RuntimeTab />
          </SettingsPanel>
        </div>
      </div>
    </div>
  );
}

function SettingsPanel({
  title,
  children,
  dataTestId,
}: {
  title: string;
  children: React.ReactNode;
  dataTestId: string;
}) {
  return (
    <section className="border-t border-border py-4 first:border-t-0" data-testid={dataTestId}>
      <h2 className="mb-2 text-xs font-semibold uppercase text-muted-foreground">{title}</h2>
      {children}
    </section>
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
  const engineConnected = useAppStore((s) => s.engineConnected);
  const runtimeLoading = useAppStore((s) => s.runtimeLoading);
  const setRuntimeLoading = useAppStore((s) => s.setRuntimeLoading);
  const addNotification = useAppStore((s) => s.addNotification);
  const [proxyInput, setProxyInput] = useState(settings.runtimeHttpProxy);
  const [proxyBridge, setProxyBridge] = useState(settings.runtimeHttpProxyBridge);
  const [proxyBindHost, setProxyBindHost] = useState(settings.runtimeHttpProxyBindHost);
  const [proxyBindPort, setProxyBindPort] = useState(String(settings.runtimeHttpProxyBindPort));
  const [proxyGuestHost, setProxyGuestHost] = useState(settings.runtimeHttpProxyGuestHost);
  const [endpointStatusInfo, setEndpointStatusInfo] = useState<EngineEndpointStatusResponse | null>(null);
  const [runtimeStatusInfo, setRuntimeStatusInfo] = useState<RuntimeStatusResponse | null>(null);
  const [diagnosticsLoading, setDiagnosticsLoading] = useState(false);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [runtimeOperationResult, setRuntimeOperationResult] = useState<RuntimeOperationResult | null>(null);

  useEffect(() => {
    setProxyInput(settings.runtimeHttpProxy);
    setProxyBridge(settings.runtimeHttpProxyBridge);
    setProxyBindHost(settings.runtimeHttpProxyBindHost);
    setProxyBindPort(String(settings.runtimeHttpProxyBindPort));
    setProxyGuestHost(settings.runtimeHttpProxyGuestHost);
  }, [
    settings.runtimeHttpProxy,
    settings.runtimeHttpProxyBridge,
    settings.runtimeHttpProxyBindHost,
    settings.runtimeHttpProxyBindPort,
    settings.runtimeHttpProxyGuestHost,
  ]);

  const loadDiagnostics = useCallback(async () => {
    setDiagnosticsLoading(true);
    setDiagnosticsError(null);
    try {
      const [endpointStatus, runtimeInfo] = await Promise.all([
        invoke<EngineEndpointStatusResponse | null>("engine_status"),
        invoke<RuntimeStatusResponse | null>("runtime_status"),
      ]);
      setEndpointStatusInfo(endpointStatus ?? null);
      setRuntimeStatusInfo(runtimeInfo ?? null);
      syncRuntimeStoreState(endpointStatus, runtimeInfo);
      return {
        endpointStatus: endpointStatus ?? null,
        runtimeInfo: runtimeInfo ?? null,
      };
    } catch (error) {
      setEndpointStatusInfo(null);
      setRuntimeStatusInfo(null);
      setDiagnosticsError(formatError(error));
      return {
        endpointStatus: null,
        runtimeInfo: null,
      };
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
      setRuntimeOperationResult(null);
      await invoke("runtime_start");
      const diagnostics = await loadDiagnostics();
      setRuntimeOperationResult(createRuntimeOperationResult({
        action: "start",
        status: "success",
        title: t("settings", "runtimeStartComplete"),
        message: t("settings", "runtimeStartCompleteDesc"),
        endpointStatus: diagnostics.endpointStatus,
        runtimeInfo: diagnostics.runtimeInfo,
      }));
      addNotification({
        type: "success",
        title: t("settings", "runtimeStarting"),
        dismissable: true,
      });
    } catch (error) {
      setRuntimeOperationResult(createRuntimeOperationResult({
        action: "start",
        status: "error",
        title: t("settings", "runtimeStartFailed"),
        message: formatError(error),
        endpointStatus: null,
        runtimeInfo: runtimeStatusInfo,
      }));
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

  const handleRuntimeProvision = async () => {
    try {
      setRuntimeLoading(true);
      setRuntimeOperationResult(null);
      await invoke("runtime_provision");
      const diagnostics = await loadDiagnostics();
      setRuntimeOperationResult(createRuntimeOperationResult({
        action: "provision",
        status: "success",
        title: t("settings", "runtimeProvisionComplete"),
        message: t("settings", "runtimeProvisionCompleteDesc"),
        endpointStatus: diagnostics.endpointStatus,
        runtimeInfo: diagnostics.runtimeInfo,
      }));
      addNotification({
        type: "success",
        title: t("settings", "runtimeProvisionComplete"),
        dismissable: true,
      });
    } catch (error) {
      setRuntimeOperationResult(createRuntimeOperationResult({
        action: "provision",
        status: "error",
        title: t("settings", "runtimeProvisionFailed"),
        message: formatError(error),
        endpointStatus: endpointStatusInfo,
        runtimeInfo: runtimeStatusInfo,
      }));
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
      setRuntimeOperationResult(null);
      await invoke("runtime_stop");
      const diagnostics = await loadDiagnostics();
      setRuntimeOperationResult(createRuntimeOperationResult({
        action: "stop",
        status: "success",
        title: t("settings", "runtimeStopComplete"),
        message: t("settings", "runtimeStopCompleteDesc"),
        endpointStatus: diagnostics.endpointStatus,
        runtimeInfo: diagnostics.runtimeInfo,
      }));
      addNotification({
        type: "success",
        title: t("settings", "runtimeStopped"),
        dismissable: true,
      });
    } catch (error) {
      setRuntimeOperationResult(createRuntimeOperationResult({
        action: "stop",
        status: "error",
        title: t("settings", "runtimeStopFailed"),
        message: formatError(error),
        endpointStatus: endpointStatusInfo,
        runtimeInfo: runtimeStatusInfo,
      }));
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
      setRuntimeOperationResult(null);
      await invoke("runtime_restart");
      const diagnostics = await loadDiagnostics();
      setRuntimeOperationResult(createRuntimeOperationResult({
        action: "restart",
        status: "success",
        title: t("settings", "runtimeRestartComplete"),
        message: t("settings", "runtimeRestartCompleteDesc"),
        endpointStatus: diagnostics.endpointStatus,
        runtimeInfo: diagnostics.runtimeInfo,
      }));
      addNotification({
        type: "success",
        title: t("settings", "runtimeRestart"),
        message: t("settings", "runtimeProxyRestartHint"),
        dismissable: true,
      });
    } catch (error) {
      setRuntimeOperationResult(createRuntimeOperationResult({
        action: "restart",
        status: "error",
        title: t("settings", "runtimeRestartFailed"),
        message: formatError(error),
        endpointStatus: endpointStatusInfo,
        runtimeInfo: runtimeStatusInfo,
      }));
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
      const parsedPort = Number(proxyBindPort);
      if (!Number.isInteger(parsedPort) || parsedPort <= 0 || parsedPort > 65535) {
        throw new Error(t("settings", "runtimeProxyInvalidPort"));
      }
      await updateSettings({
        runtimeHttpProxy: proxyInput.trim(),
        runtimeHttpProxyBridge: proxyBridge,
        runtimeHttpProxyBindHost:
          proxyBindHost.trim() || DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST,
        runtimeHttpProxyBindPort: parsedPort,
        runtimeHttpProxyGuestHost:
          proxyGuestHost.trim() || DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST,
      });
      setRuntimeOperationResult(createRuntimeOperationResult({
        action: "proxy",
        status: "success",
        title: t("settings", "runtimeProxySaved"),
        message: t("settings", "runtimeProxyRestartHint"),
        endpointStatus: endpointStatusInfo,
        runtimeInfo: runtimeStatusInfo,
      }));
      addNotification({
        type: "success",
        title: t("settings", "runtimeProxySaveSuccess"),
        message: t("settings", "runtimeProxyRestartHint"),
        dismissable: true,
      });
    } catch (error) {
      setRuntimeOperationResult(createRuntimeOperationResult({
        action: "proxy",
        status: "error",
        title: t("settings", "runtimeProxySaveFailed"),
        message: formatError(error),
        endpointStatus: endpointStatusInfo,
        runtimeInfo: runtimeStatusInfo,
      }));
      addNotification({
        type: "error",
        title: t("common", "error"),
        message: error instanceof Error ? error.message : String(error),
        dismissable: true,
      });
    }
  };

  const handleResetRuntimeProxy = async () => {
    try {
      await updateSettings(DEFAULT_RUNTIME_PROXY_SETTINGS);
      setRuntimeOperationResult(createRuntimeOperationResult({
        action: "proxy",
        status: "success",
        title: t("settings", "runtimeProxyCleared"),
        message: t("settings", "runtimeProxyRestartHint"),
        endpointStatus: endpointStatusInfo,
        runtimeInfo: runtimeStatusInfo,
      }));
      addNotification({
        type: "success",
        title: t("settings", "runtimeProxyCleared"),
        message: t("settings", "runtimeProxyRestartHint"),
        dismissable: true,
      });
    } catch (error) {
      setRuntimeOperationResult(createRuntimeOperationResult({
        action: "proxy",
        status: "error",
        title: t("settings", "runtimeProxySaveFailed"),
        message: formatError(error),
        endpointStatus: endpointStatusInfo,
        runtimeInfo: runtimeStatusInfo,
      }));
      addNotification({
        type: "error",
        title: t("common", "error"),
        message: error instanceof Error ? error.message : String(error),
        dismissable: true,
      });
    }
  };

  const runtimeProxyDirty =
    proxyInput.trim() !== settings.runtimeHttpProxy ||
    proxyBridge !== settings.runtimeHttpProxyBridge ||
    proxyBindHost.trim() !== settings.runtimeHttpProxyBindHost ||
    Number(proxyBindPort) !== settings.runtimeHttpProxyBindPort ||
    proxyGuestHost.trim() !== settings.runtimeHttpProxyGuestHost;
  const runtimeProxyDefaulted =
    settings.runtimeHttpProxy === DEFAULT_RUNTIME_HTTP_PROXY &&
    settings.runtimeHttpProxyBridge === DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE &&
    settings.runtimeHttpProxyBindHost === DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST &&
    settings.runtimeHttpProxyBindPort === DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT &&
    settings.runtimeHttpProxyGuestHost === DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST;
  const displayStatus = engineConnected
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
              engineConnected
                ? "text-green-500"
                : runtimeStatus === "error"
                  ? "text-red-500"
                  : "text-muted-foreground"
            }`}
          >
            {engineConnected
              ? t("settings", "engineSourceBuiltin")
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
            onClick={() => void handleRuntimeProvision()}
            disabled={runtimeLoading || runtimeStatus === "running"}
            size="sm"
            variant="outline"
            className="gap-1.5"
          >
            <Download size={14} />
            {t("settings", "runtimeProvision")}
          </Button>
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

      {runtimeOperationResult !== null && (
        <RuntimeOperationResultBanner result={runtimeOperationResult} />
      )}

      <RuntimeDiagnostics
        endpointStatus={endpointStatusInfo}
        runtimeInfo={runtimeStatusInfo}
        loading={diagnosticsLoading}
        error={diagnosticsError}
        onRefresh={async () => {
          await loadDiagnostics();
        }}
      />

      <EngineMaintenancePanel />

      <SettingRow
        label={t("settings", "runtimeHttpProxy")}
        description={t("settings", "runtimeHttpProxyDesc")}
      >
        <div className="flex flex-col items-end gap-2">
          <Input
            value={proxyInput}
            onChange={(e) => setProxyInput(e.target.value)}
            placeholder="127.0.0.1:7890"
            className="w-64 font-mono text-xs"
          />
          <label className="flex items-center gap-2 text-xs text-muted-foreground">
            <input
              type="checkbox"
              className="h-4 w-4 accent-primary"
              checked={proxyBridge}
              onChange={(event) => setProxyBridge(event.currentTarget.checked)}
            />
            {t("settings", "runtimeHttpProxyBridge")}
          </label>
        </div>
      </SettingRow>

      <SettingRow
        label={t("settings", "runtimeHttpProxyBindHost")}
        description={t("settings", "runtimeHttpProxyBridgeDesc")}
      >
        <Input
          value={proxyBindHost}
          onChange={(e) => setProxyBindHost(e.target.value)}
          placeholder="0.0.0.0"
          className="w-40 font-mono text-xs"
        />
      </SettingRow>

      <SettingRow
        label={t("settings", "runtimeHttpProxyBindPort")}
        description={t("settings", "runtimeHttpProxyGuestHost")}
      >
        <div className="flex items-center gap-2">
          <Input
            value={proxyBindPort}
            onChange={(e) => setProxyBindPort(e.target.value)}
            placeholder="3128"
            inputMode="numeric"
            className="w-24 font-mono text-xs"
          />
          <Input
            value={proxyGuestHost}
            onChange={(e) => setProxyGuestHost(e.target.value)}
            placeholder="192.168.64.1"
            className="w-40 font-mono text-xs"
          />
        </div>
      </SettingRow>

      <div className="flex items-center justify-between border-b border-border py-3">
        <p className="text-xs text-muted-foreground">{t("settings", "runtimeProxyRestartHint")}</p>
        <div className="flex items-center gap-2">
          <Button
            size="sm"
            variant="ghost"
            onClick={() => void handleResetRuntimeProxy()}
            disabled={!runtimeProxyDirty && runtimeProxyDefaulted}
            className="gap-1.5"
          >
            <RotateCcw size={14} />
            {t("settings", "runtimeProxyReset")}
          </Button>
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
      </div>

      <div className="mt-6 border-t border-border pt-4">
        <RegistryMirrorsSection />
      </div>
    </div>
  );
}

function RuntimeOperationResultBanner({ result }: { result: RuntimeOperationResult }) {
  const { t } = useI18n();
  const isError = result.status === "error";

  return (
    <div
      className={`mt-3 rounded-md border px-3 py-2 text-xs ${
        isError
          ? "border-destructive/30 bg-destructive/10 text-destructive"
          : "border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
      }`}
      data-testid="runtime-operation-result"
    >
      <div className="flex items-center gap-2 font-medium">
        {isError ? <AlertTriangle size={13} /> : <CheckCircle2 size={13} />}
        <span>{result.title}</span>
      </div>
      <p className="mt-1 text-muted-foreground">{result.message}</p>
      <div className="mt-1 grid gap-1 text-muted-foreground sm:grid-cols-3">
        <span>
          {t("settings", "runtimeOperationState")}: {result.runtimeState}
        </span>
        <span className="min-w-0 truncate" title={result.endpoint}>
          {t("settings", "runtimeOperationEndpoint")}: {result.endpoint}
        </span>
        <span>
          {t("settings", "completedAt")}: {formatClockTime(result.at)}
        </span>
      </div>
    </div>
  );
}

function RuntimeDiagnostics({
  endpointStatus,
  runtimeInfo,
  loading,
  error,
  onRefresh,
}: {
  endpointStatus: EngineEndpointStatusResponse | null;
  runtimeInfo: RuntimeStatusResponse | null;
  loading: boolean;
  error: string | null;
  onRefresh: () => Promise<void>;
}) {
  const { t } = useI18n();
  const engineResponsive = runtimeInfo === null ? null : runtimeEngineResponsive(runtimeInfo);
  const cpuCores = runtimeCpuCores(runtimeInfo);
  const memoryMb = runtimeMemoryMb(runtimeInfo);
  const diskGb = runtimeDiskGb(runtimeInfo);
  const usage = normalizeRuntimeResourceUsage(runtimeInfo);

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
            {t("settings", "engineEndpointDiagnostics")}
          </div>
          <div className="mt-2">
            <DiagnosticsRow
              label={t("common", "status")}
              value={
                endpointStatus === null
                  ? "—"
                  : endpointStatus.connected
                    ? t("common", "connected")
                    : t("common", "disconnected")
              }
            />
            <DiagnosticsRow
              label={t("settings", "engineVersion")}
              value={valueOrDash(engineEndpointVersion(endpointStatus))}
            />
            <DiagnosticsRow
              label={t("settings", "compatibilityApi")}
              value={valueOrDash(engineEndpointApiVersion(endpointStatus))}
            />
            <DiagnosticsRow
              label={t("settings", "endpointOsArch")}
              value={formatOsArch(endpointStatus?.os, endpointStatus?.arch)}
            />
            <DiagnosticsRow
              label={t("settings", "endpointSource")}
              value={formatEngineSource(engineEndpointSource(endpointStatus), t("settings", "engineSourceBuiltin"))}
            />
            <DiagnosticsRow
              label={t("settings", "engineEndpoint")}
              value={valueOrDash(engineEndpointSocketPath(endpointStatus))}
              monospace
            />
          </div>
        </div>

        <div className="rounded-md border border-border bg-muted/20 px-3 py-2">
          <div className="text-xs font-medium text-foreground">
            {t("settings", "engineSourceBuiltin")}
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
              label={t("settings", "engine")}
              value={runtimeInfo === null ? "—" : runtimeEngineName(runtimeInfo)}
            />
            <DiagnosticsRow
              label={t("settings", "backendRuntime")}
              value={runtimeInfo === null ? "—" : runtimeBackendRuntime(runtimeInfo)}
            />
            <DiagnosticsRow
              label={t("settings", "ociRuntime")}
              value={runtimeInfo === null ? "—" : runtimeOciRuntime(runtimeInfo)}
            />
            <DiagnosticsRow
              label={t("settings", "networkStack")}
              value={runtimeInfo === null ? "—" : runtimeNetworkStack(runtimeInfo)}
            />
            <DiagnosticsRow
              label={t("settings", "runtimeCpuCores")}
              value={runtimeInfo === null ? "—" : cpuCores}
            />
            <DiagnosticsRow
              label={t("settings", "runtimeMemoryMb")}
              value={runtimeInfo === null ? "—" : `${memoryMb} MB`}
            />
            <DiagnosticsRow
              label={t("settings", "runtimeDiskGb")}
              value={runtimeInfo === null ? "—" : `${diskGb} GB`}
            />
            <DiagnosticsRow
              label={t("settings", "runtimeCpuUsage")}
              value={usage === null ? "—" : `${formatNumber(usage.cpuPercent)}%`}
            />
            <DiagnosticsRow
              label={t("settings", "runtimeMemoryUsage")}
              value={usage === null ? "—" : `${formatNumber(usage.memoryUsedMb)} / ${formatNumber(usage.memoryTotalMb)} MB`}
            />
            <DiagnosticsRow
              label={t("settings", "runtimeDiskUsage")}
              value={usage === null ? "—" : `${formatNumber(usage.diskUsedGb)} / ${formatNumber(usage.diskTotalGb)} GB`}
            />
            <DiagnosticsRow
              label={t("settings", "runtimeContainerCount")}
              value={usage?.containerCount === undefined ? "—" : String(usage.containerCount)}
            />
            <DiagnosticsRow
              label={t("settings", "runtimeEngineResponsive")}
              value={
                engineResponsive === null
                  ? "—"
                  : engineResponsive
                    ? t("common", "connected")
                    : t("common", "disconnected")
              }
            />
            <DiagnosticsRow
              label={t("settings", "runtimeUptime")}
              value={formatUptime(runtimeUptimeSeconds(runtimeInfo))}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

function EngineMaintenancePanel() {
  const { t } = useI18n();
  const [snapshot, setSnapshot] = useState<EngineMaintenanceSnapshot>({
    contract: null,
    substrate: null,
    storageGc: null,
    shimTasks: null,
  });
  const [loading, setLoading] = useState(false);
  const [applyingGc, setApplyingGc] = useState(false);
  const [reapingTaskId, setReapingTaskId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdatedAt, setLastUpdatedAt] = useState<Date | null>(null);
  const [operationResult, setOperationResult] = useState<EngineMaintenanceResult | null>(null);

  const loadMaintenance = useCallback(async (options?: { announce?: boolean }) => {
    setLoading(true);
    setError(null);
    try {
      const diagnostics = await invoke<RuntimeDiagnosticsSnapshot>("runtime_diagnostics", {
        prune_exited_containers: true,
      });
      const contract = diagnosticValue(diagnostics.engineContract);
      const substrate = diagnosticValue(diagnostics.substrate);
      const storageGc = diagnosticValue(diagnostics.storageGc);
      const shimTasks = diagnosticValue(diagnostics.shimTasks);
      setSnapshot({ contract, substrate, storageGc, shimTasks });
      const updatedAt = new Date();
      setLastUpdatedAt(updatedAt);
      if (options?.announce) {
        setOperationResult({
          type: "refresh",
          candidateCount: numberValue(storageGc, "candidateCount"),
          reclaimableBytes: numberValue(storageGc, "reclaimableBytes"),
          shimTaskCount: arrayValue(shimTasks, "items").length,
          at: updatedAt,
        });
      }
      if (!diagnostics.ok) {
        setError(diagnosticErrors(diagnostics));
      }
    } catch (err) {
      setSnapshot({ contract: null, substrate: null, storageGc: null, shimTasks: null });
      setError(formatError(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadMaintenance();
  }, [loadMaintenance]);

  const handleApplyStorageGc = useCallback(async () => {
    setApplyingGc(true);
    setError(null);
    setOperationResult(null);
    try {
      const storageGc = await invoke<Record<string, unknown>>("engine_storage_gc", {
        apply: true,
        prune_exited_containers: true,
      });
      const shimTasks = await invoke<Record<string, unknown>>("engine_shim_tasks");
      setSnapshot((prev) => ({ ...prev, storageGc, shimTasks }));
      const updatedAt = new Date();
      setLastUpdatedAt(updatedAt);
      setOperationResult({
        type: "storage-gc",
        candidateCount: numberValue(storageGc, "candidateCount"),
        reclaimableBytes: numberValue(storageGc, "reclaimableBytes"),
        at: updatedAt,
      });
    } catch (err) {
      setError(formatError(err));
    } finally {
      setApplyingGc(false);
    }
  }, []);

  const handleReapTask = useCallback(async (id: string) => {
    setReapingTaskId(id);
    setError(null);
    setOperationResult(null);
    try {
      const reapResult = await invoke<Record<string, unknown>>("engine_shim_reap_task", { id, apply: true });
      const [storageGc, shimTasks] = await Promise.all([
        invoke<Record<string, unknown>>("engine_storage_gc", {
          apply: false,
          prune_exited_containers: true,
        }),
        invoke<Record<string, unknown>>("engine_shim_tasks"),
      ]);
      setSnapshot((prev) => ({ ...prev, storageGc, shimTasks }));
      const updatedAt = new Date();
      setLastUpdatedAt(updatedAt);
      setOperationResult({
        type: "shim-reap",
        id,
        reclaimableBytes: numberValue(reapResult, "reclaimableBytes"),
        remainingTasks: arrayValue(shimTasks, "items").length,
        at: updatedAt,
      });
    } catch (err) {
      setError(formatError(err));
    } finally {
      setReapingTaskId(null);
    }
  }, []);

  const contract = snapshot.contract;
  const substrate = snapshot.substrate;
  const storageGc = snapshot.storageGc;
  const shimTasks = arrayValue(snapshot.shimTasks, "items");

  return (
    <div className="mt-6 border-t border-border pt-4" data-testid="engine-maintenance">
      <div className="mb-3 flex items-center justify-between gap-3">
        <div>
          <h3 className="text-sm font-medium text-foreground">
            {t("settings", "engineMaintenance")}
          </h3>
          <p className="text-xs text-muted-foreground">
            {t("settings", "engineMaintenanceDesc")}
          </p>
          {lastUpdatedAt !== null && (
            <p className="mt-1 text-[11px] text-muted-foreground">
              {t("settings", "lastUpdated")}: {formatClockTime(lastUpdatedAt)}
            </p>
          )}
        </div>
        <Button
          size="sm"
          variant="ghost"
          className="gap-1.5 text-xs"
          onClick={() => void loadMaintenance({ announce: true })}
          disabled={loading}
        >
          {loading ? (
            <Loader2 size={12} className="animate-spin" />
          ) : (
            <RefreshCw size={12} />
          )}
          {t("settings", "refreshMaintenance")}
        </Button>
      </div>

      {error !== null && (
        <div className="mb-3 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      )}

      {operationResult !== null && (
        <EngineMaintenanceResultBanner result={operationResult} />
      )}

      <div className="mb-3 rounded-md border border-border bg-muted/20 px-3 py-2">
        <div className="text-xs font-medium text-foreground">
          {t("settings", "engineContract")}
        </div>
        <div className="mt-2 grid gap-x-4 md:grid-cols-2">
          <DiagnosticsRow
            label={t("settings", "engineKind")}
            value={stringAt(contract, ["kind"]) ?? "—"}
          />
          <DiagnosticsRow
            label={t("settings", "nativeApi")}
            value={stringAt(contract, ["adapter", "api"]) ?? "—"}
            monospace
          />
          <DiagnosticsRow
            label={t("settings", "engineNamespace")}
            value={stringAt(contract, ["backend", "namespace"]) ?? "—"}
          />
          <DiagnosticsRow
            label={t("settings", "compatibilityApi")}
            value={
              boolAt(contract, ["compatibility", "dockerCompatible"])
                ? t("settings", "compatibilityEnabled")
                : t("settings", "compatibilityDisabled")
            }
          />
        </div>
      </div>

      <div className="grid gap-3 md:grid-cols-2">
        <div className="rounded-md border border-border bg-muted/20 px-3 py-2">
          <div className="text-xs font-medium text-foreground">
            {t("settings", "engineSubstrate")}
          </div>
          <div className="mt-2">
            <DiagnosticsRow
              label={t("settings", "engine")}
              value={stringAt(substrate, ["engine"]) ?? "—"}
            />
            <DiagnosticsRow
              label={t("settings", "backendRuntime")}
              value={stringAt(substrate, ["shim", "backend"]) ?? "—"}
            />
            <DiagnosticsRow
              label={t("settings", "networkStack")}
              value={stringAt(substrate, ["network", "stack"]) ?? "—"}
            />
            <DiagnosticsRow
              label={t("settings", "storageManager")}
              value={stringAt(substrate, ["storage", "manager"]) ?? "—"}
            />
            <DiagnosticsRow
              label={t("settings", "compatibilityEndpoint")}
              value={stringAt(substrate, ["daemon", "compatibilityEndpoint"]) ?? "—"}
              monospace
            />
          </div>
        </div>

        <div className="rounded-md border border-border bg-muted/20 px-3 py-2">
          <div className="flex items-center justify-between gap-3">
            <div className="text-xs font-medium text-foreground">
              {t("settings", "storageGc")}
            </div>
            <Button
              size="sm"
              variant="outline"
              className="h-7 gap-1.5 text-xs"
              onClick={() => void handleApplyStorageGc()}
              disabled={loading || applyingGc || storageGc === null}
            >
              {applyingGc ? <Loader2 size={12} className="animate-spin" /> : <Package size={12} />}
              {t("settings", "applyStorageGc")}
            </Button>
          </div>
          <div className="mt-2">
            <DiagnosticsRow
              label={t("settings", "gcMode")}
              value={boolValue(storageGc, "applied") ? t("settings", "applied") : t("settings", "dryRun")}
            />
            <DiagnosticsRow
              label={t("settings", "gcCandidates")}
              value={numberValue(storageGc, "candidateCount")}
            />
            <DiagnosticsRow
              label={t("settings", "gcReclaimable")}
              value={formatBytes(numberValue(storageGc, "reclaimableBytes"))}
            />
          </div>
        </div>
      </div>

      <div className="mt-3 rounded-md border border-border bg-muted/20 px-3 py-2">
        <div className="text-xs font-medium text-foreground">
          {t("settings", "shimTasks")}
        </div>
        {shimTasks.length === 0 ? (
          <div className="mt-2 rounded-md border border-dashed border-border py-4 text-center text-xs text-muted-foreground">
            {loading ? t("common", "loading") : t("settings", "noShimTasks")}
          </div>
        ) : (
          <div className="mt-2 overflow-hidden rounded-md border border-border">
            <div className="grid grid-cols-[minmax(120px,1fr)_minmax(120px,1.2fr)_90px_88px] bg-muted/50 px-2 py-1.5 text-[11px] font-medium uppercase text-muted-foreground">
              <span>ID</span>
              <span>{t("common", "name")}</span>
              <span>{t("common", "status")}</span>
              <span className="text-right">{t("common", "actions")}</span>
            </div>
            {shimTasks.map((task) => {
              const id = String(task.id ?? "");
              return (
                <div
                  key={id}
                  className="grid grid-cols-[minmax(120px,1fr)_minmax(120px,1.2fr)_90px_88px] items-center border-t border-border px-2 py-1.5 text-xs"
                >
                  <span className="truncate font-mono" title={id}>{shortId(id)}</span>
                  <span className="truncate" title={String(task.name ?? "")}>{String(task.name ?? "—")}</span>
                  <span className="text-muted-foreground">{String(task.state ?? "—")}</span>
                  <div className="text-right">
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-7 gap-1 px-2 text-xs"
                      onClick={() => void handleReapTask(id)}
                      disabled={id.length === 0 || reapingTaskId !== null}
                    >
                      {reapingTaskId === id ? <Loader2 size={12} className="animate-spin" /> : null}
                      {t("settings", "reapTask")}
                    </Button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

function EngineMaintenanceResultBanner({ result }: { result: EngineMaintenanceResult }) {
  const { t } = useI18n();

  if (result.type === "shim-reap") {
    return (
      <div
        className="mb-3 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-xs"
        data-testid="engine-maintenance-result"
      >
        <div className="flex items-center gap-2 font-medium text-emerald-600 dark:text-emerald-400">
          <CheckCircle2 size={13} />
          <span>
            {t("settings", "shimTaskReaped")} {shortId(result.id)}
          </span>
        </div>
        <div className="mt-1 grid gap-1 text-muted-foreground sm:grid-cols-3">
          <span>
            {t("settings", "gcReclaimable")}: {formatBytes(result.reclaimableBytes)}
          </span>
          <span>
            {t("settings", "remainingShimTasks")}: {result.remainingTasks}
          </span>
          <span>
            {t("settings", "completedAt")}: {formatClockTime(result.at)}
          </span>
        </div>
      </div>
    );
  }

  const title =
    result.type === "storage-gc"
      ? t("settings", "storageGcComplete")
      : t("settings", "maintenanceRefreshed");

  return (
    <div
      className="mb-3 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-xs"
      data-testid="engine-maintenance-result"
    >
      <div className="flex items-center gap-2 font-medium text-emerald-600 dark:text-emerald-400">
        <CheckCircle2 size={13} />
        <span>{title}</span>
      </div>
      <div className="mt-1 grid gap-1 text-muted-foreground sm:grid-cols-3">
        <span>
          {t("settings", "gcCandidates")}: {result.candidateCount}
        </span>
        <span>
          {t("settings", "gcReclaimable")}: {formatBytes(result.reclaimableBytes)}
        </span>
        <span>
          {result.type === "refresh"
            ? `${t("settings", "shimTasks")}: ${result.shimTaskCount}`
            : `${t("settings", "completedAt")}: ${formatClockTime(result.at)}`}
        </span>
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

function createRuntimeOperationResult({
  action,
  status,
  title,
  message,
  endpointStatus,
  runtimeInfo,
}: {
  action: RuntimeOperationResult["action"];
  status: RuntimeOperationResult["status"];
  title: string;
  message: string;
  endpointStatus: EngineEndpointStatusResponse | null;
  runtimeInfo: RuntimeStatusResponse | null;
}): RuntimeOperationResult {
  return {
    action,
    status,
    title,
    message,
    runtimeState: valueOrDash(runtimeInfo?.state),
    endpoint: valueOrDash(engineEndpointSocketPath(endpointStatus)),
    at: new Date(),
  };
}

function formatEngineSource(source: string | null | undefined, builtinLabel: string): string {
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

function formatNumber(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

function arrayValue(value: Record<string, unknown> | null, key: string): Array<Record<string, unknown>> {
  const items = value?.[key];
  return Array.isArray(items)
    ? items.filter((item): item is Record<string, unknown> => isRecord(item))
    : [];
}

function diagnosticValue(section: DiagnosticSectionPayload): Record<string, unknown> | null {
  return isRecord(section.value) ? section.value : null;
}

function diagnosticErrors(snapshot: RuntimeDiagnosticsSnapshot): string {
  const errors = [
    ["Engine contract", snapshot.engineContract.error],
    ["Substrate", snapshot.substrate.error],
    ["Storage GC", snapshot.storageGc.error],
    ["Shim tasks", snapshot.shimTasks.error],
  ]
    .filter((entry): entry is [string, string] => typeof entry[1] === "string" && entry[1].length > 0)
    .map(([label, error]) => `${label}: ${error}`);

  return errors.length > 0 ? errors.join("; ") : "Runtime diagnostics are incomplete.";
}

function stringAt(value: Record<string, unknown> | null, path: string[]): string | null {
  let current: unknown = value;
  for (const segment of path) {
    if (!isRecord(current)) return null;
    current = current[segment];
  }
  return typeof current === "string" && current.length > 0 ? current : null;
}

function numberValue(value: Record<string, unknown> | null, key: string): number {
  const item = value?.[key];
  return typeof item === "number" && Number.isFinite(item) ? item : 0;
}

function boolValue(value: Record<string, unknown> | null, key: string): boolean {
  return value?.[key] === true;
}

function boolAt(value: Record<string, unknown> | null, path: string[]): boolean {
  let current: unknown = value;
  for (const segment of path) {
    if (!isRecord(current)) return false;
    current = current[segment];
  }
  return current === true;
}

function shortId(value: string): string {
  return value.length > 12 ? value.slice(0, 12) : value || "—";
}

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
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

function UpdatesPanel() {
  const { t } = useI18n();
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  const [state, setState] = useState<UpdateState>({ status: "checking" });

  const runCheck = useCallback(async () => {
    setState((prev) => ({ status: "checking", result: "result" in prev ? prev.result : undefined }));
    try {
      const result = await invoke<AppUpdateCheckResult>("app_update_check", {
        include_prerelease: settings.includePrereleases,
      });
      setState({ status: "ready", result });
    } catch (error) {
      setState((prev) => ({
        status: "error",
        result: "result" in prev ? prev.result : undefined,
        message: formatError(error),
      }));
    }
  }, [settings.includePrereleases]);

  useEffect(() => {
    void runCheck();
  }, [runCheck]);

  const install = async () => {
    if (state.status !== "ready" || !state.result.available) return;
    setState({ status: "installing", result: state.result });
    try {
      const result = await invoke<AppUpdateCheckResult>("app_update_install", {
        include_prerelease: settings.includePrereleases,
      });
      setState({ status: "installed", result });
    } catch (error) {
      setState({ status: "error", result: state.result, message: formatError(error) });
    }
  };

  const restart = async () => {
    if (state.status !== "installed") return;
    setState({ status: "restarting", result: state.result });
    try {
      await invoke("app_restart");
    } catch (error) {
      setState({ status: "error", result: state.result, message: formatError(error) });
    }
  };

  const result = "result" in state ? state.result : undefined;
  const busy = state.status === "checking" || state.status === "installing" || state.status === "restarting";
  const title =
    state.status === "error"
      ? t("settings", "updateError")
      : state.status === "checking"
        ? t("settings", "updateChecking")
        : state.status === "installing"
          ? t("settings", "updateInstalling")
          : state.status === "installed"
            ? t("settings", "updateInstalled")
            : result?.available
              ? t("settings", "updateAvailable")
              : t("settings", "updateCurrent");
  const description =
    state.status === "error"
      ? state.message
      : result?.message ?? (result?.available ? t("settings", "updateAvailableDesc") : t("settings", "updateCurrentDesc"));

  return (
    <div className="space-y-3" data-testid="settings-updates-panel">
      <div className="rounded-md border border-border bg-muted/20 p-3">
        <div className="flex items-start gap-3">
          <div className="mt-0.5">
            {state.status === "error" ? (
              <AlertTriangle className="h-4 w-4 text-amber-500" />
            ) : busy ? (
              <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
            ) : result?.available ? (
              <Download className="h-4 w-4 text-primary" />
            ) : (
              <CheckCircle2 className="h-4 w-4 text-emerald-500" />
            )}
          </div>
          <div className="min-w-0 flex-1">
            <div className="text-sm font-medium">{title}</div>
            <p className="mt-1 text-xs leading-relaxed text-muted-foreground">{description}</p>
            <div className="mt-3 grid gap-2 text-xs sm:grid-cols-2">
              <InfoPill label={t("settings", "currentVersion")} value={`v${result?.currentVersion ?? APP_VERSION}`} />
              <InfoPill label={t("settings", "latestVersion")} value={result?.version ? `v${result.version}` : "—"} />
            </div>
          </div>
        </div>
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <Button size="sm" variant="outline" onClick={() => void runCheck()} disabled={busy}>
            <RefreshCw className="h-3.5 w-3.5" />
            {t("settings", "checkUpdates")}
          </Button>
          <Button
            size="sm"
            onClick={() => (state.status === "installed" ? void restart() : void install())}
            disabled={busy || (state.status !== "installed" && !result?.available)}
          >
            {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Download className="h-3.5 w-3.5" />}
            {state.status === "installed" ? t("settings", "restartToUpdate") : t("settings", "installUpdate")}
          </Button>
          {result?.releaseUrl ? (
            <a
              className="inline-flex h-8 items-center gap-1.5 rounded-md px-2.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
              href={result.releaseUrl}
              target="_blank"
              rel="noreferrer"
            >
              <ExternalLink className="h-3.5 w-3.5" />
              {t("settings", "openRelease")}
            </a>
          ) : null}
        </div>
      </div>

      <label className="flex items-center justify-between gap-3 rounded-md border border-border bg-background px-3 py-2">
        <div>
          <div className="text-sm font-medium">{t("settings", "includePrereleases")}</div>
          <div className="text-xs text-muted-foreground">{t("settings", "includePrereleasesDesc")}</div>
        </div>
        <input
          type="checkbox"
          className="h-4 w-4 accent-primary"
          checked={settings.includePrereleases}
          onChange={(event) => void updateSettings({ includePrereleases: event.currentTarget.checked })}
        />
      </label>
    </div>
  );
}

function InfoPill({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md bg-background/80 px-2 py-1.5">
      <div className="text-muted-foreground">{label}</div>
      <div className="mt-0.5 font-mono">{value}</div>
    </div>
  );
}

function AboutTab() {
  const { t } = useI18n();

  return (
    <div className="flex max-w-2xl flex-col">
      <div className="flex items-center gap-4 border-b border-border pb-6">
        <div className="flex h-12 w-12 items-center justify-center rounded-md bg-primary/10">
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

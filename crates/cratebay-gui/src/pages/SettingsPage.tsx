import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "@/stores/settingsStore";
import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/lib/i18n";
import { McpPage } from "@/pages/McpPage";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { ProviderForm } from "@/components/settings/ProviderForm";
import { ProviderCard } from "@/components/settings/ProviderCard";
import { ReasoningEffort } from "@/components/settings/ReasoningEffort";
import {
  Moon,
  Sun,
  Monitor,
  Cpu,
  HardDrive,
  ExternalLink,
  Package,
  Play,
  Square,
  Plus,
  X,
  RotateCcw,
} from "lucide-react";

export function SettingsPage() {
  const { t } = useI18n();

  return (
    <div className="flex h-full flex-col overflow-auto p-6">
      <Tabs defaultValue="general" className="flex-1">
        <TabsList className="mb-4">
          <TabsTrigger value="general" data-testid="settings-tab-general">{t("settings", "general")}</TabsTrigger>
          <TabsTrigger value="providers" data-testid="settings-tab-providers">{t("settings", "providers")}</TabsTrigger>
          <TabsTrigger value="appearance" data-testid="settings-tab-appearance">{t("settings", "appearance")}</TabsTrigger>
          <TabsTrigger value="runtime" data-testid="settings-tab-runtime">{t("settings", "runtime")}</TabsTrigger>
          <TabsTrigger value="mcp" data-testid="settings-tab-mcp">MCP</TabsTrigger>
          <TabsTrigger value="advanced" data-testid="settings-tab-advanced">{t("settings", "advanced")}</TabsTrigger>
          <TabsTrigger value="about" data-testid="settings-tab-about">{t("settings", "about")}</TabsTrigger>
        </TabsList>

        <TabsContent value="general" className="mt-4">
          <GeneralTab />
        </TabsContent>

        <TabsContent value="providers" className="mt-4">
          <ProvidersTab />
        </TabsContent>

        <TabsContent value="appearance" className="mt-4">
          <AppearanceTab />
        </TabsContent>

        <TabsContent value="runtime" className="mt-4">
          <RuntimeTab />
        </TabsContent>

        <TabsContent value="mcp" className="mt-4">
          <McpPage />
        </TabsContent>

        <TabsContent value="advanced" className="mt-4">
          <AdvancedTab />
        </TabsContent>

        <TabsContent value="about" className="mt-4">
          <AboutTab />
        </TabsContent>
      </Tabs>
    </div>
  );
}

/* ---------- Setting row helper ---------- */

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
    <div className="flex items-center justify-between py-3 border-b border-border">
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

/* ---------- General Tab ---------- */

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
              {settings.language === "zh-CN" ? "简体中文" : "English"}
            </SelectValue>
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="en">English</SelectItem>
            <SelectItem value="zh-CN">简体中文</SelectItem>
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

      <SettingRow
        label={t("settings", "sendOnEnter")}
        description={t("settings", "sendOnEnterDesc")}
      >
        <Switch
          checked={settings.sendOnEnter}
          onCheckedChange={(v) => void updateSettings({ sendOnEnter: v })}
        />
      </SettingRow>

      <SettingRow
        label={t("settings", "showAgentThinking")}
        description={t("settings", "showAgentThinkingDesc")}
      >
        <Switch
          checked={settings.showAgentThinking}
          onCheckedChange={(v) => void updateSettings({ showAgentThinking: v })}
        />
      </SettingRow>
    </div>
  );
}

/* ---------- Providers Tab ---------- */

function ProvidersTab() {
  const { t } = useI18n();
  const providers = useSettingsStore((s) => s.providers);

  return (
    <div className="flex flex-col gap-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-base font-medium text-foreground">{t("settings", "llmProviders")}</h2>
          <p className="text-xs text-muted-foreground">
            {t("settings", "llmProvidersDesc")}
          </p>
        </div>
        <ProviderForm />
      </div>

      {/* Provider list */}
      {providers.length === 0 ? (
        <div className="rounded-lg border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
          {t("settings", "noProviders")}
        </div>
      ) : (
        <div className="grid gap-4">
          {providers.map((provider) => (
            <ProviderCard key={provider.id} provider={provider} />
          ))}
        </div>
      )}

      {/* Reasoning Effort (global setting) */}
      <div className="border-t border-border pt-4">
        <ReasoningEffort />
      </div>
    </div>
  );
}

/* ---------- Appearance Tab ---------- */

const ACCENT_COLORS = [
  { value: "#7c3aed", label: "Purple" },
  { value: "#3b82f6", label: "Blue" },
  { value: "#10b981", label: "Green" },
  { value: "#f59e0b", label: "Orange" },
  { value: "#ef4444", label: "Red" },
  { value: "#ec4899", label: "Pink" },
] as const;

function AppearanceTab() {
  const { t } = useI18n();
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);
  const [fontSize, setFontSize] = useState(14);
  const [accentColor, setAccentColor] = useState("#7c3aed");

  const themeOptions = [
    { key: "dark" as const, icon: Moon, label: t("settings", "themeDark") },
    { key: "light" as const, icon: Sun, label: t("settings", "themeLight") },
    { key: "system" as const, icon: Monitor, label: t("settings", "themeSystem") },
  ];

  return (
    <div className="flex max-w-2xl flex-col">
      {/* Theme mode */}
      <SettingRow
        label={t("settings", "themeMode")}
        description={t("settings", "themeModeDesc")}
      >
        <div className="flex gap-1.5">
          {themeOptions.map((opt) => {
            const Icon = opt.icon;
            const isActive = settings.theme === opt.key;
            return (
              <button
                key={opt.key}
                onClick={() => void updateSettings({ theme: opt.key })}
                className={`inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${
                  isActive
                    ? "bg-primary text-primary-foreground"
                    : "bg-muted text-muted-foreground hover:text-foreground"
                }`}
              >
                <Icon size={14} />
                {opt.label}
              </button>
            );
          })}
        </div>
      </SettingRow>

      {/* Font size */}
      <SettingRow
        label={t("settings", "fontSize")}
        description={t("settings", "fontSizeDesc")}
      >
        <div className="flex items-center gap-3">
          <input
            type="range"
            min={12}
            max={18}
            value={fontSize}
            onChange={(e) => setFontSize(Number(e.target.value))}
            className="w-32 accent-primary"
          />
          <span className="w-10 text-right text-sm font-mono text-muted-foreground">
            {fontSize}px
          </span>
        </div>
      </SettingRow>

      {/* Accent color */}
      <SettingRow
        label={t("settings", "accentColor")}
        description={t("settings", "accentColorDesc")}
      >
        <div className="flex gap-2">
          {ACCENT_COLORS.map((c) => (
            <button
              key={c.value}
              title={c.label}
              onClick={() => setAccentColor(c.value)}
              className="w-6 h-6 rounded-full cursor-pointer border-2 transition-all"
              style={{
                backgroundColor: c.value,
                borderColor:
                  accentColor === c.value
                    ? "white"
                    : "transparent",
                boxShadow:
                  accentColor === c.value
                    ? "0 0 0 2px rgba(255,255,255,0.2)"
                    : "none",
              }}
            />
          ))}
        </div>
      </SettingRow>
    </div>
  );
}

/* ---------- Runtime Tab ---------- */

function RuntimeStatusDot({
  status,
}: {
  status: "running" | "starting" | "stopped" | "error" | "connected" | "disconnected";
}) {
  const colorClass =
    status === "running" || status === "connected"
      ? "bg-green-500"
      : status === "starting"
        ? "bg-yellow-500 animate-pulse"
        : status === "error"
          ? "bg-red-500"
          : "bg-muted-foreground";

  return <span className={`inline-block w-2 h-2 rounded-full ${colorClass}`} />;
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
  const [cpuCores, setCpuCores] = useState(4);
  const [memoryGB, setMemoryGB] = useState(8);
  const [proxyInput, setProxyInput] = useState(settings.runtimeHttpProxy);

  useEffect(() => {
    setProxyInput(settings.runtimeHttpProxy);
  }, [settings.runtimeHttpProxy]);

  const handleRuntimeStart = async () => {
    try {
      setRuntimeLoading(true);
      await invoke("runtime_start");
      addNotification({
        type: "success",
        title: t("settings", "runtimeStarting"),
        message: t("settings", "runtimeStarting"),
        dismissable: true,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      addNotification({
        type: "error",
        title: t("common", "error"),
        message,
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
      addNotification({
        type: "success",
        title: t("settings", "runtimeStopped"),
        message: t("settings", "runtimeStopped"),
        dismissable: true,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      addNotification({
        type: "error",
        title: t("common", "error"),
        message,
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
      addNotification({
        type: "success",
        title: t("settings", "runtimeRestart"),
        message: t("settings", "runtimeProxyRestartHint"),
        dismissable: true,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      addNotification({
        type: "error",
        title: t("common", "error"),
        message,
        dismissable: true,
      });
    } finally {
      setRuntimeLoading(false);
    }
  };

  const handleSaveRuntimeProxy = async () => {
    const proxy = proxyInput.trim();
    try {
      await updateSettings({ runtimeHttpProxy: proxy });
      addNotification({
        type: "success",
        title: t("settings", "runtimeProxySaveSuccess"),
        message: t("settings", "runtimeProxyRestartHint"),
        dismissable: true,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      addNotification({
        type: "error",
        title: t("common", "error"),
        message,
        dismissable: true,
      });
    }
  };

  const runtimeProxyDirty = proxyInput.trim() !== settings.runtimeHttpProxy;

  return (
    <div className="flex max-w-2xl flex-col">
      {/* VM Status */}
      {/* Container Engine — unified single status row */}
      <SettingRow
        label={t("settings", "containerEngine")}
        description={t("settings", "containerEngineDesc")}
      >
        <div className="flex items-center gap-2">
          <RuntimeStatusDot status={dockerConnected ? "connected" : runtimeStatus === "starting" ? "starting" : "disconnected"} />
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

      {/* Runtime Control */}
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

      <div className="flex items-center justify-between py-3 border-b border-border">
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

      {/* CPU Cores */}
      <SettingRow
        label={t("settings", "cpuCores")}
        description={t("settings", "cpuCoresDesc")}
      >
        <div className="flex items-center gap-3">
          <Cpu size={14} className="text-muted-foreground" />
          <input
            type="range"
            min={1}
            max={16}
            value={cpuCores}
            onChange={(e) => setCpuCores(Number(e.target.value))}
            className="w-32 accent-primary"
          />
          <span className="w-6 text-right text-sm font-mono text-muted-foreground">
            {cpuCores}
          </span>
        </div>
      </SettingRow>

      {/* Memory Allocation */}
      <SettingRow
        label={t("settings", "memoryAllocation")}
        description={t("settings", "memoryAllocationDesc")}
      >
        <div className="flex items-center gap-3">
          <HardDrive size={14} className="text-muted-foreground" />
          <input
            type="range"
            min={2}
            max={32}
            step={2}
            value={memoryGB}
            onChange={(e) => setMemoryGB(Number(e.target.value))}
            className="w-32 accent-primary"
          />
          <span className="w-10 text-right text-sm font-mono text-muted-foreground">
            {memoryGB} GB
          </span>
        </div>
      </SettingRow>

      {/* Registry Mirrors */}
      <div className="mt-6 border-t border-border pt-4">
        <RegistryMirrorsSection />
      </div>
    </div>
  );
}

/* ---------- Registry Mirrors Section ---------- */

function RegistryMirrorsSection() {
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
    const updated = mirrors.filter((_, i) => i !== index);
    void updateSettings({ registryMirrors: updated });
  };

  const resetToDefaults = () => {
    // Dynamic import to get DEFAULT_REGISTRY_MIRRORS
    import("@/types/settings").then(({ DEFAULT_REGISTRY_MIRRORS }) => {
      void updateSettings({ registryMirrors: [...DEFAULT_REGISTRY_MIRRORS] });
    });
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addMirror();
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium text-foreground">Docker 镜像加速源</h3>
          <p className="text-xs text-muted-foreground">
            配置镜像源加速 Docker 镜像拉取，按顺序依次尝试
          </p>
        </div>
        <Button
          size="sm"
          variant="ghost"
          className="gap-1.5 text-xs"
          onClick={resetToDefaults}
        >
          <RotateCcw size={12} />
          恢复默认
        </Button>
      </div>

      {/* Mirror list */}
      <div className="flex flex-col gap-1.5">
        {mirrors.length === 0 ? (
          <div className="rounded-md border border-dashed border-border py-4 text-center text-xs text-muted-foreground">
            未配置镜像加速源，将直接从 Docker Hub 拉取
          </div>
        ) : (
          mirrors.map((mirror, index) => (
            <div
              key={`${mirror}-${index}`}
              className="group flex items-center justify-between rounded-md border border-border bg-muted/30 px-3 py-2"
            >
              <div className="flex items-center gap-2">
                <span className="text-xs font-mono text-muted-foreground w-5">
                  {index + 1}.
                </span>
                <span className="text-sm font-mono text-foreground">{mirror}</span>
              </div>
              <button
                onClick={() => removeMirror(index)}
                className="text-muted-foreground opacity-0 group-hover:opacity-100 hover:text-destructive transition-all"
              >
                <X size={14} />
              </button>
            </div>
          ))
        )}
      </div>

      {/* Add new mirror */}
      <div className="flex gap-2">
        <Input
          value={newMirror}
          onChange={(e) => setNewMirror(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="输入镜像源地址，如 mirror.example.com"
          className="flex-1 text-sm font-mono"
        />
        <Button
          size="sm"
          variant="outline"
          onClick={addMirror}
          disabled={!newMirror.trim()}
          className="gap-1.5"
        >
          <Plus size={14} />
          添加
        </Button>
      </div>

      <p className="text-xs text-muted-foreground">
        拉取镜像时会按上述顺序尝试加速源，全部失败后回退到 Docker Hub 直连
      </p>
    </div>
  );
}

/* ---------- Advanced Tab ---------- */

function AdvancedTab() {
  const { t } = useI18n();
  const settings = useSettingsStore((s) => s.settings);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="flex max-w-2xl flex-col">
      <SettingRow
        label={t("settings", "containerTtl")}
        description={t("settings", "containerTtlDesc")}
      >
        <Input
          type="number"
          min={1}
          max={168}
          value={settings.containerDefaultTtlHours}
          onChange={(e) =>
            void updateSettings({
              containerDefaultTtlHours: Number(e.target.value) || 8,
            })
          }
          className="w-32"
        />
      </SettingRow>

      <SettingRow
        label={t("settings", "maxHistory")}
        description={t("settings", "maxHistoryDesc")}
      >
        <Input
          type="number"
          min={10}
          max={200}
          value={settings.maxConversationHistory}
          onChange={(e) =>
            void updateSettings({
              maxConversationHistory: Number(e.target.value) || 50,
            })
          }
          className="w-32"
        />
      </SettingRow>

      <SettingRow
        label={t("settings", "confirmDestructive")}
        description={t("settings", "confirmDestructiveDesc")}
      >
        <Switch
          checked={settings.confirmDestructiveOps}
          onCheckedChange={(v) => void updateSettings({ confirmDestructiveOps: v })}
        />
      </SettingRow>
    </div>
  );
}

/* ---------- About Tab ---------- */

function AboutTab() {
  const { t } = useI18n();

  return (
    <div className="flex max-w-2xl flex-col">
      {/* Logo + branding */}
      <div className="flex items-center gap-4 pb-6 border-b border-border">
        <div className="flex items-center justify-center w-12 h-12 rounded-xl bg-primary/10">
          <Package size={24} className="text-primary" />
        </div>
        <div>
          <h2 className="text-xl font-bold text-foreground">CrateBay</h2>
          <p className="text-sm text-muted-foreground">
            {t("settings", "aboutSubtitle")}
          </p>
        </div>
      </div>

      {/* Info rows */}
      <SettingRow label={t("common", "version")}>
        <span className="text-sm font-mono text-muted-foreground">v0.9.0</span>
      </SettingRow>

      <SettingRow label={t("settings", "builtWith")}>
        <span className="text-sm text-muted-foreground">
          Tauri v2 + React + TypeScript
        </span>
      </SettingRow>

      <SettingRow label={t("settings", "license")}>
        <span className="text-sm text-muted-foreground">MIT License</span>
      </SettingRow>

      {/* Links */}
      <div className="flex gap-3 pt-6">
        <a
          href="https://github.com/cratebay/cratebay"
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-1.5 rounded-md bg-muted px-3 py-1.5 text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
        >
          <ExternalLink size={14} />
          {t("settings", "github")}
        </a>
        <a
          href="https://cratebay.io"
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-1.5 rounded-md bg-muted px-3 py-1.5 text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
        >
          <ExternalLink size={14} />
          {t("settings", "website")}
        </a>
      </div>
    </div>
  );
}

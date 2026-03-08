import { useState, useEffect, useCallback, useMemo } from "react"
import { invoke } from "@tauri-apps/api/core"
import { I } from "../icons"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent } from "@/components/ui/card"
import { Progress } from "@/components/ui/progress"
import { cn } from "@/lib/utils"
import { iconStroke, cardActionGhost } from "@/lib/styles"
import type {
  AiSettings,
  ContainerInfo,
  ContainerStats,
  GpuStatusDto,
  McpServerStatusDto,
  OllamaModelDto,
  OllamaStatusDto,
  SandboxInfoDto,
  SandboxRuntimeUsageDto,
} from "../types"

type DashboardAiTab = "models" | "sandboxes" | "mcp" | "assistant"
type DashboardSettingsTab = "general" | "ai"

interface DashboardProps {
  containers: ContainerInfo[]
  running: ContainerInfo[]
  imgResultsCount: number
  installedImagesCount: number
  volumesCount: number
  onNavigate: (page: "containers" | "images" | "volumes") => void
  onOpenAiTab: (tab: DashboardAiTab) => void
  onOpenSettingsTab: (tab: DashboardSettingsTab) => void
  t: (key: string) => string
}

interface TotalResources {
  totalCpuPercent: number
  totalMemoryUsageMb: number
  totalMemoryLimitMb: number
}

interface SandboxGpuSummary {
  activeSandboxCount: number
  gpuProcesses: number
  gpuMemoryUsedBytes: number
}

function formatBytesHuman(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B"
  const units = ["B", "KB", "MB", "GB", "TB"]
  let value = bytes
  let unitIndex = 0
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024
    unitIndex += 1
  }
  const rounded = value >= 100 || unitIndex === 0 ? Math.round(value) : Math.round(value * 10) / 10
  return `${rounded} ${units[unitIndex]}`
}

function formatUsagePercent(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0%"
  return `${Math.round(value * 10) / 10}%`
}

function NavCard({
  value,
  label,
  icon,
  iconClassName,
  sub,
  onClick,
  testId,
}: {
  value: number
  label: string
  icon: React.ReactNode
  iconClassName: string
  sub?: React.ReactNode
  onClick: () => void
  testId: string
}) {
  return (
    <Card
      data-testid={testId}
      role="button"
      tabIndex={0}
      className="py-0 cursor-pointer transition-all hover:border-primary/40 hover:bg-accent/10 motion-safe:hover:-translate-y-px motion-safe:hover:shadow-sm focus-visible:outline-hidden focus-visible:ring-[3px] focus-visible:ring-ring/50"
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault()
          onClick()
        }
      }}
    >
      <CardContent className="py-4">
        <div className="flex items-start justify-between gap-3">
          <div
            className={cn(
              "size-10 shrink-0 rounded-lg flex items-center justify-center",
              iconStroke,
              "[&_svg]:size-[18px]",
              iconClassName
            )}
          >
            {icon}
          </div>
          {sub}
        </div>
        <div className="mt-4">
          <div className="text-[34px] leading-none font-bold text-foreground">{value}</div>
          <div className="mt-1 text-xs text-muted-foreground">{label}</div>
        </div>
      </CardContent>
    </Card>
  )
}

function MetricCard({
  title,
  value,
  icon,
  iconClassName,
  meta,
  progress,
  testId,
}: {
  title: string
  value: React.ReactNode
  icon: React.ReactNode
  iconClassName: string
  meta?: React.ReactNode
  progress?: number
  testId: string
}) {
  return (
    <Card data-testid={testId} className="py-0">
      <CardContent className="py-4">
        <div className="flex items-center gap-4">
          <div
            className={cn(
              "size-10 shrink-0 rounded-lg flex items-center justify-center",
              iconStroke,
              "[&_svg]:size-[18px]",
              iconClassName
            )}
          >
            {icon}
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex items-center justify-between gap-3">
              <span className="text-xs font-semibold text-muted-foreground">{title}</span>
              <span className="text-sm font-semibold text-foreground">{value}</span>
            </div>
            {meta ? <div className="mt-2 text-xs text-muted-foreground">{meta}</div> : null}
            {typeof progress === "number" ? (
              <div className="mt-3">
                <Progress value={Math.max(0, Math.min(progress, 100))} className="h-2" />
              </div>
            ) : null}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

export function Dashboard({
  containers,
  running,
  imgResultsCount,
  installedImagesCount,
  volumesCount,
  onNavigate,
  onOpenAiTab,
  onOpenSettingsTab,
  t,
}: DashboardProps) {
  const [totals, setTotals] = useState<TotalResources>({
    totalCpuPercent: 0,
    totalMemoryUsageMb: 0,
    totalMemoryLimitMb: 0,
  })
  const [sandboxes, setSandboxes] = useState<SandboxInfoDto[]>([])
  const [ollamaStatus, setOllamaStatus] = useState<OllamaStatusDto | null>(null)
  const [ollamaModels, setOllamaModels] = useState<OllamaModelDto[]>([])
  const [mcpServers, setMcpServers] = useState<McpServerStatusDto[]>([])
  const [aiSettings, setAiSettings] = useState<AiSettings | null>(null)
  const [gpuStatus, setGpuStatus] = useState<GpuStatusDto | null>(null)
  const [sandboxGpuSummary, setSandboxGpuSummary] = useState<SandboxGpuSummary>({
    activeSandboxCount: 0,
    gpuProcesses: 0,
    gpuMemoryUsedBytes: 0,
  })

  const fetchTotals = useCallback(async () => {
    let totalCpu = 0
    let totalMemUsage = 0
    let totalMemLimit = 0

    const containerPromises = running.map(async (container) => {
      try {
        return await invoke<ContainerStats>("container_stats", { id: container.id })
      } catch {
        return null
      }
    })

    const containerResults = await Promise.allSettled(containerPromises)

    for (const result of containerResults) {
      if (result.status === "fulfilled" && result.value) {
        totalCpu += result.value.cpu_percent
        totalMemUsage += result.value.memory_usage_mb
        totalMemLimit += result.value.memory_limit_mb
      }
    }

    setTotals({
      totalCpuPercent: totalCpu,
      totalMemoryUsageMb: totalMemUsage,
      totalMemoryLimitMb: totalMemLimit,
    })
  }, [running])

  const fetchAiOverview = useCallback(async () => {
    const [sandboxResult, ollamaStatusResult, mcpResult, settingsResult, gpuResult] = await Promise.all([
      invoke<SandboxInfoDto[]>("sandbox_list").catch(() => []),
      invoke<OllamaStatusDto>("ollama_status").catch(() => null),
      invoke<McpServerStatusDto[]>("mcp_list_servers").catch(() => []),
      invoke<AiSettings>("load_ai_settings").catch(() => null),
      invoke<GpuStatusDto>("gpu_status").catch(() => null),
    ])

    setSandboxes(sandboxResult)
    setOllamaStatus(ollamaStatusResult)
    setMcpServers(mcpResult)
    setAiSettings(settingsResult)
    setGpuStatus(gpuResult)

    if (ollamaStatusResult?.running) {
      const models = await invoke<OllamaModelDto[]>("ollama_list_models").catch(() => [])
      setOllamaModels(models)
    } else {
      setOllamaModels([])
    }

    const runningSandboxes = sandboxResult.filter((item) => item.state === "running")
    if (runningSandboxes.length === 0) {
      setSandboxGpuSummary({
        activeSandboxCount: 0,
        gpuProcesses: 0,
        gpuMemoryUsedBytes: 0,
      })
      return
    }

    const runtimeResults = await Promise.allSettled(
      runningSandboxes.map((item) => invoke<SandboxRuntimeUsageDto>("sandbox_runtime_usage", { id: item.id }))
    )

    let activeSandboxCount = 0
    let gpuProcesses = 0
    let gpuMemoryUsedBytes = 0

    for (const result of runtimeResults) {
      if (result.status === "fulfilled") {
        const usage = result.value
        if (usage.gpu_processes.length > 0) {
          activeSandboxCount += 1
          gpuProcesses += usage.gpu_processes.length
          gpuMemoryUsedBytes += usage.gpu_memory_used_bytes
        }
      }
    }

    setSandboxGpuSummary({
      activeSandboxCount,
      gpuProcesses,
      gpuMemoryUsedBytes,
    })
  }, [])

  useEffect(() => {
    const timeout = setTimeout(fetchTotals, 0)
    const iv = setInterval(fetchTotals, 5000)
    return () => {
      clearTimeout(timeout)
      clearInterval(iv)
    }
  }, [fetchTotals])

  useEffect(() => {
    const timeout = setTimeout(fetchAiOverview, 0)
    const iv = setInterval(fetchAiOverview, 5000)
    return () => {
      clearTimeout(timeout)
      clearInterval(iv)
    }
  }, [fetchAiOverview])

  const memPercent =
    totals.totalMemoryLimitMb > 0
      ? (totals.totalMemoryUsageMb / totals.totalMemoryLimitMb) * 100
      : 0
  const cpuClamped = Math.min(totals.totalCpuPercent, 100)
  const runningSandboxCount = sandboxes.filter((item) => item.state === "running").length
  const expiredSandboxCount = sandboxes.filter((item) => item.is_expired).length
  const runningMcpCount = mcpServers.filter((item) => item.running).length
  const providerProfileCount = aiSettings?.profiles.length ?? 0
  const cliAllowlistCount = aiSettings?.security_policy.cli_command_allowlist.length ?? 0

  const runningSandboxes = useMemo(
    () => sandboxes.filter((item) => item.state === "running"),
    [sandboxes]
  )

  const gpuPeakUtilization = useMemo(() => {
    if (!gpuStatus?.devices.length) return 0
    return gpuStatus.devices.reduce((max, device) => Math.max(max, device.utilization_percent ?? 0), 0)
  }, [gpuStatus])

  const gpuMemorySummary = useMemo(() => {
    if (!gpuStatus?.devices.length) return "-"
    const used = gpuStatus.devices.reduce((sum, device) => sum + (device.memory_used_bytes ?? 0), 0)
    const total = gpuStatus.devices.reduce((sum, device) => sum + (device.memory_total_bytes ?? 0), 0)
    if (total > 0) return `${formatBytesHuman(used)} / ${formatBytesHuman(total)}`
    if (used > 0) return formatBytesHuman(used)
    return "-"
  }, [gpuStatus])

  return (
    <div className="space-y-6">
      <section className="space-y-3">
        <div>
          <div className="text-sm font-semibold text-foreground">{t("dashboardAiOverview")}</div>
          <div className="text-xs text-muted-foreground">{t("aiHubDesc")}</div>
        </div>
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-4">
          <NavCard
            testId="dashboard-card-sandboxes"
            value={sandboxes.length}
            label={t("sandboxes")}
            icon={I.box}
            iconClassName="bg-primary/10 text-primary"
            sub={
              <div className="flex flex-wrap justify-end gap-2">
                {runningSandboxCount > 0 ? (
                  <Badge
                    variant="secondary"
                    className="rounded-full gap-2 border border-brand-green/15 bg-brand-green/10 px-2 py-0.5 text-[11px] text-brand-green"
                  >
                    <span className="size-1.5 rounded-full bg-brand-green shadow-[0_0_10px_hsl(var(--brand-green)/0.6)]" />
                    {runningSandboxCount} {t("runningCount")}
                  </Badge>
                ) : (
                  <span className="text-xs text-muted-foreground">{t("noRunning")}</span>
                )}
                {expiredSandboxCount > 0 ? (
                  <Badge variant="secondary" className="rounded-full px-2 py-0.5 text-[11px]">
                    {expiredSandboxCount} {t("sandboxExpired")}
                  </Badge>
                ) : null}
              </div>
            }
            onClick={() => onOpenAiTab("sandboxes")}
          />

          <NavCard
            testId="dashboard-card-models"
            value={ollamaModels.length}
            label={t("models")}
            icon={I.layers}
            iconClassName="bg-brand-cyan/10 text-brand-cyan"
            sub={
              ollamaStatus?.running ? (
                <Badge
                  variant="secondary"
                  className="rounded-full border border-brand-cyan/15 bg-brand-cyan/10 px-2 py-0.5 text-[11px] text-brand-cyan"
                >
                  {ollamaStatus.version || t("running")}
                </Badge>
              ) : (
                <span className="text-xs text-muted-foreground">
                  {ollamaStatus?.installed ? t("stopped") : t("notInstalled")}
                </span>
              )
            }
            onClick={() => onOpenAiTab("models")}
          />

          <NavCard
            testId="dashboard-card-mcp"
            value={mcpServers.length}
            label={t("mcp")}
            icon={I.server}
            iconClassName="bg-brand-green/10 text-brand-green"
            sub={
              runningMcpCount > 0 ? (
                <Badge
                  variant="secondary"
                  className="rounded-full border border-brand-green/15 bg-brand-green/10 px-2 py-0.5 text-[11px] text-brand-green"
                >
                  {runningMcpCount} {t("mcpRunning")}
                </Badge>
              ) : (
                <span className="text-xs text-muted-foreground">{t("mcpStopped")}</span>
              )
            }
            onClick={() => onOpenAiTab("mcp")}
          />

          <NavCard
            testId="dashboard-card-ai-settings"
            value={providerProfileCount}
            label={t("providerProfiles")}
            icon={I.settings}
            iconClassName="bg-yellow-500/10 text-yellow-500 dark:text-yellow-400"
            sub={
              <Badge variant="secondary" className="rounded-full px-2 py-0.5 text-[11px]">
                {cliAllowlistCount} {t("cliAllowlist")}
              </Badge>
            }
            onClick={() => onOpenSettingsTab("ai")}
          />
        </div>
      </section>

      <section className="space-y-3">
        <div>
          <div className="text-sm font-semibold text-foreground">{t("dashboardRuntimeOverview")}</div>
          <div className="text-xs text-muted-foreground">{t("dashboardRuntimeOverviewDesc")}</div>
        </div>
        <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
          <MetricCard
            testId="dashboard-metric-cpu"
            title={t("cpuUsage")}
            value={formatUsagePercent(cpuClamped)}
            icon={I.cpu}
            iconClassName="bg-primary/10 text-primary"
            meta={running.length > 0 ? `${running.length} ${t("runningCount")}` : t("noRunning")}
            progress={cpuClamped}
          />

          <MetricCard
            testId="dashboard-metric-memory"
            title={t("memoryUsage")}
            value={`${totals.totalMemoryUsageMb.toFixed(0)} / ${totals.totalMemoryLimitMb.toFixed(0)} MB`}
            icon={I.memory}
            iconClassName="bg-brand-cyan/10 text-brand-cyan"
            meta={running.length > 0 ? formatUsagePercent(memPercent) : t("noRunning")}
            progress={memPercent}
          />

          <MetricCard
            testId="dashboard-metric-gpu"
            title={t("gpuRuntime")}
            value={gpuStatus?.available ? `${gpuStatus.devices.length} ${t("gpuDevices").toLowerCase()}` : t("gpuTelemetryUnavailable")}
            icon={I.aiAssistant}
            iconClassName="bg-brand-green/10 text-brand-green"
            meta={
              gpuStatus?.available ? (
                <>
                  {formatUsagePercent(gpuPeakUtilization)} · {gpuMemorySummary}
                  {sandboxGpuSummary.activeSandboxCount > 0
                    ? ` · ${sandboxGpuSummary.activeSandboxCount} ${t("sandboxes").toLowerCase()} · ${sandboxGpuSummary.gpuProcesses} ${t("gpuProcesses").toLowerCase()} · ${formatBytesHuman(sandboxGpuSummary.gpuMemoryUsedBytes)}`
                    : ` · ${t("sandboxGpuIdle")}`}
                </>
              ) : (
                gpuStatus?.message || t("gpuTelemetryUnavailable")
              )
            }
            progress={gpuStatus?.available ? gpuPeakUtilization : undefined}
          />
        </div>
      </section>

      <section className="space-y-3">
        <div>
          <div className="text-sm font-semibold text-foreground">{t("dashboardEngineOverview")}</div>
          <div className="text-xs text-muted-foreground">{t("dashboardEngineOverviewDesc")}</div>
        </div>
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
          <NavCard
            testId="dashboard-card-containers"
            value={containers.length}
            label={t("containers")}
            icon={I.box}
            iconClassName="bg-primary/10 text-primary"
            sub={
              running.length > 0 ? (
                <Badge
                  variant="secondary"
                  className="rounded-full gap-2 border border-brand-green/15 bg-brand-green/10 px-2 py-0.5 text-[11px] text-brand-green"
                >
                  <span className="size-1.5 rounded-full bg-brand-green shadow-[0_0_10px_hsl(var(--brand-green)/0.6)]" />
                  {running.length} {t("runningCount")}
                </Badge>
              ) : (
                <span className="text-xs text-muted-foreground">{t("noRunning")}</span>
              )
            }
            onClick={() => onNavigate("containers")}
          />

          <NavCard
            testId="dashboard-card-images"
            value={installedImagesCount}
            label={t("images")}
            icon={I.layers}
            iconClassName="bg-brand-green/10 text-brand-green"
            sub={
              imgResultsCount > 0 ? (
                <Badge
                  variant="secondary"
                  className="rounded-full border border-brand-cyan/15 bg-brand-cyan/10 px-2 py-0.5 text-[11px] text-brand-cyan"
                >
                  {imgResultsCount} {t("searchResults")}
                </Badge>
              ) : (
                <span className="text-xs text-muted-foreground">{t("available")}</span>
              )
            }
            onClick={() => onNavigate("images")}
          />

          <NavCard
            testId="dashboard-card-volumes"
            value={volumesCount}
            label={t("volumes")}
            icon={I.hardDrive}
            iconClassName="bg-yellow-500/10 text-yellow-500 dark:text-yellow-400"
            sub={<span className="text-xs text-muted-foreground">{t("available")}</span>}
            onClick={() => onNavigate("volumes")}
          />
        </div>
      </section>

      <Card className="py-0">
        <CardContent className="py-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-sm font-semibold text-foreground">{t("dashboardActiveAi")}</div>
              <div className="text-xs text-muted-foreground">
                {runningSandboxes.length > 0
                  ? `${runningSandboxes.length} ${t("runningCount")}`
                  : t("dashboardNoAiWorkloads")}
              </div>
            </div>
            {runningSandboxes.length > 5 ? (
              <Button variant="ghost" size="sm" className={cardActionGhost} onClick={() => onOpenAiTab("sandboxes")}>
                {t("viewAll")}
              </Button>
            ) : null}
          </div>

          {runningSandboxes.length > 0 ? (
            <div className="mt-4 grid grid-cols-1 gap-3 lg:grid-cols-2 xl:grid-cols-3">
              {runningSandboxes.slice(0, 5).map((sandbox) => (
                <div
                  key={sandbox.id}
                  data-testid="dashboard-sandbox-item"
                  className="rounded-xl border bg-background px-4 py-3"
                >
                  <div className="flex items-center justify-between gap-3">
                    <div className="min-w-0">
                      <div className="truncate text-sm font-semibold text-foreground">{sandbox.name}</div>
                      <div className="truncate text-xs text-muted-foreground">
                        {sandbox.template_id} · {sandbox.image}
                      </div>
                    </div>
                    <Badge
                      variant="secondary"
                      className="rounded-full gap-2 border border-brand-green/15 bg-brand-green/10 px-2 py-0.5 text-[11px] text-brand-green"
                    >
                      <span className="size-1.5 rounded-full bg-brand-green shadow-[0_0_10px_hsl(var(--brand-green)/0.6)]" />
                      {t("running")}
                    </Badge>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="mt-4 flex items-center justify-between gap-3 rounded-xl border border-dashed border-border/60 px-4 py-3">
              <div className="text-sm text-muted-foreground">{t("dashboardOpenAiHubDesc")}</div>
              <Button variant="ghost" size="sm" className={cardActionGhost} onClick={() => onOpenAiTab("sandboxes")}>
                {t("dashboardOpenAiHub")}
              </Button>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}

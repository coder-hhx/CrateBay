import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/lib/i18n";
import { APP_VERSION } from "@/lib/constants";
import {
  Activity,
  Box,
  Boxes,
  Circle,
  Database,
  Layers,
  Network,
  Settings,
  type LucideIcon,
} from "lucide-react";

type PageId = "dashboard" | "containers" | "images" | "pods" | "volumes" | "networks" | "settings";

interface NavItem {
  id: PageId;
  labelKey: "dashboard" | "containers" | "images" | "pods" | "volumes" | "networks" | "settings";
  icon: LucideIcon;
}

const navItems: NavItem[] = [
  { id: "dashboard", labelKey: "dashboard", icon: Activity },
  { id: "containers", labelKey: "containers", icon: Box },
  { id: "images", labelKey: "images", icon: Layers },
  { id: "pods", labelKey: "pods", icon: Boxes },
  { id: "volumes", labelKey: "volumes", icon: Database },
  { id: "networks", labelKey: "networks", icon: Network },
  { id: "settings", labelKey: "settings", icon: Settings },
];

export function Sidebar() {
  const { t } = useI18n();
  const currentPage = useAppStore((s) => s.currentPage);
  const setCurrentPage = useAppStore((s) => s.setCurrentPage);
  const engineConnected = useAppStore((s) => s.engineConnected);
  const runtimeStatus = useAppStore((s) => s.runtimeStatus);
  const engineLabel = getEngineLabel(engineConnected, runtimeStatus, t);

  return (
    <div className="flex h-full w-full flex-col bg-card/95">
      <div className="px-3 pb-3 pt-10" data-tauri-drag-region>
        <div className="flex items-center gap-2.5">
          <img
            src="/logo.png"
            alt="CrateBay"
            className="h-7 w-7 flex-shrink-0"
            draggable={false}
          />
          <div className="min-w-0">
            <div
              data-testid="app-title"
              className="truncate text-sm font-semibold leading-4 text-foreground"
            >
              CrateBay
            </div>
            <div className="mt-0.5 text-[10px] leading-3 text-muted-foreground">
              v{APP_VERSION}
            </div>
          </div>
        </div>
      </div>

      <nav className="flex flex-col gap-0.5 px-2">
        {navItems.map((item) => {
          const Icon = item.icon;
          const label = t("nav", item.labelKey);
          const active = currentPage === item.id;
          return (
            <button
              key={item.id}
              onClick={() => setCurrentPage(item.id)}
              data-testid={`nav-${item.id}`}
              aria-current={active ? "page" : undefined}
              className={cn(
                "flex h-8 w-full items-center gap-2 rounded-md px-2.5 text-[13px] transition-colors focus:outline-none",
                active
                  ? "bg-accent font-medium text-foreground"
                  : "text-muted-foreground hover:bg-accent/70 hover:text-foreground",
              )}
            >
              <Icon className={cn("h-4 w-4 flex-shrink-0", active ? "text-primary" : "text-muted-foreground")} />
              <span className="truncate">{label}</span>
            </button>
          );
        })}
      </nav>
      <div className="flex-1" />

      <div className="border-t border-border px-3 py-3 text-[11px] text-muted-foreground">
        <div className="flex items-center gap-1.5">
          <EngineStatusDot connected={engineConnected} status={runtimeStatus} />
          <span className="truncate">{engineLabel}</span>
        </div>
        <div className="mt-2 flex items-center justify-between gap-2">
          <span className="truncate">{t("statusbar", "engine")}</span>
          <span className="tabular-nums">v{APP_VERSION}</span>
        </div>
      </div>
    </div>
  );
}

function EngineStatusDot({ connected, status }: { connected: boolean; status: string }) {
  let colorClass = "bg-zinc-400";
  let pulse = false;
  if (connected) {
    colorClass = "text-emerald-400";
  } else if (status === "starting") {
    colorClass = "text-yellow-400";
    pulse = true;
  } else if (status === "error") {
    colorClass = "text-red-400";
  } else {
    colorClass = "text-zinc-400";
  }
  return (
    <Circle className={cn("h-2.5 w-2.5 fill-current stroke-0", colorClass, pulse && "animate-pulse")} />
  );
}

function getEngineLabel(connected: boolean, status: string, t: (namespace: string, key: string) => string): string {
  if (connected) return t("statusbar", "engineReady");
  if (status === "starting") return t("statusbar", "engineStarting");
  if (status === "error") return t("statusbar", "engineError");
  return t("statusbar", "engineDisconnected");
}

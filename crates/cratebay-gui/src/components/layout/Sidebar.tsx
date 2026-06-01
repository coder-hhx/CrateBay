import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/lib/i18n";
import { APP_VERSION } from "@/lib/constants";
import {
  Box,
  Layers,
  Settings,
  type LucideIcon,
} from "lucide-react";

type PageId = "containers" | "images" | "settings";

interface NavItem {
  id: PageId;
  labelKey: "containers" | "images" | "settings";
  icon: LucideIcon;
}

const navItems: NavItem[] = [
  { id: "containers", labelKey: "containers", icon: Box },
  { id: "images", labelKey: "images", icon: Layers },
  { id: "settings", labelKey: "settings", icon: Settings },
];

export function Sidebar() {
  const { t } = useI18n();
  const currentPage = useAppStore((s) => s.currentPage);
  const setCurrentPage = useAppStore((s) => s.setCurrentPage);
  const dockerConnected = useAppStore((s) => s.dockerConnected);
  const runtimeStatus = useAppStore((s) => s.runtimeStatus);

  return (
    <div className="flex h-full w-full flex-col bg-card">
      {/* Logo header — aligned with TopBar breadcrumb row */}
      <div className="flex items-center gap-2.5 px-3 pb-4 pt-[42px]" data-tauri-drag-region>
        <img
          src="/logo.png"
          alt="CrateBay"
          className="h-7 w-7 flex-shrink-0"
          draggable={false}
        />
        <span
          data-testid="app-title"
          className="bg-gradient-to-r from-blue-500 to-purple-500 bg-clip-text text-sm font-semibold text-transparent"
        >
          CrateBay
        </span>
        <span className="text-[10px] tabular-nums text-muted-foreground">v{APP_VERSION}</span>
      </div>

      {/* Navigation items */}
      <nav className="flex flex-col gap-1 px-3 pt-2">
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
                "flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-sm transition-colors focus:outline-none",
                active
                  ? "bg-primary/10 font-medium text-primary"
                  : "text-muted-foreground hover:bg-muted hover:text-foreground",
              )}
            >
              <Icon className="h-4 w-4 flex-shrink-0" />
              <span className="truncate">{label}</span>
            </button>
          );
        })}
      </nav>
      <div className="flex-1" />

      {/* Engine status at bottom */}
      <div className="flex items-center gap-1.5 px-4 py-2.5 text-[11px] text-muted-foreground">
        <EngineStatusDot connected={dockerConnected} status={runtimeStatus} />
        <span>{getEngineLabel(dockerConnected, runtimeStatus, t)}</span>
      </div>
    </div>
  );
}

function EngineStatusDot({ connected, status }: { connected: boolean; status: string }) {
  let colorClass = "bg-zinc-400";
  let pulse = false;
  if (connected) {
    colorClass = "bg-emerald-400 shadow-[0_0_6px_2px_rgba(52,211,153,0.5)]";
  } else if (status === "starting") {
    colorClass = "bg-yellow-400 shadow-[0_0_6px_2px_rgba(250,204,21,0.5)]";
    pulse = true;
  } else if (status === "error") {
    colorClass = "bg-red-400 shadow-[0_0_6px_2px_rgba(248,113,113,0.5)]";
  }
  return (
    <span className={cn("inline-block h-2 w-2 rounded-full", colorClass, pulse && "animate-pulse")} />
  );
}

function getEngineLabel(connected: boolean, status: string, t: (namespace: string, key: string) => string): string {
  if (connected) return t("statusbar", "engineReady");
  if (status === "starting") return t("statusbar", "engineStarting");
  if (status === "error") return t("statusbar", "engineError");
  return t("statusbar", "engineDisconnected");
}

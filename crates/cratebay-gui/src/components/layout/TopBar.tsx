/**
 * TopBar — Top navigation bar with breadcrumbs.
 */

import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import {
  Circle,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";
import { cn } from "@/lib/utils";

type PageId = "dashboard" | "containers" | "images" | "pods" | "volumes" | "networks" | "settings";

export function TopBar() {
  const { t } = useI18n();
  const currentPage = useAppStore((s) => s.currentPage);
  const sidebarOpen = useAppStore((s) => s.sidebarOpen);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);
  const engineConnected = useAppStore((s) => s.engineConnected);
  const runtimeStatus = useAppStore((s) => s.runtimeStatus);

  return (
    <header className="flex flex-shrink-0 flex-col">
      <div className="flex h-12 items-center gap-3 border-b border-border bg-background/95 px-3" data-tauri-drag-region>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={toggleSidebar}
          aria-label={sidebarOpen ? t("topbar", "collapseSidebar") : t("topbar", "expandSidebar")}
        >
          {sidebarOpen ? (
            <PanelLeftClose className="h-4 w-4" />
          ) : (
            <PanelLeftOpen className="h-4 w-4" />
          )}
        </Button>

        <DefaultTopBarContent currentPage={currentPage} />
        <div className="ml-auto min-w-0">
          <EnginePill connected={engineConnected} status={runtimeStatus} />
        </div>
      </div>
    </header>
  );
}

/**
 * Default breadcrumb content for app pages.
 */
function DefaultTopBarContent({ currentPage }: { currentPage: PageId }) {
  const { t } = useI18n();
  return (
    <div className="flex min-w-0 items-center gap-1.5 text-sm">
      <span className="hidden text-muted-foreground sm:inline">CrateBay</span>
      <span className="hidden text-muted-foreground sm:inline">/</span>
      <span className="font-medium text-foreground">
        {t("nav", currentPage)}
      </span>
    </div>
  );
}

function EnginePill({
  connected,
  status,
}: {
  connected: boolean;
  status: "starting" | "running" | "stopped" | "error";
}) {
  const { t } = useI18n();
  const label = connected
    ? t("statusbar", "engineReady")
    : status === "starting"
      ? t("statusbar", "engineStarting")
      : status === "error"
        ? t("statusbar", "engineError")
        : t("statusbar", "engineDisconnected");
  const dotClass = connected
    ? "text-emerald-400"
    : status === "starting"
      ? "text-yellow-400"
      : status === "error"
        ? "text-red-400"
        : "text-zinc-400";

  return (
    <div className="flex max-w-[40vw] items-center gap-1.5 rounded-md border border-border bg-card px-2 py-1 text-[11px] text-muted-foreground sm:max-w-[220px]">
      <Circle className={cn("h-2.5 w-2.5 fill-current stroke-0", dotClass, status === "starting" && "animate-pulse")} />
      <span className="truncate">{label}</span>
    </div>
  );
}

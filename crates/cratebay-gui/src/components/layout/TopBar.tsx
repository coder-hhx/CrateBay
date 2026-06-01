/**
 * TopBar — Top navigation bar with breadcrumbs.
 */

import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import {
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";

export function TopBar() {
  const { t } = useI18n();
  const currentPage = useAppStore((s) => s.currentPage);
  const sidebarOpen = useAppStore((s) => s.sidebarOpen);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);

  return (
    <header className="flex flex-shrink-0 flex-col">
      <div className="flex items-center gap-3 border-b border-border px-4 pb-2 pt-4" data-tauri-drag-region>
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
      </div>
    </header>
  );
}

/**
 * Default breadcrumb content for app pages.
 */
function DefaultTopBarContent({ currentPage }: { currentPage: string }) {
  const { t } = useI18n();
  return (
    <div className="flex items-center gap-1.5 text-sm">
      <span className="text-muted-foreground">CrateBay</span>
      <span className="text-muted-foreground">/</span>
      <span className="font-medium text-foreground">
        {t("nav", currentPage as "containers" | "images" | "settings")}
      </span>
    </div>
  );
}

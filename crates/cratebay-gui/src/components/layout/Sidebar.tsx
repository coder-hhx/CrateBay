import { cn } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import { useChatStore } from "@/stores/chatStore";
import { useI18n } from "@/lib/i18n";
import { APP_VERSION } from "@/lib/constants";
import {
  MessageSquare,
  Box,
  Layers,
  Settings,
  Plus,
  Trash2,
  type LucideIcon,
} from "lucide-react";

type PageId = "chat" | "containers" | "images" | "settings";

interface NavItem {
  id: PageId;
  labelKey: "chat" | "containers" | "images" | "settings";
  icon: LucideIcon;
}

const navItems: NavItem[] = [
  { id: "chat", labelKey: "chat", icon: MessageSquare },
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
  const sessions = useChatStore((s) => s.sessions);
  const activeSessionId = useChatStore((s) => s.activeSessionId);
  const setActiveSession = useChatStore((s) => s.setActiveSession);
  const createSession = useChatStore((s) => s.createSession);
  const deleteSession = useChatStore((s) => s.deleteSession);

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

      {/* Conversation history (only when on Chat page) */}
      {currentPage === "chat" && (
        <div className="mt-3 flex min-h-0 flex-1 flex-col px-3">
          <div className="mx-0 mb-2">
            <div className="h-px bg-border" />
          </div>
          <div className="mb-2 flex items-center justify-between">
            <span className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
              {t("sidebar", "conversations")}
            </span>
            <button
              type="button"
              onClick={() => void createSession()}
              data-testid="new-session"
              className="flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus:outline-none"
              title={t("sidebar", "newConversation")}
            >
              <Plus className="h-3.5 w-3.5" />
            </button>
          </div>

          <div
            className="flex flex-1 flex-col gap-0.5 overflow-y-auto"
            data-testid="session-list"
          >
            {sessions.length === 0 ? (
              <p className="py-4 text-center text-xs text-muted-foreground">
                {t("sidebar", "noConversations")}
              </p>
            ) : (
              sessions.map((session) => (
                <div
                  key={session.id}
                  data-testid="session-item"
                  className={cn(
                    "group relative flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left transition-colors",
                    session.id === activeSessionId
                      ? "bg-primary/10 text-primary"
                      : "text-muted-foreground hover:bg-muted hover:text-foreground",
                  )}
                >
                  <button
                    type="button"
                    onClick={() => void setActiveSession(session.id)}
                    className="flex min-w-0 flex-1 items-center gap-2 focus:outline-none"
                  >
                    <MessageSquare className="h-3.5 w-3.5 flex-shrink-0" />
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-xs font-medium">
                        {session.title}
                      </div>
                      <div className="truncate text-[10px] opacity-60">
                        {formatSessionTime(session.updatedAt)}
                      </div>
                    </div>
                  </button>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      void deleteSession(session.id);
                    }}
                    data-testid="session-delete"
                    className="flex h-5 w-5 flex-shrink-0 items-center justify-center rounded opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive focus:outline-none group-hover:opacity-100"
                    title={t("sidebar", "deleteConversation")}
                  >
                    <Trash2 className="h-3 w-3" />
                  </button>
                </div>
              ))
            )}
          </div>
        </div>
      )}

      {/* Spacer */}
      {currentPage !== "chat" && <div className="flex-1" />}

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

function formatSessionTime(isoString: string): string {
  try {
    const date = new Date(isoString);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMin = Math.floor(diffMs / 60000);
    const diffHour = Math.floor(diffMs / 3600000);
    const diffDay = Math.floor(diffMs / 86400000);

    if (diffMin < 1) return "刚刚";
    if (diffMin < 60) return `${diffMin} 分钟前`;
    if (diffHour < 24) return `${diffHour} 小时前`;
    if (diffDay < 7) return `${diffDay} 天前`;
    return date.toLocaleDateString("zh-CN", { month: "short", day: "numeric" });
  } catch {
    return "";
  }
}

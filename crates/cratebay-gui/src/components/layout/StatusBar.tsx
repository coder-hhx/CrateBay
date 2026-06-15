import { useAppStore } from "@/stores/appStore";
import { useI18n } from "@/lib/i18n";
import { cn } from "@/lib/utils";
import { APP_VERSION } from "@/lib/constants";

export function StatusBar() {
  const engineConnected = useAppStore((s) => s.engineConnected);
  const runtimeStatus = useAppStore((s) => s.runtimeStatus);
  const { t } = useI18n();

  // Derive a single unified engine status
  const { color, label, pulse } = getEngineStatus(engineConnected, runtimeStatus, t);

  return (
    <footer className="flex h-7 flex-shrink-0 items-center justify-between border-t border-border px-4 text-[11px] text-muted-foreground">
      {/* Left: unified engine status */}
      <div className="flex items-center gap-1.5">
        <GlowDot color={color} pulse={pulse} />
        <span>{label}</span>
      </div>

      {/* Right: version */}
      <span className="tabular-nums">v{APP_VERSION}</span>
    </footer>
  );
}

/**
 * Derive a single status from engine + runtime states.
 */
function getEngineStatus(
  engineConnected: boolean,
  runtimeStatus: "starting" | "running" | "stopped" | "error",
  t: (namespace: string, key: string) => string,
): { color: "green" | "red" | "yellow" | "gray"; label: string; pulse: boolean } {
  // The compatibility API is connected via the built-in CrateBay Engine.
  if (engineConnected) {
    return { color: "green", label: t("statusbar", "engineReady"), pulse: false };
  }
  // Builtin runtime is trying to start
  if (runtimeStatus === "starting") {
    return { color: "yellow", label: t("statusbar", "engineStarting"), pulse: true };
  }
  if (runtimeStatus === "error") {
    return { color: "red", label: t("statusbar", "engineError"), pulse: false };
  }
  return { color: "gray", label: t("statusbar", "engineDisconnected"), pulse: false };
}

function GlowDot({ color, pulse }: { color: "green" | "red" | "yellow" | "gray"; pulse?: boolean }) {
  const styles: Record<typeof color, string> = {
    green: "bg-emerald-400",
    red: "bg-red-400",
    yellow: "bg-yellow-400",
    gray: "bg-zinc-400",
  };

  return (
    <span
      className={cn(
        "inline-block h-2 w-2 rounded-full",
        styles[color],
        pulse && "animate-pulse",
      )}
    />
  );
}

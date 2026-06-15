import { Loader2, Play } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useI18n } from "@/lib/i18n";

export function EngineOfflineCallout({
  starting,
  onStart,
}: {
  starting: boolean;
  onStart: () => void;
}) {
  const { t } = useI18n();

  return (
    <div className="flex flex-wrap items-center gap-3 rounded-md border border-border bg-card px-3 py-2.5 text-xs">
      <div className="min-w-0">
        <div className="font-medium text-foreground">{t("dashboard", "engineOfflineTitle")}</div>
        <div className="mt-0.5 text-muted-foreground">{t("dashboard", "engineOfflineDesc")}</div>
      </div>
      <Button className="ml-auto" size="sm" onClick={onStart} disabled={starting}>
        {starting ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Play className="h-3.5 w-3.5" />}
        {t("dashboard", "startEngine")}
      </Button>
    </div>
  );
}

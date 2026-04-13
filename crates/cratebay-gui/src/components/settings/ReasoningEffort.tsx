import { useSettingsStore, type ReasoningLevel, REASONING_LEVEL_LABELS } from "@/stores/settingsStore";
import { cn } from "@/lib/utils";

const LEVELS = Object.entries(REASONING_LEVEL_LABELS) as [ReasoningLevel, string][];

export function ReasoningEffort() {
  const reasoningEffort = useSettingsStore((s) => s.settings.reasoningEffort);
  const updateSettings = useSettingsStore((s) => s.updateSettings);

  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-sm font-medium text-foreground">Reasoning Effort</span>
      <p className="text-xs text-muted-foreground">
        Controls how much reasoning the model performs. Applies to models that support reasoning.
      </p>
      <div className="mt-1 inline-flex rounded-md border border-border bg-muted p-0.5">
        {LEVELS.map(([value, label]) => (
          <button
            key={value}
            onClick={() => void updateSettings({ reasoningEffort: value })}
            className={cn(
              "rounded-sm px-3 py-1 text-xs font-medium transition-colors",
              reasoningEffort === value
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}

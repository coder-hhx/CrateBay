import { useState, useEffect, useRef } from "react";
import { useI18n } from "@/lib/i18n";
import { Sparkles, ChevronRight } from "lucide-react";

interface ThinkingBlockProps {
  content: string;
  isActive: boolean;
}

/**
 * Collapsible thinking/reasoning block.
 *
 * - Auto-opens while streaming (isActive=true)
 * - Once the user manually collapses it, stays collapsed even during streaming
 * - Sparkles icon + "Thinking..." label
 */
export function ThinkingBlock({ content, isActive }: ThinkingBlockProps) {
  const { t } = useI18n();
  const [isOpen, setIsOpen] = useState(isActive);
  const userInteractedRef = useRef(false);

  // Auto-open during streaming, but respect user's manual collapse
  useEffect(() => {
    if (!userInteractedRef.current && isActive) {
      setIsOpen(true);
    }
  }, [isActive]);

  if (!/\S/.test(content || "")) return null;

  return (
    <div
      data-testid="thinking-block"
      className="group/think my-2 rounded-lg border border-border/40 bg-muted/30"
    >
      <button
        type="button"
        aria-expanded={isOpen}
        onClick={() => {
          userInteractedRef.current = true;
          setIsOpen((prev) => !prev);
        }}
        className="flex w-full cursor-pointer select-none items-center gap-2 px-3 py-2 text-[13px] text-muted-foreground transition-colors hover:text-foreground"
      >
        <Sparkles className="h-3.5 w-3.5 text-muted-foreground/70" />
        <span className="font-medium">
          {isActive ? t("chat", "thinking") : t("chat", "reasoning")}
          {isActive && "..."}
        </span>
        <ChevronRight
          className={`ml-auto h-3 w-3 transition-transform ${isOpen ? "rotate-90" : ""}`}
        />
      </button>

      {isOpen && (
        <div className="border-t border-border/30 px-3 pb-3 pt-2">
          <pre className="max-h-64 overflow-auto whitespace-pre-wrap rounded-md bg-muted/40 p-3 text-[12.5px] leading-relaxed text-muted-foreground">
            {content}
          </pre>
        </div>
      )}
    </div>
  );
}

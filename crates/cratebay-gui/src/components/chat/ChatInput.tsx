import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSettingsStore } from "@/stores/settingsStore";
import { useChatStore } from "@/stores/chatStore";
import { useContainerStore } from "@/stores/containerStore";
import { useMcpStore } from "@/stores/mcpStore";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import { Send, Square, Paperclip, AtSign, Zap, ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";

const MAX_INPUT_HEIGHT = 200;
const MIN_INPUT_HEIGHT = 48;

interface ChatInputProps {
  onSend?: (message: string) => void;
  onStop?: () => void;
  disabled?: boolean;
  placeholder?: string;
}

interface MentionItem {
  category: "tool" | "container" | "mcp";
  prefix: string;
  label: string;
  value: string;
}

/**
 * Chat input with @mention autocomplete for referencing tools, containers, and MCP servers.
 *
 * Features:
 * - @mention autocomplete with popup
 * - Multi-line input: Shift+Enter for new line, Enter to send (configurable)
 * - Ctrl+Enter always sends, Escape clears
 * - Auto-resize textarea
 * - Model selector in toolbar
 * - Attachment button (UI only)
 */
export function ChatInput({ onSend, onStop, disabled, placeholder }: ChatInputProps) {
  const { t } = useI18n();
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const mentionListRef = useRef<HTMLDivElement>(null);
  const inputDraft = useChatStore((s) => s.inputDraft);
  const setInputDraft = useChatStore((s) => s.setInputDraft);
  const isStreaming = useChatStore((s) => s.isStreaming);
  const setStreaming = useChatStore((s) => s.setStreaming);
  const mentionQuery = useChatStore((s) => s.mentionQuery);
  const setMentionQuery = useChatStore((s) => s.setMentionQuery);
  const sendOnEnter = useSettingsStore((s) => s.settings.sendOnEnter);
  const activeModelId = useSettingsStore((s) => s.activeModelId);
  const setActiveProvider = useSettingsStore((s) => s.setActiveProvider);
  const setActiveModel = useSettingsStore((s) => s.setActiveModel);
  const enabledModels = useSettingsStore((s) => s.enabledModels);

  // Model selector state
  const [modelDropdownOpen, setModelDropdownOpen] = useState(false);

  // Mention autocomplete state
  const [mentionIndex, setMentionIndex] = useState(0);
  const [mentionStart, setMentionStart] = useState<number | null>(null);

  // Build mention items from stores
  const containers = useContainerStore((s) => s.containers);
  const mcpServers = useMcpStore((s) => s.servers);
  const mcpTools = useMcpStore((s) => s.availableTools);

  const providers = useSettingsStore((s) => s.providers);

  const allEnabledModels = useMemo(() => enabledModels(), [enabledModels]);
  const activeModelName = useMemo(() => {
    if (activeModelId === null) return t("chat", "selectModel");
    const model = allEnabledModels.find((m) => m.id === activeModelId);
    return model?.name ?? model?.id ?? t("chat", "selectModel");
  }, [activeModelId, allEnabledModels]);

  // Group enabled models by provider
  const groupedModels = useMemo(() => {
    const groups: { provider: { id: string; name: string }; models: typeof allEnabledModels }[] = [];
    const providerMap = new Map<string, typeof allEnabledModels>();
    for (const model of allEnabledModels) {
      const list = providerMap.get(model.providerId) ?? [];
      list.push(model);
      providerMap.set(model.providerId, list);
    }
    for (const [providerId, models] of providerMap) {
      const provider = providers.find((p) => p.id === providerId);
      groups.push({
        provider: { id: providerId, name: provider?.name ?? providerId },
        models,
      });
    }
    return groups;
  }, [allEnabledModels, providers]);

  const mentionItems: MentionItem[] = useMemo(() => {
    const items: MentionItem[] = [];

    // Container mentions
    for (const c of containers) {
      items.push({
        category: "container",
        prefix: "@container:",
        label: c.name,
        value: `@container:${c.name}`,
      });
    }

    // MCP server/tool mentions
    for (const s of mcpServers) {
      items.push({
        category: "mcp",
        prefix: "@mcp:",
        label: s.name,
        value: `@mcp:${s.name}`,
      });
    }
    for (const t of mcpTools) {
      items.push({
        category: "mcp",
        prefix: "@mcp:",
        label: `${t.serverName}/${t.name}`,
        value: `@mcp:${t.serverName}/${t.name}`,
      });
    }

    return items;
  }, [containers, mcpServers, mcpTools]);

  // Filter mentions by query
  const filteredMentions = useMemo(() => {
    if (mentionQuery === null) return [];
    const q = mentionQuery.toLowerCase();
    return mentionItems.filter(
      (item) =>
        item.label.toLowerCase().includes(q) || item.value.toLowerCase().includes(q),
    );
  }, [mentionQuery, mentionItems]);

  const canSend = inputDraft.trim().length > 0 && !isStreaming && disabled !== true;

  const resizeTextarea = useCallback(() => {
    const el = textareaRef.current;
    if (el === null) return;
    el.style.height = "auto";
    const newHeight = Math.min(Math.max(el.scrollHeight, MIN_INPUT_HEIGHT), MAX_INPUT_HEIGHT);
    el.style.height = `${newHeight}px`;
  }, []);

  useEffect(() => {
    resizeTextarea();
  }, [inputDraft, resizeTextarea]);

  // Close model dropdown when clicking outside
  useEffect(() => {
    if (!modelDropdownOpen) return;
    const handleClickOutside = () => setModelDropdownOpen(false);
    document.addEventListener("click", handleClickOutside);
    return () => document.removeEventListener("click", handleClickOutside);
  }, [modelDropdownOpen]);

  const handleSend = useCallback(() => {
    const text = inputDraft.trim();
    if (text.length === 0 || isStreaming || disabled === true) return;
    if (onSend !== undefined) {
      onSend(text);
    }
    setInputDraft("");
    setMentionQuery(null);
    setMentionStart(null);
  }, [inputDraft, isStreaming, disabled, onSend, setInputDraft, setMentionQuery]);

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const value = e.target.value;
      const cursorPos = e.target.selectionStart;
      setInputDraft(value);

      // Detect @mention
      const textBeforeCursor = value.slice(0, cursorPos);
      const atIndex = textBeforeCursor.lastIndexOf("@");
      if (atIndex !== -1) {
        const charBefore = atIndex > 0 ? textBeforeCursor[atIndex - 1] : " ";
        // Only trigger if @ is at start or preceded by whitespace
        if (charBefore === " " || charBefore === "\n" || atIndex === 0) {
          const query = textBeforeCursor.slice(atIndex + 1);
          // Don't trigger mention if there's a space after the query started
          if (!query.includes(" ") || query.includes(":")) {
            setMentionQuery(query);
            setMentionStart(atIndex);
            setMentionIndex(0);
            return;
          }
        }
      }
      setMentionQuery(null);
      setMentionStart(null);
    },
    [setInputDraft, setMentionQuery],
  );

  const insertMention = useCallback(
    (item: MentionItem) => {
      if (mentionStart === null) return;
      const before = inputDraft.slice(0, mentionStart);
      const after = inputDraft.slice(textareaRef.current?.selectionStart ?? inputDraft.length);
      const newValue = `${before}${item.value} ${after}`;
      setInputDraft(newValue);
      setMentionQuery(null);
      setMentionStart(null);
      // Focus back to textarea
      setTimeout(() => {
        const el = textareaRef.current;
        if (el !== null) {
          const pos = mentionStart + item.value.length + 1;
          el.focus();
          el.setSelectionRange(pos, pos);
        }
      }, 0);
    },
    [inputDraft, mentionStart, setInputDraft, setMentionQuery],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Mention navigation
      if (mentionQuery !== null && filteredMentions.length > 0) {
        if (e.key === "ArrowDown") {
          e.preventDefault();
          setMentionIndex((prev) => (prev + 1) % filteredMentions.length);
          return;
        }
        if (e.key === "ArrowUp") {
          e.preventDefault();
          setMentionIndex((prev) => (prev - 1 + filteredMentions.length) % filteredMentions.length);
          return;
        }
        if (e.key === "Enter" || e.key === "Tab") {
          e.preventDefault();
          insertMention(filteredMentions[mentionIndex]);
          return;
        }
        if (e.key === "Escape") {
          e.preventDefault();
          setMentionQuery(null);
          setMentionStart(null);
          return;
        }
      }

      // Ctrl+Enter always sends
      if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        handleSend();
        return;
      }

      // Enter to send (if configured)
      if (e.key === "Enter" && !e.shiftKey && sendOnEnter) {
        e.preventDefault();
        handleSend();
        return;
      }

      // Escape clears
      if (e.key === "Escape") {
        setInputDraft("");
        setMentionQuery(null);
        setMentionStart(null);
      }
    },
    [
      mentionQuery,
      filteredMentions,
      mentionIndex,
      sendOnEnter,
      handleSend,
      insertMention,
      setInputDraft,
      setMentionQuery,
    ],
  );

  return (
    <div className="relative bg-gradient-to-t from-background via-background to-transparent px-4 pb-6 pt-4">
      {/* Mention autocomplete popup */}
      {mentionQuery !== null && filteredMentions.length > 0 && (
        <div
          ref={mentionListRef}
          className="absolute bottom-full left-1/2 mb-1 max-h-48 w-full max-w-[400px] -translate-x-1/2 overflow-y-auto rounded-lg border border-border bg-card shadow-lg"
        >
          {filteredMentions.map((item, idx) => (
            <button
              key={item.value}
              type="button"
              onClick={() => insertMention(item)}
              className={cn(
                "flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs focus:outline-none",
                idx === mentionIndex
                  ? "bg-primary/10 text-primary"
                  : "text-foreground hover:bg-muted",
              )}
            >
              <MentionCategoryBadge category={item.category} />
              <span>{item.label}</span>
            </button>
          ))}
        </div>
      )}

      <div className="mx-auto max-w-[800px]">
        <div className="flex flex-col overflow-hidden rounded-xl border border-border bg-card transition-all duration-150 focus-within:border-primary focus-within:ring-2 focus-within:ring-primary/30">
          {/* Textarea */}
          <textarea
            ref={textareaRef}
            data-testid="chat-input"
            value={inputDraft}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            placeholder={placeholder ?? t("chat", "placeholder")}
            className={cn(
              "w-full resize-none bg-transparent px-4 py-3 text-sm text-foreground placeholder:text-muted-foreground",
              "outline-none",
            )}
            style={{
              minHeight: `${MIN_INPUT_HEIGHT}px`,
              maxHeight: `${MAX_INPUT_HEIGHT}px`,
            }}
            rows={1}
            disabled={isStreaming || disabled === true}
          />

          {/* Toolbar */}
          <div className="flex items-center justify-between px-3 py-2">
            <div className="flex items-center gap-1">
              {/* Attachment button (UI only) */}
              <button
                type="button"
                className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                title={t("chat", "addAttachment")}
              >
                <Paperclip className="h-4 w-4" />
              </button>

              {/* @ mention button */}
              <button
                type="button"
                className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                title={t("chat", "mentionHint")}
                onClick={() => {
                  setInputDraft(inputDraft + "@");
                  textareaRef.current?.focus();
                }}
              >
                <AtSign className="h-4 w-4" />
              </button>

              {/* Model selector */}
              <div className="relative">
                <button
                  type="button"
                  className="flex items-center gap-1 rounded-md px-2 py-1 text-xs text-muted-foreground transition-all duration-150 hover:bg-muted hover:text-foreground"
                  onClick={(e) => {
                    e.stopPropagation();
                    setModelDropdownOpen(!modelDropdownOpen);
                  }}
                >
                  <Zap className="h-3.5 w-3.5" />
                  <span className="max-w-[120px] truncate">{activeModelName}</span>
                  <ChevronDown className="h-3 w-3" />
                </button>

                {/* Model dropdown — grouped by provider */}
                {modelDropdownOpen && groupedModels.length > 0 && (
                  <div className="absolute bottom-full left-0 mb-1 max-h-60 min-w-[240px] overflow-y-auto rounded-lg border border-border bg-card shadow-lg">
                    {groupedModels.map((group) => (
                      <div key={group.provider.id}>
                        <div className="sticky top-0 bg-card px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground border-b border-border">
                          {group.provider.name}
                        </div>
                        {group.models.map((model) => (
                          <button
                            key={model.id}
                            type="button"
                            className={cn(
                              "flex w-full items-center gap-2 px-3 py-2 text-left text-xs transition-colors",
                              model.id === activeModelId
                                ? "bg-primary/10 text-primary"
                                : "text-foreground hover:bg-muted",
                            )}
                            onClick={(e) => {
                              e.stopPropagation();
                              setActiveProvider(model.providerId);
                              setActiveModel(model.id);
                              setModelDropdownOpen(false);
                            }}
                          >
                            <Zap className="h-3 w-3 flex-shrink-0" />
                            <span className="truncate">{model.name || model.id}</span>
                          </button>
                        ))}
                      </div>
                    ))}
                  </div>
                )}
                {/* Empty state */}
                {modelDropdownOpen && groupedModels.length === 0 && (
                  <div className="absolute bottom-full left-0 mb-1 min-w-[240px] rounded-lg border border-border bg-card p-3 shadow-lg">
                    <p className="text-xs text-muted-foreground text-center">
                      No models available. Add a provider in Settings and fetch models.
                    </p>
                  </div>
                )}
              </div>
            </div>

            {/* Send button */}
            <Button
              size="icon-sm"
              variant={canSend ? "default" : "ghost"}
              data-testid="send-button"
              onClick={isStreaming ? (onStop ?? (() => setStreaming(false))) : handleSend}
              disabled={!canSend && !isStreaming}
              aria-label={isStreaming ? t("chat", "stopButton") : t("chat", "sendButton")}
              className={cn(
                "h-9 w-9 flex-shrink-0 rounded-lg",
                canSend && "bg-primary text-white hover:bg-primary/90 shadow-sm",
              )}
            >
              {isStreaming ? (
                <Square className="h-4 w-4" />
              ) : (
                <Send className="h-4 w-4" />
              )}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}

function MentionCategoryBadge({ category }: { category: MentionItem["category"] }) {
  const config = {
    tool: { label: "Tool", className: "bg-primary/20 text-primary" },
    container: { label: "Container", className: "bg-success/20 text-success" },
    mcp: { label: "MCP", className: "bg-accent/20 text-accent" },
  }[category];

  return (
    <span
      className={cn(
        "inline-flex items-center rounded-full px-1.5 py-0.5 text-[10px] font-medium",
        config.className,
      )}
    >
      {config.label}
    </span>
  );
}

import { useCallback, useEffect, useRef, useState } from "react";

import { invoke, listen } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Send, X } from "lucide-react";

interface TerminalViewProps {
  containerId: string;
  onClose?: () => void;
}

type ExecStreamChunk =
  | { type: "Stdout"; data: string }
  | { type: "Stderr"; data: string }
  | { type: "Done"; exit_code: number }
  | { type: "Error"; message: string };

type TerminalLine =
  | { id: number; kind: "command"; text: string }
  | { id: number; kind: "stdout"; text: string }
  | { id: number; kind: "stderr"; text: string }
  | { id: number; kind: "meta"; text: string };

let lineCounter = 0;

/**
 * Container exec terminal.
 *
 * Minimal, real terminal-like experience:
 * - Enter a command → runs via `container_exec_stream` using `sh -lc <cmd>`
 * - Streams stdout/stderr into the output area
 * - Maintains a simple in-memory history (ArrowUp/ArrowDown)
 */
export function TerminalView({ containerId, onClose }: TerminalViewProps) {
  const { t } = useI18n();
  const [lines, setLines] = useState<TerminalLine[]>([]);
  const [command, setCommand] = useState("");
  const [isExecuting, setIsExecuting] = useState(false);
  const [commandHistory, setCommandHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const bottomRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const mountedRef = useRef(true);

  // Auto-scroll to bottom
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ block: "end" });
  }, [lines]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const executeCommand = useCallback(
    async (raw: string) => {
      const trimmed = raw.trim();
      if (trimmed.length === 0 || isExecuting) return;

      setLines((prev) => [
        ...prev,
        { id: ++lineCounter, kind: "command", text: `$ ${trimmed}` },
      ]);
      setCommandHistory((prev) => [...prev, trimmed]);
      setHistoryIndex(-1);
      setCommand("");
      setIsExecuting(true);

      const channelId = newChannelId();
      const eventName = `exec:stream:${channelId}`;

      let unlisten: (() => void) | null = null;

      try {
        let done = false;
        let resolveDone: (() => void) | null = null;
        const donePromise = new Promise<void>((resolve) => {
          resolveDone = resolve;
        });

        unlisten = await listen<ExecStreamChunk>(eventName, (chunk) => {
          if (!mountedRef.current) return;
          switch (chunk.type) {
            case "Stdout":
              setLines((prev) => [
                ...prev,
                { id: ++lineCounter, kind: "stdout", text: chunk.data },
              ]);
              break;
            case "Stderr":
              setLines((prev) => [
                ...prev,
                { id: ++lineCounter, kind: "stderr", text: chunk.data },
              ]);
              break;
            case "Done":
              setLines((prev) => [
                ...prev,
                {
                  id: ++lineCounter,
                  kind: "meta",
                  text: `[exit code: ${chunk.exit_code}]`,
                },
              ]);
              if (!done) {
                done = true;
                resolveDone?.();
              }
              break;
            case "Error":
              setLines((prev) => [
                ...prev,
                { id: ++lineCounter, kind: "meta", text: `[error: ${chunk.message}]` },
              ]);
              if (!done) {
                done = true;
                resolveDone?.();
              }
              break;
          }
        });

        await invoke<void>("container_exec_stream", {
          id: containerId,
          cmd: ["sh", "-lc", trimmed],
          channel_id: channelId,
        });

        // Ensure we captured the final Done/Error chunk.
        await Promise.race([
          donePromise,
          new Promise<void>((resolve) => window.setTimeout(resolve, 5000)),
        ]);
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        if (mountedRef.current) {
          setLines((prev) => [
            ...prev,
            { id: ++lineCounter, kind: "meta", text: `[error: ${message}]` },
          ]);
        }
      } finally {
        unlisten?.();
        if (mountedRef.current) {
          setIsExecuting(false);
          inputRef.current?.focus();
        }
      }
    },
    [containerId, isExecuting],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter" && !isExecuting) {
        void executeCommand(command);
        return;
      }

      // History navigation
      if (e.key === "ArrowUp" && commandHistory.length > 0) {
        e.preventDefault();
        const newIndex =
          historyIndex === -1
            ? commandHistory.length - 1
            : Math.max(0, historyIndex - 1);
        setHistoryIndex(newIndex);
        setCommand(commandHistory[newIndex]);
        return;
      }

      if (e.key === "ArrowDown" && commandHistory.length > 0) {
        e.preventDefault();
        if (historyIndex === -1) return;
        const newIndex = historyIndex + 1;
        if (newIndex >= commandHistory.length) {
          setHistoryIndex(-1);
          setCommand("");
        } else {
          setHistoryIndex(newIndex);
          setCommand(commandHistory[newIndex]);
        }
      }
    },
    [command, commandHistory, executeCommand, historyIndex, isExecuting],
  );

  return (
    <div
      className="flex h-full flex-col overflow-hidden rounded-lg border border-border bg-zinc-950"
      data-testid="container-terminal"
    >
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border/60 px-3 py-2">
        <span className="text-xs font-medium text-zinc-400">
          {t("containers", "terminal")} - {containerId.slice(0, 12)}
        </span>
        {onClose !== undefined && (
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={onClose}
            aria-label="Close terminal"
          >
            <X className="h-3.5 w-3.5" />
          </Button>
        )}
      </div>

      {/* Output */}
      <ScrollArea className="max-h-64 p-3 font-mono text-xs leading-5">
        {lines.length === 0 ? (
          <div className="text-zinc-500">{t("containers", "terminalHint")}</div>
        ) : (
          lines.map((line) => (
            <div
              key={line.id}
              className={cn(
                "whitespace-pre-wrap break-all",
                line.kind === "command" && "text-zinc-400",
                line.kind === "stdout" && "text-zinc-200",
                line.kind === "stderr" && "text-red-300",
                line.kind === "meta" && "text-zinc-500",
              )}
            >
              {line.text}
            </div>
          ))
        )}
        <div ref={bottomRef} />
      </ScrollArea>

      {/* Input */}
      <div className="flex items-center gap-2 border-t border-border/60 p-2">
        <span className="text-xs font-medium text-primary">$</span>
        <Input
          ref={inputRef}
          data-testid="terminal-input"
          value={command}
          onChange={(e) => setCommand(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="sh -lc <command>"
          disabled={isExecuting}
          className="h-7 flex-1 border-none bg-transparent font-mono text-xs shadow-none focus-visible:ring-0"
          autoFocus
        />
        <Button
          variant="secondary"
          size="icon-xs"
          onClick={() => void executeCommand(command)}
          disabled={isExecuting || command.trim().length === 0}
          aria-label={t("containers", "executeCommand")}
        >
          <Send className="h-3 w-3" />
        </Button>
      </div>
    </div>
  );
}

function newChannelId(): string {
  const cryptoObj = typeof crypto !== "undefined" ? crypto : undefined;
  if (cryptoObj && "randomUUID" in cryptoObj && typeof cryptoObj.randomUUID === "function") {
    return cryptoObj.randomUUID();
  }
  return `exec-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

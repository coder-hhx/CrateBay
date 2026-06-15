import "@xterm/xterm/css/xterm.css";

import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { useEffect, useRef, useState } from "react";

import { invoke, listen } from "@/lib/tauri";
import { useI18n } from "@/lib/i18n";
import { Button } from "@/components/ui/button";
import { Loader2, RefreshCw, X } from "lucide-react";

interface TerminalViewProps {
  containerId: string;
  onClose?: () => void;
}

type TerminalStreamChunk =
  | { type: "Output"; data: string }
  | { type: "Done"; exit_code: number }
  | { type: "Error"; message: string };

export function TerminalView({ containerId, onClose }: TerminalViewProps) {
  const { t } = useI18n();
  const hostRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const [sessionNonce, setSessionNonce] = useState(0);
  const [connecting, setConnecting] = useState(true);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    const terminal = new Terminal({
      cursorBlink: true,
      convertEol: true,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
      fontSize: 12,
      theme: {
        background: "#09090b",
        foreground: "#e4e4e7",
        cursor: "#22d3ee",
      },
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(host);
    fit.fit();
    terminal.focus();
    terminal.writeln("\x1b[90mConnecting to container shell...\x1b[0m");
    terminalRef.current = terminal;

    const sessionId = `tty-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const eventName = `terminal:stream:${sessionId}`;
    let disposed = false;
    let unlisten: (() => void) | null = null;

    const openSession = async () => {
      setConnecting(true);
      setConnected(false);
      setError(null);
      try {
        unlisten = await listen<TerminalStreamChunk>(eventName, (chunk) => {
          if (disposed) return;
          if (chunk.type === "Output") {
            terminal.write(chunk.data);
          } else if (chunk.type === "Done") {
            terminal.writeln(`\r\n\x1b[90m[session exited: ${chunk.exit_code}]\x1b[0m`);
            setConnected(false);
          } else {
            terminal.writeln(`\r\n\x1b[31m[terminal error: ${chunk.message}]\x1b[0m`);
            setError(chunk.message);
            setConnected(false);
          }
        });

        await invoke("container_terminal_open", {
          id: containerId,
          session_id: sessionId,
          cols: terminal.cols,
          rows: terminal.rows,
        });
        if (!disposed) {
          setConnected(true);
          terminal.clear();
          terminal.focus();
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        if (!disposed) {
          terminal.writeln(`\r\n\x1b[31m[terminal unavailable: ${message}]\x1b[0m`);
          setError(message);
        }
      } finally {
        if (!disposed) setConnecting(false);
      }
    };

    const dataDisposable = terminal.onData((data) => {
      void invoke("container_terminal_input", { session_id: sessionId, data }).catch((err) => {
        if (!disposed) setError(err instanceof Error ? err.message : String(err));
      });
    });
    const resizeDisposable = terminal.onResize(({ cols, rows }) => {
      void invoke("container_terminal_resize", { session_id: sessionId, cols, rows }).catch(() => {});
    });
    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(host);
    void openSession();

    return () => {
      disposed = true;
      observer.disconnect();
      dataDisposable.dispose();
      resizeDisposable.dispose();
      unlisten?.();
      terminal.dispose();
      terminalRef.current = null;
      void invoke("container_terminal_close", { session_id: sessionId }).catch(() => {});
    };
  }, [containerId, sessionNonce]);

  return (
    <div
      className="flex h-[300px] flex-col overflow-hidden rounded-md border border-border bg-zinc-950"
      data-testid="container-terminal"
    >
      <div className="flex h-9 items-center justify-between border-b border-border/60 px-3">
        <div className="flex min-w-0 items-center gap-2 text-xs text-zinc-400">
          {connecting ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : null}
          <span className={connected ? "text-emerald-400" : error ? "text-red-400" : ""}>
            {t("containers", "terminal")} - {containerId.slice(0, 12)}
          </span>
        </div>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={() => setSessionNonce((value) => value + 1)}
            aria-label={t("common", "retry")}
            title={t("common", "retry")}
          >
            <RefreshCw className="h-3.5 w-3.5" />
          </Button>
          {onClose !== undefined ? (
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={onClose}
              aria-label={t("common", "close")}
            >
              <X className="h-3.5 w-3.5" />
            </Button>
          ) : null}
        </div>
      </div>
      <div ref={hostRef} className="min-h-0 flex-1 p-2" />
    </div>
  );
}

import { useEffect, useRef } from "react";
import { useAppStore } from "@/stores/appStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { useChatStore } from "@/stores/chatStore";
import { useMcpStore } from "@/stores/mcpStore";
import { invoke, listen } from "@/lib/tauri";
import { AppLayout } from "@/components/layout/AppLayout";
import { ToastContainer } from "@/components/common/Toast";
import { ChatPage } from "@/pages/ChatPage";
import { ContainersPage } from "@/pages/ContainersPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { ImagesPage } from "@/pages/ImagesPage";

/** Backend DockerStatus shape (matches api-spec.md). */
interface DockerStatusResponse {
  connected: boolean;
  version?: string | null;
  api_version?: string | null;
  os?: string | null;
  arch?: string | null;
  source: string;
  socket_path?: string | null;
}

/** Backend RuntimeStatusInfo shape (matches api-spec.md). */
interface RuntimeStatusResponse {
  state: string;
  platform: string;
  cpu_cores: number;
  memory_mb: number;
  disk_gb: number;
  docker_responsive: boolean;
  uptime_seconds: number | null;
  resource_usage?: unknown;
}

/** Payload emitted by runtime:health Tauri event (from start_health_monitor). */
interface RuntimeHealthPayload {
  /** Serde-serialized RuntimeState enum: "None" | "Ready" | {"Error":"msg"} etc. */
  runtime_state: string | Record<string, string>;
  docker_responsive: boolean;
  docker_version: string | null;
  uptime_seconds: number | null;
  last_check: string;
  /** Which Docker backend is connected (always "builtin" in v2) */
  docker_source: string | null;
}

const RUNTIME_HEALTH_DOWNGRADE_GRACE_MS = 90_000;

/**
 * Map backend runtime state to the AppState runtimeStatus union.
 *
 * Two formats are accepted:
 * - String from runtime_status command (format_runtime_state output):
 *   "none", "provisioning", "ready", "stopped", "error: ..."
 * - Serde enum from runtime:health event (RuntimeState):
 *   "None", "Ready", "Starting", {"Error": "msg"}, etc.
 */
function mapRuntimeState(state: string | Record<string, string>): "starting" | "running" | "stopped" | "error" {
  // Handle serde enum object form: {"Error": "message"}
  if (typeof state === "object" && state !== null) {
    if ("Error" in state) return "error";
    return "stopped";
  }
  const s = state.toLowerCase();
  if (s === "ready") return "running";
  if (s === "starting" || s === "provisioning") return "starting";
  if (s.startsWith("error")) return "error";
  return "stopped";
}

function setEngineState(
  runtimeStatus: "starting" | "running" | "stopped" | "error",
  dockerConnected: boolean,
) {
  useAppStore.setState({
    runtimeStatus,
    dockerConnected,
    // builtinRuntimeReady = true when docker is responsive
    builtinRuntimeReady: dockerConnected,
  });
}

/**
 * Query Docker and Runtime status on startup.
 * Updates appStore with initial values.
 * Both commands have 5-second timeout to prevent UI from hanging.
 */
async function initRuntimeStatus() {
  let dockerOk = false;

  // 5-second timeout for docker_status
  try {
    const dockerStatus = await Promise.race([
      invoke<DockerStatusResponse>("docker_status"),
      new Promise<DockerStatusResponse>((_, reject) =>
        setTimeout(
          () => reject(new Error("Docker status check timeout")),
          5000,
        ),
      ),
    ]);
    dockerOk = dockerStatus.connected;
  } catch {
    dockerOk = false;
  }

  // 5-second timeout for runtime_status
  try {
    const rtStatus = await Promise.race([
      invoke<RuntimeStatusResponse>("runtime_status"),
      new Promise<RuntimeStatusResponse>((_, reject) =>
        setTimeout(
          () => reject(new Error("Runtime status check timeout")),
          5000,
        ),
      ),
    ]);

    // If Docker is connected and responsive, force status to "running"
    // regardless of what runtime_status thinks (avoids "starting" flicker)
    if (dockerOk || rtStatus.docker_responsive) {
      setEngineState("running", true);
    } else {
      setEngineState(mapRuntimeState(rtStatus.state), rtStatus.docker_responsive);
    }
  } catch {
    // Runtime status check failed or timed out
    // If Docker was already confirmed connected, set running anyway
    if (dockerOk) {
      setEngineState("running", true);
    } else {
      setEngineState("stopped", false);
    }
  }
}

function App() {
  const currentPage = useAppStore((s) => s.currentPage);
  const theme = useSettingsStore((s) => s.settings.theme);
  const lastHealthyAtRef = useRef(0);

  const markHealthy = () => {
    lastHealthyAtRef.current = Date.now();
  };

  // Initialize app state on mount
  useEffect(() => {
    // Load persisted settings (language, theme, etc.)
    void useSettingsStore.getState().fetchSettings();
    // Load LLM providers + stored models for chat selector
    void useSettingsStore.getState().fetchProviders();
    // Load persisted sessions
    void useChatStore.getState().loadSessions();
    // Load MCP servers
    void useMcpStore.getState().fetchServers();

    // Query initial Docker & Runtime status
    void initRuntimeStatus();
  }, []);

  // Listen for runtime:health events from backend (emitted every 20s)
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    void listen<RuntimeHealthPayload>(
      "runtime:health",
      (payload) => {
        const nextRuntimeStatus = mapRuntimeState(payload.runtime_state);
        const nextDockerConnected = payload.docker_responsive;
        const current = useAppStore.getState();

        // Any confirmed Docker responsiveness means engine is effectively ready.
        if (nextDockerConnected) {
          setEngineState("running", true);
          markHealthy();
          return;
        }

        const isTransientDowngrade =
          current.runtimeStatus === "running" &&
          current.dockerConnected &&
          nextRuntimeStatus === "starting" &&
          Date.now() - lastHealthyAtRef.current < RUNTIME_HEALTH_DOWNGRADE_GRACE_MS;

        // Ignore one-off ping misses right after a healthy state.
        if (isTransientDowngrade) {
          return;
        }

        setEngineState(nextRuntimeStatus, nextDockerConnected);
      },
    ).then((unsub) => {
      unlisten = unsub;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  // Listen for docker:connected event (emitted after runtime auto-start succeeds)
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    void listen<boolean>(
      "docker:connected",
      () => {
        // Runtime just started Docker — refresh status
        markHealthy();
        void initRuntimeStatus();
      },
    ).then((unsub) => {
      unlisten = unsub;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const state = useAppStore.getState();
    if (state.runtimeStatus === "running" && state.dockerConnected) {
      markHealthy();
    }
  }, []);

  // Sync theme with DOM whenever settings.theme changes
  useEffect(() => {
    const isDark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    document.documentElement.classList.toggle("dark", isDark);
    document.documentElement.style.colorScheme = isDark ? "dark" : "light";

    // Listen for OS theme changes when in "system" mode
    if (theme === "system") {
      const mq = window.matchMedia("(prefers-color-scheme: dark)");
      const handler = (e: MediaQueryListEvent) => {
        document.documentElement.classList.toggle("dark", e.matches);
        document.documentElement.style.colorScheme = e.matches ? "dark" : "light";
      };
      mq.addEventListener("change", handler);
      return () => mq.removeEventListener("change", handler);
    }
  }, [theme]);

  return (
    <>
      <AppLayout>
        {currentPage === "chat" && <ChatPage />}
        {currentPage === "containers" && <ContainersPage />}
        {currentPage === "images" && <ImagesPage />}
        {currentPage === "settings" && <SettingsPage />}
      </AppLayout>
      <ToastContainer />
    </>
  );
}

export default App;

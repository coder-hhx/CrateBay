import { useEffect, useRef } from "react";
import { useAppStore } from "@/stores/appStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { invoke, listen } from "@/lib/tauri";
import {
  deriveRuntimeStoreState,
  mapRuntimeState,
  isBuiltinDockerSource,
  type DockerStatusResponse,
  type RuntimeHealthPayload,
  type RuntimeStatusResponse,
} from "@/lib/runtimeStatus";
import { AppLayout } from "@/components/layout/AppLayout";
import { ToastContainer } from "@/components/common/Toast";
import { ContainersPage } from "@/pages/ContainersPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { ImagesPage } from "@/pages/ImagesPage";

const RUNTIME_HEALTH_DOWNGRADE_GRACE_MS = 90_000;

function setEngineState(
  runtimeStatus: "starting" | "running" | "stopped" | "error",
  dockerConnected: boolean,
  builtinRuntimeReady: boolean = dockerConnected,
) {
  useAppStore.setState({
    runtimeStatus,
    dockerConnected,
    builtinRuntimeReady,
  });
}

/**
 * Query Docker and Runtime status on startup.
 * Updates appStore with initial values.
 * Both commands have 5-second timeout to prevent UI from hanging.
 */
async function initRuntimeStatus() {
  let dockerStatus: DockerStatusResponse | null = null;

  // 5-second timeout for docker_status
  try {
    dockerStatus = await Promise.race([
      invoke<DockerStatusResponse>("docker_status"),
      new Promise<DockerStatusResponse>((_, reject) =>
        setTimeout(
          () => reject(new Error("Docker status check timeout")),
          5000,
        ),
      ),
    ]);
  } catch {
    dockerStatus = null;
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
    useAppStore.setState(deriveRuntimeStoreState(dockerStatus, rtStatus));
  } catch {
    useAppStore.setState(deriveRuntimeStoreState(dockerStatus, null));
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
          setEngineState(
            "running",
            true,
            isBuiltinDockerSource(payload.docker_source),
          );
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

        setEngineState(nextRuntimeStatus, nextDockerConnected, false);
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
        {currentPage === "containers" && <ContainersPage />}
        {currentPage === "images" && <ImagesPage />}
        {currentPage === "settings" && <SettingsPage />}
      </AppLayout>
      <ToastContainer />
    </>
  );
}

export default App;

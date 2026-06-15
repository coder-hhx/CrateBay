import { useEffect, useRef } from "react";
import { useAppStore } from "@/stores/appStore";
import { useSettingsStore } from "@/stores/settingsStore";
import { invoke, listen } from "@/lib/tauri";
import {
  deriveRuntimeStoreState,
  mapRuntimeState,
  isBuiltinEngineSource,
  runtimeCompatibilityResponsive,
  runtimeEngineResponsive,
  runtimeHealthSource,
  runtimeHealthState,
  type EngineEndpointStatusResponse,
  type RuntimeHealthPayload,
  type RuntimeStatusResponse,
} from "@/lib/runtimeStatus";
import { AppLayout } from "@/components/layout/AppLayout";
import { ToastContainer } from "@/components/common/Toast";
import { DashboardPage } from "@/pages/DashboardPage";
import { ContainersPage } from "@/pages/ContainersPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { ImagesPage } from "@/pages/ImagesPage";
import { PodsPage } from "@/pages/PodsPage";
import { VolumesPage } from "@/pages/VolumesPage";
import { NetworksPage } from "@/pages/NetworksPage";

const RUNTIME_HEALTH_DOWNGRADE_GRACE_MS = 90_000;

function setEngineState(
  runtimeStatus: "starting" | "running" | "stopped" | "error",
  engineConnected: boolean,
  builtinRuntimeReady: boolean = engineConnected,
  compatibilityConnected: boolean = engineConnected,
) {
  useAppStore.setState({
    runtimeStatus,
    engineConnected,
    dockerConnected: compatibilityConnected,
    builtinRuntimeReady,
  });
}

/**
 * Query CrateBay Engine and Runtime status on startup.
 * Updates appStore with initial values.
 * Both commands have 5-second timeout to prevent UI from hanging.
 */
async function initRuntimeStatus() {
  let endpointStatus: EngineEndpointStatusResponse | null = null;

  // 5-second timeout for engine_status
  try {
    endpointStatus = await Promise.race([
      invoke<EngineEndpointStatusResponse>("engine_status"),
      new Promise<EngineEndpointStatusResponse>((_, reject) =>
        setTimeout(
          () => reject(new Error("CrateBay Engine status check timeout")),
          5000,
        ),
      ),
    ]);
  } catch {
    endpointStatus = null;
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
    useAppStore.setState(deriveRuntimeStoreState(endpointStatus, rtStatus));
  } catch {
    useAppStore.setState(deriveRuntimeStoreState(endpointStatus, null));
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

    // Query initial CrateBay Engine & Runtime status
    void initRuntimeStatus();
  }, []);

  // Listen for runtime:health events from backend (emitted every 20s)
  useEffect(() => {
    let unlisten: (() => void) | null = null;

    void listen<RuntimeHealthPayload>(
      "runtime:health",
      (payload) => {
        const nextRuntimeStatus = mapRuntimeState(runtimeHealthState(payload));
        const nextEngineConnected = runtimeEngineResponsive(payload);
        const nextCompatibilityConnected =
          runtimeCompatibilityResponsive(payload) || nextEngineConnected;
        const current = useAppStore.getState();

        // Native Engine contract responsiveness is the app-ready signal.
        if (nextEngineConnected) {
          setEngineState(
            "running",
            true,
            isBuiltinEngineSource(runtimeHealthSource(payload)),
            nextCompatibilityConnected,
          );
          markHealthy();
          return;
        }

        const isTransientDowngrade =
          current.runtimeStatus === "running" &&
          current.engineConnected &&
          nextRuntimeStatus === "starting" &&
          Date.now() - lastHealthyAtRef.current < RUNTIME_HEALTH_DOWNGRADE_GRACE_MS;

        // Ignore one-off ping misses right after a healthy state.
        if (isTransientDowngrade) {
          return;
        }

        setEngineState(
          nextRuntimeStatus,
          nextEngineConnected,
          false,
          nextCompatibilityConnected,
        );
      },
    ).then((unsub) => {
      unlisten = unsub;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  // Listen for engine connection events emitted after runtime auto-start succeeds.
  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    const refreshEngineState = () => {
      markHealthy();
      void initRuntimeStatus();
    };

    void listen<boolean>("engine:connected", refreshEngineState).then((unsub) => {
      unlisteners.push(unsub);
    });
    // Backward-compatible alias for older backends during upgrades.
    void listen<boolean>("docker:connected", refreshEngineState).then((unsub) => {
      unlisteners.push(unsub);
    });

    return () => {
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const state = useAppStore.getState();
    if (state.runtimeStatus === "running" && state.engineConnected) {
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
        {currentPage === "dashboard" && <DashboardPage />}
        {currentPage === "containers" && <ContainersPage />}
        {currentPage === "images" && <ImagesPage />}
        {currentPage === "pods" && <PodsPage />}
        {currentPage === "volumes" && <VolumesPage />}
        {currentPage === "networks" && <NetworksPage />}
        {currentPage === "settings" && <SettingsPage />}
      </AppLayout>
      <ToastContainer />
    </>
  );
}

export default App;

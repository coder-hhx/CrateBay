import { create } from "zustand";

interface Notification {
  id: string;
  type: "info" | "success" | "warning" | "error";
  title: string;
  message?: string;
  timestamp: number;
  dismissable: boolean;
}

interface AppState {
  // Navigation
  currentPage: "chat" | "containers" | "images" | "settings";
  setCurrentPage: (page: AppState["currentPage"]) => void;

  // Theme
  theme: "dark" | "light";
  toggleTheme: () => void;

  // Sidebar
  sidebarOpen: boolean;
  sidebarWidth: number;
  toggleSidebar: () => void;
  setSidebarWidth: (width: number) => void;

  // Global status — dockerConnected = any Docker available
  dockerConnected: boolean;
  runtimeStatus: "starting" | "running" | "stopped" | "error";
  setDockerConnected: (connected: boolean) => void;
  setRuntimeStatus: (status: AppState["runtimeStatus"]) => void;

  // Built-in runtime status (decoupled from external Docker)
  // true only when the CrateBay self-hosted VM runtime is ready.
  builtinRuntimeReady: boolean;
  setBuiltinRuntimeReady: (ready: boolean) => void;

  // Runtime control operations
  runtimeLoading: boolean;
  setRuntimeLoading: (loading: boolean) => void;

  // Notifications
  notifications: Notification[];
  addNotification: (n: Omit<Notification, "id" | "timestamp">) => void;
  dismissNotification: (id: string) => void;
}

let notificationId = 0;

export const useAppStore = create<AppState>()((set) => ({
  // Navigation
  currentPage: "chat",
  setCurrentPage: (page) => set({ currentPage: page }),

  // Theme — default dark
  theme: "dark",
  toggleTheme: () =>
    set((state) => {
      const next = state.theme === "dark" ? "light" : "dark";
      document.documentElement.classList.toggle("dark", next === "dark");
      document.documentElement.style.colorScheme = next;
      return { theme: next };
    }),

  // Sidebar
  sidebarOpen: true,
  sidebarWidth: 260,
  toggleSidebar: () => set((state) => ({ sidebarOpen: !state.sidebarOpen })),
  setSidebarWidth: (width) => set({ sidebarWidth: width }),

  // Global status
  dockerConnected: false,
  runtimeStatus: "stopped",
  setDockerConnected: (connected) => set({ dockerConnected: connected }),
  setRuntimeStatus: (status) => set({ runtimeStatus: status }),

  // Built-in runtime
  builtinRuntimeReady: false,
  setBuiltinRuntimeReady: (ready) => set({ builtinRuntimeReady: ready }),

  // Runtime control operations
  runtimeLoading: false,
  setRuntimeLoading: (loading) => set({ runtimeLoading: loading }),

  // Notifications
  notifications: [],
  addNotification: (n) =>
    set((state) => ({
      notifications: [
        ...state.notifications,
        { ...n, id: String(++notificationId), timestamp: Date.now() },
      ],
    })),
  dismissNotification: (id) =>
    set((state) => ({
      notifications: state.notifications.filter((n) => n.id !== id),
    })),
}));

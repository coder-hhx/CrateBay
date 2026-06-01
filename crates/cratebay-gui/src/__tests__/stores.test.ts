import { describe, it, expect, beforeEach, vi } from "vitest";
import { useAppStore } from "@/stores/appStore";

vi.mock("@/lib/tauri", () => ({
  invoke: vi.fn(() => Promise.reject(new Error("Tauri not available in test"))),
  listen: vi.fn(() => Promise.resolve(() => {})),
  isTauri: vi.fn(() => false),
}));

import { useSettingsStore } from "@/stores/settingsStore";

describe("appStore", () => {
  beforeEach(() => {
    useAppStore.setState({
      currentPage: "images",
      theme: "dark",
      sidebarOpen: true,
      sidebarWidth: 260,
      dockerConnected: false,
      runtimeStatus: "stopped",
      builtinRuntimeReady: false,
      runtimeLoading: false,
      notifications: [],
    });
  });

  it("sets currentPage correctly", () => {
    useAppStore.getState().setCurrentPage("settings");
    expect(useAppStore.getState().currentPage).toBe("settings");

    useAppStore.getState().setCurrentPage("containers");
    expect(useAppStore.getState().currentPage).toBe("containers");
  });

  it("toggles theme between dark and light", () => {
    expect(useAppStore.getState().theme).toBe("dark");

    useAppStore.getState().toggleTheme();
    expect(useAppStore.getState().theme).toBe("light");

    useAppStore.getState().toggleTheme();
    expect(useAppStore.getState().theme).toBe("dark");
  });

  it("updates runtime status flags", () => {
    useAppStore.getState().setDockerConnected(true);
    useAppStore.getState().setRuntimeStatus("running");
    useAppStore.getState().setBuiltinRuntimeReady(true);
    useAppStore.getState().setRuntimeLoading(true);

    expect(useAppStore.getState().dockerConnected).toBe(true);
    expect(useAppStore.getState().runtimeStatus).toBe("running");
    expect(useAppStore.getState().builtinRuntimeReady).toBe(true);
    expect(useAppStore.getState().runtimeLoading).toBe(true);
  });

  it("adds and dismisses notifications", () => {
    useAppStore.getState().addNotification({
      type: "info",
      title: "Test notification",
      dismissable: true,
    });
    expect(useAppStore.getState().notifications).toHaveLength(1);

    const id = useAppStore.getState().notifications[0].id;
    useAppStore.getState().dismissNotification(id);
    expect(useAppStore.getState().notifications).toHaveLength(0);
  });
});

describe("settingsStore", () => {
  beforeEach(() => {
    useSettingsStore.setState({
      settings: {
        language: "en",
        theme: "dark",
        registryMirrors: [],
        runtimeHttpProxy: "",
        runtimeHttpProxyBridge: false,
        runtimeHttpProxyBindHost: "0.0.0.0",
        runtimeHttpProxyBindPort: 3128,
        runtimeHttpProxyGuestHost: "192.168.64.1",
      },
    });
  });

  it("updates display settings", async () => {
    await useSettingsStore.getState().updateSettings({
      language: "zh-CN",
      theme: "system",
    });

    const settings = useSettingsStore.getState().settings;
    expect(settings.language).toBe("zh-CN");
    expect(settings.theme).toBe("system");
  });

  it("updates runtime image settings", async () => {
    await useSettingsStore.getState().updateSettings({
      registryMirrors: ["docker.example.test"],
      runtimeHttpProxy: "127.0.0.1:7890",
    });

    const settings = useSettingsStore.getState().settings;
    expect(settings.registryMirrors).toEqual(["docker.example.test"]);
    expect(settings.runtimeHttpProxy).toBe("127.0.0.1:7890");
  });
});

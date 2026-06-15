import { create } from "zustand";
import { invoke, isTauri } from "@/lib/tauri";
import type { AppSettings } from "@/types/settings";
import {
  DEFAULT_REGISTRY_MIRRORS,
  DEFAULT_RUNTIME_HTTP_PROXY,
  DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST,
  DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT,
  DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE,
  DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST,
} from "@/types/settings";

export type { AppSettings };

interface SettingsState {
  settings: AppSettings;
  fetchSettings: () => Promise<void>;
  updateSettings: (patch: Partial<AppSettings>) => Promise<void>;
}

const defaultSettings: AppSettings = {
  language: "en",
  theme: "dark",
  registryMirrors: [...DEFAULT_REGISTRY_MIRRORS],
  runtimeHttpProxy: DEFAULT_RUNTIME_HTTP_PROXY,
  runtimeHttpProxyBridge: DEFAULT_RUNTIME_HTTP_PROXY_BRIDGE,
  runtimeHttpProxyBindHost: DEFAULT_RUNTIME_HTTP_PROXY_BIND_HOST,
  runtimeHttpProxyBindPort: DEFAULT_RUNTIME_HTTP_PROXY_BIND_PORT,
  runtimeHttpProxyGuestHost: DEFAULT_RUNTIME_HTTP_PROXY_GUEST_HOST,
  includePrereleases: false,
};

const settingKeys: (keyof AppSettings)[] = [
  "language",
  "theme",
  "registryMirrors",
  "runtimeHttpProxy",
  "runtimeHttpProxyBridge",
  "runtimeHttpProxyBindHost",
  "runtimeHttpProxyBindPort",
  "runtimeHttpProxyGuestHost",
  "includePrereleases",
];

function parseRegistryMirrors(value: string): string[] {
  try {
    const parsed = JSON.parse(value);
    if (Array.isArray(parsed)) {
      return parsed.filter((item): item is string => typeof item === "string");
    }
  } catch {
    // fall through to comma/newline separated parsing
  }

  return value
    .split(/[\n,]/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function parseSettingValue(key: keyof AppSettings, value: string): AppSettings[keyof AppSettings] {
  if (key === "registryMirrors") {
    return parseRegistryMirrors(value) as AppSettings[keyof AppSettings];
  }
  if (key === "runtimeHttpProxyBridge" || key === "includePrereleases") {
    return (value === "true") as AppSettings[keyof AppSettings];
  }
  if (key === "runtimeHttpProxyBindPort") {
    return Number(value) || defaultSettings.runtimeHttpProxyBindPort;
  }
  return value as AppSettings[keyof AppSettings];
}

function serializeSettingValue(value: string | string[] | boolean | number): string {
  if (Array.isArray(value)) {
    return JSON.stringify(value);
  }
  return String(value);
}

async function readSetting(key: keyof AppSettings): Promise<string | null> {
  if (!isTauri()) {
    return null;
  }

  try {
    return await invoke<string | null>("settings_get", { key });
  } catch {
    return null;
  }
}

async function writeSetting(key: keyof AppSettings, value: string | string[] | boolean | number) {
  if (!isTauri()) {
    return;
  }
  await invoke("settings_update", { key, value: serializeSettingValue(value) });
}

export const useSettingsStore = create<SettingsState>()((set, get) => ({
  settings: defaultSettings,

  fetchSettings: async () => {
    const next = { ...defaultSettings };

    if (isTauri()) {
      const entries = await Promise.all(
        settingKeys.map(async (key) => [key, await readSetting(key)] as const),
      );

      for (const [key, value] of entries) {
        if (value !== null && value !== undefined && value.trim().length > 0) {
          (next as Record<keyof AppSettings, AppSettings[keyof AppSettings]>)[key] =
            parseSettingValue(key, value);
        }
      }
    }

    set({ settings: next });
  },

  updateSettings: async (patch) => {
    const next = { ...get().settings, ...patch };
    set({ settings: next });

    if (!isTauri()) {
      return;
    }

    await Promise.all(
      Object.entries(patch).map(async ([key, value]) => {
        if (value !== undefined) {
          await writeSetting(
            key as keyof AppSettings,
            value as string | string[] | boolean | number,
          );
        }
      }),
    );
  },
}));

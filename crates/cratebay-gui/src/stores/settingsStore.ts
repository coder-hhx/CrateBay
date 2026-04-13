import { create } from "zustand";
import { invoke, isTauri } from "@/lib/tauri";
import type {
  ApiFormat,
  LlmProviderInfo,
  LlmProviderCreateRequest,
  LlmProviderUpdateRequest,
  LlmModelInfo,
  ProviderTestResult,
  AppSettings,
  ReasoningLevel,
} from "@/types/settings";
import { DEFAULT_REGISTRY_MIRRORS, REASONING_LEVEL_LABELS, normalizeProvider, normalizeBaseUrl } from "@/types/settings";

// Re-export types for backward compatibility
export type {
  ApiFormat,
  LlmProviderInfo,
  LlmProviderCreateRequest,
  LlmProviderUpdateRequest,
  LlmModelInfo,
  ProviderTestResult,
  AppSettings,
  ReasoningLevel,
};
export { REASONING_LEVEL_LABELS, normalizeProvider, normalizeBaseUrl };

/** Stable empty references to avoid re-renders from Zustand selectors */
const EMPTY_MODELS: never[] = [];

interface SettingsState {
  // LLM Providers
  providers: LlmProviderInfo[];
  activeProviderId: string | null;
  fetchProviders: () => Promise<void>;
  setActiveProvider: (id: string) => void;
  createProvider: (request: LlmProviderCreateRequest) => Promise<LlmProviderInfo>;
  updateProvider: (id: string, request: LlmProviderUpdateRequest) => Promise<LlmProviderInfo>;
  deleteProvider: (id: string) => Promise<void>;
  testProvider: (id: string) => Promise<ProviderTestResult>;

  // Models
  models: Record<string, LlmModelInfo[]>; // providerId → models[]
  activeModelId: string | null;
  setActiveModel: (modelId: string) => void;
  fetchModels: (providerId: string) => Promise<void>;
  toggleModel: (providerId: string, modelId: string, enabled: boolean) => Promise<void>;
  enabledModels: () => LlmModelInfo[]; // computed: all enabled models across providers

  // General settings
  settings: AppSettings;
  updateSettings: (patch: Partial<AppSettings>) => Promise<void>;
  fetchSettings: () => Promise<void>;

  // API Key management (keys never leave Rust backend)
  hasApiKey: (providerId: string) => boolean;
  saveApiKey: (providerId: string, key: string) => Promise<void>;
  deleteApiKey: (providerId: string) => Promise<void>;

  // Loading states
  providersLoading: boolean;
  modelsLoading: Record<string, boolean>;
}

const defaultSettings: AppSettings = {
  language: "en",
  theme: "dark",
  sendOnEnter: true,
  showAgentThinking: true,
  maxConversationHistory: 50,
  containerDefaultTtlHours: 8,
  confirmDestructiveOps: true,
  reasoningEffort: "medium",
  registryMirrors: DEFAULT_REGISTRY_MIRRORS,
  runtimeHttpProxy: "",
  runtimeHttpProxyBridge: false,
  runtimeHttpProxyBindHost: "0.0.0.0",
  runtimeHttpProxyBindPort: 3128,
  runtimeHttpProxyGuestHost: "192.168.64.1",
};

let providerIdCounter = 0;

const SETTINGS_KEY_DEFAULT_PROVIDER = "default_provider";
const SETTINGS_KEY_DEFAULT_MODEL = "default_model";

function shouldUseMockFallback(): boolean {
  return !isTauri();
}

async function persistSelectionSetting(key: string, value: string | null): Promise<void> {
  if (!isTauri()) return;
  await invoke("settings_update", { key, value: value ?? "" });
}

export const useSettingsStore = create<SettingsState>()((set, get) => ({
  // LLM Providers
  providers: [],
  activeProviderId: null,
  providersLoading: false,

  fetchProviders: async () => {
    set({ providersLoading: true });
    try {
      const providers = await invoke<LlmProviderInfo[]>("llm_provider_list");
      set({ providers, providersLoading: false });

      // Load stored models for each provider (no network call)
      if (isTauri() && providers.length > 0) {
        const entries = await Promise.all(
          providers.map(async (p) => {
            try {
              const models = await invoke<LlmModelInfo[]>("llm_models_list", {
                provider_id: p.id,
              });
              return [p.id, models] as const;
            } catch {
              return [p.id, [] as LlmModelInfo[]] as const;
            }
          }),
        );
        set((state) => ({
          models: { ...state.models, ...Object.fromEntries(entries) },
        }));
      }

      // If no active provider is selected yet, select the first one.
      if (get().activeProviderId === null && providers.length > 0) {
        get().setActiveProvider(providers[0].id);
      }
    } catch (err) {
      set({ providersLoading: false });
      if (!shouldUseMockFallback()) {
        console.warn("[settingsStore] fetchProviders failed:", err);
        return;
      }
      // In non-Tauri env, keep current state
    }
  },

  setActiveProvider: (id) => {
    set({ activeProviderId: id });
    void persistSelectionSetting(SETTINGS_KEY_DEFAULT_PROVIDER, id);
  },

  createProvider: async (request) => {
    try {
      const provider = await invoke<LlmProviderInfo>("llm_provider_create", {
        request,
      });
      set((state) => ({
        providers: [...state.providers, provider],
        activeProviderId: provider.id,
      }));
      void persistSelectionSetting(SETTINGS_KEY_DEFAULT_PROVIDER, provider.id);
      return provider;
    } catch (err) {
      if (!shouldUseMockFallback()) {
        throw err;
      }

      // Mock for non-Tauri development only
      const provider: LlmProviderInfo = {
        id: `provider-${++providerIdCounter}-${Date.now()}`,
        name: request.name,
        apiBase: request.apiBase,
        apiFormat: request.apiFormat,
        hasApiKey: request.apiKey.length > 0,
        enabled: true,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      set((state) => ({
        providers: [...state.providers, provider],
        activeProviderId: provider.id,
      }));
      void persistSelectionSetting(SETTINGS_KEY_DEFAULT_PROVIDER, provider.id);
      return provider;
    }
  },

  updateProvider: async (id, request) => {
    try {
      const provider = await invoke<LlmProviderInfo>("llm_provider_update", {
        id,
        request,
      });
      set((state) => ({
        providers: state.providers.map((p) => (p.id === id ? provider : p)),
      }));
      return provider;
    } catch (err) {
      if (!shouldUseMockFallback()) {
        throw err;
      }

      // Mock for non-Tauri development only
      const existing = get().providers.find((p) => p.id === id);
      if (existing === undefined) throw new Error(`Provider ${id} not found`);
      const updated: LlmProviderInfo = {
        ...existing,
        ...(request.name !== undefined ? { name: request.name } : {}),
        ...(request.apiBase !== undefined ? { apiBase: request.apiBase } : {}),
        ...(request.apiFormat !== undefined ? { apiFormat: request.apiFormat } : {}),
        ...(request.enabled !== undefined ? { enabled: request.enabled } : {}),
        ...(request.apiKey !== undefined ? { hasApiKey: request.apiKey.length > 0 } : {}),
        updatedAt: new Date().toISOString(),
      };
      set((state) => ({
        providers: state.providers.map((p) => (p.id === id ? updated : p)),
      }));
      return updated;
    }
  },

  deleteProvider: async (id) => {
    if (isTauri()) {
      await invoke("llm_provider_delete", { id });
    }

    let nextActiveProvider: string | null = null;
    let nextActiveModel: string | null = null;
    set((state) => ({
      providers: state.providers.filter((p) => p.id !== id),
      activeProviderId: state.activeProviderId === id ? null : state.activeProviderId,
      activeModelId: state.activeProviderId === id ? null : state.activeModelId,
      models: Object.fromEntries(Object.entries(state.models).filter(([k]) => k !== id)),
    }));
    {
      const current = get();
      nextActiveProvider = current.activeProviderId;
      nextActiveModel = current.activeModelId;
    }
    void persistSelectionSetting(SETTINGS_KEY_DEFAULT_PROVIDER, nextActiveProvider);
    void persistSelectionSetting(SETTINGS_KEY_DEFAULT_MODEL, nextActiveModel);
  },

  testProvider: async (id) => {
    if (isTauri()) {
      return invoke<ProviderTestResult>("llm_provider_test", { provider_id: id });
    }
    return {
      success: true,
      latencyMs: 0,
      model: "mock-model",
      error: null,
    };
  },

  // Models
  models: {},
  activeModelId: null,
  modelsLoading: {},

  setActiveModel: (modelId) => {
    set({ activeModelId: modelId });
    void persistSelectionSetting(SETTINGS_KEY_DEFAULT_MODEL, modelId);
  },

  fetchModels: async (providerId) => {
    set((state) => ({
      modelsLoading: { ...state.modelsLoading, [providerId]: true },
    }));
    try {
      const models = await invoke<LlmModelInfo[]>("llm_models_fetch", { provider_id: providerId });
      set((state) => ({
        models: { ...state.models, [providerId]: models },
        modelsLoading: { ...state.modelsLoading, [providerId]: false },
      }));
    } catch (err) {
      if (!shouldUseMockFallback()) {
        set((state) => ({
          modelsLoading: { ...state.modelsLoading, [providerId]: false },
        }));
        console.warn("[settingsStore] fetchModels failed:", err);
        return;
      }

      // Mock for non-Tauri development only: generate some sample models
      const mockModels: LlmModelInfo[] = [
        {
          id: "mock-model-1",
          providerId,
          name: "Sample Model",
          isEnabled: true,
          supportsReasoning: false,
        },
      ];
      set((state) => ({
        models: { ...state.models, [providerId]: mockModels },
        modelsLoading: { ...state.modelsLoading, [providerId]: false },
      }));
    }
  },

  toggleModel: async (providerId, modelId, enabled) => {
    try {
      await invoke("llm_models_toggle", {
        provider_id: providerId,
        model_id: modelId,
        enabled,
      });
    } catch (err) {
      if (!shouldUseMockFallback()) {
        console.warn("[settingsStore] toggleModel failed:", err);
      }
      // Mock for non-Tauri development only
    }
    set((state) => ({
      models: {
        ...state.models,
        [providerId]: (state.models[providerId] ?? EMPTY_MODELS).map((m) =>
          m.id === modelId ? { ...m, isEnabled: enabled } : m,
        ),
      },
    }));
  },

  enabledModels: () => {
    const { models } = get();
    const result: LlmModelInfo[] = [];
    for (const providerModels of Object.values(models)) {
      for (const model of providerModels) {
        if (model.isEnabled) {
          result.push(model);
        }
      }
    }
    return result;
  },

  // General settings
  settings: defaultSettings,

  fetchSettings: async () => {
    try {
      // settings_get takes a single key, so we fetch each known key individually
      const keys: (keyof AppSettings)[] = [
        "language", "theme", "sendOnEnter", "showAgentThinking",
        "maxConversationHistory", "containerDefaultTtlHours",
        "confirmDestructiveOps", "reasoningEffort", "registryMirrors",
        "runtimeHttpProxy", "runtimeHttpProxyBridge", "runtimeHttpProxyBindHost",
        "runtimeHttpProxyBindPort", "runtimeHttpProxyGuestHost",
      ];
      const fetched: Partial<AppSettings> = {};
      for (const key of keys) {
        const value = await invoke<string | null>("settings_get", { key });
        if (value !== null && value !== undefined) {
          // Parse booleans and numbers back from string storage
          if (value === "true" || value === "false") {
            (fetched as Record<string, unknown>)[key] = value === "true";
          } else if (key === "registryMirrors") {
            try {
              (fetched as Record<string, unknown>)[key] = JSON.parse(value);
            } catch {
              // Invalid JSON, keep default
            }
          } else if (
            !isNaN(Number(value)) &&
            key !== "language" &&
            key !== "theme" &&
            key !== "reasoningEffort" &&
            key !== "runtimeHttpProxy" &&
            key !== "runtimeHttpProxyBindHost" &&
            key !== "runtimeHttpProxyGuestHost"
          ) {
            (fetched as Record<string, unknown>)[key] = Number(value);
          } else {
            (fetched as Record<string, unknown>)[key] = value;
          }
        }
      }
      const defaultProvider = await invoke<string | null>("settings_get", {
        key: SETTINGS_KEY_DEFAULT_PROVIDER,
      });
      const defaultModel = await invoke<string | null>("settings_get", {
        key: SETTINGS_KEY_DEFAULT_MODEL,
      });
      set({
        settings: { ...defaultSettings, ...fetched },
        activeProviderId:
          defaultProvider !== null && defaultProvider.trim().length > 0
            ? defaultProvider
            : get().activeProviderId,
        activeModelId:
          defaultModel !== null && defaultModel.trim().length > 0
            ? defaultModel
            : get().activeModelId,
      });
    } catch (err) {
      if (!shouldUseMockFallback()) {
        console.warn("[settingsStore] fetchSettings failed:", err);
        return;
      }
      // Keep defaults in non-Tauri env
    }
  },

  updateSettings: async (patch) => {
    const newSettings = { ...get().settings, ...patch };
    set({ settings: newSettings });
    try {
      // settings_update takes (key, value) pairs, so we update each changed key
      for (const [key, value] of Object.entries(patch)) {
        const serialized = Array.isArray(value) ? JSON.stringify(value) : String(value);
        await invoke("settings_update", { key, value: serialized });
      }
    } catch (err) {
      if (!shouldUseMockFallback()) {
        console.warn("[settingsStore] updateSettings failed:", err);
        return;
      }
      // Mock for non-Tauri development only
    }
  },

  // API Key management
  hasApiKey: (providerId) => {
    const provider = get().providers.find((p) => p.id === providerId);
    return provider?.hasApiKey ?? false;
  },

  saveApiKey: async (providerId, key) => {
    try {
      await invoke("api_key_save", {
        provider_id: providerId,
        api_key: key,
      });
      set((state) => ({
        providers: state.providers.map((p) =>
          p.id === providerId ? { ...p, hasApiKey: true } : p,
        ),
      }));
    } catch (err) {
      if (!shouldUseMockFallback()) {
        throw err;
      }
      // Mock for non-Tauri development only
      set((state) => ({
        providers: state.providers.map((p) =>
          p.id === providerId ? { ...p, hasApiKey: key.length > 0 } : p,
        ),
      }));
    }
  },

  deleteApiKey: async (providerId) => {
    try {
      await invoke("api_key_delete", { provider_id: providerId });
    } catch (err) {
      if (!shouldUseMockFallback()) {
        throw err;
      }
      // Mock for non-Tauri development only
    }
    set((state) => ({
      providers: state.providers.map((p) =>
        p.id === providerId ? { ...p, hasApiKey: false } : p,
      ),
    }));
  },
}));

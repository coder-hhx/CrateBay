import { create } from "zustand";
import { invoke, listen } from "@/lib/tauri";
import { useAppStore } from "@/stores/appStore";
import { useSettingsStore } from "@/stores/settingsStore";
import type {
  ContainerInfo,
  ContainerCreateRequest,
  ContainerTemplate,
  ContainerFilter,
  DockerImageInfo,
} from "@/types/container";
import type { LocalImageInfo } from "@/types/image";

// Re-export types for backward compatibility
export type { ContainerInfo, ContainerCreateRequest, ContainerTemplate, DockerImageInfo, LocalImageInfo };

interface ContainerState {
  // Container list
  containers: ContainerInfo[];
  loading: boolean;
  error: string | null;
  fetchContainers: () => Promise<void>;
  _fetchAbortController: AbortController | null;
  _transitionalRefreshTimer: ReturnType<typeof setTimeout> | null;

  // Docker images
  images: LocalImageInfo[];
  imagesLoading: boolean;
  fetchImages: () => Promise<void>;

  // Selection
  selectedContainerId: string | null;
  selectContainer: (id: string | null) => void;

  // Operations
  createContainer: (req: ContainerCreateRequest) => Promise<ContainerInfo>;
  startContainer: (id: string) => Promise<void>;
  stopContainer: (id: string) => Promise<void>;
  deleteContainer: (id: string) => Promise<void>;

  // Templates
  templates: ContainerTemplate[];
  fetchTemplates: () => Promise<void>;

  // Filters
  filter: ContainerFilter;
  setFilter: (filter: Partial<ContainerFilter>) => void;

  // Computed
  filteredContainers: () => ContainerInfo[];
}

export const useContainerStore = create<ContainerState>()((set, get) => ({
  containers: [],
  loading: false,
  error: null,
  _fetchAbortController: null,
  _transitionalRefreshTimer: null,

  fetchContainers: async () => {
    // Cancel previous request if still in progress
    const prevController = get()._fetchAbortController;
    if (prevController) {
      prevController.abort();
    }
    const prevTimer = get()._transitionalRefreshTimer;
    if (prevTimer) {
      clearTimeout(prevTimer);
      set({ _transitionalRefreshTimer: null });
    }

    // Create new abort controller for this request
    const controller = new AbortController();
    set({ _fetchAbortController: controller, loading: true, error: null });
    const requestController = controller;

    try {
      // Timeout protection: 8 seconds max
      const timeoutId = setTimeout(
        () => {
          requestController.abort();
          // If this is still the latest request, stop the spinner even though
          // we can't truly cancel the in-flight Tauri invoke. Keep the
          // controller so late results can still update the UI.
          if (get()._fetchAbortController === requestController) {
            set({
              loading: false,
              error: "刷新超时：runtime 可能正在启动或 Docker 无响应（稍后会自动更新）",
            });
          }
        },
        8000,
      );

      const result = await invoke<ContainerInfo[]>("container_list");
      clearTimeout(timeoutId);

      // Ignore stale results (superseded by a newer request or timed out).
      if (get()._fetchAbortController !== requestController) {
        return;
      }

      // Merge with any "creating" placeholders that haven't been replaced yet.
      // Drop placeholders whose name already appears in the real container list
      // (the backend has created the container, so the placeholder is stale).
      const realNames = new Set(result.map((c) => c.name));
      const placeholders = get().containers.filter(
        (c) => c.id.startsWith("__creating_") && !realNames.has(c.name),
      );

      // Preserve existing order: reorder result to match previous container list,
      // then append any new containers at the end.
      const prev = get().containers;
      const prevOrder = new Map(prev.map((c, i) => [c.name, i]));
      const merged = [...result, ...placeholders].sort((a, b) => {
        const aOrder = prevOrder.get(a.name) ?? Number.MAX_SAFE_INTEGER;
        const bOrder = prevOrder.get(b.name) ?? Number.MAX_SAFE_INTEGER;
        return aOrder - bOrder;
      });
      const hasTransitional = merged.some((c) =>
        c.status === "creating"
        || c.status === "created"
        || c.status === "restarting"
        || c.status === "removing",
      );
      const nextTimer = hasTransitional
        ? setTimeout(() => {
          void get().fetchContainers();
        }, 1500)
        : null;

      set({
        containers: merged,
        loading: false,
        error: null,
        _fetchAbortController: null,
        _transitionalRefreshTimer: nextTimer,
      });
    } catch (err) {
      if (requestController.signal.aborted) {
        console.warn("[containerStore] fetchContainers aborted");
      } else {
        const message = err instanceof Error ? err.message : String(err);
        console.warn("[containerStore] fetchContainers failed:", message);
        if (get()._fetchAbortController === requestController) {
          set({
            error:
              message.length > 0 && message !== "[object Object]"
                ? message
                : "刷新失败：请检查 runtime 连接状态",
          });
        }
      }
      // On failure, keep existing containers visible, just stop loading
      if (get()._fetchAbortController === requestController) {
        set({ loading: false, _fetchAbortController: null });
      }
    }
  },

  images: [],
  imagesLoading: false,
  fetchImages: async () => {
    set({ imagesLoading: true });
    try {
      const images = await invoke<LocalImageInfo[]>("image_list");
      set({ images, imagesLoading: false });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error("[containerStore] fetchImages failed:", message);
      set({ imagesLoading: false });
    }
  },

  selectedContainerId: null,
  selectContainer: (id) => set({ selectedContainerId: id }),

  createContainer: async (req) => {
    // Optimistic update: add a placeholder card immediately
    const placeholderId = `__creating_${Date.now()}`;
    const placeholder: ContainerInfo = {
      id: placeholderId,
      shortId: "creating...",
      name: req.name,
      image: req.image,
      status: "creating",
      state: "creating",
      createdAt: new Date().toISOString(),
      ports: [],
      labels: {},
      cpuCores: req.cpuCores,
      memoryMb: req.memoryMb,
    };
    set((state) => ({ containers: [...state.containers, placeholder], error: null }));

    const notify = useAppStore.getState().addNotification;

    // Helper to update placeholder status text
    const updatePlaceholder = (statusText: string) => {
      set((state) => ({
        containers: state.containers.map((c) =>
          c.id === placeholderId ? { ...c, shortId: statusText } : c,
        ),
      }));
    };

    try {
      // Step 1: Check if image exists locally
      let needsPull = false;
      try {
        const images = await invoke<LocalImageInfo[]>("image_list");
        const imageTag = req.image;
        needsPull = !images.some((img) =>
          img.repoTags.some((tag) => tag === imageTag),
        );
      } catch {
        // If image_list fails, try to create directly
        needsPull = false;
      }

      // Step 2: Pull image if needed (non-blocking backend, event-driven)
      if (needsPull) {
        const mirrors = useSettingsStore.getState().settings.registryMirrors;
        const hasMirrors = mirrors.length > 0;
        const pullChannelId = `pull-${Date.now()}`;

        updatePlaceholder("正在拉取镜像...");
        notify({
          type: "info",
          title: `正在拉取镜像 ${req.image}`,
          message: hasMirrors
            ? `使用镜像加速源拉取，共 ${mirrors.length} 个源...`
            : "首次使用此镜像需要下载，请稍候...",
          dismissable: true,
        });

        // image_pull now returns immediately (spawns background task)
        // Returns the channel_id string
        await invoke<string>("image_pull", {
          image: req.image,
          mirrors: hasMirrors ? mirrors : null,
          channel_id: pullChannelId,
        });

        // Wait for pull completion via event
        const pullResult = await new Promise<{ success: boolean; error?: string }>((resolve) => {
          let unlistenFn: (() => void) | null = null;

          // 5-minute overall timeout for pull
          const timeoutId = setTimeout(() => {
            unlistenFn?.();
            resolve({ success: false, error: "镜像拉取超时（5分钟）" });
          }, 300000);

          void listen<{
            status: string;
            progress_percent: number;
            complete: boolean;
            error?: string | null;
          }>(`image:pull:${pullChannelId}`, (progress) => {
            // Update placeholder with real-time status
            if (!progress.complete) {
              const pct = progress.progress_percent;
              const statusMsg = pct > 0 ? `拉取中 ${pct}%` : progress.status || "拉取中...";
              updatePlaceholder(statusMsg);
            }

            if (progress.complete) {
              clearTimeout(timeoutId);
              unlistenFn?.();
              if (progress.error) {
                resolve({ success: false, error: progress.error });
              } else {
                resolve({ success: true });
              }
            }
          }).then((unsub) => {
            unlistenFn = unsub;
          });
        });

        if (!pullResult.success) {
          // Pull failed — remove placeholder
          set((state) => ({
            containers: state.containers.filter((c) => c.id !== placeholderId),
            error: pullResult.error || "镜像拉取失败",
          }));
          notify({
            type: "error",
            title: "镜像拉取失败",
            message: pullResult.error || "未知错误",
            dismissable: true,
          });
          throw new Error(pullResult.error || "镜像拉取失败");
        }

        notify({
          type: "success",
          title: `镜像 ${req.image} 拉取完成`,
          dismissable: true,
        });
      }

      // Step 3: Create container (image already available)
      updatePlaceholder("正在创建容器...");
      const container = await invoke<ContainerInfo>("container_create", { request: req });
      // Replace placeholder with real container
      set((state) => ({
        containers: state.containers.map((c) =>
          c.id === placeholderId ? container : c,
        ),
      }));

      // Auto-refresh the container list to get accurate state from backend
      void get().fetchContainers();

      // Check if auto-start succeeded
      if (container.state === "created" || container.status === "created") {
        // Container was created but auto-start failed (e.g. invalid CMD for the image)
        notify({
          type: "warning",
          title: `容器 ${req.name} 已创建`,
          message: "容器创建成功但未能自动启动，可能是镜像不支持默认命令。请尝试手动启动。",
          dismissable: true,
        });
      } else if (container.status !== "running" && container.status !== "paused") {
        notify({
          type: "warning",
          title: `容器 ${req.name} 已创建`,
          message:
            "容器已启动但很快退出/停止。常见原因：镜像默认命令执行完毕或命令不存在。可在创建时指定命令（例如 `sleep infinity`）后重试。",
          dismissable: true,
        });
      } else {
        notify({
          type: "success",
          title: `容器 ${req.name} 创建成功`,
          dismissable: true,
        });
      }
      return container;
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error("[containerStore] createContainer failed:", message);
      // Remove the placeholder on failure
      set((state) => ({
        containers: state.containers.filter((c) => c.id !== placeholderId),
        error: message,
      }));
      notify({
        type: "error",
        title: "容器创建失败",
        message,
        dismissable: true,
      });
      throw err;
    }
  },

  startContainer: async (id) => {
    const notify = useAppStore.getState().addNotification;
    try {
      // Optimistic: mark as creating while we start (runtime may be starting)
      set((state) => ({
        containers: state.containers.map((c) =>
          c.id === id ? { ...c, status: "creating", state: "starting" } : c,
        ),
        error: null,
      }));
      notify({
        type: "info",
        title: "正在启动容器...",
        message: "如果 runtime 尚未就绪，启动可能需要几十秒",
        dismissable: true,
      });
      await invoke("container_start", { id });
      // Refresh from backend to get real status
      await get().fetchContainers();
      const updated = get().containers.find((c) => c.id === id);
      if (updated && updated.status !== "running" && updated.status !== "paused") {
        notify({
          type: "warning",
          title: "容器未保持运行",
          message:
            "容器已启动但很快退出/停止。常见原因：镜像默认命令执行完毕或命令不存在。可在创建时指定命令（例如 `sleep infinity`）后重试。",
          dismissable: true,
        });
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error("[containerStore] startContainer failed:", message);
      set({ error: message });
      notify({
        type: "error",
        title: "启动失败",
        message,
        dismissable: true,
      });
    }
  },

  stopContainer: async (id) => {
    const notify = useAppStore.getState().addNotification;
    try {
      set((state) => ({
        containers: state.containers.map((c) =>
          c.id === id ? { ...c, status: "creating", state: "stopping" } : c,
        ),
        error: null,
      }));
      await invoke("container_stop", { id });
      // Refresh from backend to get real status
      await get().fetchContainers();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error("[containerStore] stopContainer failed:", message);
      set({ error: message });
      notify({
        type: "error",
        title: "停止失败",
        message,
        dismissable: true,
      });
    }
  },

  deleteContainer: async (id) => {
    try {
      await invoke("container_delete", { id });
      set((state) => ({
        containers: state.containers.filter((c) => c.id !== id),
        selectedContainerId: state.selectedContainerId === id ? null : state.selectedContainerId,
      }));
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error("[containerStore] deleteContainer failed:", message);
      set({ error: message });
    }
  },

  templates: [],
  fetchTemplates: async () => {
    try {
      const templates = await invoke<ContainerTemplate[]>("container_templates");
      set({ templates });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error("[containerStore] fetchTemplates failed:", message);
      // Provide built-in templates as fallback (these are part of the app, not mock)
      set({
        templates: [
          {
            id: "node-dev",
            name: "Node.js Dev",
            description: "Node.js development environment",
            image: "node:20-alpine",
            defaultCommand: "sh",
            defaultCpuCores: 2,
            defaultMemoryMb: 2048,
            tags: ["node", "javascript"],
          },
          {
            id: "python-dev",
            name: "Python Dev",
            description: "Python development environment",
            image: "python:3.12-slim",
            defaultCommand: "bash",
            defaultCpuCores: 2,
            defaultMemoryMb: 2048,
            tags: ["python"],
          },
          {
            id: "rust-dev",
            name: "Rust Dev",
            description: "Rust development environment",
            image: "rust:1.75-slim",
            defaultCommand: "bash",
            defaultCpuCores: 4,
            defaultMemoryMb: 4096,
            tags: ["rust"],
          },
        ],
      });
    }
  },

  filter: { status: "all", search: "", templateId: null },
  setFilter: (patch) =>
    set((state) => ({ filter: { ...state.filter, ...patch } })),

  filteredContainers: () => {
    const { containers, filter } = get();
    return containers
      .filter((c) => {
        if (filter.status !== "all" && c.status !== filter.status) return false;
        if (filter.templateId !== null && c.labels?.["com.cratebay.template_id"] !== filter.templateId) return false;
        if (
          filter.search.length > 0 &&
          !c.name.toLowerCase().includes(filter.search.toLowerCase()) &&
          !c.image.toLowerCase().includes(filter.search.toLowerCase())
        ) {
          return false;
        }
        return true;
      })
      .sort((a, b) => {
        const aTs = Date.parse(a.createdAt);
        const bTs = Date.parse(b.createdAt);
        if (Number.isNaN(aTs) && Number.isNaN(bTs)) {
          return a.name.localeCompare(b.name);
        }
        if (Number.isNaN(aTs)) {
          return 1;
        }
        if (Number.isNaN(bTs)) {
          return -1;
        }
        return aTs - bTs;
      });
  },

}));

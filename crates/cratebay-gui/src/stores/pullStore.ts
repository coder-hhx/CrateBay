import { create } from "zustand";
import { invoke, listen } from "@/lib/tauri";
import { useSettingsStore } from "@/stores/settingsStore";

export interface PullTask {
  id: string;
  image: string;
  progress: number;
  status: string;
  complete: boolean;
  error: string | null;
  currentBytes: number;
  totalBytes: number;
  speed: number;
}

// Event payload from Tauri emit — serde keeps snake_case
interface ImagePullProgress {
  current_layer: number;
  total_layers: number;
  progress_percent: number;
  status: string;
  complete: boolean;
  error: string | null;
  current_bytes: number;
  total_bytes: number;
}

interface PullState {
  tasks: PullTask[];
  startPull: (image: string) => Promise<void>;
  removeTask: (id: string) => void;
  clearCompleted: () => void;
  isPulling: (image: string) => boolean;
}

// Speed tracking — kept outside store to avoid unnecessary re-renders
const speedState = new Map<string, { lastBytes: number; lastTime: number }>();

export const usePullStore = create<PullState>()((set, get) => ({
  tasks: [],

  startPull: async (image: string) => {
    const trimmedImage = image.trim();
    if (trimmedImage.length === 0) return;

    if (get().tasks.some((t) => t.image === trimmedImage && !t.complete)) {
      return;
    }

    const tempId = `pull-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

    set((state) => ({
      tasks: [
        {
          id: tempId,
          image: trimmedImage,
          progress: 0,
          status: "",
          complete: false,
          error: null,
          currentBytes: 0,
          totalBytes: 0,
          speed: 0,
        },
        ...state.tasks,
      ],
    }));

    try {
      const mirrors = useSettingsStore.getState().settings.registryMirrors;
      const hasMirrors = Array.isArray(mirrors) && mirrors.length > 0;

      const invokeParams: Record<string, unknown> = {
        image: trimmedImage,
        channelId: tempId,
      };
      if (hasMirrors) {
        invokeParams.mirrors = mirrors;
      }

      const channelId = await invoke<string>("image_pull", invokeParams);

      set((state) => ({
        tasks: state.tasks.map((t) =>
          t.id === tempId ? { ...t, id: channelId } : t,
        ),
      }));

      speedState.set(channelId, { lastBytes: 0, lastTime: Date.now() });

      let unlisten: (() => void) | null = null;
      const cleanup = () => {
        if (unlisten) unlisten();
        speedState.delete(channelId);
      };

      unlisten = await listen<ImagePullProgress>(
        `image:pull:${channelId}`,
        (p) => {
          const existing = get().tasks.find((t) => t.id === channelId);
          const prevBytes = existing?.currentBytes ?? 0;
          const prevTotal = existing?.totalBytes ?? 0;
          let speed = existing?.speed ?? 0;

          // Backend guarantees: current never goes backwards, total is frozen after first download
          // But status-change events may carry 0 bytes — keep previous values
          const curBytes = p.current_bytes > 0 ? p.current_bytes : prevBytes;
          const totBytes = p.total_bytes > 0 ? p.total_bytes : prevTotal;

          // Calculate speed (EMA smoothing)
          const prev = speedState.get(channelId);
          if (prev && curBytes > prev.lastBytes) {
            const now = Date.now();
            const elapsed = (now - prev.lastTime) / 1000;
            if (elapsed > 0.3) {
              const instant = (curBytes - prev.lastBytes) / elapsed;
              speed = speed > 0 ? speed * 0.7 + instant * 0.3 : instant;
              speedState.set(channelId, { lastBytes: curBytes, lastTime: now });
            }
          }

          set((state) => ({
            tasks: state.tasks.map((t) =>
              t.id === channelId
                ? {
                    ...t,
                    progress: p.progress_percent,
                    status: p.status,
                    complete: p.complete,
                    error: p.error,
                    currentBytes: curBytes,
                    totalBytes: totBytes,
                    speed: p.complete ? 0 : speed,
                  }
                : t,
            ),
          }));

          if (p.complete) cleanup();
        },
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      set((state) => ({
        tasks: state.tasks.map((t) =>
          t.id === tempId || (!t.complete && t.image === trimmedImage)
            ? { ...t, complete: true, error: message.length > 0 ? message : `Failed to pull ${trimmedImage}` }
            : t,
        ),
      }));
    }
  },

  removeTask: (id) => {
    set((state) => ({ tasks: state.tasks.filter((t) => t.id !== id) }));
    speedState.delete(id);
  },

  clearCompleted: () => {
    set((state) => ({ tasks: state.tasks.filter((t) => !t.complete) }));
  },

  isPulling: (image) => {
    return get().tasks.some((t) => t.image === image && !t.complete);
  },
}));

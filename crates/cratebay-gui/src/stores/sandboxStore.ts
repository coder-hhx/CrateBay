/**
 * Sandbox state management.
 *
 * Tracks which sandbox is bound to which chat session,
 * enabling multi-step workflows within the same sandbox.
 */

import { create } from "zustand";

interface SandboxBinding {
  sandboxId: string;
  language: string;
  status: "running" | "stopped" | "unknown";
}

interface SandboxState {
  /** Maps chat session ID → sandbox binding */
  sessionSandboxes: Record<string, SandboxBinding>;

  /** Bind a sandbox to a chat session */
  bindSandbox: (
    sessionId: string,
    sandboxId: string,
    language: string,
  ) => void;

  /** Unbind (and optionally cleanup) a sandbox from a session */
  unbindSandbox: (sessionId: string) => void;

  /** Get the sandbox bound to a session */
  getSessionSandbox: (sessionId: string) => SandboxBinding | null;

  /** Update sandbox status */
  updateSandboxStatus: (
    sessionId: string,
    status: "running" | "stopped" | "unknown",
  ) => void;
}

export const useSandboxStore = create<SandboxState>((set, get) => ({
  sessionSandboxes: {},

  bindSandbox: (sessionId, sandboxId, language) => {
    set((state) => ({
      sessionSandboxes: {
        ...state.sessionSandboxes,
        [sessionId]: { sandboxId, language, status: "running" },
      },
    }));
  },

  unbindSandbox: (sessionId) => {
    set((state) => {
      const { [sessionId]: _, ...rest } = state.sessionSandboxes;
      return { sessionSandboxes: rest };
    });
  },

  getSessionSandbox: (sessionId) => {
    return get().sessionSandboxes[sessionId] ?? null;
  },

  updateSandboxStatus: (sessionId, status) => {
    set((state) => {
      const existing = state.sessionSandboxes[sessionId];
      if (!existing) return state;
      return {
        sessionSandboxes: {
          ...state.sessionSandboxes,
          [sessionId]: { ...existing, status },
        },
      };
    });
  },
}));

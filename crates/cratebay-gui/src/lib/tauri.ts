/**
 * Tauri invoke/listen wrappers using the official @tauri-apps/api package.
 *
 * In Tauri v2, the global `window.__TAURI__` is NOT injected by default.
 * We must use the `@tauri-apps/api` imports which use the IPC postMessage bridge.
 *
 * When running in a browser (e.g., `pnpm dev` without Tauri), these wrappers
 * detect the absence of `__TAURI_INTERNALS__` and return mock data.
 */

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";

declare global {
  interface Window {
    __MOCK_TAURI_INVOKE__?: (
      cmd: string,
      args?: Record<string, unknown>,
    ) => unknown | Promise<unknown>;
    __MOCK_TAURI__?: unknown;
  }
}

/**
 * Check whether the app is running inside a Tauri webview.
 * In Tauri v2, the internal bridge is exposed as `__TAURI_INTERNALS__`.
 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Invoke a Tauri command. Falls back to a console warning and empty result
 * when Tauri is not available (browser-only development).
 */
export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const mockInvoke =
    typeof window !== "undefined" ? window.__MOCK_TAURI_INVOKE__ : undefined;
  if (typeof mockInvoke === "function") {
    return (await mockInvoke(cmd, args)) as T;
  }

  if (isTauri()) {
    return tauriInvoke<T>(cmd, args);
  }

  throw new Error(
    `[tauri] invoke("${cmd}") failed: Tauri bridge unavailable and no browser mock is configured`,
  );
}

/**
 * Listen to a Tauri event. Falls back to a no-op unsubscribe function
 * when Tauri is not available.
 */
export async function listen<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<() => void> {
  const hasMock =
    typeof window !== "undefined" &&
    (typeof window.__MOCK_TAURI_INVOKE__ === "function" ||
      window.__MOCK_TAURI__ !== undefined);

  if (isTauri() && !hasMock) {
    const unlisten = await tauriListen<T>(event, (e) => {
      handler(e.payload);
    });
    return unlisten;
  }

  if (typeof window === "undefined") {
    return () => {
      /* no-op */
    };
  }

  const domHandler: EventListener = (evt) => {
    const custom = evt as CustomEvent<unknown>;
    const detail = custom.detail;

    if (
      detail !== null &&
      typeof detail === "object" &&
      "payload" in (detail as Record<string, unknown>)
    ) {
      handler((detail as { payload: T }).payload);
      return;
    }

    handler(detail as T);
  };

  window.addEventListener(event, domHandler);
  return () => {
    window.removeEventListener(event, domHandler);
  };
}

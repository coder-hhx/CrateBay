/**
 * Centralized Tauri mock for testing.
 *
 * Provides mock implementations for the @/lib/tauri wrapper and
 * @tauri-apps/api/core & @tauri-apps/api/event modules so that
 * all frontend tests can run without a real Tauri environment.
 *
 * Usage:
 *   // In individual test files or vitest setup:
 *   import { setupTauriMocks, mockInvokeResponses, resetTauriMocks } from "@/__mocks__/tauriMock";
 *
 *   // Setup mocks before imports that use Tauri
 *   setupTauriMocks();
 *
 *   // Optionally configure specific invoke responses
 *   mockInvokeResponses({
 *     container_list: [{ id: "abc", name: "node-01", ... }],
 *   });
 */
import { vi } from "vitest";

/**
 * Storage for mock invoke responses, keyed by Tauri command name.
 */
const invokeResponses: Record<string, unknown> = {};

/**
 * The mocked invoke function. Returns a registered mock response
 * if available, otherwise rejects with "Tauri not available in test".
 */
export const mockInvoke = vi.fn(
  (command: string, _args?: Record<string, unknown>): Promise<unknown> => {
    if (command in invokeResponses) {
      return Promise.resolve(invokeResponses[command]);
    }
    return Promise.reject(new Error(`Tauri not available in test: ${command}`));
  },
);

/**
 * The mocked listen function. Returns a no-op unlisten function.
 */
type ListenMock = (event: string, handler: (payload: unknown) => void) => Promise<() => void>;

export const mockListen = vi.fn<ListenMock>(() => Promise.resolve(() => {}));

/**
 * The mocked emit function. No-op.
 */
export const mockEmit = vi.fn();

/**
 * Register mock responses for Tauri invoke commands.
 * Merges with existing responses (does not replace them all).
 */
export function mockInvokeResponses(responses: Record<string, unknown>): void {
  Object.assign(invokeResponses, responses);
}

/**
 * Clear all registered mock responses and reset mock call history.
 */
export function resetTauriMocks(): void {
  for (const key of Object.keys(invokeResponses)) {
    delete invokeResponses[key];
  }
  mockInvoke.mockClear();
  mockListen.mockClear();
  mockEmit.mockClear();
}

/**
 * Setup all Tauri-related vi.mock() calls.
 * Must be called at the top of a test file, BEFORE importing modules
 * that use Tauri (stores, hooks, etc.).
 *
 * Mocks:
 *   - @tauri-apps/api/core  (invoke)
 *   - @tauri-apps/api/event (listen, emit)
 *   - @/lib/tauri           (invoke, listen, isTauri)
 */
export function setupTauriMocks(): void {
  vi.mock("@tauri-apps/api/core", () => ({
    invoke: mockInvoke,
  }));

  vi.mock("@tauri-apps/api/event", () => ({
    listen: mockListen,
    emit: mockEmit,
  }));

  vi.mock("@/lib/tauri", () => ({
    invoke: mockInvoke,
    listen: mockListen,
    isTauri: vi.fn(() => false),
  }));
}

import "@testing-library/jest-dom";
import { vi, beforeEach, expect } from "vitest";

// Polyfill ResizeObserver for jsdom (needed by @xyflow/react and recharts)
global.ResizeObserver = class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;

// ── Tauri IPC mock ──────────────────────────────────────────────────────────

export const mockInvoke = vi.fn().mockResolvedValue({});

// Full __TAURI_INTERNALS__ mock so both invoke() and listen()/emit() work.
let _cbId = 0;
Object.defineProperty(window, "__TAURI_INTERNALS__", {
  value: {
    invoke: (...args: unknown[]) => mockInvoke(...args),
    transformCallback: (_cb: unknown) => {
      _cbId += 1;
      return _cbId;
    },
    convertFileSrc: (path: string) => path,
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
  },
  writable: true,
  configurable: true,
});

// Also set __TAURI__ for components that use it directly (e.g. CodeEditor)
Object.defineProperty(window, "__TAURI__", {
  value: { invoke: (...args: unknown[]) => mockInvoke(...args) },
  writable: true,
  configurable: true,
});

// Mock the core module
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
  convertFileSrc: (path: string) => path,
}));

// Mock the event module — listen() returns a no-op unlisten function
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
  emit: vi.fn().mockResolvedValue(undefined),
  once: vi.fn().mockResolvedValue(() => {}),
}));

// ── Test helpers ────────────────────────────────────────────────────────────

/** Mock multiple Tauri commands with persistent responses. */
export function mockCommands(map: Record<string, unknown>): void {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd in map) return Promise.resolve(map[cmd]);
    return Promise.resolve({});
  });
}

/** Mock a Tauri command to reject. Other commands fall back to the
 *  provided map (if any) or resolve with `{}` (default). */
export function mockCommandError(command: string, error: string, fallbackMap?: Record<string, unknown>): void {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === command) return Promise.reject(new Error(error));
    if (fallbackMap && cmd in fallbackMap) return Promise.resolve(fallbackMap[cmd]);
    return Promise.resolve({});
  });
}

/** Assert a specific command was invoked. */
export function expectInvoked(command: string): void {
  const calls = mockInvoke.mock.calls.filter(
    (call: unknown[]) => call[0] === command,
  );
  if (calls.length === 0) {
    throw new Error(
      `Expected "${command}" to be invoked. Actual: ${mockInvoke.mock.calls.map((c: unknown[]) => c[0]).join(", ") || "(none)"}`,
    );
  }
}

/** Assert a command was invoked with specific args. */
export function expectInvokedWith(
  command: string,
  args: Record<string, unknown>,
): void {
  const calls = mockInvoke.mock.calls.filter(
    (call: unknown[]) => call[0] === command,
  );
  if (calls.length === 0) {
    throw new Error(`Expected "${command}" to be invoked`);
  }
  expect(calls[calls.length - 1][1]).toEqual(expect.objectContaining(args));
}

// Suppress noisy console output during tests
const origWarn = console.warn;
console.warn = (...args: unknown[]) => {
  const msg = String(args[0]);
  if (msg.includes("[") && msg.includes("]")) return;
  origWarn(...args);
};

// Reset between tests
beforeEach(() => {
  mockInvoke.mockReset();
  mockInvoke.mockResolvedValue({});
});

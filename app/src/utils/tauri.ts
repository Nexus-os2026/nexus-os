/**
 * Safe Tauri invoke wrapper — never crashes if Tauri bridge isn't ready.
 */

interface TauriWindow extends Window {
  __TAURI__?: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> };
  __TAURI_INTERNALS__?: unknown;
}

function runtimeWindow(): TauriWindow | null {
  if (typeof window === "undefined") return null;
  return window as unknown as TauriWindow;
}

export function hasTauriBridge(): boolean {
  const w = runtimeWindow();
  return w !== null && w.__TAURI__ !== undefined && typeof w.__TAURI__?.invoke === "function";
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export async function safeInvoke<T = any>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T | null> {
  try {
    const w = runtimeWindow();
    if (!w || !w.__TAURI__ || typeof w.__TAURI__.invoke !== "function") {
      console.warn(`Tauri not ready, skipping invoke: ${cmd}`);
      return null;
    }
    return (await w.__TAURI__.invoke(cmd, args)) as T;
  } catch (err) {
    console.error(`Tauri invoke failed (${cmd}):`, err);
    return null;
  }
}

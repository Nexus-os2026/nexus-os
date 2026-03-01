import { invoke } from "@tauri-apps/api/core";
import type { AgentSummary, AuditEventRow, VoiceRuntimeState } from "../types";

interface TauriWindow extends Window {
  __TAURI__?: unknown;
  __TAURI_INTERNALS__?: unknown;
}

function runtimeWindow(): TauriWindow | null {
  if (typeof window === "undefined") {
    return null;
  }
  return window as TauriWindow;
}

export function hasDesktopRuntime(): boolean {
  const runtime = runtimeWindow();
  if (!runtime) {
    return false;
  }
  return runtime.__TAURI__ !== undefined || runtime.__TAURI_INTERNALS__ !== undefined;
}

async function invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!hasDesktopRuntime()) {
    throw new Error("desktop runtime unavailable");
  }
  return invoke<T>(command, args);
}

function agentArgs(agentId: string): Record<string, unknown> {
  return { agentId, agent_id: agentId };
}

export function listAgents(): Promise<AgentSummary[]> {
  return invokeDesktop<AgentSummary[]>("list_agents");
}

export function getAuditLog(): Promise<AuditEventRow[]> {
  return invokeDesktop<AuditEventRow[]>("get_audit_log");
}

export function startAgent(agentId: string): Promise<void> {
  return invokeDesktop<void>("start_agent", agentArgs(agentId));
}

export function stopAgent(agentId: string): Promise<void> {
  return invokeDesktop<void>("stop_agent", agentArgs(agentId));
}

export function pauseAgent(agentId: string): Promise<void> {
  return invokeDesktop<void>("pause_agent", agentArgs(agentId));
}

export function resumeAgent(agentId: string): Promise<void> {
  return invokeDesktop<void>("resume_agent", agentArgs(agentId));
}

export function startJarvisMode(): Promise<VoiceRuntimeState> {
  return invokeDesktop<VoiceRuntimeState>("start_jarvis_mode");
}

export function stopJarvisMode(): Promise<VoiceRuntimeState> {
  return invokeDesktop<VoiceRuntimeState>("stop_jarvis_mode");
}

export function jarvisStatus(): Promise<VoiceRuntimeState> {
  return invokeDesktop<VoiceRuntimeState>("jarvis_status");
}

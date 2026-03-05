import { invoke } from "@tauri-apps/api/core";
import type {
  AgentSummary,
  AuditEventRow,
  AvailableModel,
  ChatResponse,
  HardwareInfo,
  NexusConfig,
  OllamaStatus,
  SetupResult,
  VoiceRuntimeState
} from "../types";

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

export function createAgent(manifestJson: string): Promise<string> {
  return invokeDesktop<string>("create_agent", { manifestJson, manifest_json: manifestJson });
}

export function getAuditLog(agentId?: string, limit = 200): Promise<AuditEventRow[]> {
  return invokeDesktop<AuditEventRow[]>("get_audit_log", {
    agentId,
    agent_id: agentId,
    limit
  });
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

export function sendChat(message: string): Promise<ChatResponse> {
  return invokeDesktop<ChatResponse>("send_chat", { message });
}

export function getConfig(): Promise<NexusConfig> {
  return invokeDesktop<NexusConfig>("get_config");
}

export function saveConfig(config: NexusConfig): Promise<void> {
  return invokeDesktop<void>("save_config", { config });
}

export function transcribePushToTalk(): Promise<string> {
  return invokeDesktop<string>("transcribe_push_to_talk");
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

export function detectHardware(): Promise<HardwareInfo> {
  return invokeDesktop<HardwareInfo>("detect_hardware");
}

export function checkOllama(baseUrl?: string): Promise<OllamaStatus> {
  return invokeDesktop<OllamaStatus>("check_ollama", { baseUrl, base_url: baseUrl });
}

export function pullOllamaModel(modelName: string, baseUrl?: string): Promise<string> {
  return invokeDesktop<string>("pull_ollama_model", {
    modelName,
    model_name: modelName,
    baseUrl,
    base_url: baseUrl
  });
}

export function runSetupWizard(ollamaUrl?: string): Promise<SetupResult> {
  return invokeDesktop<SetupResult>("run_setup_wizard", {
    ollamaUrl,
    ollama_url: ollamaUrl
  });
}

export function pullModel(modelName: string, baseUrl?: string): Promise<string> {
  return invokeDesktop<string>("pull_model", {
    modelName,
    model_name: modelName,
    baseUrl,
    base_url: baseUrl
  });
}

export function ensureOllama(baseUrl?: string): Promise<boolean> {
  return invokeDesktop<boolean>("ensure_ollama", {
    baseUrl,
    base_url: baseUrl
  });
}

export function isOllamaInstalled(): Promise<boolean> {
  return invokeDesktop<boolean>("is_ollama_installed");
}

export function deleteModel(modelName: string, baseUrl?: string): Promise<void> {
  return invokeDesktop<void>("delete_model", {
    modelName,
    model_name: modelName,
    baseUrl,
    base_url: baseUrl
  });
}

export function isSetupComplete(): Promise<boolean> {
  return invokeDesktop<boolean>("is_setup_complete");
}

export function listAvailableModels(): Promise<AvailableModel[]> {
  return invokeDesktop<AvailableModel[]>("list_available_models");
}

export function chatWithOllama(
  messages: Array<{ role: string; content: string }>,
  model: string,
  baseUrl?: string
): Promise<string> {
  return invokeDesktop<string>("chat_with_ollama", {
    messages,
    model,
    baseUrl,
    base_url: baseUrl
  });
}

export function setAgentModel(agent: string, model: string): Promise<void> {
  return invokeDesktop<void>("set_agent_model", { agent, model });
}

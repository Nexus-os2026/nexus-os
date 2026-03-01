export type AgentStatus = "Created" | "Starting" | "Running" | "Paused" | "Stopping" | "Stopped" | "Destroyed";

export interface AgentSummary {
  id: string;
  name: string;
  status: AgentStatus;
  fuel_remaining: number;
  last_action: string;
}

export interface AuditEventRow {
  event_id: string;
  timestamp: number;
  agent_id: string;
  event_type: string;
  payload: Record<string, unknown>;
  hash: string;
  previous_hash: string;
}

export interface VoiceRuntimeState {
  wake_word_enabled: boolean;
  push_to_talk_enabled: boolean;
  overlay_visible: boolean;
}

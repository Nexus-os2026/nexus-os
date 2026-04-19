/**
 * Typed wrappers over the 9 swarm Tauri commands + `oracle_runtime_status`.
 *
 * JS idiom on the outside (camelCase function names), Rust contract on the
 * inside (snake_case `invoke` strings + snake_case argument keys). Callers
 * never see the `swarm_*` prefix.
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  AuditEntry,
  OracleRuntimeStatus,
  PlannedSwarmJson,
  ProviderHealth,
  Uuid,
} from "./types";

/** Server snapshot shape for `swarm_state` (Phase 1 surface). */
export interface SwarmStateSnapshot {
  run_id: Uuid;
  present: boolean;
}

export async function planSwarm(intent: string): Promise<PlannedSwarmJson> {
  return invoke<PlannedSwarmJson>("swarm_plan", { intent });
}

/** Takes a ticket id (NOT a dag id); returns the newly-spawned run id. */
export async function approveSwarm(ticketId: Uuid): Promise<Uuid> {
  return invoke<Uuid>("swarm_approve", { ticketId, ticket_id: ticketId });
}

export async function rejectSwarm(ticketId: Uuid, reason?: string): Promise<void> {
  await invoke<void>("swarm_reject", {
    ticketId,
    ticket_id: ticketId,
    reason: reason ?? null,
  });
}

export async function cancelSwarm(runId: Uuid): Promise<void> {
  await invoke<void>("swarm_cancel", { runId, run_id: runId });
}

export async function cancelSwarmNode(runId: Uuid, nodeId: string): Promise<void> {
  await invoke<void>("swarm_cancel_node", {
    runId,
    run_id: runId,
    nodeId,
    node_id: nodeId,
  });
}

export async function getSwarmState(runId: Uuid): Promise<SwarmStateSnapshot> {
  return invoke<SwarmStateSnapshot>("swarm_state", { runId, run_id: runId });
}

export async function getProviderHealth(): Promise<ProviderHealth[]> {
  return invoke<ProviderHealth[]>("swarm_provider_health");
}

/**
 * Probe every provider afresh. The return value is discarded deliberately —
 * the backend also emits a `ProviderHealthUpdate` event, which the store's
 * dispatcher will consume. Single source of truth for provider state is
 * the event stream.
 */
export async function refreshProviderHealth(): Promise<void> {
  await invoke<ProviderHealth[]>("swarm_refresh_provider_health");
}

export async function getSwarmAuditTail(runId: Uuid): Promise<AuditEntry[]> {
  return invoke<AuditEntry[]>("swarm_audit_tail", { runId, run_id: runId });
}

export async function getOracleRuntimeStatus(): Promise<OracleRuntimeStatus> {
  return invoke<OracleRuntimeStatus>("oracle_runtime_status");
}

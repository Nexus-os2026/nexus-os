/**
 * TypeScript types mirroring the `nexus-swarm` Rust crate's serde JSON output.
 *
 * Field names are kept snake_case verbatim from the Rust source so that
 * discriminated-union matching works against raw Tauri event payloads. Do
 * NOT camelCase. The source of truth lives in:
 *   - crates/nexus-swarm/src/events.rs      (SwarmEvent + ProviderHealth)
 *   - crates/nexus-swarm/src/dag.rs         (DagSnapshot + DagNode + DagNodeStatus)
 *   - crates/nexus-swarm/src/profile.rs     (TaskProfile + PrivacyClass + friends)
 *   - crates/nexus-swarm/src/budget.rs      (Budget)
 *   - crates/nexus-swarm/src/routing.rs     (RouteDenied)
 *   - crates/nexus-swarm/src/oracle_policy.rs (HighRiskEvent + OracleDecisionSummary)
 *   - app/src-tauri/src/commands/swarm.rs   (PlannedSwarmJson + AuditEntry)
 *   - app/src-tauri/src/oracle_runtime.rs   (OracleRuntimeStatus)
 */

// ── Primitives ──────────────────────────────────────────────────────────────

/** Uuid in serde is rendered as a string. */
export type Uuid = string;

// ── TaskProfile + enums (profile.rs) ────────────────────────────────────────

export type PrivacyClass = "Public" | "Sensitive" | "StrictLocal";
export type ReasoningTier = "Fast" | "Balanced" | "Deep";
export type ToolUseLevel = "None" | "Basic" | "Rich";
export type LatencyClass = "Interactive" | "Batch" | "Background";
export type ContextSize = "Small" | "Medium" | "Large";
export type CostClass = "Free" | "Cheap" | "Moderate" | "Premium";

export interface TaskProfile {
  privacy: PrivacyClass;
  reasoning: ReasoningTier;
  tool_use: ToolUseLevel;
  latency: LatencyClass;
  context: ContextSize;
  cost: CostClass;
}

// ── Budget (budget.rs) ──────────────────────────────────────────────────────

export interface Budget {
  tokens: number;
  cost_cents: number;
  wall_ms: number;
  subagent_depth: number;
}

// ── DAG (dag.rs) ────────────────────────────────────────────────────────────

/**
 * Externally-tagged enum from Rust: unit variants serialize as plain strings,
 * tuple variants serialize as `{ "Done": value }`.
 */
export type DagNodeStatus =
  | "Pending"
  | "Ready"
  | "Running"
  | "Skipped"
  | { Done: unknown }
  | { Failed: string };

export interface DagNode {
  id: string;
  capability_id: string;
  profile: TaskProfile;
  inputs: unknown;
  status: DagNodeStatus;
}

export interface DagEdge {
  from: string;
  to: string;
}

/**
 * What `dag.to_json()` emits on the wire — the `dag_json` field of
 * `PlanProposed` and the `dag` field of `PlannedSwarmJson` both take
 * this shape.
 */
export interface ExecutionDagJson {
  nodes: DagNode[];
  edges: DagEdge[];
}

// ── ProviderHealth (events.rs) ──────────────────────────────────────────────

export type ProviderHealthStatus = "Ok" | "Degraded" | "Unhealthy";

export interface ProviderHealth {
  provider_id: string;
  status: ProviderHealthStatus;
  /** Null when the probe failed before a response arrived. */
  latency_ms: number | null;
  models: string[];
  notes: string;
  checked_at_secs: number;
}

// ── Routing (routing.rs) ────────────────────────────────────────────────────

export interface RouteDeniedDetail {
  agent_id: string;
  reasons: string[];
}

// ── Oracle policy (oracle_policy.rs) ────────────────────────────────────────

export interface OracleDecisionSummary {
  approved: boolean;
  /** Opaque correlation handle; null on denial. */
  token_id: Uuid | null;
}

/**
 * High-risk runtime event vocabulary. Serde tag `kind`, snake_case variants.
 */
export type HighRiskEvent =
  | {
      kind: "cloud_call_above_threshold";
      provider_id: string;
      estimated_cents: number;
    }
  | {
      kind: "subagent_spawn_attempt";
      parent_node: string;
      depth: number;
    }
  | {
      kind: "privacy_class_escalation";
      from: PrivacyClass;
      to: PrivacyClass;
    }
  | {
      kind: "budget_soft_limit_approach";
      consumed_pct: number;
    }
  | {
      kind: "plan_drift";
      original_hash: string;
      current_hash: string;
    };

// ── NodeRef (events.rs) ─────────────────────────────────────────────────────

export interface NodeRef {
  run_id: Uuid;
  node_id: string;
}

// ── SwarmEvent discriminated union (events.rs) ──────────────────────────────
//
// Serde attributes on the Rust enum:
//   #[serde(tag = "event", rename_all = "snake_case")]
//
// The NodeRef is emitted under the JSON key `ref` because Rust uses the
// raw identifier `r#ref`. Preserve that field name verbatim — renaming
// breaks the union match.

export type SwarmEvent =
  | { event: "plan_proposed"; run_id: Uuid; dag_json: ExecutionDagJson }
  | { event: "plan_approved"; run_id: Uuid }
  | { event: "plan_rejected"; run_id: Uuid; reason: string }
  | {
      event: "node_started";
      ref: NodeRef;
      capability_id: string;
      provider_id: string;
      model_id: string;
      ticket_nonce: Uuid;
    }
  | {
      event: "node_event";
      ref: NodeRef;
      phase: string;
      payload: unknown;
      ticket_nonce: Uuid;
    }
  | {
      event: "node_completed";
      ref: NodeRef;
      result: unknown;
      ticket_nonce: Uuid;
    }
  | {
      event: "node_failed";
      ref: NodeRef;
      reason: string;
      ticket_nonce: Uuid;
    }
  | {
      event: "route_denied";
      ref: NodeRef;
      denied: RouteDeniedDetail;
    }
  | {
      event: "budget_update";
      run_id: Uuid;
      tokens_remaining: number;
      cents_remaining: number;
      wall_ms_remaining: number;
      ticket_nonce: Uuid;
    }
  | { event: "provider_health_update"; providers: ProviderHealth[] }
  | { event: "swarm_completed"; run_id: Uuid }
  | { event: "swarm_cancelled"; run_id: Uuid }
  | {
      event: "oracle_ticket_issued";
      ticket_id: Uuid;
      budget_hash: string;
      dag_content_hash: string;
    }
  | {
      event: "oracle_runtime_check";
      ticket_nonce: Uuid;
      highrisk_event: HighRiskEvent;
      decision: OracleDecisionSummary;
    }
  | {
      event: "oracle_runtime_denial";
      ticket_nonce: Uuid;
      hints: string[];
      node_id: string;
    };

/** Convenience: the discriminator value space. */
export type SwarmEventKind = SwarmEvent["event"];

// ── PlannedSwarmJson (commands/swarm.rs) ────────────────────────────────────

export interface PlannedSwarmJson {
  dag: ExecutionDagJson;
  ticket_id: Uuid;
  budget_hash: string;
  privacy_envelope: PrivacyClass;
}

// ── AuditEntry (commands/swarm.rs) ──────────────────────────────────────────
//
// `timestamp` is Rust's SystemTime which serializes as
// `{ "secs_since_epoch": u64, "nanos_since_epoch": u32 }`.

export interface AuditTimestamp {
  secs_since_epoch: number;
  nanos_since_epoch: number;
}

export interface AuditEntry {
  seq: number;
  event_kind: string;
  ticket_nonce: Uuid;
  timestamp: AuditTimestamp;
  payload_summary: string;
}

// ── OracleRuntimeStatus (oracle_runtime.rs) ─────────────────────────────────

export interface OracleRuntimeStatus {
  is_running: boolean;
  pending_requests: number;
  total_processed: number;
  uptime_seconds: number;
}

// ── Run state (frontend-only projection) ────────────────────────────────────

/**
 * In-memory projection of an active run held by the store. Not a
 * server-side type — assembled from incoming events.
 */
export interface RunState {
  run_id: Uuid;
  dag: ExecutionDagJson;
  /** Map from node id → current status as last reported by events. */
  node_states: Record<string, DagNodeStatus>;
  ticket_id: Uuid | null;
  started_at_ms: number;
}

/**
 * Track C #1: coarse grouping of `AuditEntry.event_kind` strings for
 * the Swarm Audit page's color pills + filter chips. Reuses the same
 * `EventCategory` palette as the live `EventTape` so kind-of-event
 * coloring stays consistent across pages.
 *
 * Exhaustive over `AuditEventKind`; the `never` guard catches any new
 * kind added Rust-side without a category assignment here.
 */

import type { EventCategory } from "./event_category";
import type { AuditEventKind } from "./types";

export function auditKindCategory(kind: AuditEventKind): EventCategory {
  switch (kind) {
    case "node_started":
    case "node_event":
    case "node_completed":
    case "node_failed":
      return "node";
    case "budget_update":
      return "swarm";
    case "oracle_runtime_check":
    case "oracle_runtime_denial":
      return "oracle";
    default: {
      const _exhaustive: never = kind;
      void _exhaustive;
      return "swarm";
    }
  }
}

export const AUDIT_EVENT_KINDS: readonly AuditEventKind[] = Object.freeze([
  "node_started",
  "node_event",
  "node_completed",
  "node_failed",
  "budget_update",
  "oracle_runtime_check",
  "oracle_runtime_denial",
]);

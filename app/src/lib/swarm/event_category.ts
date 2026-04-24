/**
 * Coarse grouping of `SwarmEvent` variants for UI filter chips and pill
 * colors in the event tape. Exhaustive over the 15 variants defined in
 * `types.ts`; the `never` guard catches any new variant that lands
 * without a category assignment.
 */

import type { SwarmEvent } from "./types";

export type EventCategory = "plan" | "node" | "oracle" | "provider" | "swarm";

export function eventCategory(ev: SwarmEvent): EventCategory {
  switch (ev.event) {
    case "plan_proposed":
    case "plan_approved":
    case "plan_rejected":
      return "plan";
    case "node_started":
    case "node_event":
    case "node_completed":
    case "node_failed":
    case "route_denied":
      return "node";
    case "oracle_ticket_issued":
    case "oracle_runtime_check":
    case "oracle_runtime_denial":
      return "oracle";
    case "provider_health_update":
      return "provider";
    case "budget_update":
    case "swarm_completed":
    case "swarm_cancelled":
      return "swarm";
    default: {
      const _exhaustive: never = ev;
      void _exhaustive;
      return "swarm";
    }
  }
}

export const EVENT_CATEGORIES: readonly EventCategory[] = Object.freeze([
  "plan",
  "node",
  "oracle",
  "provider",
  "swarm",
]);

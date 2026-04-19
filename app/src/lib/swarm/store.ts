/**
 * Hand-rolled swarm store backed by `useSyncExternalStore`. No Zustand — the
 * surface we need (subscribe-notify-getSnapshot) is already in the stdlib
 * of React 18, and the external events we dispatch on flow through the bus
 * singleton rather than caller-invoked setters.
 *
 * Wiring:
 *   - The store subscribes to `swarmBus` once at module load via a top-level
 *     dispatcher that maps each `SwarmEvent` variant to a state mutation.
 *   - React components read state through `useSwarmStore(selector)`.
 *   - The only caller-driven mutations are `setInitialProviderHealth` (used
 *     by the App-level bootstrap to seed the store before the first
 *     `provider_health_update` event arrives) and `reset` (for tests).
 */

import { useSyncExternalStore } from "react";
import { swarmBus } from "./swarm_bus";
import type {
  DagNodeStatus,
  PlannedSwarmJson,
  ProviderHealth,
  RunState,
  SwarmEvent,
} from "./types";

export interface SwarmState {
  providerHealth: ProviderHealth[];
  currentPlan: PlannedSwarmJson | null;
  activeRun: RunState | null;
  /** Bounded tail of recent events for components that need the stream. */
  recentEvents: SwarmEvent[];
}

const RECENT_EVENT_CAP = 100;

const initialState: SwarmState = {
  providerHealth: [],
  currentPlan: null,
  activeRun: null,
  recentEvents: [],
};

// ── Store plumbing ──────────────────────────────────────────────────────────

let state: SwarmState = initialState;
const listeners = new Set<() => void>();

function setState(updater: (prev: SwarmState) => SwarmState): void {
  const next = updater(state);
  if (next === state) return;
  state = next;
  for (const l of listeners) l();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot(): SwarmState {
  return state;
}

// ── Hook ───────────────────────────────────────────────────────────────────-

export function useSwarmStore<T>(selector: (s: SwarmState) => T): T {
  return useSyncExternalStore(
    subscribe,
    () => selector(getSnapshot()),
    () => selector(initialState),
  );
}

/** Non-hook accessor for tests and the App bootstrap. */
export function getSwarmState(): SwarmState {
  return state;
}

// ── Caller-driven mutations ────────────────────────────────────────────────

/** Seed provider health before any events arrive. */
export function setInitialProviderHealth(providers: ProviderHealth[]): void {
  setState((prev) => ({ ...prev, providerHealth: providers }));
}

/** Test-only. Resets the store and re-attaches the dispatcher subscription. */
export function __resetSwarmStoreForTest(): void {
  state = initialState;
  // Listeners intentionally preserved — test components rely on their
  // subscriptions surviving a logical reset.
}

// ── Event dispatcher ───────────────────────────────────────────────────────

function appendRecent(prev: SwarmState, ev: SwarmEvent): SwarmEvent[] {
  const next = prev.recentEvents.length >= RECENT_EVENT_CAP
    ? prev.recentEvents.slice(prev.recentEvents.length - RECENT_EVENT_CAP + 1)
    : [...prev.recentEvents];
  next.push(ev);
  return next;
}

function dispatch(ev: SwarmEvent): void {
  setState((prev) => {
    const recentEvents = appendRecent(prev, ev);

    switch (ev.event) {
      case "provider_health_update":
        return { ...prev, providerHealth: ev.providers, recentEvents };

      case "plan_proposed": {
        // The dag_json is the full DAG; we don't have budget_hash /
        // ticket_id here (those come via oracle_ticket_issued), so
        // currentPlan is only fully populated after the oracle fires. For
        // the shell, a provisional plan with empty ticket fields is
        // enough to light up the UI.
        const currentPlan: PlannedSwarmJson = {
          dag: ev.dag_json,
          ticket_id: "",
          budget_hash: "",
          privacy_envelope: "Public",
        };
        return { ...prev, currentPlan, recentEvents };
      }

      case "oracle_ticket_issued": {
        if (!prev.currentPlan) return { ...prev, recentEvents };
        return {
          ...prev,
          currentPlan: {
            ...prev.currentPlan,
            ticket_id: ev.ticket_id,
            budget_hash: ev.budget_hash,
          },
          recentEvents,
        };
      }

      case "plan_approved": {
        // The plan has been approved; clear currentPlan and stand up an
        // activeRun skeleton. Node states fill in via node_* events.
        const dag = prev.currentPlan?.dag ?? { nodes: [], edges: [] };
        const activeRun: RunState = {
          run_id: ev.run_id,
          dag,
          node_states: Object.fromEntries(
            dag.nodes.map((n): [string, DagNodeStatus] => [n.id, n.status]),
          ),
          ticket_id: prev.currentPlan?.ticket_id ?? null,
          started_at_ms: Date.now(),
        };
        return { ...prev, currentPlan: null, activeRun, recentEvents };
      }

      case "plan_rejected":
        return { ...prev, currentPlan: null, recentEvents };

      case "node_started": {
        if (!prev.activeRun || prev.activeRun.run_id !== ev.ref.run_id) {
          return { ...prev, recentEvents };
        }
        return {
          ...prev,
          activeRun: {
            ...prev.activeRun,
            node_states: { ...prev.activeRun.node_states, [ev.ref.node_id]: "Running" },
          },
          recentEvents,
        };
      }

      case "node_completed": {
        if (!prev.activeRun || prev.activeRun.run_id !== ev.ref.run_id) {
          return { ...prev, recentEvents };
        }
        return {
          ...prev,
          activeRun: {
            ...prev.activeRun,
            node_states: {
              ...prev.activeRun.node_states,
              [ev.ref.node_id]: { Done: ev.result },
            },
          },
          recentEvents,
        };
      }

      case "node_failed": {
        if (!prev.activeRun || prev.activeRun.run_id !== ev.ref.run_id) {
          return { ...prev, recentEvents };
        }
        return {
          ...prev,
          activeRun: {
            ...prev.activeRun,
            node_states: {
              ...prev.activeRun.node_states,
              [ev.ref.node_id]: { Failed: ev.reason },
            },
          },
          recentEvents,
        };
      }

      case "route_denied": {
        if (!prev.activeRun || prev.activeRun.run_id !== ev.ref.run_id) {
          return { ...prev, recentEvents };
        }
        return {
          ...prev,
          activeRun: {
            ...prev.activeRun,
            node_states: {
              ...prev.activeRun.node_states,
              [ev.ref.node_id]: { Failed: ev.denied.reasons.join("; ") },
            },
          },
          recentEvents,
        };
      }

      case "swarm_completed":
      case "swarm_cancelled": {
        if (!prev.activeRun || prev.activeRun.run_id !== ev.run_id) {
          return { ...prev, recentEvents };
        }
        return { ...prev, activeRun: null, recentEvents };
      }

      // Node progress, budget, oracle-runtime variants don't mutate the
      // top-level store shape today — they land in `recentEvents` only.
      case "node_event":
      case "budget_update":
      case "oracle_runtime_check":
      case "oracle_runtime_denial":
        return { ...prev, recentEvents };

      default: {
        // Exhaustiveness guard — adding a new SwarmEvent variant without
        // teaching the dispatcher lights up a TS error here.
        const _exhaustive: never = ev;
        void _exhaustive;
        return { ...prev, recentEvents };
      }
    }
  });
}

// Attach dispatcher to the bus on module load. This runs once per JS
// module instance — the bus's own fan-out handles the multi-consumer case.
swarmBus.subscribe(dispatch);

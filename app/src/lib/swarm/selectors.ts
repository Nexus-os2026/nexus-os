/**
 * Pure selectors over the swarm store. No React imports — these are
 * callable from hooks, event handlers, and tests alike.
 */

import type {
  DagEdge,
  DagNode,
  DagNodeStatus,
  PlannedSwarmJson,
  ProviderHealth,
  RunState,
  SwarmEvent,
} from "./types";
import type { EventFilter, SwarmState } from "./store";
import { eventCategory } from "./event_category";

// Stable empty sentinels. Selectors must return a reference-stable value
// when the underlying state is "empty" — useSyncExternalStore compares
// snapshots with Object.is and triggers an infinite render loop if a new
// array is allocated on every call.
const EMPTY_NODES: readonly never[] = Object.freeze([]);
const EMPTY_EDGES: readonly never[] = Object.freeze([]);
const EMPTY_EVENTS: readonly never[] = Object.freeze([]);

export function selectCurrentPlan(state: SwarmState): PlannedSwarmJson | null {
  return state.currentPlan;
}

export function selectActiveRun(state: SwarmState): RunState | null {
  return state.activeRun;
}

/**
 * DAG nodes come from the active run if present, otherwise from the
 * current proposed plan. Returns `[]` if neither is populated so
 * consumers can render an empty state without a null check.
 */
export function selectDagNodes(state: SwarmState): readonly DagNode[] {
  if (state.activeRun) return state.activeRun.dag.nodes;
  if (state.currentPlan) return state.currentPlan.dag.nodes;
  return EMPTY_NODES;
}

export function selectDagEdges(state: SwarmState): readonly DagEdge[] {
  if (state.activeRun) return state.activeRun.dag.edges;
  if (state.currentPlan) return state.currentPlan.dag.edges;
  return EMPTY_EDGES;
}

/**
 * Returns the live status for a node. Prefers activeRun's per-node state
 * (populated by node_* events), falls back to the DAG snapshot status,
 * and defaults to `"Pending"` if the node isn't known.
 */
export function selectNodeStatus(nodeId: string): (state: SwarmState) => DagNodeStatus {
  return (state: SwarmState): DagNodeStatus => {
    if (state.activeRun) {
      const live = state.activeRun.node_states[nodeId];
      if (live !== undefined) return live;
      const fromDag = state.activeRun.dag.nodes.find((n) => n.id === nodeId);
      if (fromDag) return fromDag.status;
    }
    if (state.currentPlan) {
      const fromPlan = state.currentPlan.dag.nodes.find((n) => n.id === nodeId);
      if (fromPlan) return fromPlan.status;
    }
    return "Pending";
  };
}

export function selectProviderHealth(state: SwarmState): ProviderHealth[] {
  return state.providerHealth;
}

/**
 * True when a plan has been proposed but not yet approved or rejected. The
 * approval card listens on this; visibility flips false the moment the
 * backend emits `plan_approved` (which nulls currentPlan and populates
 * activeRun) or `plan_rejected` (which nulls currentPlan).
 */
export function selectIsPlanPending(state: SwarmState): boolean {
  return state.currentPlan !== null && state.activeRun === null;
}

export function selectFocusedNodeId(state: SwarmState): string | null {
  return state.focusedNodeId;
}

/**
 * Nodes currently in the Running state. Driven off the live node_states
 * map in activeRun; falls back to scanning DAG status if node_states is
 * absent (e.g. immediately after plan_approved, before the first
 * node_started event). Memoized by (state, activeRun, node_states)
 * identity so useSyncExternalStore doesn't see a fresh array on every
 * render and loop forever.
 */
let _activeNodesCacheState: SwarmState | null = null;
let _activeNodesCacheResult: readonly DagNode[] = EMPTY_NODES;
export function selectActiveNodes(state: SwarmState): readonly DagNode[] {
  if (_activeNodesCacheState === state) return _activeNodesCacheResult;
  const run = state.activeRun;
  let result: readonly DagNode[];
  if (!run) {
    result = EMPTY_NODES;
  } else {
    const running: DagNode[] = [];
    for (const n of run.dag.nodes) {
      const live = run.node_states[n.id];
      const status: DagNodeStatus = live ?? n.status;
      if (status === "Running") running.push(n);
    }
    result = running.length === 0 ? EMPTY_NODES : running;
  }
  _activeNodesCacheState = state;
  _activeNodesCacheResult = result;
  return result;
}

export function selectRecentEvents(state: SwarmState): readonly SwarmEvent[] {
  return state.recentEvents.length === 0 ? EMPTY_EVENTS : state.recentEvents;
}

/** The DAG node id attached to an event, if any. */
function eventNodeId(ev: SwarmEvent): string | null {
  if ("ref" in ev) return ev.ref.node_id;
  if (ev.event === "oracle_runtime_denial") return ev.node_id;
  return null;
}

/** Events whose category matches a `byAgent` (capability_id) filter. */
function nodeMatchesCapability(
  ev: SwarmEvent,
  capability: string,
  capabilityByNode: Map<string, string>,
): boolean {
  const nid = eventNodeId(ev);
  if (nid === null) return false;
  return capabilityByNode.get(nid) === capability;
}

/**
 * Build a lookup of node_id → capability_id from the active run or current
 * plan. Returned as a `Map` so it's cheap to pass to the filter predicate.
 */
function capabilityIndex(state: SwarmState): Map<string, string> {
  const out = new Map<string, string>();
  const source = state.activeRun?.dag.nodes ?? state.currentPlan?.dag.nodes ?? [];
  for (const n of source) out.set(n.id, n.capability_id);
  return out;
}

function isFailureEvent(ev: SwarmEvent): boolean {
  return (
    ev.event === "node_failed" ||
    ev.event === "plan_rejected" ||
    ev.event === "oracle_runtime_denial" ||
    ev.event === "route_denied" ||
    (ev.event === "oracle_runtime_check" && !ev.decision.approved)
  );
}

/**
 * Applies the active event filter. Returns the frozen empty sentinel only
 * when the underlying store arrays are empty; any filter-driven empty
 * result still allocates (caller consumes as read-only).
 */
export function selectFilteredEvents(
  filter: EventFilter,
): (state: SwarmState) => readonly SwarmEvent[] {
  // Closure-local cache: each selector instance remembers the last
  // state snapshot + result. useSyncExternalStore then sees a stable
  // reference across renders and doesn't loop.
  let cacheState: SwarmState | null = null;
  let cacheResult: readonly SwarmEvent[] = EMPTY_EVENTS;
  return (state: SwarmState): readonly SwarmEvent[] => {
    if (cacheState === state) return cacheResult;
    const src = state.recentEvents;
    let result: readonly SwarmEvent[];
    if (src.length === 0) {
      result = EMPTY_EVENTS;
    } else {
      const needsAgent = filter.byAgent !== null;
      const needsType = filter.byEventType !== null;
      const needsSev = filter.severityOnly;
      if (!needsAgent && !needsType && !needsSev) {
        result = src;
      } else {
        const capIndex = needsAgent ? capabilityIndex(state) : null;
        const filtered = src.filter((ev) => {
          if (needsSev && !isFailureEvent(ev)) return false;
          if (needsType && eventCategory(ev) !== filter.byEventType) return false;
          if (
            needsAgent &&
            capIndex &&
            !nodeMatchesCapability(ev, filter.byAgent as string, capIndex)
          ) {
            return false;
          }
          return true;
        });
        result = filtered.length === 0 ? EMPTY_EVENTS : filtered;
      }
    }
    cacheState = state;
    cacheResult = result;
    return result;
  };
}

export function selectEventsByNode(
  nodeId: string,
): (state: SwarmState) => readonly SwarmEvent[] {
  let cacheState: SwarmState | null = null;
  let cacheResult: readonly SwarmEvent[] = EMPTY_EVENTS;
  return (state: SwarmState): readonly SwarmEvent[] => {
    if (cacheState === state) return cacheResult;
    const src = state.recentEvents;
    let result: readonly SwarmEvent[];
    if (src.length === 0) {
      result = EMPTY_EVENTS;
    } else {
      const filtered = src.filter((ev) => eventNodeId(ev) === nodeId);
      result = filtered.length === 0 ? EMPTY_EVENTS : filtered;
    }
    cacheState = state;
    cacheResult = result;
    return result;
  };
}

export function selectActiveRunCancellable(state: SwarmState): boolean {
  return state.activeRun !== null;
}

export function selectEventFilter(state: SwarmState): EventFilter {
  return state.eventFilter;
}

export { isFailureEvent };

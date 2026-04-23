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
} from "./types";
import type { SwarmState } from "./store";

// Stable empty sentinels. Selectors must return a reference-stable value
// when the underlying state is "empty" — useSyncExternalStore compares
// snapshots with Object.is and triggers an infinite render loop if a new
// array is allocated on every call.
const EMPTY_NODES: readonly never[] = Object.freeze([]);
const EMPTY_EDGES: readonly never[] = Object.freeze([]);

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

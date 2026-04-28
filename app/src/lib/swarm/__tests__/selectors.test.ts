import { describe, expect, it } from "vitest";
import { selectRunProgress } from "../selectors";
import type { DagNode, RunState } from "../types";
import type { EventFilter, SwarmState } from "../store";

const EMPTY_FILTER: EventFilter = Object.freeze({
  byAgent: null,
  byEventType: null,
  severityOnly: false,
});

function dagNode(id: string): DagNode {
  return {
    id,
    capability_id: "cap",
    profile: {
      reasoning: "Light",
      tool_use: "Basic",
      latency: "Interactive",
      context: "Small",
      privacy: "Public",
      cost: "Free",
    },
    inputs: null,
    status: "Pending",
  };
}

function makeState(activeRun: RunState | null): SwarmState {
  return {
    providerHealth: [],
    currentPlan: null,
    activeRun,
    recentEvents: [],
    focusedNodeId: null,
    eventFilter: EMPTY_FILTER,
  };
}

describe("selectRunProgress", () => {
  it("returns the frozen-zero sentinel when no active run", () => {
    const state = makeState(null);
    const result = selectRunProgress(state);
    expect(result).toEqual({ done: 0, total: 0, running: 0, failed: 0 });
    expect(Object.isFrozen(result)).toBe(true);
  });

  it("returns identity-stable result across calls when state is unchanged", () => {
    const state = makeState(null);
    const a = selectRunProgress(state);
    const b = selectRunProgress(state);
    expect(a).toBe(b);
  });

  it("computes mixed-state counts from node_states", () => {
    const run: RunState = {
      run_id: "11111111-2222-3333-4444-555555555555",
      dag: {
        nodes: [
          dagNode("n1"),
          dagNode("n2"),
          dagNode("n3"),
          dagNode("n4"),
          dagNode("n5"),
          dagNode("n6"),
        ],
        edges: [],
      },
      node_states: {
        n1: { Done: "ok" },
        n2: { Done: "ok" },
        n3: "Running",
        n4: { Failed: "boom" },
        // n5 falls back to dag.status (Pending); n6 too
      },
      node_budgets: {},
      ticket_id: null,
      started_at_ms: Date.now(),
    };
    const state = makeState(run);
    const result = selectRunProgress(state);
    expect(result).toEqual({ done: 2, total: 6, running: 1, failed: 1 });
  });

  it("derives a fresh result when state identity changes", () => {
    const state1 = makeState(null);
    const a = selectRunProgress(state1);
    const run: RunState = {
      run_id: "abcdef01-2222-3333-4444-555555555555",
      dag: { nodes: [dagNode("only")], edges: [] },
      node_states: { only: "Running" },
      node_budgets: {},
      ticket_id: null,
      started_at_ms: 0,
    };
    const state2 = makeState(run);
    const b = selectRunProgress(state2);
    expect(a).not.toBe(b);
    expect(b.running).toBe(1);
    expect(b.total).toBe(1);
  });
});

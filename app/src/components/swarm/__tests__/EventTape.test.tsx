import { render, screen, act, fireEvent, within } from "@testing-library/react";
import { describe, it, expect, beforeEach } from "vitest";
import { EventTape } from "../EventTape";
import { swarmBus } from "../../../lib/swarm/swarm_bus";
import { __resetSwarmStoreForTest } from "../../../lib/swarm/store";
import type { DagNode, SwarmEvent } from "../../../lib/swarm/types";

function stubNode(id: string, capability: string): DagNode {
  return {
    id,
    capability_id: capability,
    profile: {
      privacy: "Public",
      reasoning: "Balanced",
      tool_use: "Basic",
      latency: "Batch",
      context: "Small",
      cost: "Cheap",
    },
    inputs: null,
    status: "Pending",
  };
}

function planProposed(nodes: DagNode[]): SwarmEvent {
  return { event: "plan_proposed", run_id: "run-1", dag_json: { nodes, edges: [] } };
}

function nodeStarted(nodeId: string): SwarmEvent {
  return {
    event: "node_started",
    ref: { run_id: "run-1", node_id: nodeId },
    capability_id: "research",
    provider_id: "ollama",
    model_id: "gemma4:e2b",
    ticket_nonce: "00000000-0000-0000-0000-000000000000",
  };
}

function nodeFailed(nodeId: string, reason = "boom"): SwarmEvent {
  return {
    event: "node_failed",
    ref: { run_id: "run-1", node_id: nodeId },
    reason,
    ticket_nonce: "00000000-0000-0000-0000-000000000000",
  };
}

function providerHealth(): SwarmEvent {
  return {
    event: "provider_health_update",
    providers: [
      {
        provider_id: "ollama",
        status: "Ok",
        latency_ms: 20,
        models: [],
        notes: "",
        checked_at_secs: 0,
      },
    ],
  };
}

beforeEach(() => {
  __resetSwarmStoreForTest();
});

describe("EventTape", () => {
  it("empty state when no events", () => {
    render(<EventTape />);
    expect(screen.getByTestId("event-tape-empty")).toBeInTheDocument();
  });

  it("renders events newest-first", () => {
    act(() => {
      swarmBus.__injectForTest(providerHealth());
      swarmBus.__injectForTest(planProposed([stubNode("n0", "research")]));
      swarmBus.__injectForTest(nodeStarted("n0"));
    });
    render(<EventTape />);
    const rows = screen.getAllByTestId(/event-tape-row-/);
    expect(rows).toHaveLength(3);
    expect(rows[0].getAttribute("data-event-kind")).toBe("node_started");
    expect(rows[rows.length - 1].getAttribute("data-event-kind")).toBe("provider_health_update");
  });

  it("failures-only toggle excludes non-failure events", () => {
    act(() => {
      swarmBus.__injectForTest(providerHealth());
      swarmBus.__injectForTest(planProposed([stubNode("n0", "research")]));
      swarmBus.__injectForTest(nodeStarted("n0"));
      swarmBus.__injectForTest(nodeFailed("n0"));
    });
    render(<EventTape />);
    expect(screen.getAllByTestId(/event-tape-row-/)).toHaveLength(4);
    fireEvent.click(screen.getByTestId("filter-severity-only"));
    const rows = screen.getAllByTestId(/event-tape-row-/);
    expect(rows).toHaveLength(1);
    expect(rows[0].getAttribute("data-event-kind")).toBe("node_failed");
  });

  it("type-filter chip narrows the tape to one category", () => {
    act(() => {
      swarmBus.__injectForTest(providerHealth());
      swarmBus.__injectForTest(planProposed([stubNode("n0", "research")]));
      swarmBus.__injectForTest(nodeStarted("n0"));
    });
    render(<EventTape />);
    fireEvent.click(screen.getByTestId("filter-type-node"));
    const rows = screen.getAllByTestId(/event-tape-row-/);
    expect(rows.every((r) => r.getAttribute("data-category") === "node")).toBe(true);
    expect(rows).toHaveLength(1);
  });

  it("footer reports visible vs total counts and exposes clear-filters when active", () => {
    act(() => {
      swarmBus.__injectForTest(providerHealth());
      swarmBus.__injectForTest(nodeFailed("n0"));
    });
    render(<EventTape />);
    fireEvent.click(screen.getByTestId("filter-severity-only"));
    const footer = screen.getByTestId("event-tape-footer");
    expect(within(footer).getByText(/showing 1 of 2/i)).toBeInTheDocument();
    expect(screen.getByTestId("filter-clear")).toBeInTheDocument();
  });
});

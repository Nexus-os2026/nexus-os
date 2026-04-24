import { render, screen, act } from "@testing-library/react";
import { describe, it, expect, beforeEach } from "vitest";
import { mockCommands } from "../../../test/setup";
import { SwarmGrid } from "../SwarmGrid";
import { swarmBus } from "../../../lib/swarm/swarm_bus";
import { __resetSwarmStoreForTest } from "../../../lib/swarm/store";
import type { DagNode, SwarmEvent } from "../../../lib/swarm/types";

function stubNode(id: string, capability: string): DagNode {
  return {
    id,
    capability_id: capability,
    profile: {
      privacy: "Public",
      reasoning: "Medium",
      tool_use: "Basic",
      latency: "Batch",
      context: "Small",
      cost: "Low",
    },
    inputs: null,
    status: "Pending",
  };
}

function planProposed(nodes: DagNode[]): SwarmEvent {
  return { event: "plan_proposed", run_id: "run-1", dag_json: { nodes, edges: [] } };
}

function planApproved(): SwarmEvent {
  return { event: "plan_approved", run_id: "run-1" };
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

function nodeCompleted(nodeId: string): SwarmEvent {
  return {
    event: "node_completed",
    ref: { run_id: "run-1", node_id: nodeId },
    result: null,
    ticket_nonce: "00000000-0000-0000-0000-000000000000",
  };
}

beforeEach(() => {
  __resetSwarmStoreForTest();
  mockCommands({});
});

describe("SwarmGrid", () => {
  it("shows the empty state when there is no plan or run", () => {
    render(<SwarmGrid />);
    expect(screen.getByTestId("swarm-grid-empty")).toBeInTheDocument();
  });

  it("renders one AgentCard per Running node", () => {
    act(() => {
      swarmBus.__injectForTest(planProposed([stubNode("n0", "research"), stubNode("n1", "summarize")]));
      swarmBus.__injectForTest(planApproved());
      swarmBus.__injectForTest(nodeStarted("n0"));
      swarmBus.__injectForTest(nodeStarted("n1"));
    });
    render(<SwarmGrid />);
    expect(screen.getByTestId("agent-card-n0")).toBeInTheDocument();
    expect(screen.getByTestId("agent-card-n1")).toBeInTheDocument();
    expect(screen.queryByTestId("swarm-grid-empty")).not.toBeInTheDocument();
  });

  it("drops the AgentCard when its node transitions to Done", () => {
    act(() => {
      swarmBus.__injectForTest(planProposed([stubNode("n0", "research")]));
      swarmBus.__injectForTest(planApproved());
      swarmBus.__injectForTest(nodeStarted("n0"));
    });
    const { rerender } = render(<SwarmGrid />);
    expect(screen.getByTestId("agent-card-n0")).toBeInTheDocument();
    act(() => swarmBus.__injectForTest(nodeCompleted("n0")));
    rerender(<SwarmGrid />);
    expect(screen.queryByTestId("agent-card-n0")).not.toBeInTheDocument();
    expect(screen.getByTestId("swarm-grid-waiting")).toBeInTheDocument();
  });
});

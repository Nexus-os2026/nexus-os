import { render, screen, act, fireEvent } from "@testing-library/react";
import { describe, it, expect, beforeEach } from "vitest";
import { ReactFlowProvider } from "@xyflow/react";
import type { ReactNode } from "react";
import { DagViewer } from "../DagViewer";
import { AgentNode } from "../nodes/AgentNode";
import { swarmBus } from "../../../lib/swarm/swarm_bus";
import { __resetSwarmStoreForTest, getSwarmState } from "../../../lib/swarm/store";
import type { DagNode, SwarmEvent } from "../../../lib/swarm/types";

// AgentNode uses <Handle> which requires the xyflow zustand provider in
// the tree. Wrapper only needed when rendering the node in isolation.
function RFWrap({ children }: { children: ReactNode }): JSX.Element {
  return <ReactFlowProvider>{children}</ReactFlowProvider>;
}

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
  return {
    event: "plan_proposed",
    run_id: "run-1",
    dag_json: { nodes, edges: [] },
  };
}

function planApproved(runId: string): SwarmEvent {
  return { event: "plan_approved", run_id: runId };
}

function nodeStarted(runId: string, nodeId: string): SwarmEvent {
  return {
    event: "node_started",
    ref: { run_id: runId, node_id: nodeId },
    capability_id: "research",
    provider_id: "ollama",
    model_id: "gemma4:e2b",
    ticket_nonce: "00000000-0000-0000-0000-000000000000",
  };
}

function nodeCompleted(runId: string, nodeId: string): SwarmEvent {
  return {
    event: "node_completed",
    ref: { run_id: runId, node_id: nodeId },
    result: null,
    ticket_nonce: "00000000-0000-0000-0000-000000000000",
  };
}

beforeEach(() => {
  __resetSwarmStoreForTest();
});

describe("DagViewer", () => {
  it("shows the empty state when no plan or run is present", () => {
    render(<DagViewer />);
    expect(screen.getByTestId("dag-empty-state")).toBeInTheDocument();
    expect(screen.getByTestId("dag-empty-state").textContent).toMatch(/submit an intent/i);
  });

  it("hides the empty state once a plan is proposed", () => {
    render(<DagViewer />);
    act(() => {
      swarmBus.__injectForTest(planProposed([stubNode("n0", "research")]));
    });
    expect(screen.queryByTestId("dag-empty-state")).not.toBeInTheDocument();
  });

  it("AgentNode reflects live node status from the store (Pending → Running → Done)", () => {
    act(() => {
      swarmBus.__injectForTest(planProposed([stubNode("n0", "research"), stubNode("n1", "summarize")]));
      swarmBus.__injectForTest(planApproved("run-1"));
    });
    const dag = stubNode("n0", "research");
    render(
      <RFWrap>
        <AgentNode
          data={{ dag }}
          id={dag.id}
          type="agent"
          selected={false}
          zIndex={0}
          isConnectable
          dragging={false}
          selectable
          deletable
          draggable
        />
      </RFWrap>,
    );
    // After plan_approved the node_states start at Pending (from the DAG snapshot).
    expect(screen.getByTestId("agent-node-n0").getAttribute("data-status")).toBe("Pending");
    act(() => swarmBus.__injectForTest(nodeStarted("run-1", "n0")));
    expect(screen.getByTestId("agent-node-n0").getAttribute("data-status")).toBe("Running");
    act(() => swarmBus.__injectForTest(nodeCompleted("run-1", "n0")));
    expect(screen.getByTestId("agent-node-n0").getAttribute("data-status")).toBe("Done");
  });

  it("clicking an AgentNode dispatches selectFocusedNode with the node id", () => {
    const dag = stubNode("n42", "review");
    render(
      <RFWrap>
        <AgentNode
          data={{ dag }}
          id={dag.id}
          type="agent"
          selected={false}
          zIndex={0}
          isConnectable
          dragging={false}
          selectable
          deletable
          draggable
        />
      </RFWrap>,
    );
    fireEvent.click(screen.getByTestId("agent-node-n42"));
    expect(getSwarmState().focusedNodeId).toBe("n42");
  });
});

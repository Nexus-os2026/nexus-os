import { render, screen, act, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, beforeEach } from "vitest";
import { mockCommands, expectInvokedWith } from "../../../test/setup";
import { AgentCard } from "../AgentCard";
import { swarmBus } from "../../../lib/swarm/swarm_bus";
import { __resetSwarmStoreForTest, setInitialProviderHealth } from "../../../lib/swarm/store";
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

function planProposed(nodes: DagNode[], runId: string): SwarmEvent {
  return { event: "plan_proposed", run_id: runId, dag_json: { nodes, edges: [] } };
}

function planApproved(runId: string): SwarmEvent {
  return { event: "plan_approved", run_id: runId };
}

function nodeStarted(runId: string, nodeId: string, provider = "ollama", model = "gemma4:e2b"): SwarmEvent {
  return {
    event: "node_started",
    ref: { run_id: runId, node_id: nodeId },
    capability_id: "research",
    provider_id: provider,
    model_id: model,
    ticket_nonce: "00000000-0000-0000-0000-000000000000",
  };
}

function nodeEvent(runId: string, nodeId: string, phase: string, payload: unknown): SwarmEvent {
  return {
    event: "node_event",
    ref: { run_id: runId, node_id: nodeId },
    phase,
    payload,
    ticket_nonce: "00000000-0000-0000-0000-000000000000",
  };
}

const RUN = "11111111-1111-1111-1111-111111111111";

beforeEach(() => {
  __resetSwarmStoreForTest();
  mockCommands({ swarm_cancel_node: null });
});

describe("AgentCard", () => {
  it("renders with the node's capability label", () => {
    const node = stubNode("n0", "research");
    act(() => {
      swarmBus.__injectForTest(planProposed([node], RUN));
      swarmBus.__injectForTest(planApproved(RUN));
      swarmBus.__injectForTest(nodeStarted(RUN, "n0"));
    });
    render(<AgentCard node={node} />);
    const card = screen.getByTestId("agent-card-n0");
    expect(card.textContent).toContain("research");
    expect(card.textContent).toContain("n0");
  });

  it("updates phase display on NodeEvent dispatch", () => {
    const node = stubNode("n1", "summarize");
    act(() => {
      swarmBus.__injectForTest(planProposed([node], RUN));
      swarmBus.__injectForTest(planApproved(RUN));
      swarmBus.__injectForTest(nodeStarted(RUN, "n1"));
    });
    render(<AgentCard node={node} />);
    expect(screen.getByTestId("agent-card-phase-n1").textContent).toContain("…");
    act(() => {
      swarmBus.__injectForTest(nodeEvent(RUN, "n1", "plan", { tool: "web.search" }));
    });
    const phase = screen.getByTestId("agent-card-phase-n1");
    const summary = screen.getByTestId("agent-card-summary-n1");
    expect(phase.textContent).toContain("plan");
    expect(summary.textContent).toContain("tool: web.search");
  });

  it("cancel button calls swarm_cancel_node with runId + nodeId", async () => {
    const node = stubNode("n2", "review");
    act(() => {
      swarmBus.__injectForTest(planProposed([node], RUN));
      swarmBus.__injectForTest(planApproved(RUN));
      swarmBus.__injectForTest(nodeStarted(RUN, "n2"));
    });
    render(<AgentCard node={node} />);
    fireEvent.click(screen.getByTestId("agent-card-cancel-n2"));
    await waitFor(() =>
      expectInvokedWith("swarm_cancel_node", { runId: RUN, nodeId: "n2" }),
    );
  });

  it("SlmStatusBadge receives model + latency derived from node_started + provider health", () => {
    const node = stubNode("n3", "analyze");
    setInitialProviderHealth([
      {
        provider_id: "ollama",
        status: "Ok",
        latency_ms: 42,
        models: ["gemma4:e2b"],
        notes: "",
        checked_at_secs: 0,
      },
    ]);
    act(() => {
      swarmBus.__injectForTest(planProposed([node], RUN));
      swarmBus.__injectForTest(planApproved(RUN));
      swarmBus.__injectForTest(nodeStarted(RUN, "n3", "ollama", "gemma4:e2b"));
    });
    render(<AgentCard node={node} />);
    const badge = screen.getByTestId("agent-card-slm-n3");
    expect(badge.textContent).toContain("gemma4:e2b");
    expect(badge.textContent).toContain("42ms");
    expect(badge.textContent).toContain("LOCAL");
  });

  it("tokens field reflects per-node BudgetUpdate deltas", () => {
    const node = stubNode("n4", "research");
    act(() => {
      swarmBus.__injectForTest(planProposed([node], RUN));
      swarmBus.__injectForTest(planApproved(RUN));
      swarmBus.__injectForTest(nodeStarted(RUN, "n4"));
    });
    render(<AgentCard node={node} />);
    expect(screen.getByTestId("agent-card-tokens-n4").textContent).toContain("0");
    act(() => {
      swarmBus.__injectForTest({
        event: "budget_update",
        run_id: RUN,
        tokens_remaining: 0,
        cents_remaining: 0,
        wall_ms_remaining: 0,
        ticket_nonce: "00000000-0000-0000-0000-000000000000",
        node_id: "n4",
        node_tokens_consumed: 42,
        node_cost_cents_consumed: 1,
      });
    });
    expect(screen.getByTestId("agent-card-tokens-n4").textContent).toContain("42");
    // Per-node deltas accumulate.
    act(() => {
      swarmBus.__injectForTest({
        event: "budget_update",
        run_id: RUN,
        tokens_remaining: 0,
        cents_remaining: 0,
        wall_ms_remaining: 0,
        ticket_nonce: "00000000-0000-0000-0000-000000000000",
        node_id: "n4",
        node_tokens_consumed: 8,
        node_cost_cents_consumed: 0,
      });
    });
    expect(screen.getByTestId("agent-card-tokens-n4").textContent).toContain("50");
  });

  it("phase 'observe' payload updates the summary line", () => {
    const node = stubNode("n5", "summarize");
    act(() => {
      swarmBus.__injectForTest(planProposed([node], RUN));
      swarmBus.__injectForTest(planApproved(RUN));
      swarmBus.__injectForTest(nodeStarted(RUN, "n5"));
    });
    render(<AgentCard node={node} />);
    act(() => {
      swarmBus.__injectForTest({
        event: "node_event",
        ref: { run_id: RUN, node_id: "n5" },
        phase: "observe",
        payload: { response_digest: "summary of quantum paper", completion_tokens: 9 },
        ticket_nonce: "00000000-0000-0000-0000-000000000000",
      });
    });
    expect(screen.getByTestId("agent-card-phase-n5").textContent).toContain("observe");
    expect(screen.getByTestId("agent-card-summary-n5").textContent).toContain("response_digest");
  });

  it("run-scoped BudgetUpdate (node_id null) does not affect per-node tokens", () => {
    const node = stubNode("n6", "review");
    act(() => {
      swarmBus.__injectForTest(planProposed([node], RUN));
      swarmBus.__injectForTest(planApproved(RUN));
      swarmBus.__injectForTest(nodeStarted(RUN, "n6"));
    });
    render(<AgentCard node={node} />);
    act(() => {
      swarmBus.__injectForTest({
        event: "budget_update",
        run_id: RUN,
        tokens_remaining: 1000,
        cents_remaining: 50,
        wall_ms_remaining: 30000,
        ticket_nonce: "00000000-0000-0000-0000-000000000000",
        node_id: null,
      });
    });
    expect(screen.getByTestId("agent-card-tokens-n6").textContent).toContain("0");
  });
});

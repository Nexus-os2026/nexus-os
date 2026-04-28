import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import SwarmStatusBadge from "../SwarmStatusBadge";
import {
  __resetSwarmStoreForTest,
  type SwarmState,
} from "../../../lib/swarm/store";
import { swarmBus } from "../../../lib/swarm/swarm_bus";
import type { DagNode, RunState, SwarmEvent } from "../../../lib/swarm/types";

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

const RUN_ID = "11111111-2222-3333-4444-555555555555";

function planProposed(nodes: readonly string[]): SwarmEvent {
  return {
    event: "plan_proposed",
    run_id: RUN_ID,
    dag_json: {
      nodes: nodes.map(dagNode),
      edges: [],
    } as unknown as SwarmState["currentPlan"],
  };
}

function planApproved(): SwarmEvent {
  return { event: "plan_approved", run_id: RUN_ID };
}

function nodeStarted(nodeId: string): SwarmEvent {
  return {
    event: "node_started",
    ref: { run_id: RUN_ID, node_id: nodeId },
    capability_id: "cap",
    provider_id: "anthropic",
    model_id: "claude-haiku",
    ticket_nonce: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
  } as unknown as SwarmEvent;
}

function nodeCompleted(nodeId: string): SwarmEvent {
  return {
    event: "node_completed",
    ref: { run_id: RUN_ID, node_id: nodeId },
    ticket_nonce: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
    summary: { Done: "ok" },
  } as unknown as SwarmEvent;
}

function swarmCompleted(): SwarmEvent {
  return { event: "swarm_completed", run_id: RUN_ID };
}

function swarmCancelled(): SwarmEvent {
  return { event: "swarm_cancelled", run_id: RUN_ID };
}

function setReducedMotion(reduce: boolean): void {
  // jsdom doesn't ship matchMedia; define it directly. Restored in
  // afterEach by clearing the property.
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    writable: true,
    value: (query: string): MediaQueryList =>
      ({
        matches: query.includes("prefers-reduced-motion: reduce") ? reduce : false,
        media: query,
        onchange: null,
        addEventListener: () => {},
        removeEventListener: () => {},
        addListener: () => {},
        removeListener: () => {},
        dispatchEvent: () => false,
      }) as unknown as MediaQueryList,
  });
}

beforeEach(() => {
  __resetSwarmStoreForTest();
  setReducedMotion(false);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("SwarmStatusBadge", () => {
  it("renders nothing when idle (no active run, no pending plan)", () => {
    render(<SwarmStatusBadge onNavigate={() => {}} currentPage="dashboard" />);
    expect(screen.queryByTestId("swarm-status-badge")).not.toBeInTheDocument();
  });

  it("renders nothing when currentPage === 'agents', even if a plan is pending", () => {
    act(() => {
      swarmBus.__injectForTest(planProposed(["n1"]));
    });
    render(<SwarmStatusBadge onNavigate={() => {}} currentPage="agents" />);
    expect(screen.queryByTestId("swarm-status-badge")).not.toBeInTheDocument();
  });

  it("renders 'AWAITING APPROVAL' when a plan is pending", () => {
    act(() => {
      swarmBus.__injectForTest(planProposed(["n1", "n2"]));
    });
    render(<SwarmStatusBadge onNavigate={() => {}} currentPage="dashboard" />);
    const badge = screen.getByTestId("swarm-status-badge");
    expect(badge).toBeInTheDocument();
    expect(badge.getAttribute("data-state")).toBe("pending");
    expect(screen.getByTestId("swarm-status-badge-label")).toHaveTextContent(
      "AWAITING APPROVAL",
    );
  });

  it("renders 'RUNNING · X/Y' when activeRun is set", () => {
    act(() => {
      swarmBus.__injectForTest(planProposed(["n1", "n2", "n3"]));
      swarmBus.__injectForTest(planApproved());
      swarmBus.__injectForTest(nodeStarted("n1"));
      swarmBus.__injectForTest(nodeCompleted("n1"));
    });
    render(<SwarmStatusBadge onNavigate={() => {}} currentPage="dashboard" />);
    const badge = screen.getByTestId("swarm-status-badge");
    expect(badge.getAttribute("data-state")).toBe("running");
    expect(screen.getByTestId("swarm-status-badge-label")).toHaveTextContent(
      "RUNNING · 1/3",
    );
  });

  it("invokes onNavigate('agents') on click", () => {
    act(() => {
      swarmBus.__injectForTest(planProposed(["n1"]));
    });
    const onNavigate = vi.fn();
    render(<SwarmStatusBadge onNavigate={onNavigate} currentPage="dashboard" />);
    fireEvent.click(screen.getByTestId("swarm-status-badge"));
    expect(onNavigate).toHaveBeenCalledWith("agents");
  });

  it("holds 'COMPLETED ✓' for 5s after swarm_completed, then clears", async () => {
    vi.useFakeTimers();
    act(() => {
      swarmBus.__injectForTest(planProposed(["n1"]));
      swarmBus.__injectForTest(planApproved());
    });
    render(<SwarmStatusBadge onNavigate={() => {}} currentPage="dashboard" />);
    expect(screen.getByTestId("swarm-status-badge").getAttribute("data-state")).toBe(
      "running",
    );
    act(() => {
      swarmBus.__injectForTest(swarmCompleted());
    });
    const badge = screen.getByTestId("swarm-status-badge");
    expect(badge.getAttribute("data-state")).toBe("completed");
    expect(screen.getByTestId("swarm-status-badge-label")).toHaveTextContent(
      "COMPLETED ✓",
    );
    act(() => {
      vi.advanceTimersByTime(5_000);
    });
    expect(screen.queryByTestId("swarm-status-badge")).not.toBeInTheDocument();
  });

  it("holds 'CANCELLED' for 5s after swarm_cancelled, then clears", async () => {
    vi.useFakeTimers();
    act(() => {
      swarmBus.__injectForTest(planProposed(["n1"]));
      swarmBus.__injectForTest(planApproved());
    });
    render(<SwarmStatusBadge onNavigate={() => {}} currentPage="dashboard" />);
    act(() => {
      swarmBus.__injectForTest(swarmCancelled());
    });
    const badge = screen.getByTestId("swarm-status-badge");
    expect(badge.getAttribute("data-state")).toBe("cancelled");
    expect(screen.getByTestId("swarm-status-badge-label")).toHaveTextContent(
      "CANCELLED",
    );
    act(() => {
      vi.advanceTimersByTime(5_000);
    });
    expect(screen.queryByTestId("swarm-status-badge")).not.toBeInTheDocument();
  });

  it("does not apply pulse animation when prefers-reduced-motion is set", () => {
    setReducedMotion(true);
    act(() => {
      swarmBus.__injectForTest(planProposed(["n1"]));
      swarmBus.__injectForTest(planApproved());
    });
    render(<SwarmStatusBadge onNavigate={() => {}} currentPage="dashboard" />);
    const badge = screen.getByTestId("swarm-status-badge");
    // Inline animation style should be absent (undefined → no animation
    // CSS property serialized).
    expect(badge.style.animation).toBe("");
  });

  it("applies pulse animation when reduced-motion is NOT set", () => {
    setReducedMotion(false);
    act(() => {
      swarmBus.__injectForTest(planProposed(["n1"]));
      swarmBus.__injectForTest(planApproved());
    });
    render(<SwarmStatusBadge onNavigate={() => {}} currentPage="dashboard" />);
    const badge = screen.getByTestId("swarm-status-badge");
    expect(badge.style.animation).toContain("swarm-node-pulse");
  });
});

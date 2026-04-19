import { render, screen, waitFor, act } from "@testing-library/react";
import { describe, it, expect, beforeEach } from "vitest";
import { mockCommands, expectInvoked } from "../../test/setup";
import { Agents } from "../Agents";
import { swarmBus } from "../../lib/swarm/swarm_bus";
import { __resetSwarmStoreForTest } from "../../lib/swarm/store";
import type { SwarmEvent } from "../../lib/swarm/types";

function providerHealthEvent(): SwarmEvent {
  return {
    event: "provider_health_update",
    providers: [
      {
        provider_id: "ollama",
        status: "Ok",
        latency_ms: 14,
        models: ["gemma4:e2b"],
        notes: "",
        checked_at_secs: 0,
      },
      {
        provider_id: "anthropic",
        status: "Unhealthy",
        latency_ms: null,
        models: [],
        notes: "api_key not in keyring",
        checked_at_secs: 0,
      },
    ],
  };
}

function planProposedEvent(): SwarmEvent {
  return {
    event: "plan_proposed",
    run_id: "00000000-0000-0000-0000-000000000001",
    dag_json: { nodes: [], edges: [] },
  };
}

function planApprovedEvent(runId: string): SwarmEvent {
  return { event: "plan_approved", run_id: runId };
}

function nodeStartedEvent(runId: string): SwarmEvent {
  return {
    event: "node_started",
    ref: { run_id: runId, node_id: "n0" },
    capability_id: "research",
    provider_id: "ollama",
    model_id: "gemma4:e2b",
    ticket_nonce: "00000000-0000-0000-0000-000000000002",
  };
}

beforeEach(() => {
  // Reset store state between tests. The bus subscription (installed by
  // store.ts at module load) stays intact so the dispatcher keeps working.
  __resetSwarmStoreForTest();
  mockCommands({ swarm_provider_health: [] });
});

describe("Agents shell", () => {
  it("mounts the four placeholder regions without throwing", () => {
    expect(() => render(<Agents />)).not.toThrow();
    expect(screen.getByTestId("region-dag")).toBeInTheDocument();
    expect(screen.getByTestId("region-swarm")).toBeInTheDocument();
    expect(screen.getByTestId("region-events")).toBeInTheDocument();
    expect(screen.getByTestId("region-director")).toBeInTheDocument();
  });

  it("renders provider dots when a ProviderHealthUpdate event arrives", async () => {
    render(<Agents />);
    act(() => {
      swarmBus.__injectForTest(providerHealthEvent());
    });
    await waitFor(() => {
      expect(screen.getByTestId("provider-dot-ollama")).toBeInTheDocument();
      expect(screen.getByTestId("provider-dot-anthropic")).toBeInTheDocument();
    });
  });

  it("clicking a provider dot triggers swarm_refresh_provider_health", async () => {
    render(<Agents />);
    act(() => {
      swarmBus.__injectForTest(providerHealthEvent());
    });
    const dot = await screen.findByTestId("provider-dot-ollama");
    dot.click();
    await waitFor(() => expectInvoked("swarm_refresh_provider_health"));
  });

  it("shows the active run id in the footer once a run starts", async () => {
    render(<Agents />);
    // The run must be proposed → approved → node_started for the footer
    // to reflect it; the store treats plan_approved as the "run begins"
    // signal.
    const runId = "11111111-1111-1111-1111-111111111111";
    act(() => {
      swarmBus.__injectForTest({
        event: "plan_proposed",
        run_id: runId,
        dag_json: { nodes: [], edges: [] },
      });
    });
    act(() => {
      swarmBus.__injectForTest(planApprovedEvent(runId));
    });
    act(() => {
      swarmBus.__injectForTest(nodeStartedEvent(runId));
    });
    await waitFor(() => {
      expect(screen.getByTestId("run-id")).toHaveTextContent(runId);
    });
  });

  it("footer shows 'no active run' before any plan is approved", () => {
    render(<Agents />);
    act(() => {
      swarmBus.__injectForTest(planProposedEvent());
    });
    expect(screen.getByTestId("run-footer")).toHaveTextContent(/no active run/i);
  });
});

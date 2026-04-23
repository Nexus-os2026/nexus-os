import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { mockCommands, expectInvokedWith } from "../../../test/setup";
import { DirectorConsole } from "../DirectorConsole";
import { swarmBus } from "../../../lib/swarm/swarm_bus";
import { __resetSwarmStoreForTest } from "../../../lib/swarm/store";
import type { PlannedSwarmJson, SwarmEvent } from "../../../lib/swarm/types";

function plannedResponse(): PlannedSwarmJson {
  return {
    dag: { nodes: [], edges: [] },
    ticket_id: "11111111-2222-3333-4444-555555555555",
    budget_hash: "deadbeef",
    privacy_envelope: "Public",
  };
}

function planProposedEvent(): SwarmEvent {
  return {
    event: "plan_proposed",
    run_id: "11111111-2222-3333-4444-555555555555",
    dag_json: { nodes: [], edges: [] },
  };
}

beforeEach(() => {
  __resetSwarmStoreForTest();
  mockCommands({
    swarm_plan: plannedResponse(),
    swarm_reject: null,
  });
});

describe("DirectorConsole", () => {
  it("renders textarea and submit button", () => {
    render(
      <DirectorConsole
        onPlanReady={(): void => {}}
        pendingTicketId={null}
        onPlanCleared={(): void => {}}
      />,
    );
    expect(screen.getByTestId("director-textarea")).toBeInTheDocument();
    expect(screen.getByTestId("director-submit")).toBeInTheDocument();
  });

  it("submit calls swarm_plan with the entered intent and hands the response upward", async () => {
    const onPlanReady = vi.fn();
    render(
      <DirectorConsole
        onPlanReady={onPlanReady}
        pendingTicketId={null}
        onPlanCleared={(): void => {}}
      />,
    );
    const ta = screen.getByTestId("director-textarea") as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: "research top 3 papers" } });
    fireEvent.click(screen.getByTestId("director-submit"));
    await waitFor(() => {
      expectInvokedWith("swarm_plan", { intent: "research top 3 papers" });
    });
    expect(onPlanReady).toHaveBeenCalledTimes(1);
    expect(onPlanReady.mock.calls[0][0]).toMatchObject({
      ticket_id: "11111111-2222-3333-4444-555555555555",
      privacy_envelope: "Public",
    });
  });

  it("submit button is disabled while a plan is pending in the store", async () => {
    render(
      <DirectorConsole
        onPlanReady={(): void => {}}
        pendingTicketId="11111111-2222-3333-4444-555555555555"
        onPlanCleared={(): void => {}}
      />,
    );
    const ta = screen.getByTestId("director-textarea") as HTMLTextAreaElement;
    // Flip store into pending by dispatching plan_proposed through the bus.
    act(() => {
      swarmBus.__injectForTest(planProposedEvent());
    });
    fireEvent.change(ta, { target: { value: "anything" } });
    const submit = screen.getByTestId("director-submit") as HTMLButtonElement;
    expect(submit.disabled).toBe(true);
    expect(ta.disabled).toBe(true);
  });

  it("Escape dispatches swarm_reject with the pending ticket_id and clears parent state", async () => {
    const onPlanCleared = vi.fn();
    render(
      <DirectorConsole
        onPlanReady={(): void => {}}
        pendingTicketId="aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
        onPlanCleared={onPlanCleared}
      />,
    );
    const ta = screen.getByTestId("director-textarea");
    fireEvent.keyDown(ta, { key: "Escape" });
    await waitFor(() => {
      expectInvokedWith("swarm_reject", { ticketId: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee" });
    });
    await waitFor(() => expect(onPlanCleared).toHaveBeenCalledTimes(1));
  });
});

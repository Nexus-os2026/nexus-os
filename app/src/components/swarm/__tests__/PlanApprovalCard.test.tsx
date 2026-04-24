import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { mockCommands, expectInvokedWith } from "../../../test/setup";
import { PlanApprovalCard } from "../PlanApprovalCard";
import type { PlannedSwarmJson, PrivacyClass } from "../../../lib/swarm/types";

function mkPlan(privacy: PrivacyClass): PlannedSwarmJson {
  return {
    dag: {
      nodes: [
        { id: "n0", capability_id: "research", profile: stubProfile(), inputs: null, status: "Pending" },
        { id: "n1", capability_id: "summarize", profile: stubProfile(), inputs: null, status: "Pending" },
      ],
      edges: [{ from: "n0", to: "n1" }],
    },
    ticket_id: "abcd1234-0000-0000-0000-000000000000",
    budget_hash: "cafe1234",
    privacy_envelope: privacy,
  };
}

function stubProfile(): PlannedSwarmJson["dag"]["nodes"][number]["profile"] {
  return {
    privacy: "Public",
    reasoning: "Medium",
    tool_use: "Basic",
    latency: "Batch",
    context: "Small",
    cost: "Low",
  };
}

beforeEach(() => {
  mockCommands({ swarm_approve: "22222222-2222-2222-2222-222222222222", swarm_reject: null });
});

describe("PlanApprovalCard", () => {
  it("renders privacy envelope badge colored per privacy class", () => {
    for (const p of ["Public", "Sensitive", "StrictLocal"] as PrivacyClass[]) {
      const { unmount } = render(<PlanApprovalCard plan={mkPlan(p)} onDecisionSent={(): void => {}} />);
      const badge = screen.getByTestId("plan-privacy-badge");
      expect(badge.getAttribute("data-privacy")).toBe(p);
      expect(badge.textContent).toBe(p);
      unmount();
    }
  });

  it("Approve button calls swarm_approve with the correct ticket_id", async () => {
    const onDecisionSent = vi.fn();
    render(<PlanApprovalCard plan={mkPlan("Public")} onDecisionSent={onDecisionSent} />);
    fireEvent.click(screen.getByTestId("plan-approve"));
    await waitFor(() => {
      expectInvokedWith("swarm_approve", { ticketId: "abcd1234-0000-0000-0000-000000000000" });
    });
    await waitFor(() => expect(onDecisionSent).toHaveBeenCalledTimes(1));
  });

  it("Reject button calls swarm_reject with the correct ticket_id", async () => {
    const onDecisionSent = vi.fn();
    render(<PlanApprovalCard plan={mkPlan("Sensitive")} onDecisionSent={onDecisionSent} />);
    fireEvent.click(screen.getByTestId("plan-reject"));
    await waitFor(() => {
      expectInvokedWith("swarm_reject", { ticketId: "abcd1234-0000-0000-0000-000000000000" });
    });
    await waitFor(() => expect(onDecisionSent).toHaveBeenCalledTimes(1));
  });

  it("shows node and edge counts from the plan", () => {
    render(<PlanApprovalCard plan={mkPlan("Public")} onDecisionSent={(): void => {}} />);
    const card = screen.getByTestId("plan-approval-card");
    expect(card.textContent).toContain("2");
    expect(card.textContent).toContain("1");
  });
});

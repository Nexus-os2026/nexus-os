import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import SelfImprovement from "../SelfImprovement";

const MOCKS: Record<string, unknown> = {
  self_improve_get_status: {
    pipeline_state: "idle",
    signals_count: 0,
    opportunities_count: 0,
    pending_proposals: 0,
    monitoring_count: 0,
    committed_count: 0,
    rolled_back_count: 0,
    rejected_count: 0,
    fuel_budget: 5000,
    enabled_domains: ["PromptOptimization", "ConfigTuning"],
  },
  self_improve_get_signals: [],
  self_improve_get_opportunities: [],
  self_improve_get_proposals: [],
  self_improve_get_history: [],
  self_improve_get_invariants: [
    { id: 1, name: "#1 Governance kernel immutable", status: "passing" },
    { id: 2, name: "#2 Audit trail integrity", status: "passing" },
    { id: 3, name: "#3 HITL gates cannot weaken", status: "passing" },
    { id: 4, name: "#4 Capabilities cannot expand", status: "passing" },
    { id: 5, name: "#5 Fuel limits enforced", status: "passing" },
    { id: 6, name: "#6 Crypto identity immutable", status: "passing" },
    { id: 7, name: "#7 All changes reversible", status: "passing" },
    { id: 8, name: "#8 Test suite green", status: "passing" },
    { id: 9, name: "#9 HITL approval required", status: "passing" },
    { id: 10, name: "#10 Self-improvement pipeline protected", status: "passing" },
  ],
  self_improve_get_config: {
    sigma_threshold: 2.0,
    canary_duration_minutes: 30,
    fuel_budget: 5000,
    enabled_domains: ["PromptOptimization", "ConfigTuning"],
    max_proposals_per_cycle: 1,
  },
  self_improve_get_guardian_status: {
    has_baseline: true,
    baseline_hash: "abc123def456",
    baseline_created_at: 1000,
    switch_threshold: 0.8,
    current_drift: 0.01,
    drift_bound: 1.0,
    headroom: 0.79,
    decision: "continue_active",
  },
};

describe("SelfImprovement", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<SelfImprovement />);
    await waitFor(() =>
      expect(document.body.textContent?.length).toBeGreaterThan(0),
    );
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<SelfImprovement />);
    await waitFor(() => expectInvoked("self_improve_get_status"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("self_improve_get_status", "connection refused");
    const { container } = render(<SelfImprovement />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

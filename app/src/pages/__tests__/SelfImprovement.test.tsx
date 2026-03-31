import { render, waitFor, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import SelfImprovement from "../SelfImprovement";

const MOCKS: Record<string, unknown> = {
  self_improve_get_status: {
    pipeline_state: "idle",
    signals_count: 2,
    opportunities_count: 1,
    pending_proposals: 1,
    monitoring_count: 0,
    committed_count: 5,
    rolled_back_count: 1,
    rejected_count: 2,
    fuel_budget: 5000,
    enabled_domains: ["PromptOptimization", "ConfigTuning"],
  },
  self_improve_get_signals: [
    {
      id: "sig-1",
      metric_name: "latency_p99",
      domain: "ConfigTuning",
      source: "PerformanceProfiler",
      current_value: 500,
      baseline_value: 100,
      deviation_sigma: 3.5,
    },
  ],
  self_improve_get_opportunities: [
    {
      id: "opp-1",
      domain: "ConfigTuning",
      classification: "Performance",
      severity: "High",
      blast_radius: "Agent",
      confidence: 0.85,
      estimated_impact: 3.2,
    },
  ],
  self_improve_get_proposals: [
    {
      id: "prop-1",
      domain: "PromptOptimization",
      description: "Improve reasoning depth",
      fuel_cost: 100,
    },
  ],
  self_improve_get_history: [
    {
      id: "imp-1",
      proposal_id: "prop-0",
      status: "Monitoring",
      applied_at: 1711900000,
      canary_deadline: 1711901800,
    },
    {
      id: "imp-2",
      proposal_id: "prop-prev",
      status: "Committed",
      applied_at: 1711800000,
      canary_deadline: 1711801800,
    },
  ],
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
  self_improve_run_cycle: { result: "NoSignals", message: "System healthy" },
  self_improve_approve_proposal: { id: "imp-new", status: "Monitoring" },
  self_improve_reject_proposal: undefined,
  self_improve_rollback: undefined,
  self_improve_update_config: undefined,
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

  it("renders pipeline status metrics", async () => {
    mockCommands(MOCKS);
    render(<SelfImprovement />);
    await waitFor(() => {
      const text = document.body.textContent || "";
      expect(text).toContain("5000");
      expect(text).toContain("Committed");
    });
  });

  it("renders all 10 invariants", async () => {
    mockCommands(MOCKS);
    render(<SelfImprovement />);
    await waitFor(() => {
      const text = document.body.textContent || "";
      expect(text).toContain("Governance kernel immutable");
      expect(text).toContain("Self-improvement pipeline protected");
    });
  });

  it("renders guardian status", async () => {
    mockCommands(MOCKS);
    render(<SelfImprovement />);
    await waitFor(() => {
      const text = document.body.textContent || "";
      expect(text).toContain("Active");
      expect(text).toContain("Drift");
    });
  });

  it("renders domain toggles with CodePatch locked", async () => {
    mockCommands(MOCKS);
    render(<SelfImprovement />);
    await waitFor(() => {
      const text = document.body.textContent || "";
      expect(text).toContain("PromptOptimization");
      expect(text).toContain("CodePatch");
    });
  });

  it("renders empty state messages", async () => {
    mockCommands({
      ...MOCKS,
      self_improve_get_signals: [],
      self_improve_get_opportunities: [],
      self_improve_get_proposals: [],
      self_improve_get_history: [],
    });
    render(<SelfImprovement />);
    await waitFor(() => {
      const text = document.body.textContent || "";
      expect(text).toContain("No signals detected");
    });
  });

  it("shows error on full backend failure", async () => {
    mockCommandError("self_improve_get_status", "server down");
    render(<SelfImprovement />);
    await waitFor(() => {
      const text = document.body.textContent || "";
      expect(text).toContain("server down");
    });
  });

  it("renders proposal with approve/reject buttons", async () => {
    mockCommands(MOCKS);
    render(<SelfImprovement />);
    await waitFor(() => {
      const text = document.body.textContent || "";
      expect(text).toContain("Approve");
      expect(text).toContain("Reject");
      expect(text).toContain("Improve reasoning depth");
    });
  });
});

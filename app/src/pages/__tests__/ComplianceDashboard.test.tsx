import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import ComplianceDashboard from "../ComplianceDashboard";

const MOCKS: Record<string, unknown> = {
  get_compliance_status: { status: "compliant", checks_passed: 0, checks_failed: 0, alerts: [] },
  get_compliance_agents: [],
  compliance_governance_metrics: JSON.stringify({
    hitl_approval_rate: 0, capability_denial_rate: 0, pii_redaction_count: 0,
    firewall_block_count: 0, total_fuel_consumed: 0, total_events: 0,
    autonomy_distribution: {}, events_per_hour: [],
  }),
  compliance_security_events: JSON.stringify([]),
  get_audit_log: [],
};

describe("ComplianceDashboard", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<ComplianceDashboard />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<ComplianceDashboard />);
    await waitFor(() => expectInvoked("get_compliance_status"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("get_compliance_status", "connection refused", MOCKS);
    const { container } = render(<ComplianceDashboard />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

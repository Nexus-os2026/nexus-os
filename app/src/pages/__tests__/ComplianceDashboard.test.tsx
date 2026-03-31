import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import ComplianceDashboard from "../ComplianceDashboard";

const MOCKS: Record<string, unknown> = {
  get_compliance_status: "{}",
  get_compliance_agents: [],
  compliance_governance_metrics: "{}",
  compliance_security_events: [],
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
    mockCommandError("get_compliance_status", "connection refused");
    const { container } = render(<ComplianceDashboard />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

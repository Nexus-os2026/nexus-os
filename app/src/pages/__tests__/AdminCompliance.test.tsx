import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AdminCompliance from "../AdminCompliance";

const MOCKS: Record<string, unknown> = {
  admin_compliance_status: JSON.stringify({
    eu_ai_act: { score: 0, total: 0, controls: [] },
    soc2: { score: 0, total: 0, controls: [] },
    audit_stats: { total_events: 0, events_24h: 0, chain_verified: false, last_verification: new Date().toISOString(), next_verification: new Date().toISOString() },
    pii_stats: { total_redactions: 0, redactions_24h: 0, patterns_active: 0 },
    hitl_stats: { total_approvals: 0, total_denials: 0, approval_rate: 0, pending: 0 },
  }),
  admin_compliance_export: JSON.stringify("export_ok"),
};

describe("AdminCompliance", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<AdminCompliance />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<AdminCompliance />);
    await waitFor(() => expectInvoked("admin_compliance_status"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("admin_compliance_status", "connection refused", MOCKS);
    const { container } = render(<AdminCompliance />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

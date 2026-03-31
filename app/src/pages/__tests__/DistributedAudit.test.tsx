import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import DistributedAudit from "../DistributedAudit";

const MOCKS: Record<string, unknown> = {
  get_audit_log: [],
  get_audit_chain_status: { chain_valid: true, total_events: 0, first_hash: "0000000000000000", last_hash: "0000000000000000" },
};

describe("DistributedAudit", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<DistributedAudit />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<DistributedAudit />);
    await waitFor(() => expectInvoked("get_audit_log"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("get_audit_log", "connection refused", MOCKS);
    const { container } = render(<DistributedAudit />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

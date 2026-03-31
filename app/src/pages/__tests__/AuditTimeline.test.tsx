import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AuditTimeline from "../AuditTimeline";

const MOCKS: Record<string, unknown> = {
  get_audit_log: [],
  get_audit_chain_status: '{"chain_valid":true,"total_events":0}',
};

describe("AuditTimeline", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<AuditTimeline events={[]} />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<AuditTimeline events={[]} />);
    await waitFor(() => expectInvoked("get_audit_log"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("get_audit_log", "connection refused");
    render(<AuditTimeline events={[]} />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
});

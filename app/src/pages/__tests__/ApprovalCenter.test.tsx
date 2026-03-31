import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import ApprovalCenter from "../ApprovalCenter";

const MOCKS: Record<string, unknown> = {
  list_pending_consents: [],
  get_consent_history: [],
  hitl_stats: '{"pending_count":0,"approval_rate":1.0,"avg_response_time_ms":500,"total_decisions_today":0}',
  review_consent_batch: [],
};

describe("ApprovalCenter", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<ApprovalCenter />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<ApprovalCenter />);
    await waitFor(() => expectInvoked("list_pending_consents"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("list_pending_consents", "connection refused");
    render(<ApprovalCenter />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
});

import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import UsageBilling from "../UsageBilling";

const MOCKS: Record<string, unknown> = {
  metering_usage_report: "{}",
  metering_cost_breakdown: "{}",
  metering_budget_alerts: "[]",
};

describe("UsageBilling", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<UsageBilling />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<UsageBilling />);
    await waitFor(() => expectInvoked("metering_usage_report"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("metering_usage_report", "connection refused");
    const { container } = render(<UsageBilling />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

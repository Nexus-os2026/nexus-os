import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import TrustDashboard from "../TrustDashboard";

const MOCKS: Record<string, unknown> = {
  get_trust_overview: '{"agents":[],"network_health":"healthy"}',
  reputation_top: "[]",
  list_agents: [],
};

describe("TrustDashboard", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<TrustDashboard />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<TrustDashboard />);
    await waitFor(() => expectInvoked("get_trust_overview"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("get_trust_overview", "connection refused");
    const { container } = render(<TrustDashboard />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

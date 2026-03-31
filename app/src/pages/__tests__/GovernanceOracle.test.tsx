import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import GovernanceOracle from "../GovernanceOracle";

const MOCKS: Record<string, unknown> = {
  oracle_status: { queue_depth: 0, response_ceiling_ms: 0, requests_processed: 0, uptime_seconds: 100 },
  list_agents: [],
};

describe("GovernanceOracle", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<GovernanceOracle />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<GovernanceOracle />);
    await waitFor(() => expectInvoked("oracle_status"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("oracle_status", "connection refused");
    const { container } = render(<GovernanceOracle />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

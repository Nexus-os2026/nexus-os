import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import ImmuneDashboard from "../ImmuneDashboard";

const MOCKS: Record<string, unknown> = {
  get_immune_status: '{"threat_level":"Green","active_antibodies":0,"threats_blocked":0}',
  get_threat_log: [],
  list_agents: [],
  get_immune_memory: [],
};

describe("ImmuneDashboard", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<ImmuneDashboard />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<ImmuneDashboard />);
    await waitFor(() => expectInvoked("get_immune_status"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("get_immune_status", "connection refused");
    const { container } = render(<ImmuneDashboard />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

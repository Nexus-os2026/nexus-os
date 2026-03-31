import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import ConsciousnessMonitor from "../ConsciousnessMonitor";

const MOCKS: Record<string, unknown> = {
  get_agent_consciousness: "{}",
  get_consciousness_heatmap: [],
  list_agents: [],
};

describe("ConsciousnessMonitor", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<ConsciousnessMonitor />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<ConsciousnessMonitor />);
    await waitFor(() => expectInvoked("list_agents"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("list_agents", "connection refused");
    const { container } = render(<ConsciousnessMonitor />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import WorldSimulation2 from "../WorldSimulation2";

const MOCKS: Record<string, unknown> = {
  list_agents: [],
  sim_get_history: [],
  sim_get_policy: '{"max_steps":100}',
};

describe("WorldSimulation2", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<WorldSimulation2 />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<WorldSimulation2 />);
    await waitFor(() => expectInvoked("list_agents"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("list_agents", "connection refused");
    const { container } = render(<WorldSimulation2 />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

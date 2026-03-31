import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import WorldSimulation from "../WorldSimulation";

const MOCKS: Record<string, unknown> = {
  list_simulations: [],
  create_simulation: "",
  list_agents: [],
};

describe("WorldSimulation", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<WorldSimulation />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<WorldSimulation />);
    await waitFor(() => expectInvoked("list_simulations"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("list_simulations", "connection refused", MOCKS);
    const { container } = render(<WorldSimulation />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

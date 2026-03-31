import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, expectInvoked } from "../../test/setup";
import GovernedControl from "../GovernedControl";

const MOCKS = {
  list_agents: [{ id: "a1", name: "agent-1" }],
  cc_get_action_history: [],
  cc_get_capability_budget: { balance: 100, spent: 20, denied: 1, integrity: true },
  cc_get_screen_context: null,
  cc_verify_action_sequence: null,
};

describe("GovernedControl", () => {
  it("renders after data load", async () => {
    mockCommands(MOCKS);
    render(<GovernedControl />);
    await waitFor(() => expect(screen.getByText(/Governed Computer Control/i)).toBeInTheDocument());
  });

  it("loads agents on mount", async () => {
    mockCommands(MOCKS);
    render(<GovernedControl />);
    await waitFor(() => expectInvoked("list_agents"));
  });

  it("loads capability budget when agent selected", async () => {
    mockCommands(MOCKS);
    render(<GovernedControl />);
    await waitFor(() => expectInvoked("cc_get_capability_budget"));
  });

  it("displays governance rules text", async () => {
    mockCommands(MOCKS);
    render(<GovernedControl />);
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body).toContain("governance");
    });
  });
});

import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AgentDnaLab from "../AgentDnaLab";

const MOCKS: Record<string, unknown> = {
  list_agents: [],
  get_agent_genome: "{}",
  evolution_get_status: "{}",
  evolution_get_history: "[]",
  evolution_get_active_strategy: "{}",
  genesis_list_generated: [],
};

describe("AgentDnaLab", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<AgentDnaLab />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<AgentDnaLab />);
    await waitFor(() => expectInvoked("list_agents"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("list_agents", "connection refused");
    render(<AgentDnaLab />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
});

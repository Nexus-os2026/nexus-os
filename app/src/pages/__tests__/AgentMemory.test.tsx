import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AgentMemory from "../AgentMemory";

const MOCKS: Record<string, unknown> = {
  list_agents: [],
  memory_get_policy: {
    min_autonomy_level: 2,
    max_memories_per_agent: 500,
    store_cost: 1500000,
    query_cost: 500000,
  },
};

describe("AgentMemory", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<AgentMemory />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<AgentMemory />);
    await waitFor(() => expectInvoked("list_agents"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("list_agents", "connection refused");
    render(<AgentMemory />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
});

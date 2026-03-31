import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Memory from "../Memory";

const MOCKS: Record<string, unknown> = {
  list_agents: [{ id: "a1", name: "agent-1", status: "running" }],
  mk_get_stats: '{"working_count":0,"episodic_count":0,"semantic_count":0,"procedural_count":0}',
  mk_query: [],
  mk_search: [],
  mk_get_procedures: [],
  mk_get_candidates: [],
  mk_list_checkpoints: [],
};

describe("Memory", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Memory />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<Memory />);
    await waitFor(() => expectInvoked("list_agents"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("list_agents", "connection refused");
    render(<Memory />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
});

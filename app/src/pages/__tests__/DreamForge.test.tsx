import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import DreamForge from "../DreamForge";

const MOCKS: Record<string, unknown> = {
  get_dream_status: JSON.stringify({
    active_dreams: 0, completed_today: 0, next_scheduled: "", budget_remaining: 0, budget_total: 0, enabled: false,
  }),
  get_dream_queue: "[]",
  get_dream_history: "[]",
  get_morning_briefing: JSON.stringify({
    summary: "No dreams", improvements: [], agents_created: [], presolved_count: 0,
  }),
  check_llm_status: { providers: [] },
};

describe("DreamForge", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<DreamForge />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<DreamForge />);
    await waitFor(() => expectInvoked("get_dream_status"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("get_dream_status", "connection refused", MOCKS);
    const { container } = render(<DreamForge />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

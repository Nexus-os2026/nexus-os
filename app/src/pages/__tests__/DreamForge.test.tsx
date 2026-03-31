import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import DreamForge from "../DreamForge";

const MOCKS: Record<string, unknown> = {
  get_dream_status: "{}",
  get_dream_queue: [],
  get_dream_history: [],
  get_morning_briefing: '{"briefing":"No dreams"}',
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
    mockCommandError("get_dream_status", "connection refused");
    const { container } = render(<DreamForge />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

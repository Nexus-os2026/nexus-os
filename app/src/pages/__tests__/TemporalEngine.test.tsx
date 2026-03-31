import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import TemporalEngine from "../TemporalEngine";

const MOCKS = {
  get_temporal_history: { forks: [], decisions: [] },
  list_agents: [],
};

describe("TemporalEngine", () => {
  it("renders heading after load", async () => {
    mockCommands(MOCKS);
    render(<TemporalEngine />);
    await waitFor(() => expect(screen.getByText(/TEMPORAL ENGINE/i)).toBeInTheDocument());
  });

  it("calls get_temporal_history on mount", async () => {
    mockCommands(MOCKS);
    render(<TemporalEngine />);
    await waitFor(() => expectInvoked("get_temporal_history"));
  });

  it("switches tabs", async () => {
    mockCommands(MOCKS);
    render(<TemporalEngine />);
    await waitFor(() => expect(screen.getByText("TIMELINES")).toBeInTheDocument());
    fireEvent.click(screen.getByText("NEW FORK"));
    await waitFor(() => expect(screen.getByText(/Create New Fork/i)).toBeInTheDocument());
  });

  it("handles backend error gracefully", async () => {
    mockCommandError("get_temporal_history", "offline");
    mockCommands({ list_agents: [] });
    render(<TemporalEngine />);
    await waitFor(() => {
      // Page shows error or renders fallback — shouldn't crash
      const body = document.body.textContent || "";
      expect(body).toContain("TEMPORAL ENGINE");
    });
  });
});

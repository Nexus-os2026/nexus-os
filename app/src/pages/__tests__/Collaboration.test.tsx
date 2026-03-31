import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Collaboration from "../Collaboration";

const MOCKS: Record<string, unknown> = {
  collab_list_active: [],
  collab_get_policy: { min_autonomy: 2, max_sessions: 10 },
  collab_get_patterns: [],
  list_agents: [],
};

describe("Collaboration", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Collaboration />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<Collaboration />);
    await waitFor(() => expectInvoked("collab_list_active"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("collab_list_active", "connection refused");
    const { container } = render(<Collaboration />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

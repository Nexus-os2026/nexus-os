import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import BrowserAgent from "../BrowserAgent";

const MOCKS: Record<string, unknown> = {
  browser_get_policy: {
    min_autonomy_level: 0, max_sessions_per_agent: 1, max_steps_per_task: 10,
    url_denylist: [], allow_headful: false, max_task_duration_secs: 60,
  },
  browser_session_count: 0,
  list_agents: [],
};

describe("BrowserAgent", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<BrowserAgent />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<BrowserAgent />);
    await waitFor(() => expectInvoked("browser_get_policy"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("browser_get_policy", "connection refused", MOCKS);
    const { container } = render(<BrowserAgent />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Scheduler from "../Scheduler";

const MOCKS: Record<string, unknown> = {
  scheduler_list: [],
  scheduler_runner_status: "{}",
  list_agents: [],
};

describe("Scheduler", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Scheduler />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<Scheduler />);
    await waitFor(() => expectInvoked("scheduler_list"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("scheduler_list", "connection refused");
    const { container } = render(<Scheduler />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

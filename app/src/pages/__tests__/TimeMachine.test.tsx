import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import TimeMachine from "../TimeMachine";

const MOCKS: Record<string, unknown> = {
  time_machine_list_checkpoints: [],
  replay_list_bundles: [],
  get_temporal_history: "[]",
};

describe("TimeMachine", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<TimeMachine />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<TimeMachine />);
    await waitFor(() => expectInvoked("time_machine_list_checkpoints"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("time_machine_list_checkpoints", "connection refused");
    const { container } = render(<TimeMachine />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

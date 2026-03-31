import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import TimelineViewer from "../TimelineViewer";

const MOCKS: Record<string, unknown> = {
  get_temporal_history: "[]",
};

describe("TimelineViewer", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<TimelineViewer />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<TimelineViewer />);
    await waitFor(() => expectInvoked("get_temporal_history"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("get_temporal_history", "connection refused");
    const { container } = render(<TimelineViewer />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

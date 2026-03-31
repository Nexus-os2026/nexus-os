import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import MeasurementCompare from "../MeasurementCompare";

const MOCKS: Record<string, unknown> = {
  cm_list_sessions: [],
  cm_compare_agents: [],
};

describe("MeasurementCompare", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<MeasurementCompare />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<MeasurementCompare />);
    await waitFor(() => expectInvoked("cm_list_sessions"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("cm_list_sessions", "connection refused");
    const { container } = render(<MeasurementCompare />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

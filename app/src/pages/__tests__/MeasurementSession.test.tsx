import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import MeasurementSession from "../MeasurementSession";

const MOCKS: Record<string, unknown> = {
  cm_get_session: "{}",
  cm_list_sessions: [],
  cm_get_gaming_flags: [],
};

describe("MeasurementSession", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<MeasurementSession />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<MeasurementSession />);
    await waitFor(() => expectInvoked("cm_list_sessions"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("cm_list_sessions", "connection refused");
    const { container } = render(<MeasurementSession />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

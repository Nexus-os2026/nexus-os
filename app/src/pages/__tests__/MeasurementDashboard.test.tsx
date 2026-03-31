import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import MeasurementDashboard from "../MeasurementDashboard";

const MOCKS: Record<string, unknown> = {
  cm_list_sessions: [],
  cm_get_batteries: [],
  list_agents: [],
};

describe("MeasurementDashboard", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<MeasurementDashboard />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<MeasurementDashboard />);
    await waitFor(() => expectInvoked("cm_list_sessions"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("cm_list_sessions", "connection refused");
    const { container } = render(<MeasurementDashboard />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

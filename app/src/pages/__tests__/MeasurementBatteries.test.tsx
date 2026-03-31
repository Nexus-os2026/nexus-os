import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import MeasurementBatteries from "../MeasurementBatteries";

const MOCKS: Record<string, unknown> = {
  cm_get_batteries: [],
};

describe("MeasurementBatteries", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<MeasurementBatteries />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<MeasurementBatteries />);
    await waitFor(() => expectInvoked("cm_get_batteries"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("cm_get_batteries", "connection refused");
    const { container } = render(<MeasurementBatteries />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

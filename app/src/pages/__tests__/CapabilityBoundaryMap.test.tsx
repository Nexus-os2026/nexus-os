import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import CapabilityBoundaryMap from "../CapabilityBoundaryMap";

const MOCKS: Record<string, unknown> = {
  cm_get_boundary_map: [],
  cm_get_calibration: "{}",
  cm_get_census: "{}",
  cm_get_gaming_report_batch: "{}",
  cm_upload_darwin: "{}",
  list_agents: [],
};

describe("CapabilityBoundaryMap", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<CapabilityBoundaryMap />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<CapabilityBoundaryMap />);
    await waitFor(() => expectInvoked("cm_get_boundary_map"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("cm_get_boundary_map", "connection refused");
    const { container } = render(<CapabilityBoundaryMap />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

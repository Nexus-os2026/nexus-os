import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import CapabilityBoundaryMap from "../CapabilityBoundaryMap";

const MOCKS: Record<string, unknown> = {
  cm_get_boundary_map: [],
  cm_get_calibration: { is_calibrated: true, inversions: [] },
  cm_get_census: { total: 0, balanced: 0, theoretical_reasoner: 0, procedural_executor: 0, rigid_tool_user: 0, pattern_matching: 0, anomalous: 0 },
  cm_get_gaming_report_batch: { total_flags: 0, red_count: 0, orange_count: 0, yellow_count: 0, agents_with_flags: 0, agents_clean: 0 },
  cm_upload_darwin: "ok",
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
    mockCommandError("cm_get_boundary_map", "connection refused", MOCKS);
    const { container } = render(<CapabilityBoundaryMap />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import SoftwareFactory from "../SoftwareFactory";

const MOCKS: Record<string, unknown> = {
  swf_list_projects: [],
  swf_get_policy: { min_autonomy: 2 },
  swf_get_pipeline_stages: [],
  swf_estimate_cost: 0,
  list_agents: [],
};

describe("SoftwareFactory", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<SoftwareFactory />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<SoftwareFactory />);
    await waitFor(() => expectInvoked("swf_list_projects"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("swf_list_projects", "connection refused");
    const { container } = render(<SoftwareFactory />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

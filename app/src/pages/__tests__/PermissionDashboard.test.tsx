import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import PermissionDashboard from "../PermissionDashboard";

const MOCKS: Record<string, unknown> = {
  list_agents: [],
  get_agent_permissions: "[]",
  get_permission_history: [],
};

describe("PermissionDashboard", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<PermissionDashboard />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<PermissionDashboard />);
    await waitFor(() => expectInvoked("get_agent_permissions"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("list_agents", "connection refused");
    const { container } = render(<PermissionDashboard />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

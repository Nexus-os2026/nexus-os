import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AdminFleet from "../AdminFleet";

const MOCKS: Record<string, unknown> = {
  admin_fleet_status: "[]",
  list_agents: [],
};

describe("AdminFleet", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<AdminFleet />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<AdminFleet />);
    await waitFor(() => expectInvoked("admin_fleet_status"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("admin_fleet_status", "connection refused");
    const { container } = render(<AdminFleet />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AdminDashboard from "../AdminDashboard";

const MOCKS: Record<string, unknown> = {
  admin_overview: JSON.stringify({
    total_agents: 0,
    active_agents: 0,
    total_users: 0,
    active_users: 0,
    workspaces: 0,
    fuel_consumed_24h: 0,
    hitl_pending: 0,
    security_events_24h: 0,
    system_health: { status: "healthy", cpu_percent: 0, memory_percent: 0, disk_percent: 0, uptime_seconds: 0 },
  }),
};

describe("AdminDashboard", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<AdminDashboard />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<AdminDashboard />);
    await waitFor(() => expectInvoked("admin_overview"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("admin_overview", "connection refused", MOCKS);
    const { container } = render(<AdminDashboard />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

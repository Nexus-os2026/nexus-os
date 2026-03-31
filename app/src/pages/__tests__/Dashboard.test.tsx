import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Dashboard from "../Dashboard";

const MOCKS = {
  list_agents: [{ id: "a1", name: "agent-1", status: "Running", fuel_remaining: 500, autonomy_level: 3 }],
  get_audit_log: [{ id: "ev1", agent_id: "a1", timestamp: Date.now(), event_type: "ToolCall", payload: "{}" }],
  get_live_system_metrics: { cpu_percent: 42, ram_used_mb: 2048, ram_total_mb: 16384 },
};

describe("Dashboard", () => {
  it("renders heading", async () => {
    mockCommands(MOCKS);
    render(<Dashboard />);
    await waitFor(() => expect(screen.getByText(/Dashboard/i)).toBeInTheDocument());
  });

  it("loads agents and audit on mount", async () => {
    mockCommands(MOCKS);
    render(<Dashboard />);
    await waitFor(() => expectInvoked("list_agents"));
    expectInvoked("get_audit_log");
  });

  it("renders content after load", async () => {
    mockCommands(MOCKS);
    render(<Dashboard />);
    await waitFor(() => {
      // Page renders agent count or status info
      const body = document.body.textContent || "";
      expect(body).toContain("Total Agents");
    });
  });

  it("shows error state on backend failure", async () => {
    mockCommandError("list_agents", "connection refused");
    render(<Dashboard />);
    await waitFor(() => {
      const errorEl = document.querySelector('[class*="rose"]') || document.querySelector('[style*="ef4444"]');
      expect(errorEl).toBeTruthy();
    });
  });
});

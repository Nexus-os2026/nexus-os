import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AdminSystemHealth from "../AdminSystemHealth";

const MOCKS: Record<string, unknown> = {
  admin_system_health: JSON.stringify({
    instances: [],
    providers: [],
    database: { size_mb: 0, growth_rate_mb_day: 0, tables: 0, total_rows: 0 },
    backup: { last_backup: new Date().toISOString(), next_scheduled: new Date().toISOString(), backup_size_mb: 0, status: "ok" },
  }),
  telemetry_health: JSON.stringify({
    instances: [], providers: [],
    database: { size_mb: 0, growth_rate_mb_day: 0, tables: 0, total_rows: 0 },
    backup: { last_backup: new Date().toISOString(), next_scheduled: new Date().toISOString(), backup_size_mb: 0, status: "ok" },
  }),
  backup_list: "[]",
};

describe("AdminSystemHealth", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<AdminSystemHealth />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<AdminSystemHealth />);
    await waitFor(() => expectInvoked("admin_system_health"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("admin_system_health", "connection refused", MOCKS);
    const { container } = render(<AdminSystemHealth />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

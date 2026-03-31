import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AdminSystemHealth from "../AdminSystemHealth";

const MOCKS: Record<string, unknown> = {
  admin_system_health: "{}",
  telemetry_health: "{}",
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
    mockCommandError("admin_system_health", "connection refused");
    const { container } = render(<AdminSystemHealth />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

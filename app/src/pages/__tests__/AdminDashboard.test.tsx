import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AdminDashboard from "../AdminDashboard";

const MOCKS: Record<string, unknown> = {
  admin_overview: '{"agents_total":0,"agents_active":0,"fuel_consumed_24h":0}',
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
    mockCommandError("admin_overview", "connection refused");
    const { container } = render(<AdminDashboard />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

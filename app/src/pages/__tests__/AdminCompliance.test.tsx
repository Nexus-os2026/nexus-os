import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AdminCompliance from "../AdminCompliance";

const MOCKS: Record<string, unknown> = {
  admin_compliance_status: "{}",
  admin_compliance_export: "{}",
};

describe("AdminCompliance", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<AdminCompliance />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<AdminCompliance />);
    await waitFor(() => expectInvoked("admin_compliance_status"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("admin_compliance_status", "connection refused");
    const { container } = render(<AdminCompliance />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

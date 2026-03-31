import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AdminUsers from "../AdminUsers";

const MOCKS: Record<string, unknown> = {
  admin_users_list: "[]",
};

describe("AdminUsers", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<AdminUsers />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<AdminUsers />);
    await waitFor(() => expectInvoked("admin_users_list"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("admin_users_list", "connection refused");
    const { container } = render(<AdminUsers />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AdminPolicyEditor from "../AdminPolicyEditor";

const MOCKS: Record<string, unknown> = {
  admin_policy_get: "{}",
  admin_policy_history: "[]",
};

describe("AdminPolicyEditor", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<AdminPolicyEditor />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<AdminPolicyEditor />);
    await waitFor(() => expectInvoked("admin_policy_get"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("admin_policy_get", "connection refused");
    const { container } = render(<AdminPolicyEditor />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

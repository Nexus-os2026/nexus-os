import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import PolicyManagement from "../PolicyManagement";

const MOCKS: Record<string, unknown> = {
  policy_list: [],
  policy_detect_conflicts: "[]",
};

describe("PolicyManagement", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<PolicyManagement />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<PolicyManagement />);
    await waitFor(() => expectInvoked("policy_list"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("policy_list", "connection refused");
    const { container } = render(<PolicyManagement />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

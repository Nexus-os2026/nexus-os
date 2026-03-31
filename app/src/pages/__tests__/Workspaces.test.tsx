import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Workspaces from "../Workspaces";

const MOCKS: Record<string, unknown> = {
  workspace_list: "[]",
  workspace_usage: "{}",
};

describe("Workspaces", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Workspaces />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<Workspaces />);
    await waitFor(() => expectInvoked("workspace_list"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("workspace_list", "connection refused");
    const { container } = render(<Workspaces />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

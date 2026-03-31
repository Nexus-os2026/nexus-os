import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import CommandCenter from "../CommandCenter";

const MOCKS: Record<string, unknown> = {
  list_agents: [],
  get_audit_log: [],
};

describe("CommandCenter", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<CommandCenter />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<CommandCenter />);
    await waitFor(() => expectInvoked("list_agents"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("list_agents", "connection refused");
    const { container } = render(<CommandCenter />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

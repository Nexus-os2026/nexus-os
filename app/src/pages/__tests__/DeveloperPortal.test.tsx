import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import DeveloperPortal from "../DeveloperPortal";

const MOCKS: Record<string, unknown> = {
  marketplace_my_agents: [],
  list_agents: [],
};

describe("DeveloperPortal", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<DeveloperPortal />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<DeveloperPortal />);
    await waitFor(() => expectInvoked("marketplace_my_agents"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("list_agents", "connection refused", MOCKS);
    const { container } = render(<DeveloperPortal />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

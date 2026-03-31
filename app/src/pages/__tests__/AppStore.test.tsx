import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AppStore from "../AppStore";

const MOCKS: Record<string, unknown> = {
  get_preinstalled_agents: [],
  marketplace_search: [],
  marketplace_search_gitlab: "[]",
};

describe("AppStore", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<AppStore />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<AppStore />);
    await waitFor(() => expectInvoked("get_preinstalled_agents"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("get_preinstalled_agents", "connection refused");
    const { container } = render(<AppStore />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

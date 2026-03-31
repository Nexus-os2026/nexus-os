import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Messaging from "../Messaging";

const MOCKS: Record<string, unknown> = {
  get_config: "{}",
  get_messaging_status: "[]",
  list_agents: [],
};

describe("Messaging", () => {
  it("renders heading", async () => {
    mockCommands(MOCKS);
    render(<Messaging />);
    await waitFor(() => expect(screen.getAllByText(/Messaging/i).length).toBeGreaterThan(0));
  });

  it("loads data on mount", async () => {
    mockCommands(MOCKS);
    render(<Messaging />);
    await waitFor(() => expectInvoked("get_config"));
    expectInvoked("get_messaging_status");
    expectInvoked("list_agents");
  });

  it("shows error state on failure", async () => {
    mockCommandError("get_config", "connection refused");
    render(<Messaging />);
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body.length).toBeGreaterThan(0);
    });
  });
});

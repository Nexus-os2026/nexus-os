import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import ModelRouting from "../ModelRouting";

const MOCKS: Record<string, unknown> = {
  router_get_accuracy: "{}",
  router_get_models: [],
  router_get_feedback: "{}",
  list_agents: [],
};

describe("ModelRouting", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<ModelRouting />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<ModelRouting />);
    await waitFor(() => expectInvoked("router_get_accuracy"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("router_get_accuracy", "connection refused");
    const { container } = render(<ModelRouting />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

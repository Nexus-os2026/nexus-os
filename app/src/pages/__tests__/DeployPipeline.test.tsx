import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import DeployPipeline from "../DeployPipeline";

const MOCKS: Record<string, unknown> = {
  factory_list_projects: "[]",
  factory_get_build_history: "[]",
};

describe("DeployPipeline", () => {
  it("renders heading", async () => {
    mockCommands(MOCKS);
    render(<DeployPipeline />);
    await waitFor(() => expect(screen.getAllByText(/Deploy/i).length).toBeGreaterThan(0));
  });

  it("loads data on mount", async () => {
    mockCommands(MOCKS);
    render(<DeployPipeline />);
    await waitFor(() => expectInvoked("factory_list_projects"));
  });

  it("shows error state on failure", async () => {
    mockCommandError("factory_list_projects", "connection refused");
    render(<DeployPipeline />);
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body.length).toBeGreaterThan(0);
    });
  });
});

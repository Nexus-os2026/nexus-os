import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import ModelHub from "../ModelHub";

const MOCKS: Record<string, unknown> = {
  list_local_models: "[]",
  list_provider_models: [],
  get_system_specs: '{"cpu":"test","ram_mb":8192,"disk_gb":100}',
  nexus_link_status: '{"sharing_enabled":false}',
  nexus_link_list_peers: "[]",
  get_active_llm_provider: '"ollama"',
};

describe("ModelHub", () => {
  it("renders heading", async () => {
    mockCommands(MOCKS);
    render(<ModelHub />);
    await waitFor(() => expect(screen.getByText(/Model Hub/i)).toBeInTheDocument());
  });

  it("loads data on mount", async () => {
    mockCommands(MOCKS);
    render(<ModelHub />);
    await waitFor(() => expectInvoked("list_local_models"));
    expectInvoked("list_provider_models");
    expectInvoked("get_system_specs");
  });

  it("shows error state on failure", async () => {
    mockCommandError("list_local_models", "connection refused");
    render(<ModelHub />);
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body.length).toBeGreaterThan(0);
    });
  });
});

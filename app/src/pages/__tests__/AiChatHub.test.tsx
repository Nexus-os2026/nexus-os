import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AiChatHub from "../AiChatHub";

const MOCKS: Record<string, unknown> = {
  list_provider_models: [],
  get_provider_status: "{}",
  get_preinstalled_agents: [],
  get_audit_log: [],
  check_llm_status: { providers: [{ available: true, name: "ollama" }] },
};

describe("AiChatHub", () => {
  it("renders heading", async () => {
    mockCommands(MOCKS);
    render(<AiChatHub />);
    await waitFor(() => expect(screen.getAllByText(/AI Chat Hub/i).length).toBeGreaterThan(0));
  });

  it("loads data on mount", async () => {
    mockCommands(MOCKS);
    render(<AiChatHub />);
    await waitFor(() => expectInvoked("list_provider_models"));
    expectInvoked("get_provider_status");
  });

  it("handles backend failure without crashing", async () => {
    mockCommandError("list_provider_models", "connection refused", MOCKS);
    const { container } = render(<AiChatHub />);
    await waitFor(() => {
      expect(container).toBeTruthy();
    });
  });
});

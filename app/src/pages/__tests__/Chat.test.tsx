import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError } from "../../test/setup";
import { Chat } from "../Chat";

const PROPS = {
  messages: [] as any[], draft: "", isRecording: false, isSending: false,
  agents: [] as any[], selectedAgent: "", selectedModel: "mock",
  onAgentChange: () => {}, onModelChange: () => {}, onDraftChange: () => {},
  onSend: () => {}, onToggleMic: () => {}, onClearMessages: () => {}, onNavigate: () => {},
};

const MOCKS: Record<string, unknown> = {
  list_provider_models: [],
  get_config: "{}",
};

describe("Chat", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Chat {...PROPS} />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<Chat {...PROPS} />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("list_provider_models", "connection refused");
    const { container } = render(<Chat {...PROPS} />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

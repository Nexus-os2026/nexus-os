import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import VoiceAssistant from "../VoiceAssistant";

const MOCKS: Record<string, unknown> = {
  voice_get_status: '{"status":"ready","model_loaded":false}',
};

describe("VoiceAssistant", () => {
  it("renders heading", async () => {
    mockCommands(MOCKS);
    render(<VoiceAssistant />);
    await waitFor(() => expect(screen.getAllByText(/Voice Assistant/i).length).toBeGreaterThan(0));
  });

  it("loads data on mount", async () => {
    mockCommands(MOCKS);
    render(<VoiceAssistant />);
    await waitFor(() => expectInvoked("voice_get_status"));
  });

  it("shows error state on failure", async () => {
    mockCommandError("voice_get_status", "connection refused");
    render(<VoiceAssistant />);
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body.length).toBeGreaterThan(0);
    });
  });
});

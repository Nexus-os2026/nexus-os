import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import { listen } from "@tauri-apps/api/event";
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

  it("registers desktop events and releases every listener on unmount", async () => {
    const unlisteners: ReturnType<typeof vi.fn>[] = [];
    const mockedListen = vi.mocked(listen);
    mockedListen.mockClear();
    mockedListen.mockImplementation(async () => {
      const unlisten = vi.fn();
      unlisteners.push(unlisten);
      return unlisten;
    });
    mockCommands(MOCKS);
    const { unmount } = render(<AiChatHub />);
    try {
      await waitFor(() => {
        const events = mockedListen.mock.calls.map(([event]) => event);
        expect(events).toEqual(expect.arrayContaining([
          "model-downloaded",
          "consent-request-pending",
          "consent-resolved",
          "agent-evolved",
          "conductor:plan",
          "conductor:agent_completed",
          "conductor:finished",
        ]));
      });
      await vi.dynamicImportSettled();
      unmount();
      expect(unlisteners.length).toBeGreaterThanOrEqual(7);
      for (const unlisten of unlisteners) {
        expect(unlisten).toHaveBeenCalledTimes(1);
      }
    } finally {
      unmount();
      mockedListen.mockResolvedValue(() => {});
    }
  });
});

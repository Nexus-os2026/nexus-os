import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import AgentBrowser from "../AgentBrowser";

const MOCKS: Record<string, unknown> = {
  navigate_to: { allowed: true, url: "about:blank", title: "Blank" },
};

describe("AgentBrowser", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<AgentBrowser />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("shows Playwright empty state heading", async () => {
    mockCommands(MOCKS);
    render(<AgentBrowser />);
    await waitFor(() => {
      expect(document.body.textContent).toContain("Browser automation requires Playwright");
    });
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("navigate_to", "connection refused");
    render(<AgentBrowser />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
});

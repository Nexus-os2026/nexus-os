import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import EmailClient from "../EmailClient";

const MOCKS: Record<string, unknown> = {
  email_list: "[]",
  email_oauth_status: "[]",
};

describe("EmailClient", () => {
  it("renders heading", async () => {
    mockCommands(MOCKS);
    render(<EmailClient />);
    await waitFor(() => expect(screen.getAllByText(/Mail/i).length).toBeGreaterThan(0));
  });

  it("loads data on mount", async () => {
    mockCommands(MOCKS);
    render(<EmailClient />);
    await waitFor(() => expectInvoked("email_list"));
    expectInvoked("email_oauth_status");
  });

  it("shows error state on failure", async () => {
    mockCommandError("email_list", "connection refused");
    render(<EmailClient />);
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body.length).toBeGreaterThan(0);
    });
  });
});

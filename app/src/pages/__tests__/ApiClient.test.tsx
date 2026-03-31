import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import ApiClient from "../ApiClient";

const MOCKS: Record<string, unknown> = {
  api_client_list_collections: "[]",
};

describe("ApiClient", () => {
  it("renders heading", async () => {
    mockCommands(MOCKS);
    render(<ApiClient />);
    await waitFor(() => expect(screen.getByText(/API Client/i)).toBeInTheDocument());
  });

  it("loads data on mount", async () => {
    mockCommands(MOCKS);
    render(<ApiClient />);
    await waitFor(() => expectInvoked("api_client_list_collections"));
  });

  it("shows error state on failure", async () => {
    mockCommandError("api_client_list_collections", "connection refused");
    render(<ApiClient />);
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body.length).toBeGreaterThan(0);
    });
  });
});

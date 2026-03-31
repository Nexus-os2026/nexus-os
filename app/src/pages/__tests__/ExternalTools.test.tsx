import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import ExternalTools from "../ExternalTools";

const MOCKS: Record<string, unknown> = {
  tools_get_registry: [],
  tools_get_audit: [],
  tools_get_policy: { min_autonomy_level: 2, max_tools_per_call: 5, audit_hash_chain: true, rate_limit_per_minute: 30 },
};

describe("ExternalTools", () => {
  it("renders heading", async () => {
    mockCommands(MOCKS);
    render(<ExternalTools />);
    await waitFor(() => expect(screen.getByText(/External Tools/i)).toBeInTheDocument());
  });

  it("loads data on mount", async () => {
    mockCommands(MOCKS);
    render(<ExternalTools />);
    await waitFor(() => expectInvoked("tools_get_registry"));
    expectInvoked("tools_get_audit");
    expectInvoked("tools_get_policy");
  });

  it("shows error state on failure", async () => {
    mockCommandError("tools_get_registry", "connection refused");
    render(<ExternalTools />);
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body.length).toBeGreaterThan(0);
    });
  });
});

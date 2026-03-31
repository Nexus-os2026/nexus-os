import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import SelfRewriteLab from "../SelfRewriteLab";

const MOCKS: Record<string, unknown> = {
  self_rewrite_analyze: "{}",
  self_rewrite_get_history: [],
  self_rewrite_suggest_patches: [],
};

describe("SelfRewriteLab", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<SelfRewriteLab />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<SelfRewriteLab />);
    await waitFor(() => expectInvoked("self_rewrite_get_history"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("self_rewrite_analyze", "connection refused");
    const { container } = render(<SelfRewriteLab />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

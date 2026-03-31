import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError } from "../../test/setup";
import ABValidation from "../ABValidation";

const MOCKS: Record<string, unknown> = {
  list_agents: [],
  cm_run_ab_validation: "{}",
};

describe("ABValidation", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<ABValidation />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("renders validation UI elements", async () => {
    mockCommands(MOCKS);
    render(<ABValidation />);
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body).toMatch(/validat|routing|compare/i);
    });
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("cm_run_ab_validation", "connection refused");
    const { container } = render(<ABValidation />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

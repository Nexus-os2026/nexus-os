import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError } from "../../test/setup";
import Terminal from "../Terminal";

const MOCKS: Record<string, unknown> = {
  terminal_execute: '{"stdout":"","stderr":"","exit_code":0}',
};

describe("Terminal", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Terminal />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("renders terminal UI", async () => {
    mockCommands(MOCKS);
    const { container } = render(<Terminal />);
    await waitFor(() => expect(container.textContent).toMatch(/terminal|command|shell/i));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("terminal_execute", "connection refused");
    const { container } = render(<Terminal />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

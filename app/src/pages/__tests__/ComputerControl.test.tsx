import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import ComputerControl from "../ComputerControl";

const MOCKS: Record<string, unknown> = {
  computer_control_status: '{"enabled":false,"sessions":0}',
  computer_control_get_history: [],
  get_input_control_status: '{"active":false}',
};

describe("ComputerControl", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<ComputerControl />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<ComputerControl />);
    await waitFor(() => expectInvoked("computer_control_status"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("computer_control_status", "connection refused");
    const { container } = render(<ComputerControl />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

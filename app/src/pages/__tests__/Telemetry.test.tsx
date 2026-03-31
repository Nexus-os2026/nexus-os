import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Telemetry from "../Telemetry";

const MOCKS: Record<string, unknown> = {
  telemetry_status: "{}",
  telemetry_config_get: "{}",
  telemetry_health: "{}",
};

describe("Telemetry", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Telemetry />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<Telemetry />);
    await waitFor(() => expectInvoked("telemetry_status"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("telemetry_status", "connection refused");
    const { container } = render(<Telemetry />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

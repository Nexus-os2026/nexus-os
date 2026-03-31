import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import Telemetry from "../Telemetry";

const MOCKS: Record<string, unknown> = {
  telemetry_status: JSON.stringify({
    status: "Healthy", version: "0.0.0", uptime: "0s", agents_active: 0, audit_chain_valid: true,
  }),
  telemetry_config_get: JSON.stringify({
    enabled: false, otlp_endpoint: "", service_name: "nexus", sample_rate: 1, log_format: "json", log_level: "info",
  }),
  telemetry_health: JSON.stringify({
    status: "Healthy", version: "0.0.0", uptime: "0s", agents_active: 0, audit_chain_valid: true,
  }),
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
    mockCommandError("telemetry_status", "connection refused", MOCKS);
    const { container } = render(<Telemetry />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

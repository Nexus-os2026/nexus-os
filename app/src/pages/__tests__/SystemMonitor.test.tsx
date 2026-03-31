import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import SystemMonitor from "../SystemMonitor";

const LIVE_METRICS = JSON.stringify({
  cpu_name: "Test CPU",
  cpu_cores: 4,
  cpu_avg: 25.0,
  per_core_usage: [20, 30, 25, 22],
  total_ram: 16000000000,
  used_ram: 8000000000,
  available_ram: 8000000000,
  uptime_secs: 3600,
  process_count: 120,
  nexus_disk_bytes: 500000000,
  disk_total: 1000000000000,
  disk_available: 500000000000,
  agents: [],
});

const MOCKS: Record<string, unknown> = {
  get_live_system_metrics: LIVE_METRICS,
  list_agents: [],
};

describe("SystemMonitor", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<SystemMonitor />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<SystemMonitor />);
    await waitFor(() => expectInvoked("get_live_system_metrics"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("get_live_system_metrics", "connection refused");
    render(<SystemMonitor />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
});

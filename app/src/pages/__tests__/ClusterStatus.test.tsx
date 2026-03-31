import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import ClusterStatus from "../ClusterStatus";

const MOCKS: Record<string, unknown> = {
  get_live_system_metrics: JSON.stringify({
    cpu_name: "test",
    cpu_cores: 4,
    cpu_avg: 10,
    total_ram: 8000000000,
    used_ram: 4000000000,
    available_ram: 4000000000,
    uptime_secs: 1000,
    process_count: 50,
    nexus_disk_bytes: 1000000,
    disk_total: 100000000000,
    disk_available: 50000000000,
    agents: [],
  }),
  list_agents: [],
  mesh_get_peers: [],
  mesh_get_sync_status: "{}",
};

describe("ClusterStatus", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<ClusterStatus />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<ClusterStatus />);
    await waitFor(() => expectInvoked("list_agents"));
  });
  it("handles backend failure gracefully", async () => {
    mockCommandError("list_agents", "connection refused");
    const { container } = render(<ClusterStatus />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});

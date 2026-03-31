import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import MissionControl from "../MissionControl";

const MOCKS: Record<string, unknown> = {
  list_agents: [],
  get_immune_status: '{"threat_level":"Green","active_antibodies":0,"threats_blocked":0}',
  mesh_get_peers: [],
  civ_get_economy_status: '{"total_agents":0}',
  get_dream_status: '{}',
  get_morning_briefing: '{"briefing":"No dreams"}',
  get_consciousness_heatmap: [],
  get_temporal_history: "[]",
  get_audit_log: [],
  tray_status: '{"visible":true}',
  get_os_fitness: '{"score":0.8}',
  get_fitness_history: "[]",
};

describe("MissionControl", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<MissionControl onNavigate={() => {}} />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("calls backend on mount", async () => {
    mockCommands(MOCKS);
    render(<MissionControl onNavigate={() => {}} />);
    await waitFor(() => expectInvoked("list_agents"));
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("list_agents", "connection refused");
    render(<MissionControl onNavigate={() => {}} />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });
});

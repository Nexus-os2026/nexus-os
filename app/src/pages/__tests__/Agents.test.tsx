import { render, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { mockCommands, expectInvoked } from "../../test/setup";
import { Agents } from "../Agents";
import type { AgentSummary, AuditEventRow } from "../../types";

const AGENT: AgentSummary = {
  id: "a1",
  name: "test-agent",
  status: "Running",
  fuel_remaining: 500,
  fuel_budget: 1000,
  autonomy_level: 3,
  capabilities: ["web.search"],
  last_action: "web.search",
};

const baseProps = {
  agents: [AGENT] as AgentSummary[],
  auditEvents: [] as AuditEventRow[],
  onStart: vi.fn(),
  onPause: vi.fn(),
  onStop: vi.fn(),
  onCreate: vi.fn(),
  onDelete: vi.fn(),
  onClearAll: vi.fn(),
  onPermissions: vi.fn(),
  onNavigate: vi.fn(),
};

describe("Agents", () => {
  it("mounts without throwing", () => {
    mockCommands({ get_preinstalled_agents: [], list_provider_models: [], get_available_providers: [] });
    expect(() => render(<Agents {...baseProps} />)).not.toThrow();
  });

  it("loads preinstalled agents on mount", async () => {
    mockCommands({ get_preinstalled_agents: [], list_provider_models: [], get_available_providers: [] });
    render(<Agents {...baseProps} />);
    await waitFor(() => expectInvoked("get_preinstalled_agents"));
  });

  it("loads available providers on mount", async () => {
    mockCommands({ get_preinstalled_agents: [], list_provider_models: [], get_available_providers: [{ name: "ollama", available: true }] });
    render(<Agents {...baseProps} />);
    await waitFor(() => expectInvoked("get_available_providers"));
  });

  it("mounts with empty agents list without throwing", () => {
    mockCommands({ get_preinstalled_agents: [], list_provider_models: [], get_available_providers: [] });
    expect(() => render(<Agents {...baseProps} agents={[]} />)).not.toThrow();
  });
});

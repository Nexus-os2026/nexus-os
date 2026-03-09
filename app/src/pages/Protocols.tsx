import { useCallback, useEffect, useState } from "react";
import { getAgentCards, getMcpTools, getProtocolsRequests, getProtocolsStatus, hasDesktopRuntime } from "../api/backend";
import type { AgentCardSummary, McpTool, ProtocolRequest, ProtocolsStatus } from "../types";
import "./protocols.css";

// ── Mock data for non-desktop mode ──

const MOCK_STATUS: ProtocolsStatus = {
  a2a_status: "stopped",
  a2a_version: "0.2.1",
  a2a_peers: 0,
  a2a_tasks_processed: 0,
  mcp_status: "stopped",
  mcp_registered_tools: 8,
  mcp_invocations: 0,
  gateway_port: null,
  governance_bridge_active: false,
  audit_integrity: true,
};

const MOCK_TOOLS: McpTool[] = [
  { name: "web_search", description: "Search the web and return relevant results", agent: "Coder", fuel_cost: 50, requires_hitl: false, invocations: 12 },
  { name: "llm_query", description: "Query a language model with governed fuel accounting", agent: "Coder", fuel_cost: 500, requires_hitl: false, invocations: 45 },
  { name: "fs_read", description: "Read a file from the governed filesystem sandbox", agent: "Coder", fuel_cost: 10, requires_hitl: false, invocations: 89 },
  { name: "fs_write", description: "Write a file to the governed filesystem sandbox", agent: "Designer", fuel_cost: 20, requires_hitl: true, invocations: 7 },
  { name: "social_x_post", description: "Publish a post to X (Twitter)", agent: "Screen Poster", fuel_cost: 30, requires_hitl: true, invocations: 3 },
  { name: "process_exec", description: "Execute a sandboxed process with governance controls", agent: "Coder", fuel_cost: 100, requires_hitl: true, invocations: 0 },
  { name: "audit_read", description: "Read audit trail events with hash-chain verification", agent: "Self Improve", fuel_cost: 10, requires_hitl: false, invocations: 22 },
  { name: "messaging_send", description: "Send messages through governed messaging channels", agent: "Workflow Studio", fuel_cost: 20, requires_hitl: true, invocations: 1 },
];

const MOCK_CARDS: AgentCardSummary[] = [
  { agent_name: "Coder", url: "http://localhost:3000/a2a/Coder", skills_count: 4, auth_scheme: "bearer", rate_limit_rpm: 100, card_json: { name: "Coder", version: "0.2.1", skills: [{ id: "web-search", name: "Web Search" }, { id: "llm-query", name: "LLM Query" }, { id: "fs-read", name: "File Read" }, { id: "process-exec", name: "Process Execute" }] } },
  { agent_name: "Designer", url: "http://localhost:3000/a2a/Designer", skills_count: 2, auth_scheme: "bearer", rate_limit_rpm: 50, card_json: { name: "Designer", version: "0.2.1", skills: [{ id: "fs-read", name: "File Read" }, { id: "fs-write", name: "File Write" }] } },
  { agent_name: "Screen Poster", url: "http://localhost:3000/a2a/Screen Poster", skills_count: 3, auth_scheme: "bearer, mtls", rate_limit_rpm: 30, card_json: { name: "Screen Poster", version: "0.2.1", skills: [{ id: "social-x-post", name: "X Post" }, { id: "social-post", name: "Social Post" }, { id: "social-x-read", name: "X Read" }] } },
];

const MOCK_REQUESTS: ProtocolRequest[] = [
  { id: "req-001", timestamp: Date.now() - 120_000, protocol: "MCP", method: "tools/invoke", sender: "external-client", agent: "Coder", status: "completed", fuel_consumed: 50, governance_decision: "allowed" },
  { id: "req-002", timestamp: Date.now() - 60_000, protocol: "A2A", method: "tasks/send", sender: "partner-agent", agent: "Screen Poster", status: "completed", fuel_consumed: 30, governance_decision: "allowed" },
  { id: "req-003", timestamp: Date.now() - 30_000, protocol: "MCP", method: "tools/invoke", sender: "untrusted-bot", agent: "Coder", status: "rejected", fuel_consumed: 0, governance_decision: "denied" },
];

function formatTime(ts: number): string {
  const secs = Math.round((Date.now() - ts) / 1000);
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  return `${Math.floor(secs / 3600)}h ago`;
}

export default function Protocols(): JSX.Element {
  const [status, setStatus] = useState<ProtocolsStatus>(MOCK_STATUS);
  const [tools, setTools] = useState<McpTool[]>(MOCK_TOOLS);
  const [cards, setCards] = useState<AgentCardSummary[]>(MOCK_CARDS);
  const [requests, setRequests] = useState<ProtocolRequest[]>(MOCK_REQUESTS);
  const [selectedCard, setSelectedCard] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    if (!hasDesktopRuntime()) return;
    try {
      const [s, t, c, r] = await Promise.all([
        getProtocolsStatus(),
        getMcpTools(),
        getAgentCards(),
        getProtocolsRequests(),
      ]);
      setStatus(s);
      if (t.length > 0) setTools(t);
      if (c.length > 0) setCards(c);
      setRequests(r.length > 0 ? r : MOCK_REQUESTS);
    } catch {
      // Fall back to mock data
    }
  }, []);

  useEffect(() => { void loadData(); }, [loadData]);

  const totalFuelConsumed = requests.reduce((sum, r) => sum + r.fuel_consumed, 0);
  const selectedCardData = cards.find((c) => c.agent_name === selectedCard);

  return (
    <section className="proto-hub">
      <header className="proto-header">
        <h2 className="proto-title">PROTOCOLS // A2A + MCP GATEWAY</h2>
        <p className="proto-subtitle">External protocol integration with full governance enforcement</p>
      </header>

      {/* Summary stats */}
      <div className="proto-summary">
        <div className="proto-stat">
          <span className="proto-stat-value">{cards.length}</span>
          <span className="proto-stat-label">Agent Cards</span>
        </div>
        <div className="proto-stat">
          <span className="proto-stat-value">{tools.length}</span>
          <span className="proto-stat-label">MCP Tools</span>
        </div>
        <div className="proto-stat">
          <span className="proto-stat-value">{status.a2a_peers}</span>
          <span className="proto-stat-label">A2A Peers</span>
        </div>
        <div className="proto-stat">
          <span className="proto-stat-value">{requests.length}</span>
          <span className="proto-stat-label">Requests</span>
        </div>
        <div className="proto-stat">
          <span className="proto-stat-value">{totalFuelConsumed}</span>
          <span className="proto-stat-label">Fuel Used</span>
        </div>
        <div className="proto-stat">
          <span className="proto-stat-value">{status.audit_integrity ? "OK" : "FAIL"}</span>
          <span className="proto-stat-label">Audit Chain</span>
        </div>
      </div>

      {/* A2A + MCP server status */}
      <h3 className="proto-section-title">Server Status</h3>
      <div className="proto-servers">
        <div className="proto-server-card">
          <div className="proto-server-header">
            <span className="proto-server-name">A2A Server</span>
            <span className={`proto-status-badge proto-status-badge--${status.a2a_status === "running" ? "running" : "stopped"}`}>
              {status.a2a_status}
            </span>
          </div>
          <div className="proto-server-detail">
            <span>Protocol Version</span>
            <span className="proto-server-detail-value">{status.a2a_version}</span>
          </div>
          <div className="proto-server-detail">
            <span>Connected Peers</span>
            <span className="proto-server-detail-value">{status.a2a_peers}</span>
          </div>
          <div className="proto-server-detail">
            <span>Tasks Processed</span>
            <span className="proto-server-detail-value">{status.a2a_tasks_processed}</span>
          </div>
          <div className="proto-server-detail">
            <span>Endpoint</span>
            <span className="proto-server-detail-value">
              {status.gateway_port ? `localhost:${status.gateway_port}/a2a` : "—"}
            </span>
          </div>
        </div>

        <div className="proto-server-card">
          <div className="proto-server-header">
            <span className="proto-server-name">MCP Server</span>
            <span className={`proto-status-badge proto-status-badge--${status.mcp_status === "running" ? "running" : "stopped"}`}>
              {status.mcp_status}
            </span>
          </div>
          <div className="proto-server-detail">
            <span>Registered Tools</span>
            <span className="proto-server-detail-value">{status.mcp_registered_tools}</span>
          </div>
          <div className="proto-server-detail">
            <span>Tool Invocations</span>
            <span className="proto-server-detail-value">{status.mcp_invocations}</span>
          </div>
          <div className="proto-server-detail">
            <span>Endpoint</span>
            <span className="proto-server-detail-value">
              {status.gateway_port ? `localhost:${status.gateway_port}/mcp` : "—"}
            </span>
          </div>
        </div>

        <div className="proto-server-card">
          <div className="proto-server-header">
            <span className="proto-server-name">Governance Bridge</span>
            <span className={`proto-status-badge proto-status-badge--${status.governance_bridge_active ? "running" : "stopped"}`}>
              {status.governance_bridge_active ? "active" : "inactive"}
            </span>
          </div>
          <div className="proto-server-detail">
            <span>Audit Integrity</span>
            <span className="proto-server-detail-value">{status.audit_integrity ? "verified" : "FAILED"}</span>
          </div>
          <div className="proto-server-detail">
            <span>Gateway Port</span>
            <span className="proto-server-detail-value">{status.gateway_port ?? "—"}</span>
          </div>
          <div className="proto-server-detail">
            <span>JWT Auth</span>
            <span className="proto-server-detail-value">enabled</span>
          </div>
        </div>
      </div>

      {/* MCP Tool Registry */}
      <h3 className="proto-section-title">MCP Tool Registry</h3>
      {tools.length === 0 ? (
        <div className="proto-empty">No tools registered — start an agent to populate the registry.</div>
      ) : (
        <table className="proto-table">
          <thead>
            <tr>
              <th>Tool</th>
              <th>Agent</th>
              <th>Description</th>
              <th>Fuel</th>
              <th>HITL</th>
              <th>Calls</th>
            </tr>
          </thead>
          <tbody>
            {tools.map((tool) => (
              <tr key={`${tool.agent}-${tool.name}`}>
                <td>{tool.name}</td>
                <td>{tool.agent}</td>
                <td style={{ color: "rgba(165, 243, 252, 0.55)", fontFamily: "inherit" }}>{tool.description}</td>
                <td>{tool.fuel_cost}</td>
                <td>
                  <span className={`proto-hitl-badge proto-hitl-badge--${tool.requires_hitl ? "yes" : "no"}`}>
                    {tool.requires_hitl ? "required" : "auto"}
                  </span>
                </td>
                <td>{tool.invocations}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {/* Agent Card Preview */}
      <h3 className="proto-section-title">Agent Cards (A2A Discovery)</h3>
      {cards.length === 0 ? (
        <div className="proto-empty">No agent cards available — register agents to generate cards.</div>
      ) : (
        <>
          <div className="proto-card-grid">
            {cards.map((card) => (
              <div
                key={card.agent_name}
                className="proto-agent-card"
                onClick={() => setSelectedCard(selectedCard === card.agent_name ? null : card.agent_name)}
              >
                <div className="proto-agent-card-name">{card.agent_name}</div>
                <div className="proto-agent-card-row">
                  <span>Skills</span>
                  <span className="proto-agent-card-row-val">{card.skills_count}</span>
                </div>
                <div className="proto-agent-card-row">
                  <span>Auth</span>
                  <span className="proto-agent-card-row-val">{card.auth_scheme}</span>
                </div>
                <div className="proto-agent-card-row">
                  <span>Rate Limit</span>
                  <span className="proto-agent-card-row-val">{card.rate_limit_rpm} rpm</span>
                </div>
                <div className="proto-agent-card-row">
                  <span>URL</span>
                  <span className="proto-agent-card-row-val" style={{ fontSize: "0.7rem" }}>{card.url}</span>
                </div>
              </div>
            ))}
          </div>
          {selectedCardData && (
            <div className="proto-json-preview">
              {JSON.stringify(selectedCardData.card_json, null, 2)}
            </div>
          )}
        </>
      )}

      {/* Recent Protocol Requests */}
      <h3 className="proto-section-title">Recent Protocol Requests</h3>
      {requests.length === 0 ? (
        <div className="proto-empty">No protocol requests yet — start the gateway to begin receiving requests.</div>
      ) : (
        <table className="proto-table proto-requests-table">
          <thead>
            <tr>
              <th>Time</th>
              <th>Protocol</th>
              <th>Method</th>
              <th>Sender</th>
              <th>Agent</th>
              <th>Fuel</th>
              <th>Decision</th>
            </tr>
          </thead>
          <tbody>
            {requests.map((req) => (
              <tr key={req.id}>
                <td>{formatTime(req.timestamp)}</td>
                <td>{req.protocol}</td>
                <td>{req.method}</td>
                <td>{req.sender}</td>
                <td>{req.agent}</td>
                <td>{req.fuel_consumed}</td>
                <td>
                  <span className={`proto-decision-badge proto-decision-badge--${req.governance_decision}`}>
                    {req.governance_decision}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

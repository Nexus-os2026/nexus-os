import { useCallback, useEffect, useState } from "react";
import { getAgentCards, getMcpTools, getProtocolsRequests, getProtocolsStatus, hasDesktopRuntime } from "../api/backend";
import type { AgentCardSummary, McpTool, ProtocolRequest, ProtocolsStatus } from "../types";
import "./protocols.css";

const EMPTY_STATUS: ProtocolsStatus = {
  a2a_status: "stopped",
  a2a_version: "0.2.1",
  a2a_peers: 0,
  a2a_tasks_processed: 0,
  mcp_status: "stopped",
  mcp_registered_tools: 0,
  mcp_invocations: 0,
  gateway_port: null,
  governance_bridge_active: false,
  audit_integrity: true,
};

function formatTime(ts: number): string {
  const secs = Math.round((Date.now() - ts) / 1000);
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  return `${Math.floor(secs / 3600)}h ago`;
}

export default function Protocols(): JSX.Element {
  const [status, setStatus] = useState<ProtocolsStatus>(EMPTY_STATUS);
  const [tools, setTools] = useState<McpTool[]>([]);
  const [cards, setCards] = useState<AgentCardSummary[]>([]);
  const [requests, setRequests] = useState<ProtocolRequest[]>([]);
  const [selectedCard, setSelectedCard] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const loadData = useCallback(async () => {
    if (!hasDesktopRuntime()) {
      setLoading(false);
      return;
    }
    try {
      const [s, t, c, r] = await Promise.all([
        getProtocolsStatus(),
        getMcpTools(),
        getAgentCards(),
        getProtocolsRequests(),
      ]);
      setStatus(s);
      setTools(t);
      setCards(c);
      setRequests(r);
    } catch {
      // keep empty defaults
    }
    setLoading(false);
  }, []);

  useEffect(() => { void loadData(); }, [loadData]);

  const totalFuelConsumed = requests.reduce((sum, r) => sum + r.fuel_consumed, 0);
  const selectedCardData = cards.find((c) => c.agent_name === selectedCard);

  if (loading) {
    return (
      <section className="proto-hub">
        <header className="proto-header">
          <h2 className="proto-title">PROTOCOLS // A2A + MCP GATEWAY</h2>
          <p className="proto-subtitle">Loading protocol status...</p>
        </header>
      </section>
    );
  }

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
        <div className="proto-empty">No tools registered — start the gateway and register agents to populate the registry.</div>
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
        <div className="proto-empty">No agent cards available — register agents to generate A2A discovery cards.</div>
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
        <div className="proto-empty">No protocol requests yet — start the gateway to begin receiving A2A/MCP requests.</div>
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

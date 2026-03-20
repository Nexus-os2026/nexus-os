import { useCallback, useEffect, useState } from "react";
import {
  getAgentCards,
  getMcpTools,
  getProtocolsRequests,
  getProtocolsStatus,
  hasDesktopRuntime,
  mcpHostAddServer,
  mcpHostRemoveServer,
  mcpHostListServers,
  mcpHostConnect,
  mcpHostDisconnect,
  mcpHostListTools,
  mcpHostCallTool,
  a2aDiscoverAgent,
  a2aSendTask,
  a2aGetTaskStatus,
  a2aCancelTask,
  a2aKnownAgents,
} from "../api/backend";
import type { AgentCardSummary, McpTool, ProtocolRequest, ProtocolsStatus } from "../types";
import "./protocols.css";

interface McpServer {
  id: string;
  name: string;
  url: string;
  transport: string;
  connected: boolean;
}

interface McpHostTool {
  name: string;
  description: string;
  server_id?: string;
}

const EMPTY_STATUS: ProtocolsStatus = {
  a2a_status: "stopped",
  a2a_version: "",
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

  // MCP Host state
  const [mcpServers, setMcpServers] = useState<McpServer[]>([]);
  const [mcpHostTools, setMcpHostTools] = useState<McpHostTool[]>([]);
  const [mcpError, setMcpError] = useState<string | null>(null);
  const [mcpBusy, setMcpBusy] = useState<string | null>(null); // describes in-progress action

  // Add Server form
  const [addName, setAddName] = useState("");
  const [addUrl, setAddUrl] = useState("");
  const [addTransport, setAddTransport] = useState("http");
  const [addAuthToken, setAddAuthToken] = useState("");

  // Call Tool form
  const [callToolName, setCallToolName] = useState("");
  const [callToolArgs, setCallToolArgs] = useState("{}");
  const [callToolResult, setCallToolResult] = useState<string | null>(null);

  // A2A Client state
  const [a2aDiscoverUrl, setA2aDiscoverUrl] = useState("");
  const [a2aDiscoverResult, setA2aDiscoverResult] = useState<string | null>(null);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [a2aKnownList, setA2aKnownList] = useState<any[]>([]);
  const [a2aSendUrl, setA2aSendUrl] = useState("");
  const [a2aSendMessage, setA2aSendMessage] = useState("");
  const [a2aSendResult, setA2aSendResult] = useState<string | null>(null);
  const [a2aStatusUrl, setA2aStatusUrl] = useState("");
  const [a2aStatusTaskId, setA2aStatusTaskId] = useState("");
  const [a2aStatusResult, setA2aStatusResult] = useState<string | null>(null);
  const [a2aBusy, setA2aBusy] = useState<string | null>(null);
  const [a2aError, setA2aError] = useState<string | null>(null);

  const refreshMcpHost = useCallback(async () => {
    if (!hasDesktopRuntime()) return;
    try {
      const [serversRaw, toolsRaw] = await Promise.all([
        mcpHostListServers(),
        mcpHostListTools(),
      ]);
      const parsedServers: McpServer[] = (() => {
        try { return JSON.parse(serversRaw); } catch { return []; }
      })();
      const parsedTools: McpHostTool[] = (() => {
        try { return JSON.parse(toolsRaw); } catch { return []; }
      })();
      setMcpServers(parsedServers);
      setMcpHostTools(parsedTools);
    } catch (err) {
      setMcpError(String(err));
    }
  }, []);

  const handleAddServer = async () => {
    if (!addName.trim() || !addUrl.trim()) return;
    setMcpError(null);
    setMcpBusy("Adding server...");
    try {
      await mcpHostAddServer(addName.trim(), addUrl.trim(), addTransport, addAuthToken.trim() || undefined);
      setAddName("");
      setAddUrl("");
      setAddAuthToken("");
      await refreshMcpHost();
    } catch (err) {
      setMcpError(`Add server failed: ${err}`);
    }
    setMcpBusy(null);
  };

  const handleRemoveServer = async (serverId: string) => {
    setMcpError(null);
    setMcpBusy(`Removing ${serverId}...`);
    try {
      await mcpHostRemoveServer(serverId);
      await refreshMcpHost();
    } catch (err) {
      setMcpError(`Remove failed: ${err}`);
    }
    setMcpBusy(null);
  };

  const handleConnect = async (serverId: string) => {
    setMcpError(null);
    setMcpBusy(`Connecting ${serverId}...`);
    try {
      await mcpHostConnect(serverId);
      await refreshMcpHost();
    } catch (err) {
      setMcpError(`Connect failed: ${err}`);
    }
    setMcpBusy(null);
  };

  const handleDisconnect = async (serverId: string) => {
    setMcpError(null);
    setMcpBusy(`Disconnecting ${serverId}...`);
    try {
      await mcpHostDisconnect(serverId);
      await refreshMcpHost();
    } catch (err) {
      setMcpError(`Disconnect failed: ${err}`);
    }
    setMcpBusy(null);
  };

  const handleCallTool = async () => {
    if (!callToolName.trim()) return;
    setMcpError(null);
    setCallToolResult(null);
    setMcpBusy(`Calling tool ${callToolName}...`);
    try {
      const result = await mcpHostCallTool(callToolName.trim(), callToolArgs);
      setCallToolResult(result);
    } catch (err) {
      setMcpError(`Tool call failed: ${err}`);
    }
    setMcpBusy(null);
  };

  const refreshA2aKnown = useCallback(async () => {
    if (!hasDesktopRuntime()) return;
    try {
      const agents = await a2aKnownAgents();
      setA2aKnownList(Array.isArray(agents) ? agents : []);
    } catch { /* ignore */ }
  }, []);

  const handleA2aDiscover = async () => {
    if (!a2aDiscoverUrl.trim()) return;
    setA2aError(null);
    setA2aDiscoverResult(null);
    setA2aBusy("Discovering agent...");
    try {
      const card = await a2aDiscoverAgent(a2aDiscoverUrl.trim());
      setA2aDiscoverResult(JSON.stringify(card, null, 2));
      await refreshA2aKnown();
    } catch (err) {
      setA2aError(`Discovery failed: ${err}`);
    }
    setA2aBusy(null);
  };

  const handleA2aSendTask = async () => {
    if (!a2aSendUrl.trim() || !a2aSendMessage.trim()) return;
    setA2aError(null);
    setA2aSendResult(null);
    setA2aBusy("Sending task...");
    try {
      const result = await a2aSendTask(a2aSendUrl.trim(), a2aSendMessage.trim());
      setA2aSendResult(JSON.stringify(result, null, 2));
    } catch (err) {
      setA2aError(`Send task failed: ${err}`);
    }
    setA2aBusy(null);
  };

  const handleA2aGetStatus = async () => {
    if (!a2aStatusUrl.trim() || !a2aStatusTaskId.trim()) return;
    setA2aError(null);
    setA2aStatusResult(null);
    setA2aBusy("Checking status...");
    try {
      const result = await a2aGetTaskStatus(a2aStatusUrl.trim(), a2aStatusTaskId.trim());
      setA2aStatusResult(JSON.stringify(result, null, 2));
    } catch (err) {
      setA2aError(`Status check failed: ${err}`);
    }
    setA2aBusy(null);
  };

  const handleA2aCancelTask = async () => {
    if (!a2aStatusUrl.trim() || !a2aStatusTaskId.trim()) return;
    setA2aError(null);
    setA2aBusy("Canceling task...");
    try {
      await a2aCancelTask(a2aStatusUrl.trim(), a2aStatusTaskId.trim());
      setA2aStatusResult("Task canceled.");
    } catch (err) {
      setA2aError(`Cancel failed: ${err}`);
    }
    setA2aBusy(null);
  };

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

  useEffect(() => { void loadData(); void refreshMcpHost(); void refreshA2aKnown(); }, [loadData, refreshMcpHost, refreshA2aKnown]);

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
        <p className="proto-subtitle">
          {hasDesktopRuntime()
            ? "External protocol integration with full governance enforcement"
            : "Not configured"}
        </p>
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
            <span className="proto-server-detail-value">{status.a2a_version || "Not configured"}</span>
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
            <span className="proto-server-detail-value">
              {status.governance_bridge_active ? "enabled" : "Not configured"}
            </span>
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

      {/* ====== A2A Client ====== */}
      <h3 className="proto-section-title">A2A Client (Outbound)</h3>

      {a2aError && <div className="proto-mcp-error">{a2aError}</div>}
      {a2aBusy && <div className="proto-mcp-busy">{a2aBusy}</div>}

      {/* Discover Agent form */}
      <h4 style={{ color: "rgba(165, 243, 252, 0.7)", margin: "1rem 0 0.5rem" }}>Discover Agent</h4>
      <div className="proto-mcp-form">
        <input
          className="proto-mcp-input proto-mcp-input--wide"
          type="text"
          placeholder="Agent base URL (e.g. http://localhost:9000)"
          value={a2aDiscoverUrl}
          onChange={(e) => setA2aDiscoverUrl(e.target.value)}
        />
        <button
          className="proto-mcp-btn proto-mcp-btn--add"
          disabled={!!a2aBusy || !a2aDiscoverUrl.trim()}
          onClick={() => void handleA2aDiscover()}
        >
          Discover
        </button>
      </div>
      {a2aDiscoverResult && <div className="proto-json-preview">{a2aDiscoverResult}</div>}

      {/* Known Agents */}
      <h4 style={{ color: "rgba(165, 243, 252, 0.7)", margin: "1rem 0 0.5rem" }}>Known Agents ({a2aKnownList.length})</h4>
      {a2aKnownList.length === 0 ? (
        <div className="proto-empty">No external agents discovered yet — use Discover above.</div>
      ) : (
        <div className="proto-card-grid">
          {a2aKnownList.map((agent) => (
            <div key={agent.name} className="proto-agent-card">
              <div className="proto-agent-card-name">{agent.name}</div>
              <div className="proto-agent-card-row">
                <span>URL</span>
                <span className="proto-agent-card-row-val" style={{ fontSize: "0.7rem" }}>{agent.url}</span>
              </div>
              <div className="proto-agent-card-row">
                <span>Skills</span>
                <span className="proto-agent-card-row-val">{agent.skills?.length ?? 0}</span>
              </div>
              <div className="proto-agent-card-row">
                <span>Version</span>
                <span className="proto-agent-card-row-val">{agent.version}</span>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Send Task form */}
      <h4 style={{ color: "rgba(165, 243, 252, 0.7)", margin: "1rem 0 0.5rem" }}>Send Task</h4>
      <div className="proto-mcp-form">
        <input
          className="proto-mcp-input proto-mcp-input--wide"
          type="text"
          placeholder="Agent URL"
          value={a2aSendUrl}
          onChange={(e) => setA2aSendUrl(e.target.value)}
        />
        <textarea
          className="proto-mcp-textarea"
          placeholder="Task message..."
          value={a2aSendMessage}
          onChange={(e) => setA2aSendMessage(e.target.value)}
          rows={2}
        />
        <button
          className="proto-mcp-btn proto-mcp-btn--add"
          disabled={!!a2aBusy || !a2aSendUrl.trim() || !a2aSendMessage.trim()}
          onClick={() => void handleA2aSendTask()}
        >
          Send Task
        </button>
      </div>
      {a2aSendResult && <div className="proto-json-preview">{a2aSendResult}</div>}

      {/* Task Status / Cancel */}
      <h4 style={{ color: "rgba(165, 243, 252, 0.7)", margin: "1rem 0 0.5rem" }}>Task Status / Cancel</h4>
      <div className="proto-mcp-form">
        <input
          className="proto-mcp-input proto-mcp-input--wide"
          type="text"
          placeholder="Agent URL"
          value={a2aStatusUrl}
          onChange={(e) => setA2aStatusUrl(e.target.value)}
        />
        <input
          className="proto-mcp-input proto-mcp-input--wide"
          type="text"
          placeholder="Task ID"
          value={a2aStatusTaskId}
          onChange={(e) => setA2aStatusTaskId(e.target.value)}
        />
        <button
          className="proto-mcp-btn proto-mcp-btn--connect"
          disabled={!!a2aBusy || !a2aStatusUrl.trim() || !a2aStatusTaskId.trim()}
          onClick={() => void handleA2aGetStatus()}
        >
          Get Status
        </button>
        <button
          className="proto-mcp-btn proto-mcp-btn--remove"
          disabled={!!a2aBusy || !a2aStatusUrl.trim() || !a2aStatusTaskId.trim()}
          onClick={() => void handleA2aCancelTask()}
        >
          Cancel Task
        </button>
      </div>
      {a2aStatusResult && <div className="proto-json-preview">{a2aStatusResult}</div>}

      {/* ====== MCP Host Servers ====== */}
      <h3 className="proto-section-title">MCP Servers (Host)</h3>

      {mcpError && (
        <div className="proto-mcp-error">{mcpError}</div>
      )}
      {mcpBusy && (
        <div className="proto-mcp-busy">{mcpBusy}</div>
      )}

      {/* Add Server form */}
      <div className="proto-mcp-form">
        <input
          className="proto-mcp-input"
          type="text"
          placeholder="Server name"
          value={addName}
          onChange={(e) => setAddName(e.target.value)}
        />
        <input
          className="proto-mcp-input proto-mcp-input--wide"
          type="text"
          placeholder="URL (e.g. http://localhost:8080)"
          value={addUrl}
          onChange={(e) => setAddUrl(e.target.value)}
        />
        <select
          className="proto-mcp-select"
          value={addTransport}
          onChange={(e) => setAddTransport(e.target.value)}
        >
          <option value="http">HTTP</option>
          <option value="sse">SSE</option>
          <option value="stdio">Stdio</option>
        </select>
        <input
          className="proto-mcp-input"
          type="password"
          placeholder="Auth token (optional)"
          value={addAuthToken}
          onChange={(e) => setAddAuthToken(e.target.value)}
        />
        <button
          className="proto-mcp-btn proto-mcp-btn--add"
          disabled={!!mcpBusy || !addName.trim() || !addUrl.trim()}
          onClick={() => void handleAddServer()}
        >
          Add Server
        </button>
      </div>

      {/* Server list */}
      {mcpServers.length === 0 ? (
        <div className="proto-empty">No MCP host servers configured — add a server above.</div>
      ) : (
        <table className="proto-table">
          <thead>
            <tr>
              <th>ID</th>
              <th>Name</th>
              <th>URL</th>
              <th>Transport</th>
              <th>Status</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {mcpServers.map((srv) => (
              <tr key={srv.id}>
                <td style={{ fontSize: "0.7rem" }}>{srv.id}</td>
                <td>{srv.name}</td>
                <td style={{ fontSize: "0.72rem" }}>{srv.url}</td>
                <td>{srv.transport}</td>
                <td>
                  <span className={`proto-status-badge proto-status-badge--${srv.connected ? "running" : "stopped"}`}>
                    {srv.connected ? "connected" : "disconnected"}
                  </span>
                </td>
                <td className="proto-mcp-actions">
                  {srv.connected ? (
                    <button
                      className="proto-mcp-btn proto-mcp-btn--disconnect"
                      disabled={!!mcpBusy}
                      onClick={() => void handleDisconnect(srv.id)}
                    >
                      Disconnect
                    </button>
                  ) : (
                    <button
                      className="proto-mcp-btn proto-mcp-btn--connect"
                      disabled={!!mcpBusy}
                      onClick={() => void handleConnect(srv.id)}
                    >
                      Connect
                    </button>
                  )}
                  <button
                    className="proto-mcp-btn proto-mcp-btn--remove"
                    disabled={!!mcpBusy}
                    onClick={() => void handleRemoveServer(srv.id)}
                  >
                    Remove
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {/* MCP Host Tools */}
      <h3 className="proto-section-title">MCP Host Tools</h3>
      {mcpHostTools.length === 0 ? (
        <div className="proto-empty">No tools available from connected MCP servers.</div>
      ) : (
        <table className="proto-table">
          <thead>
            <tr>
              <th>Tool</th>
              <th>Description</th>
              <th>Server</th>
            </tr>
          </thead>
          <tbody>
            {mcpHostTools.map((t) => (
              <tr key={t.name}>
                <td>{t.name}</td>
                <td style={{ color: "rgba(165, 243, 252, 0.55)" }}>{t.description}</td>
                <td>{t.server_id ?? "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {/* Call Tool form */}
      <h3 className="proto-section-title">Call MCP Tool</h3>
      <div className="proto-mcp-form">
        <input
          className="proto-mcp-input"
          type="text"
          placeholder="Tool name"
          value={callToolName}
          onChange={(e) => setCallToolName(e.target.value)}
        />
        <textarea
          className="proto-mcp-textarea"
          placeholder='Arguments JSON, e.g. {"key": "value"}'
          value={callToolArgs}
          onChange={(e) => setCallToolArgs(e.target.value)}
          rows={3}
        />
        <button
          className="proto-mcp-btn proto-mcp-btn--add"
          disabled={!!mcpBusy || !callToolName.trim()}
          onClick={() => void handleCallTool()}
        >
          Call Tool
        </button>
      </div>
      {callToolResult !== null && (
        <div className="proto-json-preview">{callToolResult}</div>
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

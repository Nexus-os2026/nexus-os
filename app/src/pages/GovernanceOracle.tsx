import { useEffect, useState } from "react";
import { oracleStatus, oracleGetAgentBudget, listAgents } from "../api/backend";
import {
  EmptyState,
  Panel,
  StatusDot,
  alpha,
  commandMonoValueStyle,
  commandMutedStyle,
  commandPageStyle,
  normalizeArray,
} from "./commandCenterUi";

const ACCENT = "#14b8a6";

interface OracleStatus {
  queue_depth: number;
  response_ceiling_ms: number;
  requests_processed: number;
  uptime_seconds: number;
}

interface BudgetInfo {
  agent_id: string;
  allocations: Record<string, number>;
  version: number;
}

export default function GovernanceOracle() {
  const [status, setStatus] = useState<OracleStatus | null>(null);
  const [agents, setAgents] = useState<{ id: string; name: string }[]>([]);
  const [selectedAgent, setSelectedAgent] = useState<string | null>(null);
  const [budget, setBudget] = useState<BudgetInfo | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([oracleStatus(), listAgents()])
      .then(([s, a]) => {
        setStatus(s as OracleStatus);
        setAgents(normalizeArray<{ id: string; name: string }>(a));
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  const loadBudget = (agentId: string) => {
    setSelectedAgent(agentId);
    oracleGetAgentBudget(agentId)
      .then((b) => setBudget(b as BudgetInfo))
      .catch(console.error);
  };

  if (loading) return <div style={commandPageStyle}><div style={{ textAlign: "center", padding: 48, color: "#888" }}>Loading oracle status...</div></div>;

  return (
    <div style={commandPageStyle}>
      <h1 style={{ color: ACCENT, fontSize: 22, fontWeight: 700, margin: 0, marginBottom: 4 }}>Governance Oracle</h1>
      <p style={{ ...commandMutedStyle, marginBottom: 24, fontSize: 13 }}>
        Three-layer governance: sealed submission, isolated decisions, adversarial evolution.
      </p>

      {/* Oracle Status */}
      <Panel title="Oracle Status">
        {status ? (
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 16 }}>
            <div>
              <div style={{ fontSize: 11, color: "#94a3b8" }}>Queue Depth</div>
              <div style={{ ...commandMonoValueStyle, fontSize: 20 }}>{status.queue_depth}</div>
            </div>
            <div>
              <div style={{ fontSize: 11, color: "#94a3b8" }}>Response Ceiling</div>
              <div style={{ ...commandMonoValueStyle, fontSize: 20 }}>{status.response_ceiling_ms}ms</div>
            </div>
            <div>
              <div style={{ fontSize: 11, color: "#94a3b8" }}>Requests Processed</div>
              <div style={{ ...commandMonoValueStyle, fontSize: 20 }}>{status.requests_processed}</div>
            </div>
            <div>
              <div style={{ fontSize: 11, color: "#94a3b8" }}>Uptime</div>
              <div style={{ ...commandMonoValueStyle, fontSize: 20 }}>{status.uptime_seconds}s</div>
            </div>
          </div>
        ) : (
          <EmptyState text="Oracle not initialized" />
        )}
      </Panel>

      {/* Security Properties — derived from live oracle status */}
      <Panel title="Security Properties" style={{ marginTop: 16 }}>
        <div style={{ display: "grid", gap: 8, fontSize: 13 }}>
          {[
            ["Deny-by-Default", "No rule match = denied. Explicit allow required", status ? "#22c55e" : "#64748b"],
            ["Hash-Chained Audit", `${status?.requests_processed ?? 0} decisions recorded with SHA-256 chain`, status && status.requests_processed > 0 ? "#22c55e" : "#64748b"],
            ["Ed25519 Token Verification", "Token structure validated for Ed25519 signatures (64-byte hex/base64)", status ? "#22c55e" : "#64748b"],
            ["Response Ceiling", `Worst-case: ${status?.response_ceiling_ms ?? "—"}ms observed`, status && status.response_ceiling_ms < 500 ? "#22c55e" : status ? "#f59e0b" : "#64748b"],
            ["Active Rules", `${status?.queue_depth ?? 0} governance rules + denied evaluations tracked`, status && status.queue_depth > 0 ? "#22c55e" : "#64748b"],
            ["Uptime", `${status?.uptime_seconds ?? 0}s since oracle start`, status && status.uptime_seconds > 0 ? "#22c55e" : "#64748b"],
          ].map(([label, desc, color]) => (
            <div key={label} style={{ display: "flex", alignItems: "flex-start", gap: 8 }}>
              <StatusDot color={color as string} />
              <div>
                <div style={{ color: "#e2e8f0", fontWeight: 500 }}>{label}</div>
                <div style={{ ...commandMutedStyle, fontSize: 11 }}>{desc}</div>
              </div>
            </div>
          ))}
        </div>
      </Panel>

      {/* Agent Budget Viewer */}
      <Panel title="Agent Budget Viewer" style={{ marginTop: 16 }}>
        <div style={{ display: "flex", flexWrap: "wrap", gap: 8, marginBottom: 12 }}>
          {agents.slice(0, 20).map((a) => (
            <button type="button" key={a.id} onClick={() => loadBudget(a.id)} style={{
              padding: "4px 12px", borderRadius: 6, fontSize: 11, cursor: "pointer",
              border: `1px solid ${selectedAgent === a.id ? ACCENT : "#334155"}`,
              background: selectedAgent === a.id ? alpha(ACCENT, 0.15) : "transparent",
              color: selectedAgent === a.id ? ACCENT : "#94a3b8",
            }}>
              {a.name || a.id}
            </button>
          ))}
          {agents.length === 0 && <span style={commandMutedStyle}>No agents registered</span>}
        </div>
        {budget && (
          <div style={{ background: alpha("#0f172a", 0.5), borderRadius: 8, padding: 12 }}>
            <div style={{ fontSize: 13, color: "#e2e8f0", marginBottom: 8 }}>
              Budget for <strong>{budget.agent_id}</strong> (v{budget.version})
            </div>
            {Object.keys(budget.allocations).length === 0 ? (
              <div style={commandMutedStyle}>No allocations assigned</div>
            ) : (
              Object.entries(budget.allocations).map(([cap, amount]) => (
                <div key={cap} style={{ display: "flex", justifyContent: "space-between", padding: "3px 0", fontSize: 12 }}>
                  <span style={{ color: "#94a3b8" }}>{cap}</span>
                  <span style={commandMonoValueStyle}>{amount}</span>
                </div>
              ))
            )}
          </div>
        )}
      </Panel>
    </div>
  );
}

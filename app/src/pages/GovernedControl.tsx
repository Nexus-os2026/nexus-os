import { useEffect, useState } from "react";
import {
  ccGetActionHistory,
  ccGetCapabilityBudget,
  ccGetScreenContext,
  ccVerifyActionSequence,
  ccExecuteAction,
  listAgents,
} from "../api/backend";
import {
  alpha,
  commandMutedStyle,
  commandPageStyle,
  normalizeArray,
} from "./commandCenterUi";

const ACCENT = "#06b6d4";
const GREEN = "#22c55e";
const RED = "#ef4444";
const YELLOW = "#eab308";
const BLUE = "#3b82f6";

interface ActionEntry {
  entry_id: string;
  timestamp: number;
  agent_id: string;
  action_label: string;
  success: boolean;
  error: string | null;
  token_cost: number;
  balance_after: number;
}

interface Budget {
  agent_id: string;
  balance: number;
  total_spent: number;
  actions_executed: number;
  actions_denied: number;
}

interface ScreenCtx {
  last_screenshot_time: number | null;
  actions_this_session: number;
  recent_actions: string[];
}

interface Verification {
  valid: boolean;
  chain_verified: boolean;
  sequence_verified: boolean;
  total_entries: number;
  error: string | null;
}

const cardStyle: React.CSSProperties = {
  background: alpha("#1e1e2e", 0.7),
  borderRadius: 10,
  padding: 16,
  border: "1px solid " + alpha("#ffffff", 0.08),
};

const labelStyle: React.CSSProperties = {
  fontSize: 11,
  color: "#888",
  textTransform: "uppercase" as const,
  letterSpacing: 1,
  marginBottom: 4,
};

export default function GovernedControl() {
  const [agents, setAgents] = useState<{ id: string; name: string }[]>([]);
  const [selectedAgent, setSelectedAgent] = useState<string>("");
  const [history, setHistory] = useState<ActionEntry[]>([]);
  const [budget, setBudget] = useState<Budget | null>(null);
  const [screenCtx, setScreenCtx] = useState<ScreenCtx | null>(null);
  const [verification, setVerification] = useState<Verification | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionJson, setActionJson] = useState('{"type":"TerminalCommand","command":"echo hello","working_dir":""}');
  const [executeResult, setExecuteResult] = useState<any>(null);

  useEffect(() => {
    listAgents()
      .then((a) => {
        const list = normalizeArray<{ id: string; name: string }>(a);
        setAgents(list);
        if (list.length > 0) setSelectedAgent(list[0].id);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    if (!selectedAgent) return;
    Promise.all([
      ccGetActionHistory(selectedAgent).catch((e) => { setError(String(e)); return []; }),
      ccGetCapabilityBudget(selectedAgent).catch((e) => { setError(String(e)); return null; }),
      ccGetScreenContext(selectedAgent).catch((e) => { setError(String(e)); return null; }),
      ccVerifyActionSequence(selectedAgent).catch((e) => { setError(String(e)); return null; }),
    ]).then(([h, b, s, v]) => {
      setHistory(Array.isArray(h) ? h : []);
      setBudget(b as Budget | null);
      setScreenCtx(s as ScreenCtx | null);
      setVerification(v as Verification | null);
    });
  }, [selectedAgent]);

  if (loading) {
    return (
      <div style={commandPageStyle}>
        <div style={{ textAlign: "center", padding: 48, color: "#888" }}>Loading...</div>
      </div>
    );
  }

  return (
    <div style={commandPageStyle}>
      <h1 style={{ color: ACCENT, fontSize: 22, fontWeight: 700, margin: 0, marginBottom: 4 }}>
        Governed Computer Control
      </h1>
      <p style={{ ...commandMutedStyle, marginBottom: 16, fontSize: 13 }}>
        Desktop automation with governance gates, token economy, and hash-chained audit trail.
      </p>

      {/* Agent selector */}
      <div style={{ marginBottom: 20 }}>
        <select
          value={selectedAgent}
          onChange={(e) => setSelectedAgent(e.target.value)}
          style={{
            background: alpha("#ffffff", 0.05),
            border: "1px solid " + alpha("#ffffff", 0.1),
            color: "#ddd",
            borderRadius: 6,
            padding: "8px 12px",
            fontSize: 13,
            minWidth: 250,
          }}
        >
          {agents.map((a) => (
            <option key={a.id} value={a.id}>{a.name || a.id}</option>
          ))}
        </select>
      </div>

      {/* Dashboard grid */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr 1fr", gap: 12, marginBottom: 24 }}>
        <div style={cardStyle}>
          <div style={labelStyle}>Balance</div>
          <div style={{ fontSize: 20, fontWeight: 700, color: ACCENT, fontFamily: "monospace" }}>
            {budget ? budget.balance.toFixed(2) : "0.00"} NXC
          </div>
        </div>
        <div style={cardStyle}>
          <div style={labelStyle}>Actions Executed</div>
          <div style={{ fontSize: 20, fontWeight: 700, color: GREEN, fontFamily: "monospace" }}>
            {budget?.actions_executed ?? 0}
          </div>
        </div>
        <div style={cardStyle}>
          <div style={labelStyle}>Actions Denied</div>
          <div style={{ fontSize: 20, fontWeight: 700, color: RED, fontFamily: "monospace" }}>
            {budget?.actions_denied ?? 0}
          </div>
        </div>
        <div style={cardStyle}>
          <div style={labelStyle}>Audit Integrity</div>
          <div style={{ fontSize: 16, fontWeight: 600, color: verification?.valid ? GREEN : RED }}>
            {verification ? (verification.valid ? "Verified" : "BROKEN") : "N/A"}
          </div>
          {verification && (
            <div style={{ fontSize: 11, color: "#666", marginTop: 2 }}>
              {verification.total_entries} entries
            </div>
          )}
        </div>
      </div>

      {/* Screen context */}
      {screenCtx && screenCtx.recent_actions.length > 0 && (
        <div style={{ ...cardStyle, marginBottom: 20 }}>
          <h3 style={{ color: "#ccc", fontSize: 14, fontWeight: 600, margin: "0 0 8px" }}>Recent Actions</h3>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
            {screenCtx.recent_actions.map((a, i) => (
              <span
                key={i}
                style={{
                  background: alpha(BLUE, 0.15),
                  border: "1px solid " + alpha(BLUE, 0.2),
                  color: BLUE,
                  borderRadius: 4,
                  padding: "2px 8px",
                  fontSize: 11,
                  fontFamily: "monospace",
                }}
              >
                {a}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Error display */}
      {error && (
        <div style={{ color: "#ef4444", background: alpha("#ef4444", 0.1), padding: "8px 12px", borderRadius: 6, marginBottom: 12, fontSize: 13 }}>
          {error} <button onClick={() => setError(null)} style={{ marginLeft: 8, background: "none", border: "none", color: "#ef4444", cursor: "pointer" }}>Dismiss</button>
        </div>
      )}

      {/* Execute Action */}
      {selectedAgent && (
        <div style={{ ...cardStyle, marginBottom: 16 }}>
          <div style={labelStyle}>Execute Action</div>
          <textarea value={actionJson} onChange={(e) => setActionJson(e.target.value)} rows={3} style={{ width: "100%", padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444", fontFamily: "monospace", fontSize: 12, boxSizing: "border-box", marginTop: 8, marginBottom: 8, resize: "vertical" }} />
          <button onClick={() => {
            setError(null);
            ccExecuteAction(selectedAgent, 4, ["computer_control"], actionJson)
              .then((r) => { setExecuteResult(r); /* refresh history */ ccGetActionHistory(selectedAgent).then((h) => setHistory(Array.isArray(h) ? h : [])).catch((e) => { if (import.meta.env.DEV) console.warn("[GovernedControl]", e); }); })
              .catch((e) => setError(String(e)));
          }} style={{ padding: "6px 14px", borderRadius: 6, border: "none", cursor: "pointer", fontWeight: 600, fontSize: 12, background: "#22c55e", color: "#000" }}>
            Execute
          </button>
          {executeResult && (
            <pre style={{ fontSize: 11, color: "#22c55e", background: alpha("#000", 0.3), padding: 8, borderRadius: 6, marginTop: 8, whiteSpace: "pre-wrap" }}>{JSON.stringify(executeResult, null, 2)}</pre>
          )}
        </div>
      )}

      {/* Action history */}
      <h3 style={{ color: "#ccc", fontSize: 14, fontWeight: 600, marginBottom: 8 }}>Action History</h3>
      {history.length === 0 && (
        <div style={{ textAlign: "center", padding: 32, color: "#666" }}>
          No computer control actions recorded for this agent.
        </div>
      )}
      <div style={{ display: "grid", gap: 6 }}>
        {history.map((e) => (
          <div
            key={e.entry_id}
            style={{
              ...cardStyle,
              display: "grid",
              gridTemplateColumns: "100px 1fr 80px 80px 60px",
              alignItems: "center",
              gap: 8,
              padding: "8px 14px",
            }}
          >
            <div style={{ fontSize: 11, color: "#666", fontFamily: "monospace" }}>
              {new Date(e.timestamp * 1000).toLocaleTimeString()}
            </div>
            <div style={{ fontSize: 12, color: "#aaa", fontFamily: "monospace" }}>{e.action_label}</div>
            <div
              style={{
                fontSize: 13,
                fontWeight: 600,
                fontFamily: "monospace",
                color: RED,
                textAlign: "right",
              }}
            >
              -{e.token_cost.toFixed(1)}
            </div>
            <div style={{ fontSize: 12, color: "#888", fontFamily: "monospace", textAlign: "right" }}>
              {e.balance_after.toFixed(1)}
            </div>
            <div
              style={{
                fontSize: 10,
                fontWeight: 600,
                textAlign: "center",
                color: e.success ? GREEN : RED,
                background: e.success ? alpha(GREEN, 0.1) : alpha(RED, 0.1),
                borderRadius: 3,
                padding: "2px 4px",
              }}
            >
              {e.success ? "OK" : "FAIL"}
            </div>
          </div>
        ))}
      </div>

      {/* Governance rules */}
      <div style={{ ...cardStyle, marginTop: 24 }}>
        <h3 style={{ color: "#ccc", fontSize: 14, fontWeight: 600, margin: "0 0 8px" }}>Governance Rules</h3>
        <div style={{ fontSize: 12, color: "#aaa", lineHeight: 1.8 }}>
          <div><span style={{ color: GREEN, fontWeight: 600 }}>L3:</span> Screenshot, mouse move, read clipboard, wait</div>
          <div><span style={{ color: YELLOW, fontWeight: 600 }}>L4:</span> Mouse click, keyboard, write clipboard, open apps</div>
          <div><span style={{ color: RED, fontWeight: 600 }}>L5:</span> Terminal commands (allowlisted only)</div>
          <div style={{ marginTop: 6 }}><span style={{ color: ACCENT, fontWeight: 600 }}>Token costs:</span> Screenshot 1 | Click 2 | Type 1/char | Terminal 50 | Open App 10</div>
          <div><span style={{ color: ACCENT, fontWeight: 600 }}>Sandbox:</span> Terminal working dir must be within agent workspace</div>
        </div>
      </div>
    </div>
  );
}

import { useEffect, useState } from "react";
import { simGetHistory, simGetPolicy, simSubmit, simRun, simGetResult, simGetRisk, listAgents } from "../api/backend";
import {
  alpha,
  commandMutedStyle,
  commandPageStyle,
  normalizeArray,
} from "./commandCenterUi";

const ACCENT = "#a855f7";
const GREEN = "#22c55e";
const RED = "#ef4444";
const YELLOW = "#eab308";
const BLUE = "#3b82f6";

interface ScenarioSummary {
  id: string;
  agent_id: string;
  description: string;
  step_count: number;
  created_at: number;
  status: string;
}

interface Policy {
  min_autonomy_level: number;
  max_steps: number;
  max_concurrent_per_agent: number;
  allow_branching: boolean;
  cost_per_step: number;
  base_cost: number;
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

export default function WorldSimulation2() {
  const [agents, setAgents] = useState<{ id: string; name: string }[]>([]);
  const [selectedAgent, setSelectedAgent] = useState("");
  const [history, setHistory] = useState<ScenarioSummary[]>([]);
  const [policy, setPolicy] = useState<Policy | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [scenarioDesc, setScenarioDesc] = useState("");
  const [actionsJson, setActionsJson] = useState('[{"action":"deploy","target":"staging","risk":0.3}]');
  const [runningId, setRunningId] = useState<string | null>(null);
  const [simResult, setSimResult] = useState<any>(null);
  const [simRisk, setSimRisk] = useState<any>(null);

  useEffect(() => {
    Promise.all([
      listAgents().catch((e) => { setError(String(e)); return []; }),
      simGetPolicy().catch((e) => { setError(String(e)); return null; }),
    ])
      .then(([a, p]) => {
        const list = normalizeArray<{ id: string; name: string }>(a);
        setAgents(list);
        if (list.length > 0) setSelectedAgent(list[0].id);
        setPolicy(p as Policy | null);
      })
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    if (!selectedAgent) return;
    simGetHistory(selectedAgent)
      .then((h) => setHistory(Array.isArray(h) ? h : []))
      .catch((e) => { setHistory([]); setError(String(e)); });
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
        World Simulation Engine
      </h1>
      <p style={{ ...commandMutedStyle, marginBottom: 16, fontSize: 13 }}>
        Multi-step action scenario simulation with risk assessment and what-if branching.
      </p>

      {error && (
        <div style={{ color: "#ef4444", background: alpha("#ef4444", 0.1), padding: "8px 12px", borderRadius: 6, marginBottom: 12, fontSize: 13 }}>
          {error} <button onClick={() => setError(null)} style={{ marginLeft: 8, background: "none", border: "none", color: "#ef4444", cursor: "pointer" }}>Dismiss</button>
        </div>
      )}

      {/* Scenario Builder */}
      <div style={{ background: alpha("#1e1e2e", 0.7), borderRadius: 10, padding: 16, border: "1px solid " + alpha("#ffffff", 0.08), marginBottom: 16 }}>
        <div style={{ fontSize: 11, color: "#888", textTransform: "uppercase", letterSpacing: 1, marginBottom: 8 }}>Run Simulation</div>
        <div style={{ display: "flex", gap: 8, marginBottom: 8 }}>
          <input placeholder="Scenario description..." value={scenarioDesc} onChange={(e) => setScenarioDesc(e.target.value)} style={{ flex: 1, padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444" }} />
        </div>
        <textarea placeholder='Actions JSON array' value={actionsJson} onChange={(e) => setActionsJson(e.target.value)} rows={3} style={{ width: "100%", padding: 8, borderRadius: 6, background: "#2a2a3e", color: "#e0e0e0", border: "1px solid #444", fontFamily: "monospace", fontSize: 12, boxSizing: "border-box", resize: "vertical", marginBottom: 8 }} />
        <div style={{ display: "flex", gap: 8 }}>
          <button onClick={async () => {
            if (!selectedAgent || !scenarioDesc.trim()) return;
            setError(null);
            try {
              const id = await simSubmit(selectedAgent, scenarioDesc, actionsJson);
              setRunningId(id);
              const result = await simRun(id);
              setSimResult(result);
              const risk = await simGetRisk(id);
              setSimRisk(risk);
            } catch (e) {
              setError(String(e));
            }
          }} disabled={!selectedAgent || !scenarioDesc.trim()} style={{ padding: "8px 16px", borderRadius: 6, border: "none", cursor: "pointer", fontWeight: 600, fontSize: 12, background: selectedAgent && scenarioDesc.trim() ? ACCENT : "#444", color: "#fff" }}>
            Submit & Run
          </button>
          {simResult && <span style={{ color: GREEN, fontSize: 12, alignSelf: "center" }}>Simulation complete</span>}
        </div>
        {simResult && (
          <div style={{ marginTop: 12, padding: 10, background: alpha("#000", 0.3), borderRadius: 6, fontSize: 12 }}>
            <div style={{ color: "#888", marginBottom: 4 }}>Result:</div>
            <pre style={{ color: GREEN, whiteSpace: "pre-wrap", maxHeight: 200, overflow: "auto", margin: 0 }}>{JSON.stringify(simResult, null, 2)}</pre>
          </div>
        )}
        {simRisk && (
          <div style={{ marginTop: 8, padding: 10, background: alpha("#000", 0.3), borderRadius: 6, fontSize: 12 }}>
            <div style={{ color: "#888", marginBottom: 4 }}>Risk Assessment:</div>
            <pre style={{ color: YELLOW, whiteSpace: "pre-wrap", margin: 0 }}>{JSON.stringify(simRisk, null, 2)}</pre>
          </div>
        )}
      </div>

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

      {/* Policy overview */}
      {policy && (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 12, marginBottom: 24 }}>
          <div style={cardStyle}>
            <div style={labelStyle}>Min Level</div>
            <div style={{ fontSize: 18, fontWeight: 700, color: YELLOW }}>L{policy.min_autonomy_level}+</div>
          </div>
          <div style={cardStyle}>
            <div style={labelStyle}>Max Steps</div>
            <div style={{ fontSize: 18, fontWeight: 700, color: BLUE }}>{policy.max_steps}</div>
          </div>
          <div style={cardStyle}>
            <div style={labelStyle}>Base Cost</div>
            <div style={{ fontSize: 18, fontWeight: 700, color: ACCENT, fontFamily: "monospace" }}>
              {policy.base_cost.toFixed(1)} NXC
            </div>
          </div>
          <div style={cardStyle}>
            <div style={labelStyle}>Per Step</div>
            <div style={{ fontSize: 18, fontWeight: 700, color: ACCENT, fontFamily: "monospace" }}>
              {policy.cost_per_step.toFixed(1)} NXC
            </div>
          </div>
        </div>
      )}

      {/* Simulation history */}
      <h3 style={{ color: "#ccc", fontSize: 14, fontWeight: 600, marginBottom: 8 }}>Simulation History</h3>
      {history.length === 0 && (
        <div style={{ textAlign: "center", padding: 32, color: "#666" }}>
          No simulations run for this agent yet.
        </div>
      )}
      <div style={{ display: "grid", gap: 8 }}>
        {history.map((s) => {
          const statusColor = s.status === "Completed" ? GREEN : s.status.startsWith("Failed") ? RED : YELLOW;
          return (
            <div key={s.id} style={{ ...cardStyle, display: "grid", gridTemplateColumns: "1fr 80px 80px 100px", alignItems: "center", gap: 12 }}>
              <div>
                <div style={{ fontSize: 13, fontWeight: 600, color: "#ddd" }}>{s.description}</div>
                <div style={{ fontSize: 11, color: "#666" }}>{s.id.slice(0, 12)}...</div>
              </div>
              <div style={{ fontSize: 12, color: "#888", textAlign: "center" }}>{s.step_count} steps</div>
              <div style={{ fontSize: 12, color: "#888", textAlign: "center" }}>
                {new Date(s.created_at * 1000).toLocaleDateString()}
              </div>
              <div style={{ fontSize: 11, fontWeight: 600, color: statusColor, textAlign: "right" }}>
                {s.status}
              </div>
            </div>
          );
        })}
      </div>

      {/* Risk levels legend */}
      <div style={{ ...cardStyle, marginTop: 24 }}>
        <h3 style={{ color: "#ccc", fontSize: 14, fontWeight: 600, margin: "0 0 8px" }}>Risk Assessment Guide</h3>
        <div style={{ fontSize: 12, color: "#aaa", lineHeight: 1.8 }}>
          <div><span style={{ color: GREEN, fontWeight: 600 }}>Low:</span> Read-only or fully reversible — proceed with confidence</div>
          <div><span style={{ color: YELLOW, fontWeight: 600 }}>Medium:</span> State changes but recoverable — monitor during execution</div>
          <div><span style={{ color: RED, fontWeight: 600 }}>High:</span> Destructive or service-disrupting — create backups first</div>
          <div style={{ marginTop: 6 }}><span style={{ color: ACCENT, fontWeight: 600 }}>Branching:</span> Create what-if alternatives to compare outcomes before deciding</div>
        </div>
      </div>
    </div>
  );
}

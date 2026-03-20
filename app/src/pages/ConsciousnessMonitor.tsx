import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  getAgentConsciousness,
  getConsciousnessHeatmap,
  getConsciousnessHistory,
  getUserBehaviorState,
  reportUserKeystroke,
} from "../api/backend";

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface ConsciousnessState {
  agent_id: string;
  confidence: number;
  fatigue: number;
  frustration: number;
  curiosity: number;
  flow_state: boolean;
  needs_handoff: boolean;
  should_escalate: boolean;
  exploration_mode: boolean;
}

interface ConsciousnessSnapshot {
  timestamp: number;
  confidence: number;
  fatigue: number;
  frustration: number;
  curiosity: number;
}

interface UserBehavior {
  typing_speed_wpm: number;
  baseline_wpm: number;
  deletion_rate: number;
  inferred_mood: string;
  mood_confidence: number;
  response_adaptation: string;
}

interface AgentEntry {
  id: string;
  name: string;
  status: string;
  autonomy_level?: number;
}

/* ================================================================== */
/*  Helpers                                                            */
/* ================================================================== */

function stateBarColor(value: number, inverted: boolean): string {
  const v = inverted ? value : 1 - value;
  if (v > 0.7) return "#ef4444";
  if (v > 0.4) return "#eab308";
  return "#22c55e";
}

function moodColor(mood: string): string {
  switch (mood.toLowerCase()) {
    case "focused": return "#22d3ee";
    case "relaxed": return "#22c55e";
    case "frustrated": return "#f97316";
    case "fatigued": return "#ef4444";
    case "exploring": return "#a78bfa";
    default: return "#94a3b8";
  }
}

/* ================================================================== */
/*  SVG Line Chart                                                     */
/* ================================================================== */

function HistoryChart({ history }: { history: ConsciousnessSnapshot[] }): JSX.Element {
  if (history.length < 2) {
    return <div style={{ color: "#64748b", fontSize: "0.82rem" }}>Waiting for data...</div>;
  }

  const W = 560;
  const H = 160;
  const PAD = 30;
  const plotW = W - PAD * 2;
  const plotH = H - PAD * 2;

  const toPath = (data: number[]): string => {
    return data
      .map((v, i) => {
        const x = PAD + (i / (data.length - 1)) * plotW;
        const y = PAD + (1 - v) * plotH;
        return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");
  };

  const lines: { key: string; color: string; data: number[] }[] = [
    { key: "confidence", color: "#22c55e", data: history.map((h) => h.confidence) },
    { key: "fatigue", color: "#ef4444", data: history.map((h) => h.fatigue) },
    { key: "frustration", color: "#f97316", data: history.map((h) => h.frustration) },
    { key: "curiosity", color: "#a78bfa", data: history.map((h) => h.curiosity) },
  ];

  return (
    <div>
      <svg viewBox={`0 0 ${W} ${H}`} style={{ width: "100%", maxWidth: W, height: "auto" }}>
        {/* Grid lines */}
        {[0, 0.25, 0.5, 0.75, 1].map((v) => (
          <g key={v}>
            <line
              x1={PAD} y1={PAD + (1 - v) * plotH}
              x2={PAD + plotW} y2={PAD + (1 - v) * plotH}
              stroke="#1e293b" strokeWidth={1}
            />
            <text
              x={PAD - 4} y={PAD + (1 - v) * plotH + 3}
              fill="#64748b" fontSize={9} textAnchor="end" fontFamily="monospace"
            >
              {v.toFixed(1)}
            </text>
          </g>
        ))}
        {/* Data lines */}
        {lines.map((line) => (
          <path
            key={line.key}
            d={toPath(line.data)}
            fill="none"
            stroke={line.color}
            strokeWidth={2}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        ))}
      </svg>
      {/* Legend */}
      <div style={{ display: "flex", gap: 16, marginTop: 8, justifyContent: "center" }}>
        {lines.map((l) => (
          <span key={l.key} style={{ display: "flex", alignItems: "center", gap: 4, fontSize: "0.72rem" }}>
            <span style={{ width: 12, height: 3, background: l.color, borderRadius: 2, display: "inline-block" }} />
            <span style={{ color: "#94a3b8" }}>{l.key}</span>
          </span>
        ))}
      </div>
    </div>
  );
}

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function ConsciousnessMonitor(): JSX.Element {
  const [agents, setAgents] = useState<AgentEntry[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [state, setState] = useState<ConsciousnessState | null>(null);
  const [history, setHistory] = useState<ConsciousnessSnapshot[]>([]);
  const [behavior, setBehavior] = useState<UserBehavior | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [heatmap, setHeatmap] = useState<ConsciousnessState[]>([]);

  const loadAgents = useCallback(async () => {
    try {
      const list = await invoke<AgentEntry[]>("list_agents");
      setAgents(Array.isArray(list) ? list : []);
    } catch { /* ignore */ }
  }, []);

  useEffect(() => { void loadAgents(); }, [loadAgents]);

  // Load heatmap on mount
  const loadHeatmap = useCallback(async () => {
    try {
      const raw = await getConsciousnessHeatmap();
      const parsed = JSON.parse(raw);
      setHeatmap(Array.isArray(parsed) ? parsed : []);
    } catch { /* ignore */ }
  }, []);

  useEffect(() => {
    void loadHeatmap();
    const iv = setInterval(() => void loadHeatmap(), 15_000);
    return () => clearInterval(iv);
  }, [loadHeatmap]);

  // Track keystrokes for behavior analysis
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const isDeletion = e.key === "Backspace" || e.key === "Delete";
      if (e.key.length === 1 || isDeletion) {
        void reportUserKeystroke(isDeletion, Date.now());
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const refresh = useCallback(async () => {
    if (!selectedId) return;
    try {
      const results = await Promise.allSettled([
        getAgentConsciousness(selectedId).then(r => JSON.parse(r) as ConsciousnessState),
        getConsciousnessHistory(selectedId, 20).then(r => JSON.parse(r) as ConsciousnessSnapshot[]),
        getUserBehaviorState().then(r => JSON.parse(r) as UserBehavior),
      ]);
      if (results[0].status === "fulfilled") setState(results[0].value);
      if (results[1].status === "fulfilled") setHistory(Array.isArray(results[1].value) ? results[1].value : []);
      if (results[2].status === "fulfilled") setBehavior(results[2].value);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [selectedId]);

  useEffect(() => {
    void refresh();
    const iv = setInterval(() => void refresh(), 5_000);
    return () => clearInterval(iv);
  }, [refresh]);

  const handleReset = useCallback(async () => {
    if (!selectedId) return;
    try {
      await invoke("reset_agent_consciousness", { agentId: selectedId });
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [selectedId, refresh]);

  return (
    <div style={{ padding: 24, color: "#e2e8f0", maxWidth: 1200, margin: "0 auto" }}>
      <h1 style={{ fontFamily: "monospace", fontSize: "1.8rem", color: "#22d3ee", marginBottom: 8 }}>
        CONSCIOUSNESS MONITOR
      </h1>
      <p style={{ color: "#94a3b8", marginBottom: 20, fontSize: "0.85rem" }}>
        Real-time emotional state tracking, derived states, behavioral analysis
      </p>

      {error && <div style={{ color: "#f87171", marginBottom: 12, fontSize: "0.85rem" }}>{error}</div>}

      {/* Agent Selector */}
      <div style={{ display: "flex", gap: 12, marginBottom: 24, alignItems: "center" }}>
        <select
          value={selectedId}
          onChange={(e) => { setSelectedId(e.target.value); setState(null); setHistory([]); }}
          style={selectStyle}
        >
          <option value="">Select agent...</option>
          {agents.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name} (L{a.autonomy_level ?? 0}) — {a.status}
            </option>
          ))}
        </select>
        {selectedId && (
          <button type="button" onClick={() => void handleReset()} style={btnDangerStyle}>
            Reset
          </button>
        )}
      </div>

      {state && (
        <>
          {/* State Bars */}
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 16, marginBottom: 24 }}>
            <StateCard label="Confidence" value={state.confidence} inverted={false} />
            <StateCard label="Fatigue" value={state.fatigue} inverted={true} />
            <StateCard label="Frustration" value={state.frustration} inverted={true} />
            <StateCard label="Curiosity" value={state.curiosity} inverted={false} />
          </div>

          {/* Derived States */}
          <div style={{ ...panelStyle, marginBottom: 24 }}>
            <h3 style={headStyle}>Derived States</h3>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: 12 }}>
              <DerivedIndicator label="Flow state" active={state.flow_state} />
              <DerivedIndicator label="Needs handoff" active={state.needs_handoff} warn />
              <DerivedIndicator label="Should escalate" active={state.should_escalate} warn />
              <DerivedIndicator label="Exploration mode" active={state.exploration_mode} />
            </div>
          </div>

          {/* History Chart */}
          <div style={{ ...panelStyle, marginBottom: 24 }}>
            <h3 style={headStyle}>Emotional History (last 20 snapshots)</h3>
            <HistoryChart history={history} />
          </div>

          {/* User Behavior */}
          {behavior && (
            <div style={panelStyle}>
              <h3 style={headStyle}>User Behavior Analysis</h3>
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
                <StatRow label="Typing speed" value={`${behavior.typing_speed_wpm} WPM (baseline: ${behavior.baseline_wpm})`} />
                <StatRow label="Deletion rate" value={behavior.deletion_rate.toFixed(2)} />
                <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "3px 0", fontSize: "0.82rem" }}>
                  <span style={{ color: "#94a3b8" }}>Inferred mood</span>
                  <span style={{ fontFamily: "monospace", color: moodColor(behavior.inferred_mood), fontWeight: 600 }}>
                    {behavior.inferred_mood}
                  </span>
                  <span style={{ color: "#64748b", fontSize: "0.72rem" }}>
                    (conf: {(behavior.mood_confidence * 100).toFixed(0)}%)
                  </span>
                </div>
                <StatRow label="Response adaptation" value={behavior.response_adaptation} />
              </div>
            </div>
          )}
        </>
      )}

      {!state && selectedId && (
        <div style={{ ...panelStyle, textAlign: "center", color: "#64748b" }}>Loading consciousness data...</div>
      )}
      {!selectedId && (
        <div style={{ ...panelStyle, textAlign: "center", color: "#64748b" }}>Select an agent to monitor</div>
      )}

      {/* Consciousness Heatmap (all agents) */}
      {heatmap.length > 0 && (
        <div style={{ ...panelStyle, marginTop: 24 }}>
          <h3 style={headStyle}>Consciousness Heatmap (All Agents)</h3>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(120px, 1fr))", gap: 10 }}>
            {heatmap.map(c => (
              <div key={c.agent_id}
                onClick={() => setSelectedId(c.agent_id)}
                style={{
                  padding: 10, borderRadius: 8, cursor: "pointer",
                  background: c.flow_state ? "rgba(34,197,94,0.1)" : c.fatigue > 0.7 ? "rgba(239,68,68,0.1)" : "rgba(15,23,42,0.5)",
                  border: `1px solid ${c.flow_state ? "#22c55e" : c.fatigue > 0.7 ? "#ef4444" : "#1e293b"}`,
                }}
              >
                <div style={{ fontSize: "0.72rem", color: "#94a3b8", marginBottom: 4, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {c.agent_id.slice(0, 14)}
                </div>
                <div style={{ display: "flex", gap: 6, fontSize: "0.7rem" }}>
                  <span style={{ color: "#22c55e" }}>C:{(c.confidence * 100).toFixed(0)}%</span>
                  <span style={{ color: "#ef4444" }}>F:{(c.fatigue * 100).toFixed(0)}%</span>
                </div>
                {c.flow_state && <span style={{ fontSize: "0.65rem", color: "#22c55e" }}>FLOW</span>}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

/* ================================================================== */
/*  Sub-components                                                     */
/* ================================================================== */

function StateCard({ label, value, inverted }: { label: string; value: number; inverted: boolean }): JSX.Element {
  const color = stateBarColor(value, inverted);
  const pct = Math.round(value * 100);
  return (
    <div style={panelStyle}>
      <div style={{ fontSize: "0.72rem", color: "#94a3b8", textTransform: "uppercase", marginBottom: 8 }}>{label}</div>
      <div style={{ height: 8, background: "#1e293b", borderRadius: 4, overflow: "hidden", marginBottom: 6 }}>
        <div style={{ width: `${pct}%`, height: "100%", background: color, borderRadius: 4, transition: "width 0.3s ease" }} />
      </div>
      <div style={{ fontSize: "1.2rem", fontWeight: 700, color, fontFamily: "monospace" }}>{value.toFixed(2)}</div>
    </div>
  );
}

function DerivedIndicator({ label, active, warn }: { label: string; active: boolean; warn?: boolean }): JSX.Element {
  const dotColor = active
    ? (warn ? "#f97316" : "#22c55e")
    : "#475569";
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8, fontSize: "0.82rem" }}>
      <span style={{
        width: 10, height: 10, borderRadius: "50%", background: dotColor, display: "inline-block",
        boxShadow: active ? `0 0 8px ${dotColor}` : "none",
      }} />
      <span style={{ color: active ? "#e2e8f0" : "#64748b" }}>{label}: {active ? "YES" : "NO"}</span>
    </div>
  );
}

function StatRow({ label, value }: { label: string; value: string | number }): JSX.Element {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", padding: "3px 0", fontSize: "0.82rem" }}>
      <span style={{ color: "#94a3b8" }}>{label}</span>
      <span style={{ fontFamily: "monospace", color: "#e2e8f0" }}>{value}</span>
    </div>
  );
}

/* ================================================================== */
/*  Styles                                                             */
/* ================================================================== */

const panelStyle: React.CSSProperties = {
  background: "rgba(15,23,42,0.7)",
  border: "1px solid #1e293b",
  borderRadius: 10,
  padding: 20,
  backdropFilter: "blur(8px)",
};

const headStyle: React.CSSProperties = {
  fontFamily: "monospace",
  fontSize: "0.95rem",
  color: "#22d3ee",
  marginBottom: 14,
  paddingBottom: 8,
  borderBottom: "1px solid #1e293b",
};

const selectStyle: React.CSSProperties = {
  flex: 1,
  maxWidth: 400,
  padding: "8px 12px",
  background: "#0f172a",
  border: "1px solid #334155",
  borderRadius: 6,
  color: "#e2e8f0",
  fontFamily: "monospace",
  fontSize: "0.82rem",
};

const btnDangerStyle: React.CSSProperties = {
  padding: "8px 20px",
  background: "rgba(239,68,68,0.15)",
  border: "1px solid #ef4444",
  borderRadius: 6,
  color: "#ef4444",
  cursor: "pointer",
  fontFamily: "monospace",
  fontSize: "0.82rem",
  fontWeight: 600,
};

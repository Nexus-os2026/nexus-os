import { useEffect, useState } from "react";
import {
  cmListSessions,
  cmGetBatteries,
  cmGetScorecard,
  cmStartSession,
  listAgents,
} from "../api/backend";
import {
  ActionButton,
  EmptyState,
  Panel,
  StatusDot,
  alpha,
  commandHeaderMetaStyle,
  commandInsetStyle,
  commandLabelStyle,
  commandMonoValueStyle,
  commandMutedStyle,
  commandPageStyle,
  commandScrollStyle,
  formatTimestamp,
  normalizeArray,
  toTitleCase,
} from "./commandCenterUi";

const ACCENT = "#a78bfa";
const ACCENT2 = "#6366f1";

interface BatterySummary {
  vector: string;
  problem_count: number;
  locked_count: number;
  version: string;
}

interface VectorScorecard {
  vector: string;
  score: number;
}

interface Scorecard {
  agent_id: string;
  agent_autonomy_level: number;
  measured_at: number;
  vectors: VectorScorecard[];
  overall: { composite: number; floor: number; ceiling: number };
  classification: string | { [key: string]: unknown };
  gaming_flags: unknown[];
  audit_hash: string;
}

interface Session {
  id: string;
  agent_id: string;
  agent_autonomy_level: number;
  started_at: number;
  completed_at: number | null;
  vector_results: unknown[];
}

function classificationLabel(c: unknown): string {
  if (typeof c === "string") return toTitleCase(c);
  if (c && typeof c === "object") {
    const key = Object.keys(c as Record<string, unknown>)[0];
    return key ? toTitleCase(key) : "Unknown";
  }
  return "Unknown";
}

function flagColor(count: number): string {
  if (count === 0) return "#22c55e";
  if (count <= 2) return "#eab308";
  return "#ef4444";
}

const VECTOR_SHORT: Record<string, string> = {
  ReasoningDepth: "Reason",
  PlanningCoherence: "Plan",
  AdaptationUnderUncertainty: "Adapt",
  ToolUseIntegrity: "Tools",
};

const VECTOR_COLORS: Record<string, string> = {
  ReasoningDepth: "#818cf8",
  PlanningCoherence: "#34d399",
  AdaptationUnderUncertainty: "#fbbf24",
  ToolUseIntegrity: "#f472b6",
};

export default function MeasurementDashboard() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [batteries, setBatteries] = useState<BatterySummary[]>([]);
  const [scorecards, setScorecards] = useState<Scorecard[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showStartModal, setShowStartModal] = useState(false);
  const [agents, setAgents] = useState<{ id: string; name: string }[]>([]);
  const [selectedDetail, setSelectedDetail] = useState<string | null>(null);

  const load = () => {
    setLoading(true);
    setError(null);
    Promise.all([cmListSessions(), cmGetBatteries()])
      .then(async ([sess, bat]) => {
        const s = normalizeArray<Session>(sess);
        setSessions(s);
        setBatteries(normalizeArray<BatterySummary>(bat));
        // Load scorecards for unique agents
        const agentIds = [...new Set(s.map((x) => x.agent_id))];
        const cards: Scorecard[] = [];
        for (const id of agentIds) {
          try {
            const card = await cmGetScorecard(id);
            if (card) cards.push(card as Scorecard);
          } catch {
            /* no scorecard yet */
          }
        }
        setScorecards(cards);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(load, []);

  const handleStartSession = () => {
    listAgents()
      .then((a) => {
        setAgents(normalizeArray<{ id: string; name: string }>(a));
        setShowStartModal(true);
      })
      .catch(console.error);
  };

  const doStart = (agentId: string, level: number) => {
    cmStartSession(agentId, level)
      .then(() => {
        setShowStartModal(false);
        load();
      })
      .catch((e) => setError(String(e)));
  };

  const totalLocked = batteries.reduce((a, b) => a + b.locked_count, 0);
  const sortedCards = [...scorecards].sort(
    (a, b) => (b.overall?.composite ?? 0) - (a.overall?.composite ?? 0),
  );

  return (
    <div style={commandPageStyle}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 24 }}>
        <div>
          <h1 style={{ color: ACCENT, fontSize: 22, fontWeight: 700, margin: 0 }}>
            Capability Measurement
          </h1>
          <p style={{ ...commandMutedStyle, marginTop: 4, fontSize: 13 }}>
            {totalLocked} locked problems &middot; {sessions.length} sessions completed
          </p>
        </div>
        <ActionButton accent={ACCENT} onClick={handleStartSession}>Run New Measurement</ActionButton>
      </div>

      {error && (
        <div style={{ background: alpha("#ef4444", 0.15), border: "1px solid #ef4444", borderRadius: 8, padding: 12, marginBottom: 16, color: "#fca5a5", fontSize: 13 }}>
          {error}
          <button onClick={load} style={{ marginLeft: 12, color: ACCENT, background: "none", border: "none", cursor: "pointer", textDecoration: "underline" }}>Retry</button>
        </div>
      )}

      {loading && (
        <div style={{ textAlign: "center", padding: 48, color: "#888" }}>Loading measurement data...</div>
      )}

      {!loading && scorecards.length === 0 && (
        <EmptyState text="No measurements yet — run your first capability evaluation to see agent scorecards here." />
      )}

      {/* Agent Scorecard Grid */}
      {!loading && sortedCards.length > 0 && (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(300px, 1fr))", gap: 16, marginBottom: 24 }}>
          {sortedCards.map((card) => (
            <div key={card.agent_id} onClick={() => setSelectedDetail(selectedDetail === card.agent_id ? null : card.agent_id)}
              style={{ cursor: "pointer", background: alpha("#0f172a", 0.6), border: `1px solid ${selectedDetail === card.agent_id ? ACCENT : "#1e293b"}`, borderRadius: 10, padding: 16, transition: "border-color 0.2s" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                <div>
                  <div style={{ fontWeight: 600, fontSize: 15, color: "#e2e8f0" }}>{card.agent_id}</div>
                  <div style={{ ...commandMutedStyle, fontSize: 12 }}>L{card.agent_autonomy_level}</div>
                </div>
                <div style={{ textAlign: "right" }}>
                  <div style={{ fontSize: 28, fontWeight: 700, color: ACCENT, fontFamily: "monospace" }}>
                    {((card.overall?.composite ?? 0) * 100).toFixed(0)}
                  </div>
                  <div style={{ ...commandMutedStyle, fontSize: 11 }}>/ 100</div>
                </div>
              </div>

              {/* Mini vector bars */}
              <div style={{ display: "flex", gap: 6, marginTop: 12, height: 32 }}>
                {(card.vectors || []).map((v: VectorScorecard) => (
                  <div key={v.vector} style={{ flex: 1, display: "flex", flexDirection: "column", alignItems: "center" }}>
                    <div style={{ width: "100%", background: alpha("#334155", 0.5), borderRadius: 3, height: 20, position: "relative", overflow: "hidden" }}>
                      <div style={{ position: "absolute", bottom: 0, width: "100%", height: `${Math.round(v.score * 100)}%`, background: VECTOR_COLORS[v.vector] || "#818cf8", borderRadius: 3 }} />
                    </div>
                    <div style={{ fontSize: 9, color: "#94a3b8", marginTop: 2 }}>{VECTOR_SHORT[v.vector] || v.vector}</div>
                  </div>
                ))}
              </div>

              {/* Classification + flags */}
              <div style={{ display: "flex", justifyContent: "space-between", marginTop: 10, alignItems: "center" }}>
                <span style={{ fontSize: 11, background: alpha(ACCENT2, 0.2), color: ACCENT, padding: "2px 8px", borderRadius: 4 }}>
                  {classificationLabel(card.classification)}
                </span>
                <span style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 11 }}>
                  <StatusDot color={flagColor(card.gaming_flags?.length ?? 0)} />
                  {card.gaming_flags?.length ?? 0} flags
                </span>
              </div>

              {/* Expanded detail */}
              {selectedDetail === card.agent_id && (
                <div style={{ marginTop: 12, paddingTop: 12, borderTop: "1px solid #334155" }}>
                  <div style={{ ...commandMutedStyle, fontSize: 11, marginBottom: 6 }}>Per-vector scores:</div>
                  {(card.vectors || []).map((v: VectorScorecard) => (
                    <div key={v.vector} style={{ display: "flex", justifyContent: "space-between", fontSize: 12, padding: "2px 0" }}>
                      <span style={{ color: VECTOR_COLORS[v.vector] || "#e2e8f0" }}>{v.vector}</span>
                      <span style={commandMonoValueStyle}>{(v.score * 100).toFixed(1)}%</span>
                    </div>
                  ))}
                  <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12, marginTop: 6, paddingTop: 6, borderTop: "1px solid #1e293b" }}>
                    <span style={{ color: "#94a3b8" }}>Floor / Ceiling</span>
                    <span style={commandMonoValueStyle}>
                      {((card.overall?.floor ?? 0) * 100).toFixed(0)} / {((card.overall?.ceiling ?? 0) * 100).toFixed(0)}
                    </span>
                  </div>
                  <div style={{ ...commandMutedStyle, fontSize: 11, marginTop: 8 }}>
                    Measured: {formatTimestamp(card.measured_at)}
                  </div>
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Battery Status */}
      {!loading && batteries.length > 0 && (
        <Panel title="Test Batteries">
          <div style={{ ...commandScrollStyle, maxHeight: 200 }}>
            {batteries.map((b) => (
              <div key={b.vector} style={{ display: "flex", justifyContent: "space-between", padding: "6px 0", borderBottom: "1px solid #1e293b", fontSize: 13 }}>
                <span style={{ color: VECTOR_COLORS[b.vector] || "#e2e8f0" }}>{b.vector}</span>
                <span style={commandMonoValueStyle}>
                  {b.locked_count}/{b.problem_count} locked &middot; {b.version}
                </span>
              </div>
            ))}
          </div>
        </Panel>
      )}

      {/* Start Session Modal */}
      {showStartModal && (
        <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.6)", zIndex: 100, display: "flex", alignItems: "center", justifyContent: "center" }}
          onClick={() => setShowStartModal(false)}>
          <div style={{ ...commandInsetStyle, maxWidth: 440, width: "90%", padding: 24 }}
            onClick={(e) => e.stopPropagation()}>
            <div style={{ ...commandLabelStyle, marginBottom: 16 }}>Select Agent to Evaluate</div>
            <div style={{ ...commandScrollStyle, maxHeight: 320 }}>
              {agents.length === 0 && <div style={commandMutedStyle}>No agents available</div>}
              {agents.map((a) => (
                <div key={a.id} style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "8px 0", borderBottom: "1px solid #1e293b" }}>
                  <span style={{ color: "#e2e8f0", fontSize: 13 }}>{a.name || a.id}</span>
                  <ActionButton accent={ACCENT} onClick={() => doStart(a.id, 3)}>Evaluate</ActionButton>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

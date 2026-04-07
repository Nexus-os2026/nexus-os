import { useEffect, useState } from "react";
import { cmCompareAgents, cmListSessions } from "../api/backend";
import {
  ActionButton,
  EmptyState,
  Panel,
  StatusDot,
  alpha,
  commandLabelStyle,
  commandMonoValueStyle,
  commandMutedStyle,
  commandPageStyle,
  commandScrollStyle,
  normalizeArray,
  toTitleCase,
} from "./commandCenterUi";

const ACCENT = "#06b6d4";

const VECTOR_COLORS: Record<string, string> = {
  ReasoningDepth: "#818cf8",
  PlanningCoherence: "#34d399",
  AdaptationUnderUncertainty: "#fbbf24",
  ToolUseIntegrity: "#f472b6",
};

const VECTORS = ["ReasoningDepth", "PlanningCoherence", "AdaptationUnderUncertainty", "ToolUseIntegrity"];

interface VectorScorecard {
  vector: string;
  score: number;
}

interface Scorecard {
  agent_id: string;
  agent_autonomy_level: number;
  vectors: VectorScorecard[];
  overall: { composite: number; floor: number; ceiling: number };
  classification: string | { [key: string]: unknown };
  gaming_flags: unknown[];
}

function classLabel(c: unknown): string {
  if (typeof c === "string") return toTitleCase(c);
  if (c && typeof c === "object") return toTitleCase(Object.keys(c as Record<string, unknown>)[0] || "Unknown");
  return "Unknown";
}

export default function MeasurementCompare() {
  const [availableAgents, setAvailableAgents] = useState<string[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scorecards, setScorecards] = useState<Scorecard[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    cmListSessions()
      .then((sess) => {
        const ids = [...new Set(normalizeArray<{ agent_id: string }>(sess).map((s) => s.agent_id))];
        setAvailableAgents(ids);
      })
      .catch(console.error);
  }, []);

  const toggleAgent = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else if (next.size < 4) next.add(id);
      return next;
    });
  };

  const doCompare = () => {
    const ids = [...selected];
    if (ids.length < 2) return;
    setLoading(true);
    setError(null);
    cmCompareAgents(ids)
      .then((cards) => setScorecards(normalizeArray<Scorecard>(cards)))
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  };

  const getVectorScore = (card: Scorecard, vector: string): number => {
    return card.vectors?.find((v) => v.vector === vector)?.score ?? 0;
  };

  return (
    <div style={commandPageStyle}>
      <h1 style={{ color: ACCENT, fontSize: 22, fontWeight: 700, margin: 0, marginBottom: 8 }}>
        Agent Comparison
      </h1>
      <p style={{ ...commandMutedStyle, marginBottom: 20, fontSize: 13 }}>
        Select 2-4 agents to compare their capability profiles side by side.
      </p>

      {/* Agent Selector */}
      <Panel title={`Select Agents (${selected.size}/4)`} action={<ActionButton accent={ACCENT} onClick={doCompare}>Compare</ActionButton>}>
        <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
          {availableAgents.map((id) => (
            <button key={id} onClick={() => toggleAgent(id)} style={{
              padding: "6px 14px", borderRadius: 6, fontSize: 12, cursor: "pointer", border: "1px solid",
              borderColor: selected.has(id) ? ACCENT : "#334155",
              background: selected.has(id) ? alpha(ACCENT, 0.15) : "transparent",
              color: selected.has(id) ? ACCENT : "#94a3b8",
              transition: "all 0.15s",
            }}>
              {id}
            </button>
          ))}
          {availableAgents.length === 0 && <span style={commandMutedStyle}>No evaluated agents yet</span>}
        </div>
      </Panel>

      {error && <div style={{ color: "#ef4444", marginTop: 12, fontSize: 13 }}>{error}</div>}
      {loading && <div style={{ textAlign: "center", padding: 32, color: "#888" }}>Loading comparison...</div>}

      {/* Comparison Table */}
      {scorecards.length >= 2 && (
        <Panel title="Capability Comparison" style={{ marginTop: 16 }}>
          <div style={{ overflowX: "auto" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 13 }}>
              <thead>
                <tr style={{ borderBottom: "2px solid #334155" }}>
                  <th style={{ textAlign: "left", padding: "8px 12px", color: "#94a3b8", fontWeight: 500 }}>Vector</th>
                  {scorecards.map((c) => (
                    <th key={c.agent_id} style={{ textAlign: "center", padding: "8px 12px", color: ACCENT, fontWeight: 600 }}>
                      {c.agent_id}
                      <div style={{ fontSize: 10, color: "#94a3b8", fontWeight: 400 }}>L{c.agent_autonomy_level}</div>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {VECTORS.map((v) => (
                  <tr key={v} style={{ borderBottom: "1px solid #1e293b" }}>
                    <td style={{ padding: "8px 12px", color: VECTOR_COLORS[v] || "#e2e8f0" }}>{v}</td>
                    {scorecards.map((c) => {
                      const score = getVectorScore(c, v);
                      return (
                        <td key={c.agent_id} style={{ textAlign: "center", padding: "8px 12px" }}>
                          <span style={{ ...commandMonoValueStyle, color: score >= 0.7 ? "#22c55e" : score >= 0.4 ? "#eab308" : "#ef4444" }}>
                            {(score * 100).toFixed(0)}%
                          </span>
                        </td>
                      );
                    })}
                  </tr>
                ))}
                {/* Composite row */}
                <tr style={{ borderTop: "2px solid #334155" }}>
                  <td style={{ padding: "8px 12px", color: "#e2e8f0", fontWeight: 600 }}>Composite</td>
                  {scorecards.map((c) => (
                    <td key={c.agent_id} style={{ textAlign: "center", padding: "8px 12px", fontWeight: 700, fontSize: 15, fontFamily: "monospace", color: ACCENT }}>
                      {((c.overall?.composite ?? 0) * 100).toFixed(0)}
                    </td>
                  ))}
                </tr>
                {/* Classification row */}
                <tr>
                  <td style={{ padding: "8px 12px", color: "#94a3b8" }}>Classification</td>
                  {scorecards.map((c) => (
                    <td key={c.agent_id} style={{ textAlign: "center", padding: "8px 12px" }}>
                      <span style={{ fontSize: 11, background: alpha(ACCENT, 0.15), color: ACCENT, padding: "2px 8px", borderRadius: 4 }}>
                        {classLabel(c.classification)}
                      </span>
                    </td>
                  ))}
                </tr>
                {/* Flags row */}
                <tr>
                  <td style={{ padding: "8px 12px", color: "#94a3b8" }}>Gaming Flags</td>
                  {scorecards.map((c) => (
                    <td key={c.agent_id} style={{ textAlign: "center", padding: "8px 12px" }}>
                      <span style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
                        <StatusDot color={c.gaming_flags?.length ? "#f97316" : "#22c55e"} />
                        {c.gaming_flags?.length ?? 0}
                      </span>
                    </td>
                  ))}
                </tr>
              </tbody>
            </table>
          </div>
        </Panel>
      )}

      {scorecards.length === 0 && !loading && selected.size >= 2 && (
        <EmptyState text="Select agents above and click Compare to see side-by-side results." />
      )}
    </div>
  );
}

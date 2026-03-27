import { useState } from "react";
import { cmRunAbValidation } from "../api/backend";
import {
  ActionButton,
  EmptyState,
  Panel,
  StatusDot,
  alpha,
  commandMonoValueStyle,
  commandMutedStyle,
  commandPageStyle,
  commandScrollStyle,
} from "./commandCenterUi";

const ACCENT = "#10b981";

const VECTOR_COLORS: Record<string, string> = {
  ReasoningDepth: "#818cf8",
  PlanningCoherence: "#34d399",
  AdaptationUnderUncertainty: "#fbbf24",
  ToolUseIntegrity: "#f472b6",
};

interface VectorDelta { vector: string; baseline_score: number; routed_score: number; delta: number }
interface AgentComp { agent_id: string; autonomy_level: number; baseline_composite: number; routed_composite: number; delta: number; vector_deltas: VectorDelta[]; ceiling_improved: boolean; classification_changed: boolean; baseline_classification: string; routed_classification: string }
interface VectorAgg { vector: string; avg_baseline: number; avg_routed: number; avg_delta: number; agents_improved: number }
interface Aggregate { agents_evaluated: number; agents_improved: number; agents_unchanged: number; agents_degraded: number; avg_composite_delta: number; avg_ceiling_delta: number; vector_aggregates: VectorAgg[]; most_improved: string | null; most_improved_delta: number }
interface ABResult { agent_comparisons: AgentComp[]; aggregate: Aggregate; timestamp: number }

function deltaColor(d: number): string {
  if (d > 0.01) return "#22c55e";
  if (d < -0.01) return "#ef4444";
  return "#94a3b8";
}

export default function ABValidation() {
  const [result, setResult] = useState<ABResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState<string | null>(null);

  const runValidation = () => {
    setLoading(true);
    cmRunAbValidation([])
      .then((r) => setResult(r as ABResult))
      .catch(console.error)
      .finally(() => setLoading(false));
  };

  const agg = result?.aggregate;

  return (
    <div style={commandPageStyle}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 24 }}>
        <div>
          <h1 style={{ color: ACCENT, fontSize: 22, fontWeight: 700, margin: 0 }}>Predictive Routing Validation</h1>
          <p style={{ ...commandMutedStyle, marginTop: 4, fontSize: 13 }}>
            A/B comparison: fixed model assignment vs predictive routing
          </p>
        </div>
        <ActionButton accent={ACCENT} onClick={runValidation}>
          {loading ? "Running..." : "Run A/B Validation"}
        </ActionButton>
      </div>

      {loading && <div style={{ textAlign: "center", padding: 48, color: "#888" }}>Running validation (baseline + routed)...</div>}

      {!loading && !result && <EmptyState text="Run an A/B validation to compare fixed vs predictive model routing." />}

      {/* Aggregate Summary */}
      {agg && (
        <Panel title="Aggregate Summary">
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 16, marginBottom: 16 }}>
            <div style={{ textAlign: "center" }}>
              <div style={{ fontSize: 11, color: "#94a3b8" }}>Avg Improvement</div>
              <div style={{ fontSize: 28, fontWeight: 700, color: deltaColor(agg.avg_composite_delta), fontFamily: "monospace" }}>
                {agg.avg_composite_delta >= 0 ? "+" : ""}{(agg.avg_composite_delta * 100).toFixed(1)}%
              </div>
            </div>
            <div style={{ textAlign: "center" }}>
              <div style={{ fontSize: 11, color: "#94a3b8" }}>Agents</div>
              <div style={{ fontSize: 14 }}>
                <span style={{ color: "#22c55e" }}>{agg.agents_improved} improved</span> | <span style={{ color: "#94a3b8" }}>{agg.agents_unchanged} same</span> | <span style={{ color: "#ef4444" }}>{agg.agents_degraded} degraded</span>
              </div>
            </div>
            <div style={{ textAlign: "center" }}>
              <div style={{ fontSize: 11, color: "#94a3b8" }}>Ceiling Delta</div>
              <div style={{ ...commandMonoValueStyle, fontSize: 18 }}>
                {agg.avg_ceiling_delta >= 0 ? "+" : ""}{agg.avg_ceiling_delta.toFixed(1)} levels
              </div>
            </div>
            <div style={{ textAlign: "center" }}>
              <div style={{ fontSize: 11, color: "#94a3b8" }}>Most Improved</div>
              <div style={{ color: "#22c55e", fontSize: 13 }}>{agg.most_improved || "—"}</div>
              <div style={{ fontSize: 11, color: "#94a3b8" }}>+{(agg.most_improved_delta * 100).toFixed(1)}%</div>
            </div>
          </div>
        </Panel>
      )}

      {/* Per-Vector Comparison */}
      {agg && agg.vector_aggregates.length > 0 && (
        <Panel title="Per-Vector Comparison" style={{ marginTop: 16 }}>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 12 }}>
            {agg.vector_aggregates.map((va) => (
              <div key={va.vector} style={{ textAlign: "center", padding: 12, background: alpha("#0f172a", 0.5), borderRadius: 8 }}>
                <div style={{ fontSize: 11, color: VECTOR_COLORS[va.vector] || "#94a3b8", fontWeight: 600 }}>{va.vector.replace("UnderUncertainty", "")}</div>
                <div style={{ display: "flex", justifyContent: "center", gap: 8, marginTop: 6 }}>
                  <div><div style={{ fontSize: 9, color: "#94a3b8" }}>Base</div><div style={commandMonoValueStyle}>{(va.avg_baseline * 100).toFixed(0)}%</div></div>
                  <div style={{ color: deltaColor(va.avg_delta), fontSize: 14, alignSelf: "center" }}>→</div>
                  <div><div style={{ fontSize: 9, color: "#94a3b8" }}>Route</div><div style={commandMonoValueStyle}>{(va.avg_routed * 100).toFixed(0)}%</div></div>
                </div>
                <div style={{ color: deltaColor(va.avg_delta), fontSize: 12, marginTop: 4 }}>
                  {va.avg_delta >= 0 ? "+" : ""}{(va.avg_delta * 100).toFixed(1)}% ({va.agents_improved} improved)
                </div>
              </div>
            ))}
          </div>
        </Panel>
      )}

      {/* Agent Comparison Table */}
      {result && result.agent_comparisons.length > 0 && (
        <Panel title="Agent Comparison" style={{ marginTop: 16 }}>
          <div style={{ ...commandScrollStyle, maxHeight: 400 }}>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12 }}>
              <thead>
                <tr style={{ borderBottom: "2px solid #334155" }}>
                  <th style={{ textAlign: "left", padding: "6px 8px", color: "#94a3b8" }}>Agent</th>
                  <th style={{ textAlign: "center", padding: "6px 8px", color: "#94a3b8" }}>Level</th>
                  <th style={{ textAlign: "center", padding: "6px 8px", color: "#94a3b8" }}>Baseline</th>
                  <th style={{ textAlign: "center", padding: "6px 8px", color: "#94a3b8" }}>Routed</th>
                  <th style={{ textAlign: "center", padding: "6px 8px", color: "#94a3b8" }}>Delta</th>
                  <th style={{ textAlign: "center", padding: "6px 8px", color: "#94a3b8" }}>Ceiling</th>
                  <th style={{ textAlign: "center", padding: "6px 8px", color: "#94a3b8" }}>Class</th>
                </tr>
              </thead>
              <tbody>
                {result.agent_comparisons.map((ac) => (
                  <tr key={ac.agent_id} style={{ borderBottom: "1px solid #1e293b", cursor: "pointer" }}
                    onClick={() => setExpanded(expanded === ac.agent_id ? null : ac.agent_id)}>
                    <td style={{ padding: "6px 8px", color: "#e2e8f0" }}>{ac.agent_id}</td>
                    <td style={{ textAlign: "center", padding: "6px 8px", color: "#94a3b8" }}>L{ac.autonomy_level}</td>
                    <td style={{ textAlign: "center", padding: "6px 8px", ...commandMonoValueStyle }}>{(ac.baseline_composite * 100).toFixed(0)}%</td>
                    <td style={{ textAlign: "center", padding: "6px 8px", ...commandMonoValueStyle }}>{(ac.routed_composite * 100).toFixed(0)}%</td>
                    <td style={{ textAlign: "center", padding: "6px 8px", color: deltaColor(ac.delta), fontWeight: 600 }}>
                      {ac.delta >= 0 ? "+" : ""}{(ac.delta * 100).toFixed(1)}%
                    </td>
                    <td style={{ textAlign: "center", padding: "6px 8px" }}>
                      {ac.ceiling_improved ? <StatusDot color="#22c55e" /> : <StatusDot color="#94a3b8" />}
                    </td>
                    <td style={{ textAlign: "center", padding: "6px 8px" }}>
                      {ac.classification_changed ? <StatusDot color="#f97316" /> : <StatusDot color="#94a3b8" />}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Expanded detail */}
          {expanded && result.agent_comparisons.find((c) => c.agent_id === expanded) && (
            <div style={{ marginTop: 12, padding: 12, background: alpha("#0f172a", 0.5), borderRadius: 8 }}>
              <div style={{ fontSize: 13, color: "#e2e8f0", marginBottom: 8 }}>
                Per-vector breakdown for <strong>{expanded}</strong>
              </div>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 8 }}>
                {result.agent_comparisons.find((c) => c.agent_id === expanded)?.vector_deltas.map((vd) => (
                  <div key={vd.vector} style={{ textAlign: "center", fontSize: 11 }}>
                    <div style={{ color: VECTOR_COLORS[vd.vector] || "#94a3b8" }}>{vd.vector}</div>
                    <div>{(vd.baseline_score * 100).toFixed(0)}% → {(vd.routed_score * 100).toFixed(0)}%</div>
                    <div style={{ color: deltaColor(vd.delta), fontWeight: 600 }}>
                      {vd.delta >= 0 ? "+" : ""}{(vd.delta * 100).toFixed(1)}%
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </Panel>
      )}
    </div>
  );
}

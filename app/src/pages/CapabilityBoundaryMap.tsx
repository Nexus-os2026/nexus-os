import { useEffect, useState } from "react";
import {
  cmGetBoundaryMap,
  cmGetCalibration,
  cmGetCensus,
  cmGetGamingReportBatch,
  cmUploadDarwin,
} from "../api/backend";
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
const LEVELS = ["Level1", "Level2", "Level3", "Level4", "Level5"];

interface VectorCeiling { vector: string; ceiling_level: string | null; score_at_ceiling: number }
interface AgentBoundary { agent_id: string; autonomy_level: number; overall_ceiling: string | null; vector_ceilings: VectorCeiling[]; composite_score: number }
interface CalibrationInversion { vector: string; lower_level: string; lower_avg: number; higher_level: string; higher_avg: number }
interface CalibrationReport { is_calibrated: boolean; inversions: CalibrationInversion[] }
interface ClassificationCensus { total: number; balanced: number; theoretical_reasoner: number; procedural_executor: number; rigid_tool_user: number; pattern_matching: number; anomalous: number }
interface GamingReport { total_flags: number; red_count: number; orange_count: number; yellow_count: number; agents_with_flags: number; agents_clean: number }

function scoreColor(s: number): string {
  if (s >= 0.7) return "#22c55e";
  if (s >= 0.5) return "#eab308";
  if (s > 0) return "#ef4444";
  return "#1e293b";
}

export default function CapabilityBoundaryMap() {
  const [boundaries, setBoundaries] = useState<AgentBoundary[]>([]);
  const [calibration, setCalibration] = useState<CalibrationReport | null>(null);
  const [census, setCensus] = useState<ClassificationCensus | null>(null);
  const [gaming, setGaming] = useState<GamingReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [uploadResult, setUploadResult] = useState<string | null>(null);

  const load = () => {
    setLoading(true);
    Promise.all([cmGetBoundaryMap(), cmGetCalibration(), cmGetCensus(), cmGetGamingReportBatch()])
      .then(([b, c, ce, g]) => {
        setBoundaries(normalizeArray<AgentBoundary>(b).sort((a, b) => b.composite_score - a.composite_score));
        setCalibration(c as CalibrationReport);
        setCensus(ce as ClassificationCensus);
        setGaming(g as GamingReport);
      })
      .catch(console.error)
      .finally(() => setLoading(false));
  };

  useEffect(load, []);

  const handleUpload = () => {
    cmUploadDarwin()
      .then((r: { agents_uploaded: number; fitness_signals: number; reevaluation_triggers: number; mutation_targets: number }) =>
        setUploadResult(`Uploaded: ${r.agents_uploaded} agents, ${r.fitness_signals} signals, ${r.reevaluation_triggers} triggers, ${r.mutation_targets} mutations`))
      .catch((e: unknown) => setUploadResult(`Error: ${e}`));
  };

  if (loading) return <div style={commandPageStyle}><div style={{ textAlign: "center", padding: 48, color: "#888" }}>Loading boundary data...</div></div>;

  return (
    <div style={commandPageStyle}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 24 }}>
        <div>
          <h1 style={{ color: ACCENT, fontSize: 22, fontWeight: 700, margin: 0 }}>Capability Boundary Map</h1>
          <p style={{ ...commandMutedStyle, marginTop: 4, fontSize: 13 }}>
            {boundaries.length} agents evaluated &middot; Calibration: {calibration?.is_calibrated ? "OK" : "INVERSIONS DETECTED"}
          </p>
        </div>
        <ActionButton accent={ACCENT} onClick={handleUpload}>Upload to Darwin</ActionButton>
      </div>

      {uploadResult && <div style={{ background: alpha("#22c55e", 0.1), border: "1px solid #22c55e", borderRadius: 8, padding: 10, marginBottom: 16, color: "#86efac", fontSize: 12 }}>{uploadResult}</div>}

      {boundaries.length === 0 && (
        <>
          <EmptyState icon={<svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="#475569" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18M3 15h18M9 3v18M15 3v18"/></svg>} text="No boundary data yet — run a batch evaluation to generate the capability map." />
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 16, marginTop: 8 }}>
            {VECTORS.map(v => (
              <div key={v} style={{ padding: 20, borderRadius: 12, background: alpha("#0f172a", 0.6), border: `1px solid ${alpha(VECTOR_COLORS[v], 0.2)}`, textAlign: "center" }}>
                <div style={{ fontSize: 14, fontWeight: 600, color: VECTOR_COLORS[v], marginBottom: 12 }}>{v.replace(/([A-Z])/g, " $1").trim()}</div>
                <div style={{ display: "flex", gap: 4, justifyContent: "center" }}>
                  {LEVELS.map((l, i) => (
                    <div key={l} style={{ width: 32, height: 20, borderRadius: 4, background: alpha(VECTOR_COLORS[v], 0.06 + i * 0.04), border: `1px solid ${alpha(VECTOR_COLORS[v], 0.1)}` }} />
                  ))}
                </div>
                <div style={{ fontSize: 11, color: "#475569", marginTop: 8 }}>L1 — L5</div>
              </div>
            ))}
          </div>
        </>
      )}

      {/* Heatmap */}
      {boundaries.length > 0 && (
        <Panel title="Boundary Heatmap">
          <div style={{ overflowX: "auto" }}>
            <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 11 }}>
              <thead>
                <tr>
                  <th style={{ textAlign: "left", padding: 4, color: "#94a3b8", minWidth: 140 }}>Agent</th>
                  {VECTORS.map(v => LEVELS.map(l => (
                    <th key={`${v}-${l}`} style={{ padding: 2, color: VECTOR_COLORS[v], fontSize: 9, textAlign: "center", minWidth: 28 }}>
                      {l.replace("Level", "")}
                    </th>
                  )))}
                  <th style={{ textAlign: "center", padding: 4, color: "#94a3b8" }}>Ceil</th>
                  <th style={{ textAlign: "center", padding: 4, color: "#94a3b8" }}>Score</th>
                </tr>
                <tr>
                  <th />
                  {VECTORS.map(v => (
                    <th key={v} colSpan={5} style={{ textAlign: "center", padding: "0 2px", color: VECTOR_COLORS[v], fontSize: 9, borderBottom: `1px solid ${alpha(VECTOR_COLORS[v] || "#334155", 0.3)}` }}>
                      {v.replace("UnderUncertainty", "")}
                    </th>
                  ))}
                  <th /><th />
                </tr>
              </thead>
              <tbody>
                {boundaries.map(agent => (
                  <tr key={agent.agent_id} style={{ borderBottom: "1px solid #1e293b" }}>
                    <td style={{ padding: 4, color: "#e2e8f0", fontSize: 11, whiteSpace: "nowrap" }}>
                      {agent.agent_id.replace("nexus-", "")}
                      <span style={{ color: "#64748b", marginLeft: 4, fontSize: 9 }}>L{agent.autonomy_level}</span>
                    </td>
                    {VECTORS.map(v => {
                      const vc = agent.vector_ceilings.find(c => c.vector === v);
                      return LEVELS.map(l => {
                        const isCeiling = vc?.ceiling_level === l;
                        return (
                          <td key={`${v}-${l}`} style={{ padding: 1, textAlign: "center" }}>
                            <div style={{
                              width: 22, height: 16, borderRadius: 2, margin: "0 auto",
                              background: isCeiling ? alpha("#ef4444", 0.3) : alpha(VECTOR_COLORS[v] || "#334155", 0.15),
                              border: isCeiling ? "1px solid #ef4444" : "1px solid transparent",
                            }} />
                          </td>
                        );
                      });
                    })}
                    <td style={{ textAlign: "center", padding: 4, fontSize: 10, color: agent.overall_ceiling ? "#ef4444" : "#22c55e" }}>
                      {agent.overall_ceiling ? agent.overall_ceiling.replace("Level", "L") : "∞"}
                    </td>
                    <td style={{ textAlign: "center", padding: 4, ...commandMonoValueStyle, fontSize: 11 }}>
                      {(agent.composite_score * 100).toFixed(0)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Panel>
      )}

      {/* Census + Calibration + Gaming side by side */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 16, marginTop: 16 }}>
        {/* Census */}
        {census && (
          <Panel title="Classification Census">
            {[
              ["Balanced", census.balanced],
              ["Theoretical Reasoner", census.theoretical_reasoner],
              ["Procedural Executor", census.procedural_executor],
              ["Rigid Tool User", census.rigid_tool_user],
              ["Pattern Matching", census.pattern_matching],
              ["Anomalous", census.anomalous],
            ].map(([label, count]) => (
              <div key={label as string} style={{ display: "flex", justifyContent: "space-between", padding: "3px 0", fontSize: 12 }}>
                <span style={{ color: "#94a3b8" }}>{label as string}</span>
                <span style={commandMonoValueStyle}>{count as number}</span>
              </div>
            ))}
            <div style={{ borderTop: "1px solid #334155", paddingTop: 4, marginTop: 4, display: "flex", justifyContent: "space-between", fontSize: 12 }}>
              <span style={{ color: "#e2e8f0", fontWeight: 600 }}>Total</span>
              <span style={commandMonoValueStyle}>{census.total}</span>
            </div>
          </Panel>
        )}

        {/* Calibration */}
        {calibration && (
          <Panel title="Calibration Status">
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
              <StatusDot color={calibration.is_calibrated ? "#22c55e" : "#ef4444"} />
              <span style={{ color: calibration.is_calibrated ? "#22c55e" : "#ef4444", fontWeight: 600, fontSize: 14 }}>
                {calibration.is_calibrated ? "Calibrated" : "Inversions Detected"}
              </span>
            </div>
            {calibration.inversions.length > 0 && (
              <div style={commandScrollStyle}>
                {calibration.inversions.map((inv, i) => (
                  <div key={i} style={{ fontSize: 11, color: "#f97316", padding: "4px 0", borderBottom: "1px solid #1e293b" }}>
                    {inv.vector}: {inv.lower_level} ({(inv.lower_avg * 100).toFixed(0)}%) &lt; {inv.higher_level} ({(inv.higher_avg * 100).toFixed(0)}%)
                  </div>
                ))}
              </div>
            )}
            {calibration.inversions.length === 0 && <div style={commandMutedStyle}>Difficulty spectrum is monotonically increasing across all vectors.</div>}
          </Panel>
        )}

        {/* Gaming */}
        {gaming && (
          <Panel title="Gaming Detection">
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8, fontSize: 12 }}>
              <div><span style={{ color: "#94a3b8" }}>Clean agents:</span> <span style={{ color: "#22c55e" }}>{gaming.agents_clean}</span></div>
              <div><span style={{ color: "#94a3b8" }}>Flagged agents:</span> <span style={{ color: "#f97316" }}>{gaming.agents_with_flags}</span></div>
              <div><StatusDot color="#ef4444" /> <span style={{ color: "#ef4444" }}>{gaming.red_count} red</span></div>
              <div><StatusDot color="#f97316" /> <span style={{ color: "#f97316" }}>{gaming.orange_count} orange</span></div>
              <div><StatusDot color="#eab308" /> <span style={{ color: "#eab308" }}>{gaming.yellow_count} yellow</span></div>
              <div><span style={{ color: "#94a3b8" }}>Total flags:</span> <span style={commandMonoValueStyle}>{gaming.total_flags}</span></div>
            </div>
          </Panel>
        )}
      </div>
    </div>
  );
}

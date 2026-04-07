import { useEffect, useState } from "react";
import { cmGetBatteries } from "../api/backend";
import {
  EmptyState,
  alpha,
  commandMonoValueStyle,
  commandMutedStyle,
  commandPageStyle,
  normalizeArray,
} from "./commandCenterUi";

const ACCENT = "#06b6d4";

const VECTOR_COLORS: Record<string, string> = {
  ReasoningDepth: "#818cf8",
  PlanningCoherence: "#34d399",
  AdaptationUnderUncertainty: "#fbbf24",
  ToolUseIntegrity: "#f472b6",
};

const VECTOR_DESCRIPTIONS: Record<string, string> = {
  ReasoningDepth: "Measures ability to trace multi-hop causal chains, distinguish correlation from causation, and recognize underspecification.",
  PlanningCoherence: "Measures ability to construct plans with correct dependencies, rollback paths, halt conditions, and parallel execution.",
  AdaptationUnderUncertainty: "Measures ability to revise plans when new information arrives, assess source reliability, and maintain epistemic honesty.",
  ToolUseIntegrity: "Measures ability to select correct tools, use outputs faithfully, recognize limitations, and avoid fabrication.",
};

interface BatterySummary {
  vector: string;
  problem_count: number;
  locked_count: number;
  version: string;
}

export default function MeasurementBatteries() {
  const [batteries, setBatteries] = useState<BatterySummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [expanded, setExpanded] = useState<string | null>(null);

  useEffect(() => {
    cmGetBatteries()
      .then((b) => setBatteries(normalizeArray<BatterySummary>(b)))
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  const totalProblems = batteries.reduce((a, b) => a + b.problem_count, 0);
  const totalLocked = batteries.reduce((a, b) => a + b.locked_count, 0);

  return (
    <div style={commandPageStyle}>
      <h1 style={{ color: ACCENT, fontSize: 22, fontWeight: 700, margin: 0, marginBottom: 4 }}>
        Test Batteries
      </h1>
      <p style={{ ...commandMutedStyle, marginBottom: 24, fontSize: 13 }}>
        {totalProblems} problems across 4 vectors &middot; {totalLocked} locked for evaluation
      </p>

      {loading && <div style={{ textAlign: "center", padding: 48, color: "#888" }}>Loading batteries...</div>}

      {!loading && batteries.length === 0 && (
        <>
          <EmptyState icon={<svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="#475569" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M10 2v6m4-6v6M8 8h8l1 12H7L8 8z"/><path d="M10 12v4m4-4v4"/></svg>} text="No test batteries loaded yet" />
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 16, marginTop: 8 }}>
            {Object.entries(VECTOR_COLORS).map(([name, color]) => (
              <div key={name} style={{
                padding: 20, borderRadius: 12,
                background: alpha("#0f172a", 0.6), border: `1px solid ${alpha(color, 0.2)}`,
                textAlign: "center",
              }}>
                <div style={{ width: 10, height: 10, borderRadius: "50%", background: color, margin: "0 auto 12px" }} />
                <div style={{ fontSize: 14, fontWeight: 600, color, marginBottom: 6 }}>{name.replace(/([A-Z])/g, " $1").trim()}</div>
                <div style={{ fontSize: 12, color: "#64748b", lineHeight: 1.5 }}>{VECTOR_DESCRIPTIONS[name]?.slice(0, 80) || ""}...</div>
                <div style={{ display: "flex", gap: 6, justifyContent: "center", marginTop: 12 }}>
                  {[1,2,3,4,5].map(l => (
                    <div key={l} style={{ width: 28, height: 28, borderRadius: 6, background: alpha(color, 0.08), border: `1px solid ${alpha(color, 0.15)}`, display: "flex", alignItems: "center", justifyContent: "center", fontSize: 11, color: "#475569", fontWeight: 600 }}>L{l}</div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      {!loading && batteries.length > 0 && (
        <div style={{ display: "grid", gap: 16 }}>
          {batteries.map((b) => (
            <div key={b.vector}
              onClick={() => setExpanded(expanded === b.vector ? null : b.vector)}
              style={{ cursor: "pointer", background: alpha("#0f172a", 0.6), border: `1px solid ${expanded === b.vector ? (VECTOR_COLORS[b.vector] || "#334155") : "#1e293b"}`, borderRadius: 10, padding: 16, transition: "border-color 0.2s" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <div>
                  <div style={{ color: VECTOR_COLORS[b.vector] || "#e2e8f0", fontWeight: 600, fontSize: 16 }}>{b.vector}</div>
                  <div style={{ ...commandMutedStyle, fontSize: 12, marginTop: 2 }}>{VECTOR_DESCRIPTIONS[b.vector] || ""}</div>
                </div>
                <div style={{ textAlign: "right", flexShrink: 0 }}>
                  <div style={commandMonoValueStyle}>{b.locked_count}/{b.problem_count} locked</div>
                  <div style={{ ...commandMutedStyle, fontSize: 11 }}>{b.version}</div>
                </div>
              </div>

              {/* Level breakdown */}
              <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
                {["Level1", "Level2", "Level3", "Level4", "Level5"].map((level, i) => (
                  <div key={level} style={{
                    flex: 1, padding: "8px 0", textAlign: "center", borderRadius: 6,
                    background: alpha(VECTOR_COLORS[b.vector] || "#818cf8", 0.1 + i * 0.04),
                    border: `1px solid ${alpha(VECTOR_COLORS[b.vector] || "#818cf8", 0.2)}`,
                  }}>
                    <div style={{ fontSize: 14, fontWeight: 600, color: VECTOR_COLORS[b.vector] || "#e2e8f0" }}>L{i + 1}</div>
                    <div style={{ fontSize: 10, color: "#94a3b8" }}>\u25A0</div>
                  </div>
                ))}
              </div>

              {expanded === b.vector && (
                <div style={{ marginTop: 12, paddingTop: 12, borderTop: `1px solid ${alpha(VECTOR_COLORS[b.vector] || "#334155", 0.3)}` }}>
                  <div style={{ ...commandMutedStyle, fontSize: 11, marginBottom: 8 }}>
                    All {b.problem_count} problems are locked (immutable). Difficulty progresses from L1 (single constraint) to L5 (underspecified/adversarial).
                  </div>
                  <div style={{ fontSize: 12, color: "#cbd5e1" }}>
                    <div style={{ marginBottom: 4 }}><strong style={{ color: VECTOR_COLORS[b.vector] }}>Scoring:</strong> Asymmetric — gaps penalized more than redundancy, hallucination is worst.</div>
                    <div style={{ marginBottom: 4 }}><strong style={{ color: VECTOR_COLORS[b.vector] }}>Articulation:</strong> 3 binary dimensions per level, testing explanation quality.</div>
                    <div><strong style={{ color: VECTOR_COLORS[b.vector] }}>Gaming Detection:</strong> Cross-level inversion, confident L5 answers, output fidelity checks.</div>
                  </div>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

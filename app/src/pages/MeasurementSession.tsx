import { useEffect, useState } from "react";
import { cmGetSession, cmGetGamingFlags } from "../api/backend";
import {
  EmptyState,
  Panel,
  StatusDot,
  alpha,
  commandHeaderMetaStyle,
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

const VECTOR_COLORS: Record<string, string> = {
  ReasoningDepth: "#818cf8",
  PlanningCoherence: "#34d399",
  AdaptationUnderUncertainty: "#fbbf24",
  ToolUseIntegrity: "#f472b6",
};

const SEVERITY_COLORS: Record<string, string> = {
  Yellow: "#eab308",
  Orange: "#f97316",
  Red: "#ef4444",
};

interface LevelResult {
  level: string;
  problem_id: string;
  problem_version: string;
  agent_response: string;
  primary_score: { raw_score: number; adjusted_score: number; penalties: { reason: string; severity: string; weight: number }[] };
  articulation_score: { total: number; dimensions: { name: string; score: number; evidence: string }[] };
  gaming_flags: GamingFlag[];
}

interface VectorResult {
  vector: string;
  level_results: LevelResult[];
  gaming_flags: GamingFlag[];
  vector_score: number;
}

interface GamingFlag {
  flag_type: string | { [key: string]: unknown };
  evidence: string;
  severity: string;
  requires_human_review?: boolean;
}

interface SessionData {
  id: string;
  agent_id: string;
  agent_autonomy_level: number;
  started_at: number;
  completed_at: number | null;
  vector_results: VectorResult[];
  cross_vector_analysis: {
    capability_profile: { reasoning_depth: number; planning_coherence: number; adaptation: number; tool_use: number };
    overall_classification: string | { [key: string]: unknown };
    anomalies: string[];
  } | null;
  audit_hash: string;
}

function flagTypeLabel(ft: unknown): string {
  if (typeof ft === "string") return ft;
  if (ft && typeof ft === "object") return Object.keys(ft as Record<string, unknown>)[0] || "Unknown";
  return "Unknown";
}

function classLabel(c: unknown): string {
  if (typeof c === "string") return toTitleCase(c);
  if (c && typeof c === "object") {
    const key = Object.keys(c as Record<string, unknown>)[0];
    return key ? toTitleCase(key) : "Unknown";
  }
  return "Unknown";
}

function levelLabel(l: string): string {
  return l.replace("Level", "L");
}

function scoreColor(s: number): string {
  if (s >= 0.7) return "#22c55e";
  if (s >= 0.4) return "#eab308";
  return "#ef4444";
}

export default function MeasurementSession({ sessionId }: { sessionId: string }) {
  const [session, setSession] = useState<SessionData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedLevel, setExpandedLevel] = useState<string | null>(null);

  useEffect(() => {
    if (!sessionId) return;
    setLoading(true);
    cmGetSession(sessionId)
      .then((s) => setSession(s as SessionData))
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [sessionId]);

  if (loading) return <div style={commandPageStyle}><div style={{ textAlign: "center", padding: 48, color: "#888" }}>Loading session...</div></div>;
  if (error) return <div style={commandPageStyle}><div style={{ color: "#ef4444", padding: 24 }}>{error}</div></div>;
  if (!session) return <div style={commandPageStyle}><EmptyState text="Session not found — no measurement session found with this ID." /></div>;

  const profile = session.cross_vector_analysis?.capability_profile;
  const classification = session.cross_vector_analysis?.overall_classification;
  const allFlags: GamingFlag[] = session.vector_results.flatMap((vr) => normalizeArray<GamingFlag>(vr.gaming_flags));
  const composite = profile ? (profile.reasoning_depth + profile.planning_coherence + profile.adaptation + profile.tool_use) / 4 : 0;

  return (
    <div style={commandPageStyle}>
      {/* Header */}
      <div style={{ marginBottom: 24 }}>
        <h1 style={{ color: ACCENT, fontSize: 22, fontWeight: 700, margin: 0 }}>
          {session.agent_id}
        </h1>
        <div style={{ ...commandMutedStyle, marginTop: 4, fontSize: 13 }}>
          L{session.agent_autonomy_level} &middot; Measured {formatTimestamp(session.started_at)}
        </div>
        <div style={{ display: "flex", gap: 24, marginTop: 16 }}>
          <div>
            <div style={{ fontSize: 11, color: "#94a3b8" }}>Composite</div>
            <div style={{ fontSize: 36, fontWeight: 700, color: ACCENT, fontFamily: "monospace" }}>{(composite * 100).toFixed(0)}</div>
          </div>
          {profile && (
            <>
              <div>
                <div style={{ fontSize: 11, color: "#94a3b8" }}>Floor</div>
                <div style={commandMonoValueStyle}>{(Math.min(profile.reasoning_depth, profile.planning_coherence, profile.adaptation, profile.tool_use) * 100).toFixed(0)}</div>
              </div>
              <div>
                <div style={{ fontSize: 11, color: "#94a3b8" }}>Ceiling</div>
                <div style={commandMonoValueStyle}>{(Math.max(profile.reasoning_depth, profile.planning_coherence, profile.adaptation, profile.tool_use) * 100).toFixed(0)}</div>
              </div>
            </>
          )}
          {classification && (
            <div>
              <div style={{ fontSize: 11, color: "#94a3b8" }}>Classification</div>
              <span style={{ fontSize: 13, background: alpha(ACCENT, 0.2), color: ACCENT, padding: "2px 8px", borderRadius: 4 }}>
                {classLabel(classification)}
              </span>
            </div>
          )}
        </div>
      </div>

      {/* Vector Score Panels */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))", gap: 16, marginBottom: 24 }}>
        {session.vector_results.map((vr) => (
          <Panel key={vr.vector} title={vr.vector}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
              <span style={{ color: VECTOR_COLORS[vr.vector] || "#e2e8f0", fontWeight: 600, fontSize: 14 }}>{vr.vector}</span>
              <span style={{ fontSize: 20, fontWeight: 700, color: scoreColor(vr.vector_score), fontFamily: "monospace" }}>
                {(vr.vector_score * 100).toFixed(0)}%
              </span>
            </div>

            {/* Level bars */}
            <div style={{ display: "flex", gap: 4, height: 48, alignItems: "flex-end" }}>
              {vr.level_results.map((lr) => {
                const key = `${vr.vector}-${lr.level}`;
                return (
                  <div key={lr.level} style={{ flex: 1, display: "flex", flexDirection: "column", alignItems: "center", cursor: "pointer" }}
                    onClick={() => setExpandedLevel(expandedLevel === key ? null : key)}>
                    <div style={{ width: "100%", background: alpha("#334155", 0.5), borderRadius: 3, height: 40, position: "relative", overflow: "hidden" }}>
                      <div style={{ position: "absolute", bottom: 0, width: "100%", height: `${Math.round(lr.primary_score.adjusted_score * 100)}%`, background: scoreColor(lr.primary_score.adjusted_score), borderRadius: 3, transition: "height 0.3s" }} />
                    </div>
                    <div style={{ fontSize: 9, color: "#94a3b8", marginTop: 2 }}>{levelLabel(lr.level)}</div>
                  </div>
                );
              })}
            </div>

            {/* Articulation + flags */}
            {vr.level_results.length > 0 && (
              <div style={{ display: "flex", justifyContent: "space-between", marginTop: 8, fontSize: 11 }}>
                <span style={{ color: "#94a3b8" }}>Articulation: {(vr.level_results.reduce((a, lr) => a + lr.articulation_score.total, 0) / vr.level_results.length).toFixed(1)}/3</span>
                <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
                  <StatusDot color={vr.gaming_flags.length > 0 ? "#f97316" : "#22c55e"} />
                  {vr.gaming_flags.length} flags
                </span>
              </div>
            )}

            {/* Expanded level detail */}
            {vr.level_results.map((lr) => {
              const key = `${vr.vector}-${lr.level}`;
              if (expandedLevel !== key) return null;
              return (
                <div key={`detail-${lr.level}`} style={{ marginTop: 12, paddingTop: 12, borderTop: "1px solid #334155", fontSize: 12 }}>
                  <div style={{ color: "#94a3b8", marginBottom: 4 }}>{lr.problem_id} ({lr.problem_version})</div>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span>Raw: {(lr.primary_score.raw_score * 100).toFixed(0)}%</span>
                    <span>Adjusted: {(lr.primary_score.adjusted_score * 100).toFixed(0)}%</span>
                  </div>
                  {lr.primary_score.penalties.length > 0 && (
                    <div style={{ marginTop: 4 }}>
                      {lr.primary_score.penalties.map((p, i) => (
                        <div key={i} style={{ color: "#f97316", fontSize: 11 }}>-{p.weight.toFixed(2)}: {p.reason} ({p.severity})</div>
                      ))}
                    </div>
                  )}
                  {lr.agent_response && (
                    <div style={{ marginTop: 8, background: alpha("#1e293b", 0.5), padding: 8, borderRadius: 4, maxHeight: 120, overflow: "auto", fontSize: 11, color: "#cbd5e1", whiteSpace: "pre-wrap" }}>
                      {lr.agent_response.slice(0, 500)}{lr.agent_response.length > 500 ? "..." : ""}
                    </div>
                  )}
                </div>
              );
            })}
          </Panel>
        ))}
      </div>

      {/* Gaming Flags */}
      {allFlags.length > 0 && (
        <Panel title={`Gaming Flags (${allFlags.length})`}>
          <div style={commandScrollStyle}>
            {allFlags.map((flag, i) => (
              <div key={i} style={{ display: "flex", gap: 8, alignItems: "flex-start", padding: "8px 0", borderBottom: "1px solid #1e293b" }}>
                <StatusDot color={SEVERITY_COLORS[flag.severity] || "#eab308"} />
                <div style={{ flex: 1 }}>
                  <div style={{ fontSize: 13, color: "#e2e8f0" }}>{flagTypeLabel(flag.flag_type)}</div>
                  <div style={{ fontSize: 11, color: "#94a3b8" }}>{flag.evidence}</div>
                </div>
                <span style={{ fontSize: 10, color: SEVERITY_COLORS[flag.severity] || "#eab308", fontWeight: 600 }}>{flag.severity}</span>
                {flag.requires_human_review && <span style={{ fontSize: 10, color: "#f97316" }}>⚠ Review</span>}
              </div>
            ))}
          </div>
        </Panel>
      )}

      {/* Audit Hash */}
      <div style={{ ...commandMutedStyle, fontSize: 11, marginTop: 16 }}>
        Audit hash: <span style={commandMonoValueStyle}>{session.audit_hash.slice(0, 32)}...</span>
      </div>
    </div>
  );
}

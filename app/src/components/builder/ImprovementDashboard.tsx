/**
 * ImprovementDashboard — Phase 16 self-improving builder admin panel.
 *
 * Shows: projects analysed, proposals (pending/applied/rejected), defaults modified.
 * Actions: Run Analysis, Validate, Apply, Reject, Rollback, Reset All.
 */

import { useState, useEffect, useCallback } from "react";
import {
  builderImprovementStatus,
  builderImprovementRunAnalysis,
  builderImprovementGetProposals,
  builderImprovementValidateProposal,
  builderImprovementApplyProposal,
  builderImprovementRollbackProposal,
  builderImprovementResetDefaults,
  type ImprovementStatus,
  type ImprovementProposal,
} from "../../api/backend";

const C = {
  surface: "#111820",
  surfaceAlt: "#0d1117",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  warn: "#f59e0b",
  error: "#ef4444",
  success: "#22c55e",
  sans: "system-ui,-apple-system,sans-serif",
  mono: "SF Mono,Consolas,monospace",
};

interface Props {
  open: boolean;
  onClose: () => void;
}

export default function ImprovementDashboard({ open, onClose }: Props) {
  const [status, setStatus] = useState<ImprovementStatus | null>(null);
  const [proposals, setProposals] = useState<ImprovementProposal[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setError(null);
      const [s, p] = await Promise.all([
        builderImprovementStatus(),
        builderImprovementGetProposals(),
      ]);
      setStatus(s);
      setProposals(p);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    if (open) refresh();
  }, [open, refresh]);

  const runAnalysis = useCallback(async () => {
    setLoading(true);
    try {
      await builderImprovementRunAnalysis();
      await refresh();
    } catch (e) {
      setError(String(e));
    }
    setLoading(false);
  }, [refresh]);

  const validateProposal = useCallback(async (id: string) => {
    try {
      await builderImprovementValidateProposal(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }, [refresh]);

  const applyProposal = useCallback(async (id: string) => {
    try {
      await builderImprovementApplyProposal(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }, [refresh]);

  const rollbackProposal = useCallback(async (id: string) => {
    try {
      await builderImprovementRollbackProposal(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }, [refresh]);

  const resetAll = useCallback(async () => {
    try {
      await builderImprovementResetDefaults();
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }, [refresh]);

  if (!open) return null;

  const statusIcon = (s: string) => {
    switch (s) {
      case "Applied": return "\u2705";
      case "Validated": return "\u2714\uFE0F";
      case "Pending": return "\u23F3";
      case "ValidationFailed": return "\u274C";
      case "Rejected": return "\u26D4";
      case "RolledBack": return "\u21A9\uFE0F";
      default: return "\u2022";
    }
  };

  return (
    <div style={{
      position: "fixed", inset: 0, zIndex: 9000,
      display: "flex", alignItems: "center", justifyContent: "center",
      background: "rgba(0,0,0,0.7)", fontFamily: C.sans,
    }} onClick={onClose}>
      <div style={{
        background: C.surface, border: `1px solid ${C.border}`,
        borderRadius: 12, width: 640, maxHeight: "80vh",
        overflow: "auto", padding: 24, color: C.text,
      }} onClick={(e) => e.stopPropagation()}>

        {/* Header */}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 20 }}>
          <div>
            <h2 style={{ margin: 0, fontSize: 18, fontWeight: 600 }}>Self-Improving Builder</h2>
            <p style={{ margin: "4px 0 0", fontSize: 12, color: C.muted }}>
              Governed, auditable, reversible improvements
            </p>
          </div>
          <button onClick={onClose} style={{
            background: "none", border: "none", color: C.muted,
            fontSize: 20, cursor: "pointer", padding: 4,
          }}>&times;</button>
        </div>

        {error && (
          <div style={{
            background: "rgba(239,68,68,0.1)", border: `1px solid ${C.error}`,
            borderRadius: 8, padding: 10, marginBottom: 16, fontSize: 13, color: C.error,
          }}>{error}</div>
        )}

        {/* Stats */}
        {status && (
          <div style={{
            display: "grid", gridTemplateColumns: "repeat(4, 1fr)",
            gap: 12, marginBottom: 20,
          }}>
            {[
              { label: "Projects", value: status.projects_analyzed, color: C.accent },
              { label: "Pending", value: status.proposals_pending, color: C.warn },
              { label: "Applied", value: status.proposals_applied, color: C.success },
              { label: "Defaults", value: `${status.defaults_modified}/6`, color: C.muted },
            ].map((s) => (
              <div key={s.label} style={{
                background: C.surfaceAlt, borderRadius: 8, padding: 12, textAlign: "center",
                border: `1px solid ${C.border}`,
              }}>
                <div style={{ fontSize: 22, fontWeight: 700, color: s.color, fontFamily: C.mono }}>
                  {s.value}
                </div>
                <div style={{ fontSize: 11, color: C.muted, marginTop: 2 }}>{s.label}</div>
              </div>
            ))}
          </div>
        )}

        {/* Proposals */}
        <div style={{ marginBottom: 16 }}>
          <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: 10, color: C.muted }}>
            Proposals ({proposals.length})
          </h3>
          {proposals.length === 0 ? (
            <div style={{
              textAlign: "center", padding: 24, color: C.dim, fontSize: 13,
              background: C.surfaceAlt, borderRadius: 8, border: `1px solid ${C.border}`,
            }}>
              No proposals yet. Run analysis to discover improvements.
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {proposals.map((p) => (
                <div key={p.id} style={{
                  background: C.surfaceAlt, borderRadius: 8, padding: 14,
                  border: `1px solid ${C.border}`,
                }}>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
                    <div style={{ flex: 1 }}>
                      <div style={{ fontSize: 13, fontWeight: 500 }}>
                        {statusIcon(p.status)} {p.description}
                      </div>
                      <div style={{ fontSize: 11, color: C.muted, marginTop: 4 }}>
                        Evidence: {p.evidence_summary}
                      </div>
                      <div style={{ fontSize: 11, color: C.dim, marginTop: 2 }}>
                        {p.before_value} &rarr; {p.after_value}
                      </div>
                    </div>
                    <div style={{ display: "flex", gap: 6, marginLeft: 12, flexShrink: 0 }}>
                      {(p.status === "Pending") && (
                        <SmallButton label="Validate" color={C.accent} onClick={() => validateProposal(p.id)} />
                      )}
                      {(p.status === "Validated") && (
                        <SmallButton label="Apply" color={C.success} onClick={() => applyProposal(p.id)} />
                      )}
                      {(p.status === "Applied") && (
                        <SmallButton label="Rollback" color={C.warn} onClick={() => rollbackProposal(p.id)} />
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Actions */}
        <div style={{ display: "flex", gap: 10, justifyContent: "flex-end", paddingTop: 8, borderTop: `1px solid ${C.border}` }}>
          <button onClick={resetAll} style={{
            background: "rgba(239,68,68,0.1)", border: `1px solid ${C.error}`,
            borderRadius: 6, padding: "6px 14px", color: C.error,
            fontSize: 12, cursor: "pointer", fontWeight: 500,
          }}>
            Reset All to Factory
          </button>
          <button onClick={runAnalysis} disabled={loading} style={{
            background: C.accentDim, border: `1px solid ${C.accent}`,
            borderRadius: 6, padding: "6px 14px", color: C.accent,
            fontSize: 12, cursor: loading ? "wait" : "pointer", fontWeight: 500,
            opacity: loading ? 0.6 : 1,
          }}>
            {loading ? "Analysing..." : "Run Analysis"}
          </button>
        </div>
      </div>
    </div>
  );
}

function SmallButton({ label, color, onClick }: { label: string; color: string; onClick: () => void }) {
  return (
    <button onClick={onClick} style={{
      background: `${color}15`, border: `1px solid ${color}`,
      borderRadius: 4, padding: "3px 10px", color,
      fontSize: 11, cursor: "pointer", fontWeight: 500,
      whiteSpace: "nowrap",
    }}>
      {label}
    </button>
  );
}

/**
 * ConversionReportCard — displays conversion check results with scores and auto-fix.
 *
 * Shows four conversion scores (CTA, Above-Fold, Trust Signals, Copy Clarity)
 * with progress bars, expandable issue details, top recommendation, and auto-fix.
 * All inline styles per project convention.
 */

import { useState, useCallback } from "react";
import {
  builderConversionCheck,
  builderConversionAutoFix,
  type ConversionReport,
  type QualityCheckResult,
} from "../../api/backend";

const C = {
  bg: "#0a0e14",
  surface: "#111820",
  surfaceAlt: "#0d1219",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  green: "#22c55e",
  greenDim: "rgba(34,197,94,0.12)",
  amber: "#f59e0b",
  amberDim: "rgba(245,158,11,0.12)",
  red: "#ef4444",
  redDim: "rgba(239,68,68,0.12)",
  purple: "#a855f7",
  purpleDim: "rgba(168,85,247,0.12)",
  sans: "system-ui,-apple-system,sans-serif",
};

function scoreColor(score: number): string {
  if (score >= 90) return C.green;
  if (score >= 70) return C.amber;
  return C.red;
}

function scoreColorDim(score: number): string {
  if (score >= 90) return C.greenDim;
  if (score >= 70) return C.amberDim;
  return C.redDim;
}

function severityIcon(sev: string): string {
  if (sev === "Error") return "\u25cf";
  if (sev === "Warning") return "\u25b2";
  return "\u25cb";
}

function severityColor(sev: string): string {
  if (sev === "Error") return C.red;
  if (sev === "Warning") return C.amber;
  return C.muted;
}

interface ConversionReportCardProps {
  projectId: string;
  onHtmlChanged?: () => void;
}

export default function ConversionReportCard({ projectId, onHtmlChanged }: ConversionReportCardProps) {
  const [report, setReport] = useState<ConversionReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [fixing, setFixing] = useState(false);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [error, setError] = useState("");

  const runCheck = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const r = await builderConversionCheck(projectId);
      setReport(r);
    } catch (e: any) {
      setError(e?.toString() ?? "Conversion check failed");
    }
    setLoading(false);
  }, [projectId]);

  const fixAll = useCallback(async () => {
    if (!report) return;
    setFixing(true);
    try {
      const allIndices: number[] = [];
      let idx = 0;
      for (const c of report.checks) {
        for (const issue of c.issues) {
          if (issue.fix) allIndices.push(idx);
          idx++;
        }
      }
      await builderConversionAutoFix(projectId, allIndices);
      onHtmlChanged?.();
      // Re-run check after fixing
      const r = await builderConversionCheck(projectId);
      setReport(r);
    } catch (e: any) {
      setError(e?.toString() ?? "Auto-fix failed");
    }
    setFixing(false);
  }, [projectId, report, onHtmlChanged]);

  if (!report && !loading) {
    return (
      <button onClick={runCheck} style={{
        background: C.purpleDim, border: "1px solid rgba(168,85,247,0.2)",
        borderRadius: 4, padding: "4px 10px", color: C.purple, fontSize: 10,
        cursor: "pointer", fontWeight: 500, fontFamily: C.sans,
      }}>
        Conversion Check
      </button>
    );
  }

  if (loading) {
    return (
      <div style={{ color: C.muted, fontSize: 10, fontFamily: C.sans, padding: "4px 10px" }}>
        Analyzing conversion...
      </div>
    );
  }

  if (error) {
    return (
      <div style={{ color: C.red, fontSize: 10, fontFamily: C.sans, padding: "4px 10px" }}>
        {error}
      </div>
    );
  }

  if (!report) return null;

  return (
    <div style={{
      background: C.surfaceAlt, border: `1px solid ${C.border}`, borderRadius: 6,
      padding: 12, fontFamily: C.sans, minWidth: 280,
    }}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
        <span style={{ color: C.text, fontSize: 11, fontWeight: 600 }}>Conversion Report</span>
        <div style={{
          background: scoreColorDim(report.overall_score),
          color: scoreColor(report.overall_score),
          borderRadius: 4, padding: "2px 8px", fontSize: 11, fontWeight: 700,
        }}>
          {report.overall_score}/100 {report.overall_pass ? "PASS" : "FAIL"}
        </div>
      </div>

      {/* Score bars */}
      {report.checks.map((c: QualityCheckResult) => (
        <div key={c.check_id} style={{ marginBottom: 6 }}>
          <div
            style={{ display: "flex", justifyContent: "space-between", alignItems: "center", cursor: "pointer" }}
            onClick={() => setExpanded(expanded === c.check_id ? null : c.check_id)}
          >
            <span style={{ color: C.muted, fontSize: 10, width: 100 }}>{c.check_name}</span>
            <div style={{ flex: 1, height: 6, background: C.border, borderRadius: 3, margin: "0 8px", overflow: "hidden" }}>
              <div style={{
                width: `${c.score}%`, height: "100%",
                background: scoreColor(c.score), borderRadius: 3,
                transition: "width 0.3s ease",
              }} />
            </div>
            <span style={{ color: scoreColor(c.score), fontSize: 10, fontWeight: 600, width: 40, textAlign: "right" }}>
              {c.score}
            </span>
          </div>

          {/* Expanded issues */}
          {expanded === c.check_id && c.issues.length > 0 && (
            <div style={{ marginTop: 4, paddingLeft: 8, borderLeft: `2px solid ${C.border}` }}>
              {c.issues.map((issue, idx) => (
                <div key={idx} style={{ display: "flex", gap: 6, marginBottom: 3, alignItems: "flex-start" }}>
                  <span style={{ color: severityColor(issue.severity), fontSize: 9, lineHeight: "14px" }}>
                    {severityIcon(issue.severity)}
                  </span>
                  <span style={{ color: C.muted, fontSize: 9, lineHeight: "14px", flex: 1 }}>
                    {issue.message}
                  </span>
                  {issue.fix && (
                    <span style={{ color: C.purple, fontSize: 8, whiteSpace: "nowrap" }}>fixable</span>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      ))}

      {/* Top recommendation */}
      {report.top_recommendation && report.overall_score < 100 && (
        <div style={{
          background: C.purpleDim, border: "1px solid rgba(168,85,247,0.15)",
          borderRadius: 4, padding: "6px 8px", marginTop: 8,
        }}>
          <span style={{ color: C.purple, fontSize: 9, fontWeight: 600 }}>Top recommendation: </span>
          <span style={{ color: C.muted, fontSize: 9 }}>{report.top_recommendation}</span>
        </div>
      )}

      {/* Footer: issue count + auto-fix */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: 8, paddingTop: 8, borderTop: `1px solid ${C.border}` }}>
        <span style={{ color: C.dim, fontSize: 9 }}>
          {report.total_issues} issue{report.total_issues !== 1 ? "s" : ""}
          {report.auto_fixable_count > 0 && ` (${report.auto_fixable_count} fixable)`}
        </span>
        <div style={{ display: "flex", gap: 6 }}>
          {report.auto_fixable_count > 0 && (
            <button onClick={fixAll} disabled={fixing} style={{
              background: C.purple, border: "none", borderRadius: 3,
              padding: "2px 8px", color: "#fff", fontSize: 9,
              fontWeight: 600, cursor: fixing ? "default" : "pointer",
              opacity: fixing ? 0.6 : 1,
            }}>
              {fixing ? "Fixing..." : `Auto-fix ${report.auto_fixable_count}`}
            </button>
          )}
          <button onClick={runCheck} style={{
            background: "transparent", border: `1px solid ${C.border}`,
            borderRadius: 3, padding: "2px 8px", color: C.muted, fontSize: 9,
            cursor: "pointer",
          }}>
            Re-check
          </button>
        </div>
      </div>
    </div>
  );
}

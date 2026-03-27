import { useCallback, useEffect, useState } from "react";
import {
  adminComplianceStatus,
  adminComplianceExport,
} from "../api/backend";
import "./admin.css";

interface ControlStatus {
  id: string;
  name: string;
  category: string;
  status: "pass" | "fail" | "warn" | "na";
  details: string;
  last_checked: string;
}

interface ComplianceData {
  eu_ai_act: { score: number; total: number; controls: ControlStatus[] };
  soc2: { score: number; total: number; controls: ControlStatus[] };
  audit_stats: {
    total_events: number;
    events_24h: number;
    chain_verified: boolean;
    last_verification: string;
    next_verification: string;
  };
  pii_stats: {
    total_redactions: number;
    redactions_24h: number;
    patterns_active: number;
  };
  hitl_stats: {
    total_approvals: number;
    total_denials: number;
    approval_rate: number;
    pending: number;
  };
}

const EMPTY_COMPLIANCE: ComplianceData = {
  eu_ai_act: { score: 0, total: 0, controls: [] },
  soc2: { score: 0, total: 0, controls: [] },
  audit_stats: {
    total_events: 0,
    events_24h: 0,
    chain_verified: false,
    last_verification: new Date().toISOString(),
    next_verification: new Date().toISOString(),
  },
  pii_stats: { total_redactions: 0, redactions_24h: 0, patterns_active: 0 },
  hitl_stats: { total_approvals: 0, total_denials: 0, approval_rate: 0, pending: 0 },
};

function statusColor(s: ControlStatus["status"]): string {
  if (s === "pass") return "var(--nexus-accent, #4af7d3)";
  if (s === "fail") return "var(--nexus-danger, #ff6d7a)";
  if (s === "warn") return "var(--nexus-amber, #ffb85c)";
  return "var(--text-muted)";
}

function statusLabel(s: ControlStatus["status"]): string {
  if (s === "pass") return "PASS";
  if (s === "fail") return "FAIL";
  if (s === "warn") return "WARN";
  return "N/A";
}

export default function AdminCompliance() {
  const [data, setData] = useState<ComplianceData>(EMPTY_COMPLIANCE);
  const [loading, setLoading] = useState(true);
  const [tab, setTab] = useState<"overview" | "eu-ai-act" | "soc2" | "audit">("overview");
  const [exporting, setExporting] = useState(false);
  const [statusMsg, setStatusMsg] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const status = await adminComplianceStatus();
      setData(status);
    } catch {
      // keep empty state; backend may not be running in web-only mode
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleExport = async (format: "json" | "csv" | "pdf") => {
    setExporting(true);
    try {
      const result = await adminComplianceExport(format);
      setStatusMsg(`Report exported: ${result}`);
    } catch {
      /* no-op */
    } finally {
      setExporting(false);
    }
  };

  const euPercent = data.eu_ai_act.total > 0 ? Math.round((data.eu_ai_act.score / data.eu_ai_act.total) * 100) : 0;
  const socPercent = data.soc2.total > 0 ? Math.round((data.soc2.score / data.soc2.total) * 100) : 0;

  return (
    <div className="admin-shell">
      <h1>Compliance Dashboard</h1>
      <p className="admin-subtitle">
        EU AI Act, SOC 2, audit integrity, and privacy compliance
        {loading && " — loading..."}
      </p>

      <div style={{ display: "flex", gap: "0.5rem", marginBottom: "1rem" }}>
        <button className="admin-btn" onClick={() => void handleExport("json")} disabled={exporting}>Export JSON</button>
        <button className="admin-btn" onClick={() => void handleExport("csv")} disabled={exporting}>Export CSV</button>
      </div>

      <div className="admin-tabs">
        {(["overview", "eu-ai-act", "soc2", "audit"] as const).map((t) => (
          <button key={t} className={`admin-tab ${tab === t ? "admin-tab--active" : ""}`} onClick={() => setTab(t)}>
            {t === "eu-ai-act" ? "EU AI Act" : t === "soc2" ? "SOC 2" : t === "audit" ? "Audit & Privacy" : "Overview"}
          </button>
        ))}
      </div>

      {tab === "overview" && (
        <>
          <div className="admin-metrics">
            <div className="admin-metric">
              <span className="admin-metric__label">EU AI Act</span>
              <span className="admin-metric__value" style={{ color: euPercent >= 80 ? "var(--nexus-accent)" : "var(--nexus-amber)" }}>{euPercent}%</span>
              <span className="admin-metric__sub">{data.eu_ai_act.score}/{data.eu_ai_act.total} controls</span>
            </div>
            <div className="admin-metric">
              <span className="admin-metric__label">SOC 2</span>
              <span className="admin-metric__value" style={{ color: socPercent >= 80 ? "var(--nexus-accent)" : "var(--nexus-amber)" }}>{socPercent}%</span>
              <span className="admin-metric__sub">{data.soc2.score}/{data.soc2.total} controls</span>
            </div>
            <div className="admin-metric">
              <span className="admin-metric__label">Hash Chain</span>
              <span className="admin-metric__value" style={{ color: data.audit_stats.chain_verified ? "var(--nexus-accent)" : "var(--nexus-danger)" }}>
                {data.audit_stats.chain_verified ? "OK" : "FAIL"}
              </span>
            </div>
            <div className="admin-metric">
              <span className="admin-metric__label">PII Redactions (24h)</span>
              <span className="admin-metric__value">{data.pii_stats.redactions_24h}</span>
            </div>
            <div className="admin-metric">
              <span className="admin-metric__label">HITL Approval Rate</span>
              <span className="admin-metric__value">{data.hitl_stats.approval_rate}%</span>
              <span className="admin-metric__sub">{data.hitl_stats.pending} pending</span>
            </div>
          </div>
        </>
      )}

      {tab === "eu-ai-act" && <ControlTable controls={data.eu_ai_act.controls} title="EU AI Act Self-Assessment" />}
      {tab === "soc2" && <ControlTable controls={data.soc2.controls} title="SOC 2 Controls" />}

      {tab === "audit" && (
        <div className="admin-grid-2">
          <div className="admin-card">
            <div className="admin-card__title">Audit Trail</div>
            <StatRow label="Total Events" value={data.audit_stats.total_events.toLocaleString()} />
            <StatRow label="Events (24h)" value={String(data.audit_stats.events_24h)} />
            <StatRow label="Chain Verified" value={data.audit_stats.chain_verified ? "Yes" : "No"} color={data.audit_stats.chain_verified ? "var(--nexus-accent)" : "var(--nexus-danger)"} />
            <StatRow label="Last Verification" value={new Date(data.audit_stats.last_verification).toLocaleString()} />
            <StatRow label="Next Verification" value={new Date(data.audit_stats.next_verification).toLocaleString()} />
          </div>
          <div className="admin-card">
            <div className="admin-card__title">Privacy & HITL</div>
            <StatRow label="Total PII Redactions" value={data.pii_stats.total_redactions.toLocaleString()} />
            <StatRow label="Active Patterns" value={String(data.pii_stats.patterns_active)} />
            <StatRow label="HITL Approvals" value={String(data.hitl_stats.total_approvals)} />
            <StatRow label="HITL Denials" value={String(data.hitl_stats.total_denials)} />
            <StatRow label="Approval Rate" value={`${data.hitl_stats.approval_rate}%`} />
          </div>
        </div>
      )}
    </div>
  );
}

function ControlTable({ controls, title }: { controls: ControlStatus[]; title: string }) {
  return (
    <div className="admin-card">
      <div className="admin-card__title">{title}</div>
      <table className="admin-table">
        <thead>
          <tr>
            <th>Status</th>
            <th>Control</th>
            <th>Category</th>
            <th>Details</th>
            <th>Last Checked</th>
          </tr>
        </thead>
        <tbody>
          {controls.map((c) => (
            <tr key={c.id}>
              <td>
                <span style={{ color: statusColor(c.status), fontWeight: 700, fontSize: "0.75rem", fontFamily: "var(--font-mono)" }}>
                  {statusLabel(c.status)}
                </span>
              </td>
              <td style={{ color: "var(--text-primary)", fontWeight: 500 }}>{c.name}</td>
              <td>{c.category}</td>
              <td>{c.details}</td>
              <td>{new Date(c.last_checked).toLocaleDateString()}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function StatRow({ label, value, color }: { label: string; value: string; color?: string }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", padding: "0.35rem 0", fontSize: "0.82rem", borderBottom: "1px solid rgba(90,142,190,0.08)" }}>
      <span style={{ color: "var(--text-secondary)" }}>{label}</span>
      <span style={{ color: color ?? "var(--text-primary)", fontFamily: "var(--font-mono)" }}>{value}</span>
    </div>
  );
}

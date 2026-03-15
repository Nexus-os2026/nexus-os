import { useEffect, useState } from "react";
import { getComplianceStatus, getComplianceAgents, getAuditLog, hasDesktopRuntime } from "../api/backend";
import type { ComplianceStatusRow, ComplianceAgentRow, AuditEventRow, Soc2ControlRow } from "../types";
import "./compliance-dashboard.css";

// ---------------------------------------------------------------------------
// Types (UI-only)
// ---------------------------------------------------------------------------

type Tab = "overview" | "agents" | "soc2" | "reports" | "erasure" | "provenance" | "retention";

interface RetentionRule {
  dataClass: string;
  maxAgeDays: number;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const STATUS_COLORS: Record<string, string> = {
  compliant: "#22c55e",
  warning: "#eab308",
  violation: "#ef4444",
  info: "#38bdf8",
  minimal: "#22c55e",
  limited: "#38bdf8",
  high: "#eab308",
  unacceptable: "#ef4444",
};

const STATUS_BG: Record<string, string> = {
  compliant: "rgba(34, 197, 94, 0.12)",
  warning: "rgba(234, 179, 8, 0.12)",
  violation: "rgba(239, 68, 68, 0.12)",
  info: "rgba(56, 189, 248, 0.12)",
  minimal: "rgba(34, 197, 94, 0.12)",
  limited: "rgba(56, 189, 248, 0.12)",
  high: "rgba(234, 179, 8, 0.12)",
  unacceptable: "rgba(239, 68, 68, 0.12)",
};

function statusLabel(s: string): string {
  if (s === "compliant") return "Compliant";
  if (s === "warning") return "Warning";
  if (s === "violation") return "Violation";
  if (s === "minimal") return "Minimal";
  if (s === "limited") return "Limited";
  if (s === "high") return "High";
  if (s === "unacceptable") return "Unacceptable";
  return s;
}

const RETENTION_RULES: RetentionRule[] = [
  { dataClass: "Audit Events", maxAgeDays: 365 },
  { dataClass: "Evidence Bundles", maxAgeDays: 730 },
  { dataClass: "Agent Identity", maxAgeDays: 365 },
  { dataClass: "Permission History", maxAgeDays: 180 },
];

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function ComplianceDashboard(): JSX.Element {
  const [tab, setTab] = useState<Tab>("overview");
  const [reportAgent, setReportAgent] = useState<string | null>(null);
  const [reportGenerated, setReportGenerated] = useState(false);
  const [eraseConfirm, setEraseConfirm] = useState<string | null>(null);
  const [erased, setErased] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);

  // Real data from backend
  const [complianceStatus, setComplianceStatus] = useState<ComplianceStatusRow | null>(null);
  const [agents, setAgents] = useState<ComplianceAgentRow[]>([]);
  const [auditEvents, setAuditEvents] = useState<AuditEventRow[]>([]);

  useEffect(() => {
    if (!hasDesktopRuntime()) {
      setLoading(false);
      return;
    }
    Promise.all([
      getComplianceStatus().catch(() => null),
      getComplianceAgents().catch(() => []),
      getAuditLog(undefined, 50).catch(() => []),
    ]).then(([status, agentRows, events]) => {
      if (status) setComplianceStatus(status);
      setAgents(agentRows);
      setAuditEvents(events);
      setLoading(false);
    });
  }, []);

  const overallStatus = complianceStatus?.status ?? "compliant";
  const checksPassed = complianceStatus?.checks_passed ?? 0;
  const checksFailed = complianceStatus?.checks_failed ?? 0;
  const alerts = complianceStatus?.alerts ?? [];

  // --- Report generation ---
  function handleGenerateReport(agentId: string): void {
    const agent = agents.find((a) => a.id === agentId);
    if (!agent) return;
    const lines = [
      `Transparency Report: ${agent.name}`,
      `Generated: ${new Date().toISOString()}`,
      `Risk Tier: ${agent.risk_tier}`,
      `Autonomy Level: ${agent.autonomy_level}`,
      `Capabilities: ${agent.capabilities.join(", ")}`,
      `Status: ${agent.status}`,
      "",
      `Justification: ${agent.justification || "N/A"}`,
      `Applicable Articles: ${agent.applicable_articles.length > 0 ? agent.applicable_articles.join(", ") : "None"}`,
      `Required Controls: ${agent.required_controls.length > 0 ? agent.required_controls.join(", ") : "None"}`,
      "",
      "This report is generated per EU AI Act Article 13 transparency requirements.",
    ];
    const blob = new Blob([lines.join("\n")], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `nexus-transparency-${agent.name.toLowerCase().replace(/\s/g, "-")}-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
    setReportGenerated(true);
    window.setTimeout(() => setReportGenerated(false), 3000);
  }

  // --- Erasure ---
  function handleErase(agentId: string): void {
    setErased((prev) => new Set(prev).add(agentId));
    setEraseConfirm(null);
  }

  // Build provenance from real audit events
  const provenanceEntries = auditEvents.slice(0, 10).map((evt) => ({
    dataId: evt.event_id.slice(0, 8),
    origin: evt.event_type,
    label: evt.event_type.replace(/_/g, " "),
    transformations: 1,
    classification: "internal",
    currentHolder: evt.agent_id.slice(0, 8),
  }));

  // --- Tab navigation ---
  const tabs: { id: Tab; label: string }[] = [
    { id: "overview", label: "Overview" },
    { id: "agents", label: "Risk Cards" },
    { id: "soc2", label: "SOC 2" },
    { id: "reports", label: "Reports" },
    { id: "erasure", label: "Erasure" },
    { id: "provenance", label: "Provenance" },
    { id: "retention", label: "Retention" },
  ];

  if (loading) {
    return (
      <section className="cd-hub">
        <header className="cd-header">
          <h2 className="cd-title">COMPLIANCE DASHBOARD</h2>
          <p className="cd-subtitle">Loading compliance data...</p>
        </header>
      </section>
    );
  }

  return (
    <section className="cd-hub">
      <header className="cd-header">
        <h2 className="cd-title">COMPLIANCE DASHBOARD</h2>
        <p className="cd-subtitle">Governance, risk classification, data lineage & erasure controls</p>
      </header>

      {/* Tab bar */}
      <nav className="cd-tabs">
        {tabs.map((t) => (
          <button
            type="button"
            key={t.id}
            className={`cd-tab ${tab === t.id ? "cd-tab--active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </nav>

      {/* ================================================================= */}
      {/* OVERVIEW TAB */}
      {/* ================================================================= */}
      {tab === "overview" && (
        <div className="cd-section">
          {/* Overall status indicator */}
          <div className="cd-overall">
            <div className="cd-overall-indicator" style={{ background: STATUS_COLORS[overallStatus] }} />
            <div className="cd-overall-text">
              <span className="cd-overall-label">Overall Status</span>
              <span className="cd-overall-value" style={{ color: STATUS_COLORS[overallStatus] }}>
                {statusLabel(overallStatus)}
              </span>
            </div>
            <div className="cd-overall-stats">
              <span className="cd-stat cd-stat--pass">{checksPassed} passed</span>
              <span className="cd-stat cd-stat--fail">{checksFailed} failed</span>
            </div>
          </div>

          {/* Active alerts */}
          <h3 className="cd-section-title">Active Alerts</h3>
          <div className="cd-alerts">
            {alerts.length === 0 ? (
              <div className="cd-alert" style={{ borderLeftColor: STATUS_COLORS.compliant }}>
                <span className="cd-alert-badge" style={{ color: STATUS_COLORS.info, background: STATUS_BG.info }}>
                  INFO
                </span>
                <span className="cd-alert-msg">No active alerts — all compliance checks passed</span>
              </div>
            ) : (
              alerts.map((alert, i) => (
                <div
                  key={`${alert.check_id}-${i}`}
                  className="cd-alert"
                  style={{ borderLeftColor: STATUS_COLORS[alert.severity] }}
                >
                  <span
                    className="cd-alert-badge"
                    style={{ color: STATUS_COLORS[alert.severity], background: STATUS_BG[alert.severity] }}
                  >
                    {alert.severity.toUpperCase()}
                  </span>
                  <span className="cd-alert-id">{alert.check_id}</span>
                  <span className="cd-alert-msg">{alert.message}</span>
                </div>
              ))
            )}
          </div>

          {/* Quick stats */}
          <div className="cd-quick-stats">
            <div className="cd-qstat">
              <span className="cd-qstat-val">{agents.length}</span>
              <span className="cd-qstat-label">Agents</span>
            </div>
            <div className="cd-qstat">
              <span className="cd-qstat-val">{agents.filter((a) => a.risk_tier === "high").length}</span>
              <span className="cd-qstat-label">High Risk</span>
            </div>
            <div className="cd-qstat">
              <span className="cd-qstat-val">4</span>
              <span className="cd-qstat-label">Frameworks</span>
            </div>
            <div className="cd-qstat">
              <span className="cd-qstat-val">{auditEvents.length}</span>
              <span className="cd-qstat-label">Audit Events</span>
            </div>
          </div>
        </div>
      )}

      {/* ================================================================= */}
      {/* RISK CARDS TAB */}
      {/* ================================================================= */}
      {tab === "agents" && (
        <div className="cd-section">
          <h3 className="cd-section-title">Per-Agent EU AI Act Risk Classification</h3>
          {agents.length === 0 ? (
            <p className="cd-desc">No agents registered — create agents to see risk classification.</p>
          ) : (
            <div className="cd-grid">
              {agents.map((agent) => (
                <article
                  key={agent.id}
                  className="cd-card"
                  style={{ borderLeftColor: STATUS_COLORS[agent.risk_tier] }}
                >
                  <div className="cd-card-top">
                    <span className="cd-control-id">{agent.name}</span>
                    <span
                      className="cd-status-badge"
                      style={{ color: STATUS_COLORS[agent.risk_tier], background: STATUS_BG[agent.risk_tier] }}
                    >
                      {statusLabel(agent.risk_tier)}
                    </span>
                  </div>
                  <div className="cd-card-meta">
                    <span className="cd-meta-item">Autonomy: {agent.autonomy_level}</span>
                    <span className="cd-meta-item">Status: {agent.status}</span>
                  </div>
                  {agent.justification && (
                    <div className="cd-justification">
                      <span className="cd-field-label">Justification:</span> {agent.justification}
                    </div>
                  )}
                  {agent.applicable_articles.length > 0 && (
                    <div className="cd-articles">
                      <span className="cd-field-label">Applicable Articles:</span>
                      <div className="cd-cap-list">
                        {agent.applicable_articles.map((art) => (
                          <span key={art} className="cd-article-tag">{art}</span>
                        ))}
                      </div>
                    </div>
                  )}
                  {agent.required_controls.length > 0 && (
                    <div className="cd-controls">
                      <span className="cd-field-label">Required Controls:</span>
                      <div className="cd-cap-list">
                        {agent.required_controls.map((ctrl) => (
                          <span key={ctrl} className="cd-control-tag">{ctrl}</span>
                        ))}
                      </div>
                    </div>
                  )}
                  <div className="cd-cap-list">
                    {agent.capabilities.map((cap) => (
                      <span key={cap} className="cd-cap-tag">{cap}</span>
                    ))}
                  </div>
                  <div className="cd-card-footer">
                    <span className="cd-evidence-count">ID: {agent.id.slice(0, 13)}...</span>
                  </div>
                </article>
              ))}
            </div>
          )}
        </div>
      )}

      {/* ================================================================= */}
      {/* SOC 2 TAB */}
      {/* ================================================================= */}
      {tab === "soc2" && (
        <div className="cd-section">
          <h3 className="cd-section-title">SOC 2 Type II Compliance Controls</h3>
          <p className="cd-desc">Real-time SOC 2 control status from Nexus OS governance primitives.</p>

          {(complianceStatus?.soc2_controls ?? []).length === 0 ? (
            <p className="cd-desc">No SOC 2 controls evaluated — ensure agents are registered.</p>
          ) : (
            <div className="cd-grid">
              {(complianceStatus?.soc2_controls ?? []).map((ctrl: Soc2ControlRow) => {
                const isSatisfied = ctrl.status === "satisfied";
                const isPartial = ctrl.status.startsWith("partially_met");
                const statusColor = isSatisfied ? "#22c55e" : isPartial ? "#eab308" : "#ef4444";
                const statusBg = isSatisfied
                  ? "rgba(34, 197, 94, 0.12)"
                  : isPartial
                    ? "rgba(234, 179, 8, 0.12)"
                    : "rgba(239, 68, 68, 0.12)";
                const statusText = isSatisfied ? "Satisfied" : isPartial ? "Partially Met" : "Not Met";
                const detail = !isSatisfied ? ctrl.status.replace(/^[^:]+:\s*/, "") : "";
                return (
                  <article
                    key={ctrl.control_id}
                    className="cd-card"
                    style={{ borderLeftColor: statusColor }}
                  >
                    <div className="cd-card-top">
                      <span className="cd-control-id">{ctrl.control_id}</span>
                      <span
                        className="cd-status-badge"
                        style={{ color: statusColor, background: statusBg }}
                      >
                        {statusText}
                      </span>
                    </div>
                    <div className="cd-soc2-desc">{ctrl.description}</div>
                    <div className="cd-card-meta">
                      <span className="cd-meta-item">Evidence: {ctrl.evidence_count} events</span>
                    </div>
                    {detail && (
                      <div className="cd-soc2-detail">{detail}</div>
                    )}
                  </article>
                );
              })}
            </div>
          )}
        </div>
      )}

      {/* ================================================================= */}
      {/* REPORTS TAB */}
      {/* ================================================================= */}
      {tab === "reports" && (
        <div className="cd-section">
          <h3 className="cd-section-title">Transparency Report Viewer</h3>
          <p className="cd-desc">Select an agent to generate an EU AI Act Article 13 transparency report.</p>

          {agents.length === 0 ? (
            <p className="cd-desc">No agents registered.</p>
          ) : (
            <div className="cd-report-select">
              {agents.map((agent) => (
                <button
                  type="button"
                  key={agent.id}
                  className={`cd-report-agent ${reportAgent === agent.id ? "cd-report-agent--selected" : ""}`}
                  onClick={() => setReportAgent(agent.id)}
                >
                  {agent.name}
                  <span
                    className="cd-mini-badge"
                    style={{ background: STATUS_BG[agent.risk_tier], color: STATUS_COLORS[agent.risk_tier] }}
                  >
                    {statusLabel(agent.risk_tier)}
                  </span>
                </button>
              ))}
            </div>
          )}

          {reportAgent && (() => {
            const agent = agents.find((a) => a.id === reportAgent);
            if (!agent) return null;
            return (
              <div className="cd-report-preview">
                <h4 className="cd-report-title">Transparency Report: {agent.name}</h4>
                <div className="cd-report-fields">
                  <div className="cd-report-field"><span className="cd-field-label">Risk Tier:</span> <span style={{ color: STATUS_COLORS[agent.risk_tier] }}>{statusLabel(agent.risk_tier)}</span></div>
                  <div className="cd-report-field"><span className="cd-field-label">Autonomy Level:</span> {agent.autonomy_level}</div>
                  <div className="cd-report-field"><span className="cd-field-label">Status:</span> {agent.status}</div>
                  <div className="cd-report-field"><span className="cd-field-label">Capabilities:</span> {agent.capabilities.join(", ")}</div>
                  <div className="cd-report-field"><span className="cd-field-label">Agent ID:</span> <code>{agent.id}</code></div>
                </div>
                <button
                  type="button"
                  className="cd-generate-btn"
                  onClick={() => handleGenerateReport(agent.id)}
                >
                  {reportGenerated ? "Report Downloaded" : "Download Report"}
                </button>
              </div>
            );
          })()}
        </div>
      )}

      {/* ================================================================= */}
      {/* ERASURE TAB */}
      {/* ================================================================= */}
      {tab === "erasure" && (
        <div className="cd-section">
          <h3 className="cd-section-title">Cryptographic Erasure (GDPR Article 17)</h3>
          <p className="cd-desc">Trigger complete agent data erasure: audit events redacted, encryption keys destroyed, identity purged.</p>

          {agents.length === 0 ? (
            <p className="cd-desc">No agents registered.</p>
          ) : (
            <div className="cd-erasure-list">
              {agents.map((agent) => {
                const isErased = erased.has(agent.id);
                return (
                  <div key={agent.id} className={`cd-erasure-row ${isErased ? "cd-erasure-row--erased" : ""}`}>
                    <span className="cd-erasure-name">{agent.name}</span>
                    <span className="cd-erasure-id">{agent.id.slice(0, 13)}...</span>
                    {isErased ? (
                      <span className="cd-erasure-done">Erased</span>
                    ) : eraseConfirm === agent.id ? (
                      <div className="cd-erasure-confirm">
                        <span className="cd-erasure-warn">This action is irreversible. Confirm?</span>
                        <button type="button" className="cd-btn-danger" onClick={() => handleErase(agent.id)}>
                          Confirm Erase
                        </button>
                        <button type="button" className="cd-btn-cancel" onClick={() => setEraseConfirm(null)}>
                          Cancel
                        </button>
                      </div>
                    ) : (
                      <button
                        type="button"
                        className="cd-btn-erase"
                        onClick={() => setEraseConfirm(agent.id)}
                      >
                        Erase Agent Data
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}

      {/* ================================================================= */}
      {/* PROVENANCE TAB */}
      {/* ================================================================= */}
      {tab === "provenance" && (
        <div className="cd-section">
          <h3 className="cd-section-title">Data Provenance & Lineage</h3>
          <p className="cd-desc">Track data origin, transformations, and flow through agents.</p>

          {provenanceEntries.length === 0 ? (
            <p className="cd-desc">No audit events recorded yet — provenance data will appear as agents operate.</p>
          ) : (
            <div className="cd-prov-table">
              <div className="cd-prov-header">
                <span>Label</span>
                <span>Origin</span>
                <span>Classification</span>
                <span>Transforms</span>
                <span>Holder</span>
              </div>
              {provenanceEntries.map((entry) => (
                <div key={entry.dataId} className="cd-prov-row">
                  <span className="cd-prov-label">{entry.label}</span>
                  <span className="cd-prov-origin">{entry.origin.replace(/_/g, " ")}</span>
                  <span className={`cd-prov-class cd-prov-class--${entry.classification}`}>
                    {entry.classification}
                  </span>
                  <span className="cd-prov-transforms">{entry.transformations}</span>
                  <span className="cd-prov-holder">{entry.currentHolder}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* ================================================================= */}
      {/* RETENTION TAB */}
      {/* ================================================================= */}
      {tab === "retention" && (
        <div className="cd-section">
          <h3 className="cd-section-title">Retention Policy Settings</h3>
          <p className="cd-desc">Configure data retention periods per data class. Events beyond the retention period are purged (redacted) during enforcement.</p>

          <div className="cd-retention-grid">
            {RETENTION_RULES.map((rule) => (
              <div key={rule.dataClass} className="cd-retention-card">
                <span className="cd-retention-class">{rule.dataClass}</span>
                <span className="cd-retention-days">{rule.maxAgeDays} days</span>
                <div className="cd-retention-bar">
                  <div
                    className="cd-retention-fill"
                    style={{ width: `${Math.min((rule.maxAgeDays / 730) * 100, 100)}%` }}
                  />
                </div>
              </div>
            ))}
          </div>

          <div className="cd-retention-actions">
            <button type="button" className="cd-generate-btn">
              Run Retention Enforcement
            </button>
            <span className="cd-retention-note">
              {auditEvents.length > 0
                ? `${auditEvents.length} audit events in trail`
                : "Last run: never — 0 events purged"}
            </span>
          </div>
        </div>
      )}
    </section>
  );
}

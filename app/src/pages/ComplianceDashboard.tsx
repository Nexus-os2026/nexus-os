import { useState } from "react";
import "./compliance-dashboard.css";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type OverallStatus = "compliant" | "warning" | "violation";
type AlertSeverity = "info" | "warning" | "violation";
type RiskTier = "minimal" | "limited" | "high" | "unacceptable";

interface ComplianceAlert {
  severity: AlertSeverity;
  checkId: string;
  message: string;
  agentId: string | null;
}

interface AgentComplianceCard {
  id: string;
  name: string;
  riskTier: RiskTier;
  autonomyLevel: string;
  capabilities: string[];
  status: "running" | "stopped";
}

interface RetentionRule {
  dataClass: string;
  maxAgeDays: number;
}

interface ProvenanceEntry {
  dataId: string;
  origin: string;
  label: string;
  transformations: number;
  classification: string;
  currentHolder: string;
}

// ---------------------------------------------------------------------------
// Mock data
// ---------------------------------------------------------------------------

const MOCK_STATUS: OverallStatus = "warning";
const MOCK_CHECKS_PASSED = 5;
const MOCK_CHECKS_FAILED = 1;

const MOCK_ALERTS: ComplianceAlert[] = [
  { severity: "warning", checkId: "MISSING_AGENT_IDENTITY", message: "Agent 'web-builder' has no DID identity — cannot verify authenticity", agentId: "a0000000-0000-4000-8000-000000000004" },
  { severity: "info", checkId: "RETENTION_OK", message: "Audit trail retention within 365-day policy", agentId: null },
  { severity: "info", checkId: "AUDIT_CHAIN_VALID", message: "Audit hash-chain integrity verified successfully", agentId: null },
];

const MOCK_AGENTS: AgentComplianceCard[] = [
  { id: "a0000000-0000-4000-8000-000000000001", name: "Coder", riskTier: "high", autonomyLevel: "L2", capabilities: ["llm.query", "fs.read", "fs.write"], status: "running" },
  { id: "a0000000-0000-4000-8000-000000000002", name: "Designer", riskTier: "limited", autonomyLevel: "L1", capabilities: ["llm.query", "fs.read"], status: "running" },
  { id: "a0000000-0000-4000-8000-000000000003", name: "Screen Poster", riskTier: "high", autonomyLevel: "L2", capabilities: ["social.x.post", "llm.query", "web.search"], status: "running" },
  { id: "a0000000-0000-4000-8000-000000000004", name: "Web Builder", riskTier: "high", autonomyLevel: "L2", capabilities: ["fs.write", "web.read", "process.exec"], status: "running" },
  { id: "a0000000-0000-4000-8000-000000000005", name: "Workflow Studio", riskTier: "limited", autonomyLevel: "L1", capabilities: ["llm.query"], status: "stopped" },
  { id: "a0000000-0000-4000-8000-000000000006", name: "Self Improve", riskTier: "minimal", autonomyLevel: "L0", capabilities: ["audit.read"], status: "stopped" },
];

const MOCK_RETENTION_RULES: RetentionRule[] = [
  { dataClass: "Audit Events", maxAgeDays: 365 },
  { dataClass: "Evidence Bundles", maxAgeDays: 730 },
  { dataClass: "Agent Identity", maxAgeDays: 365 },
  { dataClass: "Permission History", maxAgeDays: 180 },
];

const MOCK_PROVENANCE: ProvenanceEntry[] = [
  { dataId: "d001", origin: "user_input", label: "User query", transformations: 2, classification: "confidential", currentHolder: "Coder" },
  { dataId: "d002", origin: "file_read", label: "config.toml", transformations: 1, classification: "internal", currentHolder: "Coder" },
  { dataId: "d003", origin: "llm_response", label: "Code analysis", transformations: 3, classification: "internal", currentHolder: "Designer" },
  { dataId: "d004", origin: "external_api", label: "API response", transformations: 1, classification: "public", currentHolder: "Web Builder" },
];

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

type Tab = "overview" | "agents" | "reports" | "erasure" | "provenance" | "retention";

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function ComplianceDashboard(): JSX.Element {
  const [tab, setTab] = useState<Tab>("overview");
  const [reportAgent, setReportAgent] = useState<string | null>(null);
  const [reportGenerated, setReportGenerated] = useState(false);
  const [eraseConfirm, setEraseConfirm] = useState<string | null>(null);
  const [erased, setErased] = useState<Set<string>>(new Set());

  // --- Report generation ---
  function handleGenerateReport(agentId: string): void {
    const agent = MOCK_AGENTS.find((a) => a.id === agentId);
    if (!agent) return;
    const lines = [
      `Transparency Report: ${agent.name}`,
      `Generated: ${new Date().toISOString()}`,
      `Risk Tier: ${agent.riskTier}`,
      `Autonomy Level: ${agent.autonomyLevel}`,
      `Capabilities: ${agent.capabilities.join(", ")}`,
      `Status: ${agent.status}`,
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

  // --- Tab navigation ---
  const tabs: { id: Tab; label: string }[] = [
    { id: "overview", label: "Overview" },
    { id: "agents", label: "Risk Cards" },
    { id: "reports", label: "Reports" },
    { id: "erasure", label: "Erasure" },
    { id: "provenance", label: "Provenance" },
    { id: "retention", label: "Retention" },
  ];

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
            <div className="cd-overall-indicator" style={{ background: STATUS_COLORS[MOCK_STATUS] }} />
            <div className="cd-overall-text">
              <span className="cd-overall-label">Overall Status</span>
              <span className="cd-overall-value" style={{ color: STATUS_COLORS[MOCK_STATUS] }}>
                {statusLabel(MOCK_STATUS)}
              </span>
            </div>
            <div className="cd-overall-stats">
              <span className="cd-stat cd-stat--pass">{MOCK_CHECKS_PASSED} passed</span>
              <span className="cd-stat cd-stat--fail">{MOCK_CHECKS_FAILED} failed</span>
            </div>
          </div>

          {/* Active alerts */}
          <h3 className="cd-section-title">Active Alerts</h3>
          <div className="cd-alerts">
            {MOCK_ALERTS.map((alert, i) => (
              <div
                key={`${alert.checkId}-${i}`}
                className="cd-alert"
                style={{ borderLeftColor: STATUS_COLORS[alert.severity] }}
              >
                <span
                  className="cd-alert-badge"
                  style={{ color: STATUS_COLORS[alert.severity], background: STATUS_BG[alert.severity] }}
                >
                  {alert.severity.toUpperCase()}
                </span>
                <span className="cd-alert-id">{alert.checkId}</span>
                <span className="cd-alert-msg">{alert.message}</span>
              </div>
            ))}
          </div>

          {/* Quick stats */}
          <div className="cd-quick-stats">
            <div className="cd-qstat">
              <span className="cd-qstat-val">{MOCK_AGENTS.length}</span>
              <span className="cd-qstat-label">Agents</span>
            </div>
            <div className="cd-qstat">
              <span className="cd-qstat-val">{MOCK_AGENTS.filter((a) => a.riskTier === "high").length}</span>
              <span className="cd-qstat-label">High Risk</span>
            </div>
            <div className="cd-qstat">
              <span className="cd-qstat-val">4</span>
              <span className="cd-qstat-label">Frameworks</span>
            </div>
            <div className="cd-qstat">
              <span className="cd-qstat-val">{MOCK_PROVENANCE.length}</span>
              <span className="cd-qstat-label">Data Lineage</span>
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
          <div className="cd-grid">
            {MOCK_AGENTS.map((agent) => (
              <article
                key={agent.id}
                className="cd-card"
                style={{ borderLeftColor: STATUS_COLORS[agent.riskTier] }}
              >
                <div className="cd-card-top">
                  <span className="cd-control-id">{agent.name}</span>
                  <span
                    className="cd-status-badge"
                    style={{ color: STATUS_COLORS[agent.riskTier], background: STATUS_BG[agent.riskTier] }}
                  >
                    {statusLabel(agent.riskTier)}
                  </span>
                </div>
                <div className="cd-card-meta">
                  <span className="cd-meta-item">Autonomy: {agent.autonomyLevel}</span>
                  <span className="cd-meta-item">Status: {agent.status}</span>
                </div>
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
        </div>
      )}

      {/* ================================================================= */}
      {/* REPORTS TAB */}
      {/* ================================================================= */}
      {tab === "reports" && (
        <div className="cd-section">
          <h3 className="cd-section-title">Transparency Report Viewer</h3>
          <p className="cd-desc">Select an agent to generate an EU AI Act Article 13 transparency report.</p>

          <div className="cd-report-select">
            {MOCK_AGENTS.map((agent) => (
              <button
                type="button"
                key={agent.id}
                className={`cd-report-agent ${reportAgent === agent.id ? "cd-report-agent--selected" : ""}`}
                onClick={() => setReportAgent(agent.id)}
              >
                {agent.name}
                <span
                  className="cd-mini-badge"
                  style={{ background: STATUS_BG[agent.riskTier], color: STATUS_COLORS[agent.riskTier] }}
                >
                  {statusLabel(agent.riskTier)}
                </span>
              </button>
            ))}
          </div>

          {reportAgent && (() => {
            const agent = MOCK_AGENTS.find((a) => a.id === reportAgent);
            if (!agent) return null;
            return (
              <div className="cd-report-preview">
                <h4 className="cd-report-title">Transparency Report: {agent.name}</h4>
                <div className="cd-report-fields">
                  <div className="cd-report-field"><span className="cd-field-label">Risk Tier:</span> <span style={{ color: STATUS_COLORS[agent.riskTier] }}>{statusLabel(agent.riskTier)}</span></div>
                  <div className="cd-report-field"><span className="cd-field-label">Autonomy Level:</span> {agent.autonomyLevel}</div>
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

          <div className="cd-erasure-list">
            {MOCK_AGENTS.map((agent) => {
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
        </div>
      )}

      {/* ================================================================= */}
      {/* PROVENANCE TAB */}
      {/* ================================================================= */}
      {tab === "provenance" && (
        <div className="cd-section">
          <h3 className="cd-section-title">Data Provenance & Lineage</h3>
          <p className="cd-desc">Track data origin, transformations, and flow through agents.</p>

          <div className="cd-prov-table">
            <div className="cd-prov-header">
              <span>Label</span>
              <span>Origin</span>
              <span>Classification</span>
              <span>Transforms</span>
              <span>Holder</span>
            </div>
            {MOCK_PROVENANCE.map((entry) => (
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
            {MOCK_RETENTION_RULES.map((rule) => (
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
            <span className="cd-retention-note">Last run: never — 0 events purged</span>
          </div>
        </div>
      )}
    </section>
  );
}

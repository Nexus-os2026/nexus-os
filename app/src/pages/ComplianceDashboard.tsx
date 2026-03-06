import { useState } from "react";
import "./compliance-dashboard.css";

interface ComplianceControl {
  id: string;
  description: string;
  status: "Satisfied" | "PartiallyMet" | "NotMet";
  evidenceCount: number;
}

const SOC2_CONTROLS: ComplianceControl[] = [
  { id: "CC6.1", description: "Logical and physical access controls are implemented to protect information assets", status: "Satisfied", evidenceCount: 24 },
  { id: "CC6.2", description: "System access is authenticated and authorized prior to granting access", status: "Satisfied", evidenceCount: 18 },
  { id: "CC6.3", description: "Access to data and software is restricted to authorized personnel", status: "PartiallyMet", evidenceCount: 12 },
  { id: "CC7.1", description: "Change management processes are in place to manage system changes", status: "Satisfied", evidenceCount: 31 },
  { id: "CC7.2", description: "System components are monitored and anomalies are identified and addressed", status: "NotMet", evidenceCount: 5 },
];

const STATUS_COLORS: Record<string, string> = {
  Satisfied: "#22c55e",
  PartiallyMet: "#eab308",
  NotMet: "#ef4444",
};

const STATUS_BG: Record<string, string> = {
  Satisfied: "rgba(34, 197, 94, 0.12)",
  PartiallyMet: "rgba(234, 179, 8, 0.12)",
  NotMet: "rgba(239, 68, 68, 0.12)",
};

export default function ComplianceDashboard(): JSX.Element {
  const satisfied = SOC2_CONTROLS.filter((c) => c.status === "Satisfied").length;
  const total = SOC2_CONTROLS.length;
  const [reportGenerated, setReportGenerated] = useState(false);

  function handleGenerateReport(): void {
    const lines = [
      "SOC 2 Type II Compliance Report",
      `Generated: ${new Date().toISOString()}`,
      `Framework: Trust Services Criteria`,
      "",
      `Overall: ${satisfied}/${total} controls satisfied`,
      "",
      ...SOC2_CONTROLS.map((c) => `${c.id} - ${c.status} (${c.evidenceCount} evidence items) - ${c.description}`),
    ];
    const blob = new Blob([lines.join("\n")], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `nexus-soc2-report-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
    setReportGenerated(true);
    window.setTimeout(() => setReportGenerated(false), 3000);
  }

  return (
    <section className="cd-hub">
      <header className="cd-header">
        <h2 className="cd-title">COMPLIANCE DASHBOARD // SOC2</h2>
        <p className="cd-subtitle">Framework: SOC 2 Type II Trust Services Criteria</p>
      </header>

      <div className="cd-summary">
        <div className="cd-summary-bar">
          <div className="cd-summary-fill" style={{ width: `${(satisfied / total) * 100}%` }} />
        </div>
        <p className="cd-summary-text">
          <span className="cd-summary-count">{satisfied}</span> of <span className="cd-summary-count">{total}</span> controls satisfied
        </p>
        <button type="button" className="cd-generate-btn" onClick={handleGenerateReport}>
          {reportGenerated ? "Report Downloaded" : "Generate Report"}
        </button>
      </div>

      <div className="cd-grid">
        {SOC2_CONTROLS.map((control) => (
          <article key={control.id} className="cd-card" style={{ borderLeftColor: STATUS_COLORS[control.status] }}>
            <div className="cd-card-top">
              <span className="cd-control-id">{control.id}</span>
              <span
                className="cd-status-badge"
                style={{ color: STATUS_COLORS[control.status], background: STATUS_BG[control.status] }}
              >
                {control.status === "PartiallyMet" ? "Partially Met" : control.status === "NotMet" ? "Not Met" : "Satisfied"}
              </span>
            </div>
            <p className="cd-card-desc">{control.description}</p>
            <div className="cd-card-footer">
              <span className="cd-evidence-count">{control.evidenceCount} evidence items</span>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

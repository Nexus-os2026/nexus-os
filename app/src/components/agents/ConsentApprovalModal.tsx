import { useState } from "react";
import type { ApprovalDisplay } from "../../types";
import "./consent-approval-modal.css";

const RISK_COLORS: Record<string, string> = {
  "LOW RISK": "consent-risk-low",
  "MEDIUM RISK": "consent-risk-medium",
  "HIGH RISK \u2014 Review Carefully": "consent-risk-high",
  "CRITICAL RISK \u2014 Verify All Parameters": "consent-risk-critical",
};

function riskClassName(badge: string): string {
  for (const [key, cls] of Object.entries(RISK_COLORS)) {
    if (badge.startsWith(key.split(" ")[0])) return cls;
  }
  return "consent-risk-medium";
}

interface ConsentApprovalModalProps {
  display: ApprovalDisplay;
  requestId: string;
  onApprove: (requestId: string) => void;
  onDeny: (requestId: string) => void;
}

export function ConsentApprovalModal({
  display,
  requestId,
  onApprove,
  onDeny,
}: ConsentApprovalModalProps) {
  const [showRaw, setShowRaw] = useState(false);

  return (
    <div className="consent-modal-backdrop" onClick={() => onDeny(requestId)}>
      <div
        className="consent-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="consent-summary">{display.summary}</h3>

        <div className={`consent-risk-badge ${riskClassName(display.risk_badge)}`}>
          {display.risk_badge}
        </div>

        {display.warnings.length > 0 && (
          <ul className="consent-warnings">
            {display.warnings.map((w, i) => (
              <li key={i} className="consent-warning-item">
                <span className="consent-warning-icon" aria-hidden="true">
                  &#9888;
                </span>
                {w}
              </li>
            ))}
          </ul>
        )}

        <table className="consent-details-table">
          <tbody>
            {display.details.map(([key, value], i) => (
              <tr key={i}>
                <td className="consent-detail-key">{key}</td>
                <td className="consent-detail-value">
                  <code>{value}</code>
                </td>
              </tr>
            ))}
          </tbody>
        </table>

        {display.agent_description && display.agent_provided && (
          <div className="consent-agent-desc">
            <span className="consent-agent-desc-label">
              Agent's Description (unverified)
            </span>
            <p className="consent-agent-desc-text">
              {display.agent_description}
            </p>
          </div>
        )}

        <div className="consent-raw-toggle">
          <button
            className="consent-raw-btn"
            onClick={() => setShowRaw(!showRaw)}
            type="button"
          >
            {showRaw ? "Hide Raw View" : "Show Raw View"}
          </button>
          {showRaw && (
            <pre className="consent-raw-view">{display.raw_command}</pre>
          )}
        </div>

        <div className="consent-modal-actions">
          <button
            className="consent-btn-deny"
            onClick={() => onDeny(requestId)}
            type="button"
          >
            Deny
          </button>
          <button
            className="consent-btn-approve"
            onClick={() => onApprove(requestId)}
            type="button"
          >
            Approve
          </button>
        </div>
      </div>
    </div>
  );
}

export default ConsentApprovalModal;

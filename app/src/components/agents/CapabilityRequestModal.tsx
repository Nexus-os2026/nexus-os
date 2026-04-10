import type { CapabilityRequest, PermissionRiskLevel } from "../../types";
import "../../pages/permission-dashboard.css";

const RISK_LABELS: Record<PermissionRiskLevel, { label: string; className: string }> = {
  low: { label: "Low", className: "risk-low" },
  medium: { label: "Medium", className: "risk-medium" },
  high: { label: "High", className: "risk-high" },
  critical: { label: "Critical", className: "risk-critical" },
};

interface CapabilityRequestModalProps {
  request: CapabilityRequest;
  onApprove: (capabilityKey: string) => void;
  onDeny: () => void;
}

export function CapabilityRequestModal({
  request,
  onApprove,
  onDeny,
}: CapabilityRequestModalProps) {
  const risk = RISK_LABELS[request.risk_level];

  return (
    <div className="perm-modal-backdrop" onClick={onDeny}>
      <div className="perm-modal perm-request-modal" onClick={(e) => e.stopPropagation()}>
        <h3>Capability Request</h3>
        <p className="perm-request-reason">Reason: {request.reason}</p>
        <div className="perm-request-comparison">
          <div className="perm-request-col">
            <h4>Current Permissions</h4>
            <ul>
              {request.current_capabilities.map((c) => (
                <li key={c} className="perm-request-cap">{c}</li>
              ))}
            </ul>
          </div>
          <div className="perm-request-arrow">&rarr;</div>
          <div className="perm-request-col">
            <h4>Requested Permissions</h4>
            <ul>
              {request.requested_capabilities.map((c) => (
                <li
                  key={c}
                  className={`perm-request-cap ${!request.current_capabilities.includes(c) ? "perm-request-new" : ""}`}
                >
                  {c}{" "}
                  {!request.current_capabilities.includes(c) && (
                    <span className="perm-new-badge">NEW</span>
                  )}
                </li>
              ))}
            </ul>
          </div>
        </div>
        <div className={`perm-request-risk ${risk.className}`}>
          Risk: {risk.label}
        </div>
        <details className="perm-request-explain">
          <summary>What does this mean?</summary>
          <p>
            Granting &ldquo;{request.requested_capability}&rdquo; allows this agent to
            perform the associated action. Review the risk level before approving.
          </p>
        </details>
        <div className="perm-modal-actions">
          <button type="button" className="perm-modal-cancel" onClick={onDeny}>
            Deny
          </button>
          <button type="button"
            className="perm-modal-confirm"
            onClick={() => onApprove(request.requested_capability)}
          >
            Approve
          </button>
        </div>
      </div>
    </div>
  );
}

export default CapabilityRequestModal;

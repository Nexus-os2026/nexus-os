import type { SimulationPreview } from "../../types";

interface SpeculativePreviewProps {
  preview: SimulationPreview;
  onApprove: () => void;
  onReject: () => void;
}

function riskColor(risk: string): string {
  switch (risk) {
    case "critical":
      return "#ef4444";
    case "high":
      return "#f97316";
    case "medium":
      return "#f59e0b";
    case "low":
      return "#22c55e";
    default:
      return "#94a3b8";
  }
}

function riskLabel(risk: string): string {
  return risk.charAt(0).toUpperCase() + risk.slice(1);
}

export function SpeculativePreview({
  preview,
  onApprove,
  onReject
}: SpeculativePreviewProps): JSX.Element {
  const color = riskColor(preview.risk_level);

  return (
    <div className="speculative-preview">
      <div className="speculative-preview-header">
        <h4 className="speculative-preview-title">
          Speculative Preview
        </h4>
        <span
          className="speculative-risk-badge"
          style={{ borderColor: color, color }}
        >
          {riskLabel(preview.risk_level)} Risk
        </span>
      </div>

      <p className="speculative-summary">{preview.summary}</p>

      <div className="speculative-subtitle">If this proceeds:</div>

      {preview.predicted_changes.length > 0 && (
        <ul className="speculative-changes">
          {preview.predicted_changes.map((change, i) => (
            <li key={i} className="speculative-change-item">
              {change.type === "file_change" && (
                <span className="speculative-change-file">
                  <span className="speculative-icon">📄</span>
                  {change.change_kind} {change.path}
                  {change.size_after != null && (
                    <span className="speculative-size">
                      {" "}({change.size_after} bytes)
                    </span>
                  )}
                </span>
              )}
              {change.type === "network_call" && (
                <span className="speculative-change-network">
                  <span className="speculative-icon">🌐</span>
                  {change.method} → {change.target}
                </span>
              )}
              {change.type === "data_modification" && (
                <span className="speculative-change-data">
                  <span className="speculative-icon">⚙️</span>
                  {change.description}
                </span>
              )}
              {change.type === "llm_call" && (
                <span className="speculative-change-llm">
                  <span className="speculative-icon">🧠</span>
                  LLM call: {change.prompt_len} chars, max {change.max_tokens} tokens ({change.estimated_fuel} fuel)
                </span>
              )}
            </li>
          ))}
        </ul>
      )}

      <div className="speculative-impact">
        <div className="speculative-impact-row">
          <span className="speculative-impact-label">Fuel Cost</span>
          <span className="speculative-impact-value">{preview.resource_impact.fuel_cost}</span>
        </div>
        {preview.resource_impact.llm_calls > 0 && (
          <div className="speculative-impact-row">
            <span className="speculative-impact-label">LLM Calls</span>
            <span className="speculative-impact-value">{preview.resource_impact.llm_calls}</span>
          </div>
        )}
        {preview.resource_impact.network_calls > 0 && (
          <div className="speculative-impact-row">
            <span className="speculative-impact-label">Network Calls</span>
            <span className="speculative-impact-value">{preview.resource_impact.network_calls}</span>
          </div>
        )}
        {preview.resource_impact.file_operations > 0 && (
          <div className="speculative-impact-row">
            <span className="speculative-impact-label">File Operations</span>
            <span className="speculative-impact-value">{preview.resource_impact.file_operations}</span>
          </div>
        )}
        {preview.resource_impact.disk_bytes_delta !== 0 && (
          <div className="speculative-impact-row">
            <span className="speculative-impact-label">Disk Impact</span>
            <span className="speculative-impact-value">
              {preview.resource_impact.disk_bytes_delta > 0 ? "+" : ""}
              {preview.resource_impact.disk_bytes_delta} bytes
            </span>
          </div>
        )}
      </div>

      <div className="speculative-actions">
        <button
          type="button"
          className="speculative-btn approve"
          onClick={onApprove}
        >
          Approve
        </button>
        <button
          type="button"
          className="speculative-btn reject"
          onClick={onReject}
        >
          Reject
        </button>
      </div>
    </div>
  );
}

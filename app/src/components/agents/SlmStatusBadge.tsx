import type { SlmStatus } from "../../types";

interface SlmStatusBadgeProps {
  status: SlmStatus;
}

function routingLabel(routing: SlmStatus["governance_routing"]): string {
  switch (routing) {
    case "local":
      return "LOCAL";
    case "cloud":
      return "CLOUD";
    case "fallback":
      return "FALLBACK";
  }
}

function routingColor(routing: SlmStatus["governance_routing"]): string {
  switch (routing) {
    case "local":
      return "#00ffd5";
    case "cloud":
      return "#60a5fa";
    case "fallback":
      return "#f59e0b";
  }
}

function latencyIndicator(ms: number): { label: string; color: string } {
  if (ms === 0) {
    return { label: "N/A", color: "#6b7280" };
  }
  if (ms < 100) {
    return { label: `${ms}ms`, color: "#00ffd5" };
  }
  if (ms < 500) {
    return { label: `${ms}ms`, color: "#f59e0b" };
  }
  return { label: `${ms}ms`, color: "#ef4444" };
}

export function SlmStatusBadge({ status }: SlmStatusBadgeProps): JSX.Element {
  const latency = latencyIndicator(status.avg_latency_ms);
  const routing = routingLabel(status.governance_routing);
  const routeColor = routingColor(status.governance_routing);

  return (
    <div className="slm-status-badge">
      <div className="slm-badge-row">
        <span
          className="slm-model-dot"
          style={{ background: status.loaded ? "#00ffd5" : "#6b7280" }}
        />
        <span className="slm-badge-label">
          {status.loaded ? status.model_id ?? "SLM" : "No Model"}
        </span>
      </div>

      <div className="slm-badge-metrics">
        <span className="slm-metric">
          <span className="slm-metric-label">Latency</span>
          <span className="slm-metric-value" style={{ color: latency.color }}>
            {latency.label}
          </span>
        </span>

        <span className="slm-metric">
          <span className="slm-metric-label">Routing</span>
          <span className="slm-metric-value" style={{ color: routeColor }}>
            {routing}
          </span>
        </span>

        {status.loaded && status.ram_usage_mb > 0 && (
          <span className="slm-metric">
            <span className="slm-metric-label">RAM</span>
            <span className="slm-metric-value">{status.ram_usage_mb}MB</span>
          </span>
        )}
      </div>
    </div>
  );
}

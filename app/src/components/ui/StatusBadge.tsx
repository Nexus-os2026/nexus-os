type RuntimeStatus = "Running" | "Paused" | "Stopped" | string;

interface StatusBadgeProps {
  status: RuntimeStatus;
}

function normalizeStatus(status: RuntimeStatus): "running" | "paused" | "stopped" {
  const lowered = status.toLowerCase();
  if (lowered === "running") {
    return "running";
  }
  if (lowered === "paused") {
    return "paused";
  }
  return "stopped";
}

export function StatusBadge({ status }: StatusBadgeProps): JSX.Element {
  const normalized = normalizeStatus(status);
  return (
    <span className={`status-badge status-badge--${normalized}`}>
      <span className="status-badge__dot" />
      {status}
    </span>
  );
}

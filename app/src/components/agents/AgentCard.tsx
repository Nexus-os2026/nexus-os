import type { AgentSummary, AuditEventRow } from "../../types";
import { Avatar } from "./Avatar";

interface AgentCardProps {
  agent: AgentSummary;
  selected: boolean;
  latestEvent: AuditEventRow | null;
  onOpen: (agentId: string) => void;
  onStart: (agentId: string) => void;
  onPause: (agentId: string) => void;
  onStop: (agentId: string) => void;
  onLogs: (agentId: string) => void;
}

function statusTone(status: AgentSummary["status"]): "running" | "paused" | "stopped" | "idle" {
  if (status === "Running" || status === "Starting") {
    return "running";
  }
  if (status === "Paused") {
    return "paused";
  }
  if (status === "Stopped" || status === "Stopping" || status === "Destroyed") {
    return "stopped";
  }
  return "idle";
}

function gaugeColor(percentage: number): string {
  if (percentage > 50) {
    return "#00f0ff";
  }
  if (percentage > 20) {
    return "#ffaa00";
  }
  return "#ff0040";
}

function fuelPercentage(fuelRemaining: number): number {
  return Math.max(0, Math.min(100, Math.round(fuelRemaining / 100)));
}

function summarizePayload(payload: Record<string, unknown>): string {
  const compact = JSON.stringify(payload);
  if (compact.length <= 80) {
    return compact;
  }
  return `${compact.slice(0, 77)}...`;
}

function formatEventTimestamp(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleTimeString("en-GB", {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  });
}

function inferRole(agentName: string): "coding" | "social" | "design" | "general" {
  const lowered = agentName.toLowerCase();
  if (lowered.includes("code") || lowered.includes("dev") || lowered.includes("rust")) {
    return "coding";
  }
  if (lowered.includes("social") || lowered.includes("content") || lowered.includes("post")) {
    return "social";
  }
  if (lowered.includes("design") || lowered.includes("web") || lowered.includes("ui")) {
    return "design";
  }
  return "general";
}

export function AgentCard({
  agent,
  selected,
  latestEvent,
  onOpen,
  onStart,
  onPause,
  onStop,
  onLogs
}: AgentCardProps): JSX.Element {
  const percentage = fuelPercentage(agent.fuel_remaining);
  const stroke = gaugeColor(percentage);
  const radius = 44;
  const circumference = 2 * Math.PI * radius;
  const dashOffset = circumference * (1 - percentage / 100);
  const tone = statusTone(agent.status);

  const eventLine = latestEvent
    ? `${formatEventTimestamp(latestEvent.timestamp)} // ${summarizePayload(latestEvent.payload)}`
    : `${new Date().toLocaleTimeString("en-GB", { hour12: false })} // ${agent.last_action}`;
  const role = inferRole(agent.name);
  const avatarState = tone === "running" ? "running" : tone === "paused" ? "paused" : tone === "stopped" ? "stopped" : "idle";

  return (
    <article
      className={`agent-card ${selected ? "active" : ""}`}
      onClick={() => onOpen(agent.id)}
      aria-label={`Agent ${agent.name}`}
    >
      <span className={`agent-card-accent ${tone}`} />

      <header className="agent-card-head">
        <div>
          <div className="agent-card-identity">
            <Avatar agentName={agent.name} role={role} state={avatarState} />
            <h3 className="agent-card-title">{agent.name}</h3>
          </div>
          <span className={`agent-status-badge ${tone}`}>
            <span className="agent-status-dot" />
            {agent.status}
          </span>
        </div>

        <div className="agent-card-gauge-wrap">
          <svg className="agent-gauge" viewBox="0 0 108 108" aria-hidden="true">
            <circle className="agent-gauge-bg" cx="54" cy="54" r={radius} />
            <circle
              className="agent-gauge-fill"
              cx="54"
              cy="54"
              r={radius}
              stroke={stroke}
              strokeDasharray={circumference}
              strokeDashoffset={dashOffset}
            />
            <text className="agent-gauge-value" x="54" y="54">
              {percentage}%
            </text>
          </svg>
          <span className="agent-gauge-sub">Fuel</span>
        </div>
      </header>

      <p className="agent-card-last">Last action: {eventLine}</p>

      <div className="agent-card-actions">
        <button
          type="button"
          className="agent-action-btn"
          onClick={(event) => {
            event.stopPropagation();
            onStart(agent.id);
          }}
        >
          Start
        </button>
        <button
          type="button"
          className="agent-action-btn"
          onClick={(event) => {
            event.stopPropagation();
            onPause(agent.id);
          }}
        >
          Pause
        </button>
        <button
          type="button"
          className="agent-action-btn danger"
          onClick={(event) => {
            event.stopPropagation();
            onStop(agent.id);
          }}
        >
          Stop
        </button>
        <button
          type="button"
          className="agent-action-btn logs"
          onClick={(event) => {
            event.stopPropagation();
            onLogs(agent.id);
          }}
        >
          Logs
        </button>
      </div>
    </article>
  );
}

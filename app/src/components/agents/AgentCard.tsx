import { useEffect, useState } from "react";
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
  onDelete: (agentId: string) => void;
  onPermissions?: (agentId: string) => void;
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
    return "#00ffd5";
  }
  if (percentage > 20) {
    return "#f59e0b";
  }
  return "#ef4444";
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
  onLogs,
  onDelete,
  onPermissions
}: AgentCardProps): JSX.Element {
  const percentage = fuelPercentage(agent.fuel_remaining);
  const stroke = gaugeColor(percentage);
  const radius = 44;
  const circumference = 2 * Math.PI * radius;
  const dashOffset = circumference * (1 - percentage / 100);
  const tone = statusTone(agent.status);
  const [pendingAction, setPendingAction] = useState<"starting" | "stopping" | "pausing" | null>(null);
  const [localAction, setLocalAction] = useState<string | null>(null);

  // Clear transient label when agent status actually changes
  useEffect(() => {
    if (pendingAction) {
      const timer = setTimeout(() => {
        setPendingAction(null);
        setLocalAction(null);
      }, 1000);
      return () => clearTimeout(timer);
    }
  }, [pendingAction]);

  const isRunning = agent.status === "Running" || agent.status === "Starting";
  const isStopped = agent.status === "Stopped" || agent.status === "Destroyed" || agent.status === "Created";

  const eventLine = localAction
    ? `${new Date().toLocaleTimeString("en-GB", { hour12: false })} // ${localAction}`
    : latestEvent
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
          <div className="agent-badge-row">
            <span className={`agent-type-badge ${agent.isSystem ? "system" : "custom"}`}>
              {agent.isSystem ? "SYSTEM" : "CUSTOM"}
            </span>
            {agent.sandbox_runtime === "wasmtime" && (
              <span className="agent-isolation-badge wasmtime">Isolated (wasmtime)</span>
            )}
            <span className={`agent-status-badge ${tone}`}>
              <span className="agent-status-dot" />
              {agent.status}
            </span>
          </div>
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
          {agent.fuel_budget != null && agent.fuel_budget > 0 && (
            <span className="agent-fuel-usage">
              {agent.fuel_remaining} / {agent.fuel_budget}
            </span>
          )}
        </div>
      </header>

      {agent.memory_usage_bytes != null && agent.memory_usage_bytes > 0 && (
        <p className="agent-card-mem">
          Mem: {(agent.memory_usage_bytes / 1024).toFixed(0)} KB
          {agent.capabilities && agent.capabilities.length > 0 && (
            <> | Caps: {agent.capabilities.slice(0, 3).join(", ")}
              {agent.capabilities.length > 3 && ` +${agent.capabilities.length - 3}`}
            </>
          )}
        </p>
      )}

      <p className="agent-card-last">Last action: {eventLine}</p>

      <div className="agent-card-actions">
        <button
          type="button"
          className="agent-action-btn"
          disabled={isRunning || pendingAction === "starting"}
          onClick={(event) => {
            event.stopPropagation();
            setPendingAction("starting");
            setLocalAction("Start requested");
            onStart(agent.id);
          }}
        >
          {pendingAction === "starting" ? "Starting..." : "Start"}
        </button>
        <button
          type="button"
          className="agent-action-btn"
          disabled={!isRunning || pendingAction === "pausing"}
          onClick={(event) => {
            event.stopPropagation();
            setPendingAction("pausing");
            setLocalAction("Pause requested");
            onPause(agent.id);
          }}
        >
          {pendingAction === "pausing" ? "Pausing..." : "Pause"}
        </button>
        <button
          type="button"
          className="agent-action-btn danger"
          disabled={isStopped || pendingAction === "stopping"}
          onClick={(event) => {
            event.stopPropagation();
            setPendingAction("stopping");
            setLocalAction("Stop requested");
            onStop(agent.id);
          }}
        >
          {pendingAction === "stopping" ? "Stopping..." : "Stop"}
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
        {onPermissions && (
          <button
            type="button"
            className="agent-action-btn permissions"
            onClick={(event) => {
              event.stopPropagation();
              onPermissions(agent.id);
            }}
          >
            Perms
          </button>
        )}
        {!agent.isSystem && (
          <button
            type="button"
            className="agent-action-btn delete"
            onClick={(event) => {
              event.stopPropagation();
              if (window.confirm(`Delete agent "${agent.name}"? This cannot be undone.`)) {
                onDelete(agent.id);
              }
            }}
          >
            Delete
          </button>
        )}
      </div>
    </article>
  );
}

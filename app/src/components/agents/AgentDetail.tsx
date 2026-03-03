import { useEffect, useMemo, useState } from "react";
import type { AgentSummary, AuditEventRow } from "../../types";

export type AgentDetailTab = "overview" | "logs" | "audit" | "config";

interface AgentDetailProps {
  open: boolean;
  agent: AgentSummary | null;
  auditEvents: AuditEventRow[];
  activeTab: AgentDetailTab;
  onTabChange: (tab: AgentDetailTab) => void;
  onClose: () => void;
}

function fuelPercentage(fuelRemaining: number): number {
  return Math.max(0, Math.min(100, Math.round(fuelRemaining / 100)));
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

function integrityForEvent(event: AuditEventRow, previous: AuditEventRow | null): boolean {
  if (!previous) {
    return event.previous_hash === "genesis" || event.previous_hash.length > 0;
  }
  return event.previous_hash === previous.hash;
}

function formatTimelineTime(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleString("en-GB", {
    hour12: false,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  });
}

function summarizePayload(payload: Record<string, unknown>): string {
  const compact = JSON.stringify(payload);
  if (compact.length <= 150) {
    return compact;
  }
  return `${compact.slice(0, 147)}...`;
}

function inferLogLevel(eventType: string, payload: Record<string, unknown>): "info" | "warn" | "error" {
  const loweredType = eventType.toLowerCase();
  const loweredPayload = JSON.stringify(payload).toLowerCase();
  if (loweredType.includes("error") || loweredPayload.includes("error") || loweredPayload.includes("failed")) {
    return "error";
  }
  if (loweredType.includes("approval") || loweredType.includes("warn") || loweredPayload.includes("retry")) {
    return "warn";
  }
  return "info";
}

export function AgentDetail({
  open,
  agent,
  auditEvents,
  activeTab,
  onTabChange,
  onClose
}: AgentDetailProps): JSX.Element {
  const agentEvents = useMemo(() => {
    if (!agent) {
      return [];
    }
    return auditEvents
      .filter((event) => event.agent_id === agent.id)
      .sort((left, right) => right.timestamp - left.timestamp);
  }, [agent, auditEvents]);

  const totalActions = agentEvents.length;
  const errorCount = agentEvents.filter((event) => inferLogLevel(event.event_type, event.payload) === "error").length;

  const uptimeSeconds = useMemo(() => {
    if (agentEvents.length === 0) {
      return 0;
    }
    const first = agentEvents[agentEvents.length - 1].timestamp;
    const latest = agentEvents[0].timestamp;
    return Math.max(0, latest - first);
  }, [agentEvents]);

  const [manifestText, setManifestText] = useState("");

  useEffect(() => {
    if (!agent) {
      setManifestText("");
      return;
    }
    const inferred = {
      name: agent.name,
      version: "2.0.0",
      fuel_budget: 10_000,
      status: agent.status,
      llm_model: "claude-sonnet-4-5",
      capabilities: ["web.search", "llm.query", "fs.read"]
    };
    setManifestText(JSON.stringify(inferred, null, 2));
  }, [agent]);

  const percentage = agent ? fuelPercentage(agent.fuel_remaining) : 0;
  const stroke = gaugeColor(percentage);
  const radius = 44;
  const circumference = 2 * Math.PI * radius;
  const dashOffset = circumference * (1 - percentage / 100);

  return (
    <div className="agent-detail-overlay">
      <aside className={`agent-detail-panel ${open ? "open" : ""}`}>
        <header className="agent-detail-head">
          <h3 className="agent-detail-title">
            {agent ? `${agent.name} // DETAIL` : "Agent Detail"}
          </h3>
          <button type="button" className="agent-detail-close" onClick={onClose} aria-label="Close detail view">
            ✕
          </button>
        </header>

        <nav className="agent-detail-tabs">
          <button
            type="button"
            className={`agent-detail-tab ${activeTab === "overview" ? "active" : ""}`}
            onClick={() => onTabChange("overview")}
          >
            Overview
          </button>
          <button
            type="button"
            className={`agent-detail-tab ${activeTab === "logs" ? "active" : ""}`}
            onClick={() => onTabChange("logs")}
          >
            Logs
          </button>
          <button
            type="button"
            className={`agent-detail-tab ${activeTab === "audit" ? "active" : ""}`}
            onClick={() => onTabChange("audit")}
          >
            Audit
          </button>
          <button
            type="button"
            className={`agent-detail-tab ${activeTab === "config" ? "active" : ""}`}
            onClick={() => onTabChange("config")}
          >
            Config
          </button>
        </nav>

        <section className="agent-detail-body">
          {!agent ? (
            <p className="agent-card-last">Select an agent to inspect details.</p>
          ) : null}

          {agent && activeTab === "overview" ? (
            <>
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
                <span className="agent-gauge-sub">Fuel Remaining</span>
              </div>

              <div className="agent-overview-grid">
                <article className="agent-overview-stat">
                  <p className="agent-overview-stat-label">Uptime</p>
                  <p className="agent-overview-stat-value">{uptimeSeconds}s</p>
                </article>
                <article className="agent-overview-stat">
                  <p className="agent-overview-stat-label">Total Actions</p>
                  <p className="agent-overview-stat-value">{totalActions}</p>
                </article>
                <article className="agent-overview-stat">
                  <p className="agent-overview-stat-label">Error Count</p>
                  <p className="agent-overview-stat-value">{errorCount}</p>
                </article>
                <article className="agent-overview-stat">
                  <p className="agent-overview-stat-label">Current Status</p>
                  <p className="agent-overview-stat-value">{agent.status}</p>
                </article>
              </div>
            </>
          ) : null}

          {agent && activeTab === "logs" ? (
            <div className="agent-log-stream">
              {agentEvents.length === 0 ? (
                <p className="agent-log-line info">No logs available for this agent.</p>
              ) : (
                agentEvents.map((event) => {
                  const level = inferLogLevel(event.event_type, event.payload);
                  return (
                    <p key={event.event_id} className={`agent-log-line ${level}`}>
                      [{formatTimelineTime(event.timestamp)}] {event.event_type}: {summarizePayload(event.payload)}
                    </p>
                  );
                })
              )}
            </div>
          ) : null}

          {agent && activeTab === "audit" ? (
            <div className="agent-audit-timeline">
              {agentEvents.length === 0 ? (
                <p className="agent-log-line info">No audit events available for this agent.</p>
              ) : (
                agentEvents.map((event, index) => {
                  const previous = index < agentEvents.length - 1 ? agentEvents[index + 1] : null;
                  const valid = integrityForEvent(event, previous);
                  return (
                    <article key={event.event_id} className="agent-audit-item">
                      <div className="agent-audit-item-head">
                        <span className="agent-audit-item-label">
                          <span className={valid ? "agent-audit-valid" : "agent-audit-invalid"}>
                            {valid ? "✓" : "✕"}
                          </span>
                          {event.event_type}
                        </span>
                        <span className="agent-audit-item-time">{formatTimelineTime(event.timestamp)}</span>
                      </div>
                      <p className="agent-audit-item-payload">{summarizePayload(event.payload)}</p>
                    </article>
                  );
                })
              )}
            </div>
          ) : null}

          {agent && activeTab === "config" ? (
            <textarea
              className="agent-config-editor"
              value={manifestText}
              onChange={(event) => setManifestText(event.target.value)}
              spellCheck={false}
            />
          ) : null}
        </section>
      </aside>
    </div>
  );
}

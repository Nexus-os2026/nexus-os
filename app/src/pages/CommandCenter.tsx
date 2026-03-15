import { useCallback, useEffect, useState } from "react";
import { listAgents, startAgent, stopAgent, pauseAgent, resumeAgent, hasDesktopRuntime, getAuditLog } from "../api/backend";
import type { AgentSummary, AuditEventRow } from "../types";
import "./command-center.css";

const STATUS_COLORS: Record<string, string> = {
  Running: "var(--nexus-accent)",
  Stopped: "#6b7280",
  Failed: "#ef4444",
  Paused: "#f59e0b",
  Starting: "#60a5fa",
  Idle: "#6b7280",
};

function formatTime(ts: number): string {
  if (ts === 0) return "—";
  const d = new Date(ts * 1000);
  return d.toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });
}

export default function CommandCenter(): JSX.Element {
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [lastEvents, setLastEvents] = useState<Record<string, AuditEventRow>>({});
  const [loading, setLoading] = useState(true);

  const loadData = useCallback(async () => {
    if (!hasDesktopRuntime()) {
      setLoading(false);
      return;
    }
    try {
      const [agentList, auditEvents] = await Promise.all([listAgents(), getAuditLog(undefined, 500)]);
      setAgents(agentList);
      // Find last audit event per agent
      const eventMap: Record<string, AuditEventRow> = {};
      for (const event of auditEvents) {
        if (!eventMap[event.agent_id] || event.timestamp > eventMap[event.agent_id].timestamp) {
          eventMap[event.agent_id] = event;
        }
      }
      setLastEvents(eventMap);
    } catch {
      // ignore
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    void loadData();
    if (!hasDesktopRuntime()) return;
    const timer = setInterval(() => void loadData(), 5000);
    return () => clearInterval(timer);
  }, [loadData]);

  async function handleAction(id: string, action: "start" | "stop" | "pause" | "resume"): Promise<void> {
    if (!hasDesktopRuntime()) return;
    try {
      if (action === "start") await startAgent(id);
      else if (action === "stop") await stopAgent(id);
      else if (action === "pause") await pauseAgent(id);
      else if (action === "resume") await resumeAgent(id);
      await loadData();
    } catch {
      // ignore
    }
  }

  const runningCount = agents.filter((a) => a.status === "Running").length;

  return (
    <section className="cc-hub">
      <header className="cc-header">
        <h2 className="cc-title">COMMAND CENTER // LIVE AGENT GRID</h2>
        <p className="cc-subtitle">
          {agents.length > 0 ? `${runningCount} running / ${agents.length} total` : "No agents registered"}
        </p>
      </header>

      {loading && <div style={{ padding: "2rem", textAlign: "center", opacity: 0.5 }}>Loading agents...</div>}

      {!loading && agents.length === 0 && (
        <div style={{ padding: "3rem", textAlign: "center", opacity: 0.5 }}>
          No agents registered. Create an agent to get started.
        </div>
      )}

      <div className="cc-grid">
        {agents.map((agent) => {
          const fuelPct = agent.fuel_budget && agent.fuel_budget > 0
            ? Math.round((agent.fuel_remaining / agent.fuel_budget) * 100)
            : 0;
          const lastEvent = lastEvents[agent.id];
          const status = agent.status;
          return (
            <article key={agent.id} className="cc-card">
              <div className="cc-card-top">
                <div className="cc-card-name-row">
                  <span className="cc-status-dot" style={{ background: STATUS_COLORS[status] ?? "#6b7280" }} />
                  <h3 className="cc-card-name">{agent.name}</h3>
                </div>
                <span className="cc-card-status">{status}</span>
              </div>

              <div className="cc-card-autonomy">
                <span className="cc-label">Autonomy</span>
                <span className="cc-value">Not configured</span>
              </div>

              <div className="cc-card-fuel">
                <div className="cc-fuel-header">
                  <span className="cc-label">Fuel</span>
                  <span className="cc-value">{agent.fuel_remaining.toLocaleString()} / {(agent.fuel_budget ?? 0).toLocaleString()}</span>
                </div>
                <div className="cc-fuel-track">
                  <div
                    className="cc-fuel-fill"
                    style={{
                      width: `${fuelPct}%`,
                      background: fuelPct > 50 ? "var(--nexus-accent)" : fuelPct > 20 ? "#f59e0b" : "#ef4444",
                    }}
                  />
                </div>
              </div>

              <div className="cc-card-audit">
                <span className="cc-label">Last Event</span>
                <span className="cc-audit-text">{lastEvent ? `${lastEvent.event_type}: ${JSON.stringify(lastEvent.payload).slice(0, 40)}` : agent.last_action || "—"}</span>
                <span className="cc-audit-time">{lastEvent ? formatTime(lastEvent.timestamp) : "—"}</span>
              </div>

              <div className="cc-card-actions">
                <button
                  type="button"
                  className="cc-btn cc-btn-start"
                  disabled={status === "Running"}
                  onClick={() => void handleAction(agent.id, status === "Paused" ? "resume" : "start")}
                >
                  {status === "Paused" ? "Resume" : "Start"}
                </button>
                <button
                  type="button"
                  className="cc-btn cc-btn-stop"
                  disabled={status === "Paused" || status === "Stopped"}
                  onClick={() => void handleAction(agent.id, "pause")}
                >
                  Pause
                </button>
                <button
                  type="button"
                  className="cc-btn cc-btn-stop"
                  disabled={status === "Stopped"}
                  onClick={() => void handleAction(agent.id, "stop")}
                >
                  Stop
                </button>
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}

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
        <div style={{ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", padding: "48px 24px", textAlign: "center", minHeight: 320 }}>
          <div style={{ width: 72, height: 72, borderRadius: 16, background: "rgba(6, 182, 212, 0.06)", border: "1px solid rgba(6, 182, 212, 0.12)", display: "flex", alignItems: "center", justifyContent: "center", marginBottom: 24 }}>
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="#475569" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="8" width="18" height="12" rx="2"/><path d="M12 8V5a2 2 0 0 0-2-2h0a2 2 0 0 0-2 2v0M16 8V5a2 2 0 0 0-2-2h0"/><circle cx="9" cy="14" r="1.5" fill="#475569"/><circle cx="15" cy="14" r="1.5" fill="#475569"/></svg>
          </div>
          <h3 style={{ fontSize: 18, fontWeight: 600, color: "#e2e8f0", margin: "0 0 8px" }}>No agents registered</h3>
          <p style={{ fontSize: 14, color: "#64748b", maxWidth: 400, lineHeight: 1.6, margin: "0 0 24px" }}>Create your first agent to see it appear in the live command grid. Each agent shows real-time status, fuel consumption, and audit events.</p>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 16, width: "100%", maxWidth: 560 }}>
            {[{ svg: <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#06b6d4" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z"/></svg>, label: "Real-time Status", desc: "Live monitoring of agent state" },
              { svg: <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#06b6d4" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M12 2v4m0 12v4M4.93 4.93l2.83 2.83m8.48 8.48l2.83 2.83M2 12h4m12 0h4M4.93 19.07l2.83-2.83m8.48-8.48l2.83-2.83"/><circle cx="12" cy="12" r="4"/></svg>, label: "Fuel Tracking", desc: "Budget consumption at a glance" },
              { svg: <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="#06b6d4" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6M8 13h8M8 17h8M8 9h2"/></svg>, label: "Audit Events", desc: "Latest actions per agent" },
            ].map(f => (
              <div key={f.label} style={{ padding: 16, borderRadius: 10, background: "rgba(255,255,255,0.02)", border: "1px solid rgba(255,255,255,0.05)", textAlign: "center" }}>
                <div style={{ marginBottom: 8, display: "flex", justifyContent: "center" }}>{f.svg}</div>
                <div style={{ fontSize: 13, fontWeight: 600, color: "#e2e8f0", marginBottom: 4 }}>{f.label}</div>
                <div style={{ fontSize: 11, color: "#64748b" }}>{f.desc}</div>
              </div>
            ))}
          </div>
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
                <span className="cc-value">L{agent.autonomy_level ?? 0}</span>
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
                <button type="button"
                  className="cc-btn cc-btn-start"
                  disabled={status === "Running"}
                  onClick={() => void handleAction(agent.id, status === "Paused" ? "resume" : "start")}
                >
                  {status === "Paused" ? "Resume" : "Start"}
                </button>
                <button type="button"
                  className="cc-btn cc-btn-stop"
                  disabled={status === "Paused" || status === "Stopped"}
                  onClick={() => void handleAction(agent.id, "pause")}
                >
                  Pause
                </button>
                <button type="button"
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

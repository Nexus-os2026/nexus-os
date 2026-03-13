import { useState } from "react";
import "./command-center.css";

interface CommandAgent {
  id: string;
  name: string;
  status: "running" | "stopped" | "error" | "paused";
  autonomy: number;
  fuelRemaining: number;
  fuelBudget: number;
  lastAuditEvent: string;
  lastAuditTimestamp: number;
}

const MOCK_AGENTS: CommandAgent[] = [
  { id: "a0000000-0000-4000-8000-000000000001", name: "Coder", status: "running", autonomy: 3, fuelRemaining: 9200, fuelBudget: 10000, lastAuditEvent: "ToolExec: fix_bug", lastAuditTimestamp: 1700100470 },
  { id: "a0000000-0000-4000-8000-000000000003", name: "Screen Poster", status: "paused", autonomy: 2, fuelRemaining: 4100, fuelBudget: 10000, lastAuditEvent: "ApprovalRequired: social.post", lastAuditTimestamp: 1700100410 },
  { id: "a0000000-0000-4000-8000-000000000004", name: "Web Builder", status: "running", autonomy: 3, fuelRemaining: 7800, fuelBudget: 10000, lastAuditEvent: "ToolExec: deploy staging", lastAuditTimestamp: 1700100430 },
  { id: "a0000000-0000-4000-8000-000000000005", name: "Workflow Studio", status: "stopped", autonomy: 1, fuelRemaining: 2300, fuelBudget: 10000, lastAuditEvent: "StateChange: Stopped", lastAuditTimestamp: 1700100460 },
  { id: "a0000000-0000-4000-8000-000000000006", name: "Self-Improve", status: "running", autonomy: 4, fuelRemaining: 8400, fuelBudget: 10000, lastAuditEvent: "ToolExec: optimize_prompt", lastAuditTimestamp: 1700100455 },
  { id: "a0000000-0000-4000-8000-000000000002", name: "Designer", status: "running", autonomy: 2, fuelRemaining: 6500, fuelBudget: 10000, lastAuditEvent: "ToolExec: image_gen", lastAuditTimestamp: 1700100380 },
];

const STATUS_COLORS: Record<string, string> = {
  running: "var(--nexus-accent)",
  stopped: "#6b7280",
  error: "#ef4444",
  paused: "#f59e0b",
};

const AUTONOMY_LABELS = ["L0 Inert", "L1 Suggest", "L2 Act+Approve", "L3 Act+Report", "L4 Autonomous", "L5 Full"];

function formatTime(ts: number): string {
  const d = new Date(ts * 1000);
  return d.toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });
}

export default function CommandCenter(): JSX.Element {
  const [agents, setAgents] = useState<CommandAgent[]>(MOCK_AGENTS);

  function setStatus(id: string, status: CommandAgent["status"]): void {
    setAgents((prev) => prev.map((a) => (a.id === id ? { ...a, status } : a)));
  }

  return (
    <section className="cc-hub">
      <header className="cc-header">
        <h2 className="cc-title">COMMAND CENTER // LIVE AGENT GRID</h2>
        <p className="cc-subtitle">
          {agents.filter((a) => a.status === "running").length} running / {agents.length} total
        </p>
      </header>

      <div className="cc-grid">
        {agents.map((agent) => {
          const fuelPct = Math.round((agent.fuelRemaining / agent.fuelBudget) * 100);
          return (
            <article key={agent.id} className="cc-card">
              <div className="cc-card-top">
                <div className="cc-card-name-row">
                  <span className="cc-status-dot" style={{ background: STATUS_COLORS[agent.status] }} />
                  <h3 className="cc-card-name">{agent.name}</h3>
                </div>
                <span className="cc-card-status">{agent.status}</span>
              </div>

              <div className="cc-card-autonomy">
                <span className="cc-label">Autonomy</span>
                <span className="cc-value">{AUTONOMY_LABELS[agent.autonomy] ?? `L${agent.autonomy}`}</span>
              </div>

              <div className="cc-card-fuel">
                <div className="cc-fuel-header">
                  <span className="cc-label">Fuel</span>
                  <span className="cc-value">{agent.fuelRemaining.toLocaleString()} / {agent.fuelBudget.toLocaleString()}</span>
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
                <span className="cc-audit-text">{agent.lastAuditEvent}</span>
                <span className="cc-audit-time">{formatTime(agent.lastAuditTimestamp)}</span>
              </div>

              <div className="cc-card-actions">
                <button
                  type="button"
                  className="cc-btn cc-btn-start"
                  disabled={agent.status === "running"}
                  onClick={() => setStatus(agent.id, "running")}
                >
                  {agent.status === "paused" ? "Resume" : "Start"}
                </button>
                <button
                  type="button"
                  className="cc-btn cc-btn-stop"
                  disabled={agent.status === "paused" || agent.status === "stopped"}
                  onClick={() => setStatus(agent.id, "paused")}
                >
                  Pause
                </button>
                <button
                  type="button"
                  className="cc-btn cc-btn-stop"
                  disabled={agent.status === "stopped"}
                  onClick={() => setStatus(agent.id, "stopped")}
                >
                  Stop
                </button>
                <button
                  type="button"
                  className="cc-btn cc-btn-kill"
                  disabled={agent.status === "stopped"}
                  onClick={() => setStatus(agent.id, "stopped")}
                >
                  Kill
                </button>
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}

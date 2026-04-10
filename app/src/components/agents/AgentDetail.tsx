import { useCallback, useEffect, useMemo, useState } from "react";
import type { AgentSummary, AuditEventRow } from "../../types";
import { getSelfEvolutionMetrics, getSelfEvolutionStrategies, triggerCrossAgentLearning } from "../../api/backend";

export type AgentDetailTab = "overview" | "logs" | "audit" | "config" | "evolution";

interface AgentDetailProps {
  open: boolean;
  agent: AgentSummary | null;
  auditEvents: AuditEventRow[];
  activeTab: AgentDetailTab;
  onTabChange: (tab: AgentDetailTab) => void;
  onClose: () => void;
  onStart?: (id: string) => void;
  onStop?: (id: string) => void;
  onPause?: (id: string) => void;
  onResume?: (id: string) => void;
}

interface EvolutionEvent {
  agent_id: string;
  new_score: number;
  generation: number;
  strategy_hash?: string;
  received_at: number;
}

function fuelPercentage(fuelRemaining: number): number {
  return Math.max(0, Math.min(100, Math.round(fuelRemaining / 100)));
}

function gaugeColor(percentage: number): string {
  if (percentage > 50) {
    return "var(--nexus-accent)";
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
  onClose,
  onStart,
  onStop,
  onPause,
  onResume
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

  // Evolution state
  const [evoMetrics, setEvoMetrics] = useState<Record<string, unknown> | null>(null);
  const [evoStrategies, setEvoStrategies] = useState<Record<string, unknown>[]>([]);
  const [evoLoading, setEvoLoading] = useState(false);
  const [evolutionEvents, setEvolutionEvents] = useState<EvolutionEvent[]>([]);

  const historicalEvolutionEvents = useMemo(() => {
    if (!agent) {
      return [];
    }
    return agentEvents
      .filter((event) => event.payload?.action === "agent_evolved_strategy")
      .map((event) => ({
        agent_id: agent.id,
        new_score: Number(event.payload?.new_score ?? 0),
        generation: Number(event.payload?.generation ?? 0),
        strategy_hash: typeof event.payload?.strategy_hash === "string" ? event.payload.strategy_hash : undefined,
        received_at: event.timestamp * 1000,
      }));
  }, [agent, agentEvents]);

  const combinedEvolutionEvents = useMemo(
    () =>
      [...evolutionEvents, ...historicalEvolutionEvents]
        .sort((left, right) => right.received_at - left.received_at)
        .slice(0, 8),
    [evolutionEvents, historicalEvolutionEvents]
  );

  const loadEvolution = useCallback(async (agentId: string) => {
    setEvoLoading(true);
    try {
      const [metrics, strategies] = await Promise.all([
        getSelfEvolutionMetrics(agentId),
        getSelfEvolutionStrategies(agentId),
      ]);
      setEvoMetrics(metrics);
      setEvoStrategies(strategies);
    } catch {
      setEvoMetrics(null);
      setEvoStrategies([]);
    } finally {
      setEvoLoading(false);
    }
  }, []);

  useEffect(() => {
    if (agent && activeTab === "evolution") {
      loadEvolution(agent.id);
    }
  }, [agent, activeTab, loadEvolution]);

  useEffect(() => {
    if (!agent || activeTab !== "evolution") {
      return;
    }

    let disposed = false;
    let unlisten: (() => void) | undefined;

    import("@tauri-apps/api/event")
      .then(({ listen }) =>
        listen("agent-evolved", (event) => {
          const payload = event.payload as Partial<EvolutionEvent>;
          if (disposed || payload.agent_id !== agent.id) {
            return;
          }
          setEvolutionEvents((current) => [
            {
              agent_id: agent.id,
              new_score: Number(payload.new_score ?? 0),
              generation: Number(payload.generation ?? 0),
              strategy_hash: payload.strategy_hash,
              received_at: Date.now(),
            },
            ...current,
          ].slice(0, 10));
          void loadEvolution(agent.id);
        })
      )
      .then((dispose) => {
        unlisten = dispose;
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [agent, activeTab, loadEvolution]);

  useEffect(() => {
    if (!agent) {
      setEvolutionEvents([]);
    }
  }, [agent]);

  useEffect(() => {
    if (!agent) {
      setManifestText("");
      return;
    }
    const inferred = {
      name: agent.name,
      status: agent.status,
      fuel_budget: agent.fuel_budget ?? 0,
      fuel_remaining: agent.fuel_remaining,
      sandbox_runtime: agent.sandbox_runtime ?? "in-process",
      capabilities: agent.capabilities ?? [],
      ...(agent.did ? { did: agent.did } : {})
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
          <button type="button"
            className={`agent-detail-tab ${activeTab === "overview" ? "active" : ""}`}
            onClick={() => onTabChange("overview")}
          >
            Overview
          </button>
          <button type="button"
            className={`agent-detail-tab ${activeTab === "logs" ? "active" : ""}`}
            onClick={() => onTabChange("logs")}
          >
            Logs
          </button>
          <button type="button"
            className={`agent-detail-tab ${activeTab === "audit" ? "active" : ""}`}
            onClick={() => onTabChange("audit")}
          >
            Audit
          </button>
          <button type="button"
            className={`agent-detail-tab ${activeTab === "config" ? "active" : ""}`}
            onClick={() => onTabChange("config")}
          >
            Config
          </button>
          <button type="button"
            className={`agent-detail-tab ${activeTab === "evolution" ? "active" : ""}`}
            onClick={() => onTabChange("evolution")}
          >
            Evolution
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

              <div className="agent-card-actions" style={{ marginTop: "1rem" }}>
                {(agent.status === "Stopped" || agent.status === "Created" || agent.status === "Destroyed") && onStart && (
                  <button type="button" className="agent-action-btn" onClick={() => onStart(agent.id)}>
                    Start
                  </button>
                )}
                {(agent.status === "Running" || agent.status === "Starting") && onPause && (
                  <button type="button" className="agent-action-btn" onClick={() => onPause(agent.id)}>
                    Pause
                  </button>
                )}
                {agent.status === "Paused" && onResume && (
                  <button type="button" className="agent-action-btn" onClick={() => onResume(agent.id)}>
                    Resume
                  </button>
                )}
                {(agent.status === "Running" || agent.status === "Starting" || agent.status === "Paused") && onStop && (
                  <button type="button" className="agent-action-btn danger" onClick={() => onStop(agent.id)}>
                    Stop
                  </button>
                )}
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

          {agent && activeTab === "evolution" ? (
            <div className="agent-evolution-panel">
              {evoLoading ? (
                <p className="agent-card-last">Loading evolution data...</p>
              ) : !evoMetrics ? (
                <p className="agent-card-last">No evolution data available for this agent.</p>
              ) : (
                <>
                  <div className="agent-overview-grid">
                    <article className="agent-overview-stat">
                      <p className="agent-overview-stat-label">Tasks Completed</p>
                      <p className="agent-overview-stat-value">
                        {(evoMetrics.total_tasks_completed as number) ?? 0}
                      </p>
                    </article>
                    <article className="agent-overview-stat">
                      <p className="agent-overview-stat-label">Success Rate</p>
                      <p className="agent-overview-stat-value">
                        {((evoMetrics.overall_success_rate as number) * 100).toFixed(1)}%
                      </p>
                    </article>
                    <article className="agent-overview-stat">
                      <p className="agent-overview-stat-label">Improvement</p>
                      <p className="agent-overview-stat-value">
                        {(evoMetrics.improvement_percentage as number) > 0 ? "+" : ""}
                        {(evoMetrics.improvement_percentage as number).toFixed(1)}%
                        {(evoMetrics.improvement_percentage as number) > 0 ? " \u2191" : (evoMetrics.improvement_percentage as number) < 0 ? " \u2193" : ""}
                      </p>
                    </article>
                    <article className="agent-overview-stat">
                      <p className="agent-overview-stat-label">Cross-Agent Learnings</p>
                      <p className="agent-overview-stat-value">
                        {(evoMetrics.cross_agent_learnings_received as number) ?? 0}
                      </p>
                    </article>
                  </div>

                  <h4 style={{ margin: "1rem 0 0.5rem", color: "var(--nexus-accent)" }}>
                    Top Strategies
                  </h4>
                  {evoStrategies.length === 0 ? (
                    <p className="agent-card-last">No strategies recorded yet.</p>
                  ) : (
                    <div className="agent-audit-timeline">
                      {evoStrategies.slice(0, 5).map((s, i) => (
                        <article key={i} className="agent-audit-item">
                          <div className="agent-audit-item-head">
                            <span className="agent-audit-item-label">
                              {(s.strategy_hash as string).slice(0, 12)}...
                            </span>
                            <span className="agent-audit-item-time">
                              Score: {(s.composite_score as number).toFixed(3)}
                            </span>
                          </div>
                          <p className="agent-audit-item-payload">
                            Type: {s.goal_type as string} | Uses: {s.uses as number} |
                            Success: {((s.success_rate as number) * 100).toFixed(0)}%
                          </p>
                        </article>
                      ))}
                    </div>
                  )}

                  <h4 style={{ margin: "1rem 0 0.5rem", color: "var(--nexus-accent)" }}>
                    Evolution Events
                  </h4>
                  {combinedEvolutionEvents.length === 0 ? (
                    <p className="agent-card-last">No evolution events recorded yet.</p>
                  ) : (
                    <div className="agent-audit-timeline">
                      {combinedEvolutionEvents.map((event, index) => (
                        <article key={`${event.received_at}-${index}`} className="agent-audit-item">
                          <div className="agent-audit-item-head">
                            <span className="agent-audit-item-label">
                              Generation {event.generation || 1}
                            </span>
                            <span className="agent-audit-item-time">
                              Score: {event.new_score.toFixed(3)}
                            </span>
                          </div>
                          <p className="agent-audit-item-payload">
                            {new Date(event.received_at).toLocaleString()} {event.strategy_hash ? `| Strategy ${event.strategy_hash.slice(0, 12)}...` : ""}
                          </p>
                        </article>
                      ))}
                    </div>
                  )}

                  <h4 style={{ margin: "1rem 0 0.5rem", color: "var(--nexus-accent)" }}>
                    Success Rate Trend
                  </h4>
                  {(evoMetrics.success_rate_trend as [string, number][])?.length > 0 ? (
                    <div style={{ display: "flex", gap: "2px", alignItems: "flex-end", height: "60px" }}>
                      {(evoMetrics.success_rate_trend as [string, number][]).map(([label, rate], i) => (
                        <div
                          key={i}
                          title={`${label}: ${(rate * 100).toFixed(0)}%`}
                          style={{
                            flex: 1,
                            height: `${Math.max(4, rate * 100)}%`,
                            background: rate > 0.7 ? "var(--nexus-accent)" : rate > 0.4 ? "#ffaa00" : "#ff0040",
                            borderRadius: "2px 2px 0 0",
                          }}
                        />
                      ))}
                    </div>
                  ) : (
                    <p className="agent-card-last">No trend data yet.</p>
                  )}

                  <button type="button"
                    className="agent-detail-tab"
                    style={{ marginTop: "1rem", width: "100%" }}
                    onClick={async () => {
                      try {
                        const count = await triggerCrossAgentLearning();
                        alert(`Shared ${count} strategies across agents.`);
                        if (agent) loadEvolution(agent.id);
                      } catch (e) {
                        alert(`Error: ${e}`);
                      }
                    }}
                  >
                    Trigger Cross-Agent Learning
                  </button>
                </>
              )}
            </div>
          ) : null}
        </section>
      </aside>
    </div>
  );
}

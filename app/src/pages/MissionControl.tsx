import React, { useCallback, useEffect, useMemo, useState } from "react";
import {
  trayStatus,
  listAgents,
  getImmuneStatus,
  meshGetPeers,
  civGetEconomyStatus,
  getDreamStatus,
  getMorningBriefing,
  getConsciousnessHeatmap,
  getTemporalHistory,
  getAuditLog,
  getLiveSystemMetricsJson,
} from "../api/backend";
import { Users, Brain, ShieldCheck, Coins, Moon, GitBranch, Network, Activity, Zap, Clock, TrendingUp } from "lucide-react";
import "./mission-control.css";

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface AgentSummary {
  id: string;
  name: string;
  status: string;
  autonomy_level?: number;
  fuel_remaining: number;
}

interface ImmuneStatus {
  threat_level: string;
  active_antibodies: number;
  threats_blocked: number;
  last_scan: number;
  privacy_violations_blocked: number;
}

interface MeshPeer {
  peer_id: string;
  address: string;
  port: number;
  name: string;
  status: string;
}

interface EconomyStatus {
  total_agents: number;
  total_tokens_circulating: number;
  transactions_today: number;
}

interface DreamStatus {
  active_dreams: number;
  completed_today: number;
  next_scheduled: string;
  budget_remaining?: number;
  budget_total?: number;
}

interface DreamBriefing {
  summary: string;
  improvements: string[];
  agents_created: string[];
  presolved_count: number;
}

interface ConsciousnessEntry {
  agent_id: string;
  confidence: number;
  fatigue: number;
  frustration: number;
  curiosity?: number;
  flow_state: boolean;
}

interface TemporalStatus {
  active_forks: number;
  decisions_made: number;
  best_timeline_score: number;
}

interface AuditEventRow {
  event_id: string;
  timestamp: number;
  agent_id: string;
  event_type: string;
}

/* ================================================================== */
/*  Helpers                                                            */
/* ================================================================== */

function threatColor(level: string): string {
  switch (level) {
    case "Green": return "#22c55e";
    case "Yellow": return "#eab308";
    case "Orange": return "#f97316";
    case "Red": return "#ef4444";
    default: return "#6b7280";
  }
}

function peerStatusColor(status: string): string {
  switch (status) {
    case "Connected":
    case "Authenticated": return "#22c55e";
    case "Discovered": return "#eab308";
    default: return "#6b7280";
  }
}

function emotionColor(entry: ConsciousnessEntry): string {
  if (entry.flow_state) return "#22c55e";            // green = flow
  if (entry.fatigue > 0.7) return "#ef4444";         // red = fatigued
  if (entry.frustration > 0.5) return "#f97316";     // orange = frustrated
  if ((entry.curiosity ?? 0) > 0.6) return "#3b82f6"; // blue = exploring
  if (entry.confidence > 0.5) return "#eab308";      // yellow = working
  return "#6b7280";                                   // grey = idle
}

function formatTime(ts: number): string {
  if (ts === 0) return "—";
  const d = new Date(ts * 1000);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function MissionControl({ onNavigate }: { onNavigate?: (page: string) => void }): JSX.Element {
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [immune, setImmune] = useState<ImmuneStatus | null>(null);
  const [peers, setPeers] = useState<MeshPeer[]>([]);
  const [economy, setEconomy] = useState<EconomyStatus | null>(null);
  const [dreams, setDreams] = useState<DreamStatus | null>(null);
  const [briefing, setBriefing] = useState<DreamBriefing | null>(null);
  const [consciousness, setConsciousness] = useState<ConsciousnessEntry[]>([]);
  const [temporal, setTemporal] = useState<TemporalStatus | null>(null);
  const [auditEvents, setAuditEvents] = useState<AuditEventRow[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [uptime, setUptime] = useState("0m");
  const [tray, setTray] = useState<{ visible: boolean; tooltip: string; badge: number } | null>(null);

  const refresh = useCallback(async () => {
    try {
      const parse = <T,>(raw: string): T => JSON.parse(raw) as T;
      const results = await Promise.allSettled([
        listAgents() as Promise<AgentSummary[]>,
        getImmuneStatus().then((r) => parse<ImmuneStatus>(r)),
        meshGetPeers().then((r) => parse<MeshPeer[]>(r)),
        civGetEconomyStatus().then((r) => parse<EconomyStatus>(r)),
        getDreamStatus().then((r) => parse<DreamStatus>(r)),
        getMorningBriefing().then((r) => parse<DreamBriefing>(r)),
        getConsciousnessHeatmap().then((r) => parse<ConsciousnessEntry[]>(r)),
        getTemporalHistory().then((r) => parse<TemporalStatus>(r)),
        getAuditLog() as Promise<AuditEventRow[]>,
      ]);
      if (results[0].status === "fulfilled") setAgents(Array.isArray(results[0].value) ? results[0].value : []);
      if (results[1].status === "fulfilled") setImmune(results[1].value);
      if (results[2].status === "fulfilled") setPeers(Array.isArray(results[2].value) ? results[2].value : []);
      if (results[3].status === "fulfilled") setEconomy(results[3].value);
      if (results[4].status === "fulfilled") setDreams(results[4].value);
      if (results[5].status === "fulfilled") setBriefing(results[5].value);
      if (results[6].status === "fulfilled") setConsciousness(Array.isArray(results[6].value) ? results[6].value : []);
      if (results[7].status === "fulfilled") setTemporal(results[7].value);
      if (results[8].status === "fulfilled") setAuditEvents(Array.isArray(results[8].value) ? results[8].value.slice(0, 8) : []);

      // Fetch tray status separately (uses backend wrapper)
      try {
        const trayRaw = await trayStatus();
        setTray(JSON.parse(trayRaw));
      } catch { /* tray not available */ }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
    const iv = setInterval(() => void refresh(), 10_000);
    return () => clearInterval(iv);
  }, [refresh]);

  // System uptime from backend
  useEffect(() => {
    let active = true;
    function poll(): void {
      getLiveSystemMetricsJson<{ uptime_secs?: number }>()
        .then((metrics) => {
          if (!active || !metrics) return;
          const secs = (metrics as { uptime_secs?: number }).uptime_secs ?? 0;
          if (secs < 3600) setUptime(`${Math.floor(secs / 60)}m`);
          else setUptime(`${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`);
        })
        .catch(() => {});
    }
    poll();
    const iv = setInterval(poll, 30_000);
    return () => { active = false; clearInterval(iv); };
  }, []);

  const activeAgents = useMemo(() => agents.filter((a) => a.status === "Running").length, [agents]);
  const genesisAgents = useMemo(() => agents.filter((a) => (a as AgentSummary & { description?: string }).name?.includes("genesis") || false).length, [agents]);
  const avgConfidence = useMemo(() => {
    if (consciousness.length === 0) return 0;
    return consciousness.reduce((s, c) => s + c.confidence, 0) / consciousness.length;
  }, [consciousness]);
  const avgFatigue = useMemo(() => {
    if (consciousness.length === 0) return 0;
    return consciousness.reduce((s, c) => s + c.fatigue, 0) / consciousness.length;
  }, [consciousness]);

  // Calculate fitness score
  const fitnessScore = useMemo(() => {
    let score = 50;
    if (activeAgents > 0) score += 15;
    if (immune?.threat_level === "Green") score += 15;
    else if (immune?.threat_level === "Yellow") score += 5;
    if (avgConfidence > 0.5) score += 10;
    if (avgFatigue < 0.5) score += 10;
    return Math.min(100, score);
  }, [activeAgents, immune, avgConfidence, avgFatigue]);

  const fitnessColor = fitnessScore >= 80 ? "var(--nexus-accent)" : fitnessScore >= 50 ? "#eab308" : "#ef4444";
  const constellationNodes = useMemo(() => {
    const positions = [
      { x: 16, y: 64, z: 12 },
      { x: 28, y: 28, z: -6 },
      { x: 47, y: 18, z: 16 },
      { x: 72, y: 28, z: 8 },
      { x: 80, y: 62, z: -10 },
      { x: 48, y: 78, z: 20 },
    ];

    return agents.slice(0, 6).map((agent, index) => {
      const tone =
        agent.status === "Running"
          ? "healthy"
          : agent.status === "Paused" || agent.status === "Starting"
            ? "busy"
            : "alert";

      return {
        ...positions[index % positions.length],
        agent,
        tone,
      };
    });
  }, [agents]);

  const nav = (page: string) => onNavigate?.(page);

  return (
    <div className="mc-shell nx-stagger">
      <section className="mc-hero nx-spatial-container">
        <div className="mc-panel mc-panel--intro nx-spatial-layer-mid">
          <div className="mc-kicker">System Core</div>
          <h1 className="mc-title">Mission Control</h1>
          <p className="mc-copy">
            Govern the AI brain through live telemetry, cognition flows, mesh health, and controlled agent orchestration.
          </p>

          <div className="mc-chip-row">
            <span className="mc-chip">
              <Users size={13} aria-hidden="true" />
              {agents.length} agents // {activeAgents} active
            </span>
            <span className="mc-chip">
              <Clock size={13} aria-hidden="true" />
              uptime {uptime}
            </span>
            <span className="mc-chip">
              <Zap size={13} aria-hidden="true" />
              v10.6.0 runtime
            </span>
            {tray && (
              <span className="mc-chip" style={{ color: tray.visible ? "var(--nexus-accent)" : "var(--text-secondary)" }}>
                <span
                  className="mc-chip__dot"
                  style={{
                    background: tray.visible ? "var(--nexus-accent)" : "var(--text-muted)",
                    boxShadow: tray.visible ? "0 0 10px rgba(74,247,211,0.7)" : "none",
                  }}
                />
                tray {tray.visible ? "visible" : "hidden"}{tray.badge > 0 ? ` // ${tray.badge} badge` : ""}
              </span>
            )}
          </div>

          <div className="mc-action-row">
            <button type="button" className="nx-btn nx-btn-primary" onClick={() => nav("agents")}>
              Open Agents
            </button>
            <button type="button" className="nx-btn nx-btn-ghost" onClick={() => nav("chat")}>
              Open Chat
            </button>
            <button type="button" className="nx-btn nx-btn-ghost" onClick={() => nav("consciousness")}>
              Inspect Cognition
            </button>
          </div>
        </div>

        <div className="mc-panel mc-panel--reactor nx-spatial-layer-front">
          <div className="mc-reactor-ring">
            <svg width="200" height="200" viewBox="0 0 200 200" aria-label="OS fitness score">
              <circle cx="100" cy="100" r="78" fill="none" stroke="rgba(118,190,255,0.1)" strokeWidth="10" />
              <circle
                cx="100"
                cy="100"
                r="78"
                fill="none"
                stroke={fitnessColor}
                strokeWidth="10"
                strokeDasharray={`${fitnessScore * 4.9} 490`}
                strokeLinecap="round"
                transform="rotate(-90 100 100)"
                style={{
                  transition: "stroke-dasharray 0.8s ease, stroke 0.5s ease",
                  filter: `drop-shadow(0 0 10px ${fitnessColor})`,
                }}
              />
              <circle cx="100" cy="100" r="56" fill="rgba(5,11,21,0.88)" stroke="rgba(74,247,211,0.08)" />
              <text x="100" y="96" textAnchor="middle" fill={fitnessColor} fontSize="38" fontFamily="var(--font-display)" fontWeight="700">
                {fitnessScore}
              </text>
              <text
                x="100"
                y="118"
                textAnchor="middle"
                fill="var(--text-secondary)"
                fontSize="12"
                fontFamily="var(--font-mono)"
                letterSpacing="0.24em"
                style={{ textTransform: "uppercase" }}
              >
                fitness
              </text>
            </svg>
          </div>
          <div className="mc-reactor-meta">
            <div>
              <span className="mc-reactor-label">Confidence</span>
              <strong>{avgConfidence.toFixed(2)}</strong>
            </div>
            <div>
              <span className="mc-reactor-label">Fatigue</span>
              <strong>{avgFatigue.toFixed(2)}</strong>
            </div>
            <div>
              <span className="mc-reactor-label">Threat Level</span>
              <strong style={{ color: immune ? threatColor(immune.threat_level) : "var(--text-primary)" }}>
                {immune?.threat_level ?? "Unknown"}
              </strong>
            </div>
          </div>
        </div>

        <div className="mc-panel mc-panel--network nx-spatial-layer-back">
          <div className="mc-network-head">
            <div>
              <div className="mc-kicker">Agent Constellation</div>
              <h2 className="mc-network-title">Runtime neural map</h2>
            </div>
            <span className="mc-chip">
              <TrendingUp size={13} aria-hidden="true" />
              {activeAgents} live nodes
            </span>
          </div>

          <div className="mc-network-stage">
            <div className="mc-network-core" />
            <svg className="mc-network-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
              {constellationNodes.map((node, index) => (
                <React.Fragment key={node.agent.id}>
                  <line x1="50" y1="50" x2={node.x} y2={node.y} />
                  {index > 0 ? (
                    <line x1={constellationNodes[index - 1].x} y1={constellationNodes[index - 1].y} x2={node.x} y2={node.y} />
                  ) : null}
                </React.Fragment>
              ))}
            </svg>

            {constellationNodes.map((node) => (
              <button type="button"
                key={node.agent.id}
                className={`mc-node mc-node--${node.tone}`}
                style={{
                  left: `${node.x}%`,
                  top: `${node.y}%`,
                  transform: `translate3d(-50%, -50%, ${node.z}px)`,
                }}
                onClick={() => nav("agents")}
              >
                <span className="mc-node__halo" />
                <span className="mc-node__name">{node.agent.name}</span>
                <span className="mc-node__meta">{node.agent.status}</span>
              </button>
            ))}
          </div>
        </div>
      </section>

      {error ? <div className="mc-alert">Error: {error}</div> : null}

      <div className="mc-stat-grid">
        <StatCard icon={<Users size={18} />} title="Agents" value={agents.length} color="var(--nexus-accent)" onClick={() => nav("agents")}>
          <StatRow label="Active" value={activeAgents} />
          <StatRow label="Genesis" value={genesisAgents} />
        </StatCard>

        <StatCard icon={<Brain size={18} />} title="Consciousness" value={avgConfidence.toFixed(2)} color="#2ad39d" onClick={() => nav("consciousness")}>
          <StatRow label="Avg confidence" value={avgConfidence.toFixed(2)} />
          <StatRow label="Avg fatigue" value={avgFatigue.toFixed(2)} />
        </StatCard>

        <StatCard icon={<ShieldCheck size={18} />} title="Immune" value={immune?.threat_level ?? "—"} color={immune ? threatColor(immune.threat_level) : "#6b7280"} onClick={() => nav("immune-dashboard")}>
          {immune ? <StatRow label="Blocked today" value={immune.threats_blocked} /> : <EmptyState />}
        </StatCard>

        <StatCard icon={<Coins size={18} />} title="Economy" value={economy ? Math.round(economy.total_tokens_circulating).toLocaleString() : "—"} color="var(--nexus-amber)" onClick={() => nav("civilization")}>
          {economy ? <StatRow label="Txns today" value={economy.transactions_today} /> : <EmptyState />}
        </StatCard>
      </div>

      <section
        className="mc-section mc-section--wide"
        role="button"
        tabIndex={0}
        onClick={() => nav("consciousness")}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            nav("consciousness");
          }
        }}
      >
        <div className="mc-section__head">
          <div>
            <div className="mc-kicker">Cognition Field</div>
            <h2 className="mc-section__title">Consciousness heatmap</h2>
          </div>
          <span className="mc-chip">
            <Brain size={13} aria-hidden="true" />
            {consciousness.length} tracked agents
          </span>
        </div>

        {consciousness.length > 0 ? (
          <>
            <div className="mc-heatmap">
              {consciousness.map((c) => (
                <div
                  key={c.agent_id}
                  className="mc-heatmap__cell"
                  title={`${c.agent_id.slice(0, 12)} — conf:${(c.confidence * 100).toFixed(0)}% fat:${(c.fatigue * 100).toFixed(0)}%${c.flow_state ? " FLOW" : ""}`}
                  style={{
                    background: emotionColor(c),
                    boxShadow: `0 0 14px ${emotionColor(c)}55`,
                  }}
                />
              ))}
            </div>
            <div className="mc-legend">
              {[
                { color: "#22c55e", label: "flow" },
                { color: "#eab308", label: "working" },
                { color: "#f97316", label: "frustrated" },
                { color: "#ef4444", label: "fatigued" },
                { color: "#3b82f6", label: "exploring" },
                { color: "#6b7280", label: "idle" },
              ].map((legend) => (
                <span key={legend.label} className="mc-legend__item">
                  <span className="mc-legend__swatch" style={{ background: legend.color }} />
                  {legend.label}
                </span>
              ))}
            </div>
          </>
        ) : <EmptyState />}
      </section>

      <div className="mc-lower-grid">
        <section className="mc-section" role="button" tabIndex={0} onClick={() => nav("dreams")} onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); nav("dreams"); } }}>
          <div className="mc-section__head">
            <div>
              <div className="mc-kicker">Subconscious Loop</div>
              <h2 className="mc-section__title">Dream Forge</h2>
            </div>
            <Moon size={16} aria-hidden="true" style={{ color: "var(--nexus-purple)" }} />
          </div>
          {dreams ? (
            <>
              <StatRow label="Status" value={dreams.active_dreams > 0 ? "Active" : "Idle"} />
              <StatRow label="Completed today" value={dreams.completed_today} />
              <StatRow label="Next dream" value={dreams.next_scheduled} />
              {briefing?.summary ? (
                <p className="mc-inline-note">
                  {briefing.summary.slice(0, 120)}{briefing.summary.length > 120 ? "..." : ""}
                </p>
              ) : null}
            </>
          ) : <EmptyState />}
        </section>

        <section className="mc-section" role="button" tabIndex={0} onClick={() => nav("temporal")} onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); nav("temporal"); } }}>
          <div className="mc-section__head">
            <div>
              <div className="mc-kicker">Timeline Engine</div>
              <h2 className="mc-section__title">Temporal Engine</h2>
            </div>
            <GitBranch size={16} aria-hidden="true" style={{ color: "var(--nexus-info)" }} />
          </div>
          {temporal ? (
            <>
              <StatRow label="Active forks" value={temporal.active_forks} />
              <StatRow label="Decisions made" value={temporal.decisions_made} />
              <StatRow label="Best score" value={`${(temporal.best_timeline_score * 10).toFixed(1)}/10`} />
            </>
          ) : <EmptyState />}
        </section>

        <section className="mc-section" role="button" tabIndex={0} onClick={() => nav("identity")} onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); nav("identity"); } }}>
          <div className="mc-section__head">
            <div>
              <div className="mc-kicker">Distributed Mesh</div>
              <h2 className="mc-section__title">Mesh Status</h2>
            </div>
            <Network size={16} aria-hidden="true" style={{ color: "var(--nexus-accent)" }} />
          </div>
          <StatRow label="Peers" value={`${peers.length} (${peers.length === 0 ? "local only" : "connected"})`} />
          <StatRow label="Agents here" value={agents.length} />
          <StatRow label="Shared knowledge" value={peers.length > 0 ? agents.length * peers.length : 0} />
          {peers.length > 0 ? (
            <div className="mc-peer-list">
              {peers.slice(0, 3).map((peer) => (
                <div key={peer.peer_id} className="mc-peer-row">
                  <span
                    className="mc-peer-dot"
                    style={{
                      background: peerStatusColor(peer.status),
                      boxShadow: `0 0 10px ${peerStatusColor(peer.status)}`,
                    }}
                  />
                  <span>{peer.name || peer.address}</span>
                </div>
              ))}
            </div>
          ) : null}
        </section>

        <section className="mc-section">
          <div className="mc-section__head">
            <div>
              <div className="mc-kicker">Audit Stream</div>
              <h2 className="mc-section__title">Recent Activity</h2>
            </div>
            <Activity size={16} aria-hidden="true" style={{ color: "var(--nexus-accent)" }} />
          </div>
          {auditEvents.length > 0 ? auditEvents.map((evt) => (
            <div key={evt.event_id} className="mc-activity-row">
              <span className="mc-activity-time">{formatTime(evt.timestamp)}</span>
              <span className="mc-activity-agent">{evt.agent_id.slice(0, 10)}</span>
              <span className="mc-activity-type">{evt.event_type}</span>
            </div>
          )) : (
            <div className="mc-empty-state">No recent events</div>
          )}
        </section>
      </div>
    </div>
  );
}

/* ================================================================== */
/*  Shared sub-components                                              */
/* ================================================================== */

function StatCard({ icon, title, value, color, onClick, children }: {
  icon: JSX.Element;
  title: string;
  value: string | number;
  color: string;
  onClick?: () => void;
  children?: React.ReactNode;
}): JSX.Element {
  return (
    <div
      className="mc-stat-card"
      onClick={onClick}
      role="button"
      tabIndex={0}
      onKeyDown={(event) => {
        if (!onClick) return;
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onClick();
        }
      }}
      style={{ "--mc-stat-accent": color } as React.CSSProperties}
    >
      <div className="mc-stat-card__head">
        <span className="mc-stat-card__icon" style={{ color }}>{icon}</span>
        <h3 className="mc-stat-card__title">{title}</h3>
      </div>
      <div className="mc-stat-card__value" style={{ color }}>{value}</div>
      {children}
    </div>
  );
}

function StatRow({ label, value }: { label: string; value: string | number }): JSX.Element {
  return (
    <div className="mc-stat-row">
      <span>{label}</span>
      <span>{value}</span>
    </div>
  );
}

function EmptyState(): JSX.Element {
  return (
    <div className="mc-empty-state">
      Loading...
    </div>
  );
}

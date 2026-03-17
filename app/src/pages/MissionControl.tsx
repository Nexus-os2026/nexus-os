import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

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

  const refresh = useCallback(async () => {
    try {
      const results = await Promise.allSettled([
        invoke<AgentSummary[]>("list_agents"),
        invoke<ImmuneStatus>("get_immune_status"),
        invoke<MeshPeer[]>("mesh_get_peers"),
        invoke<EconomyStatus>("civ_get_economy_status"),
        invoke<DreamStatus>("get_dream_status"),
        invoke<DreamBriefing>("get_morning_briefing"),
        invoke<ConsciousnessEntry[]>("get_consciousness_heatmap"),
        invoke<TemporalStatus>("get_temporal_history"),
        invoke<AuditEventRow[]>("get_audit_log"),
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
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
    const iv = setInterval(() => void refresh(), 10_000);
    return () => clearInterval(iv);
  }, [refresh]);

  // Uptime counter
  useEffect(() => {
    const start = Date.now();
    const tick = () => {
      const mins = Math.floor((Date.now() - start) / 60000);
      if (mins < 60) setUptime(`${mins}m`);
      else setUptime(`${Math.floor(mins / 60)}h ${mins % 60}m`);
    };
    tick();
    const iv = setInterval(tick, 60_000);
    return () => clearInterval(iv);
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

  const nav = (page: string) => onNavigate?.(page);

  return (
    <div style={{ padding: 24, color: "#e2e8f0", maxWidth: 1400, margin: "0 auto" }}>
      {/* Header */}
      <div style={{ ...cardStyle, marginBottom: 20, borderColor: "rgba(34,211,238,0.2)" }}>
        <h1 style={{ fontFamily: "monospace", fontSize: "1.6rem", color: "#22d3ee", marginBottom: 6 }}>
          NEXUS OS v9.0.0 — MISSION CONTROL
        </h1>
        <div style={{ display: "flex", gap: 20, fontSize: "0.78rem", color: "#94a3b8", flexWrap: "wrap" }}>
          <span>2,997 tests</span>
          <span>{agents.length}+{genesisAgents} agents</span>
          <span>26 modules</span>
          <span>uptime: {uptime}</span>
        </div>
      </div>

      {error && <div style={{ color: "#f87171", marginBottom: 16, fontSize: "0.85rem" }}>Error: {error}</div>}

      {/* Top Stats Row */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 16, marginBottom: 20 }}>
        {/* Agents */}
        <div style={cardStyle} onClick={() => nav("agents")} role="button" tabIndex={0}>
          <h3 style={headingStyle}>Agents</h3>
          <div style={{ fontSize: "1.8rem", fontWeight: 700, color: "#22d3ee", fontFamily: "monospace" }}>{agents.length}</div>
          <StatRow label="Active" value={activeAgents} />
          <StatRow label="Genesis" value={genesisAgents} />
        </div>

        {/* Consciousness */}
        <div style={cardStyle} onClick={() => nav("consciousness")} role="button" tabIndex={0}>
          <h3 style={headingStyle}>Consciousness</h3>
          <div style={{ fontSize: "1.8rem", fontWeight: 700, color: "#22c55e", fontFamily: "monospace" }}>
            {avgConfidence.toFixed(2)}
          </div>
          <StatRow label="Avg confidence" value={avgConfidence.toFixed(2)} />
          <StatRow label="Avg fatigue" value={avgFatigue.toFixed(2)} />
        </div>

        {/* Immune */}
        <div style={cardStyle} onClick={() => nav("immune-dashboard")} role="button" tabIndex={0}>
          <h3 style={headingStyle}>Immune System</h3>
          {immune ? (
            <>
              <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
                <span style={{
                  width: 14, height: 14, borderRadius: "50%",
                  background: threatColor(immune.threat_level), display: "inline-block",
                  boxShadow: `0 0 8px ${threatColor(immune.threat_level)}`,
                }} />
                <span style={{ fontSize: "1.2rem", fontWeight: 700, color: threatColor(immune.threat_level), fontFamily: "monospace" }}>
                  {immune.threat_level}
                </span>
              </div>
              <StatRow label="Blocked today" value={immune.threats_blocked} />
            </>
          ) : <EmptyState />}
        </div>

        {/* Economy */}
        <div style={cardStyle} onClick={() => nav("civilization")} role="button" tabIndex={0}>
          <h3 style={headingStyle}>Economy</h3>
          {economy ? (
            <>
              <div style={{ fontSize: "1.8rem", fontWeight: 700, color: "#eab308", fontFamily: "monospace" }}>
                {Math.round(economy.total_tokens_circulating).toLocaleString()}
              </div>
              <StatRow label="Total tokens" value="" />
              <StatRow label="Txns today" value={economy.transactions_today} />
            </>
          ) : <EmptyState />}
        </div>
      </div>

      {/* Consciousness Heatmap */}
      <div style={{ ...cardStyle, marginBottom: 20 }} onClick={() => nav("consciousness")} role="button" tabIndex={0}>
        <h3 style={headingStyle}>Consciousness Heatmap</h3>
        {consciousness.length > 0 ? (
          <div>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(28px, 1fr))", gap: 5 }}>
              {consciousness.map((c) => (
                <div key={c.agent_id} title={`${c.agent_id.slice(0, 12)} — conf:${(c.confidence * 100).toFixed(0)}% fat:${(c.fatigue * 100).toFixed(0)}%${c.flow_state ? " FLOW" : ""}`}
                  style={{
                    width: 24, height: 24, borderRadius: "50%",
                    background: emotionColor(c),
                    boxShadow: `0 0 6px ${emotionColor(c)}50`,
                    cursor: "pointer",
                    transition: "transform 0.2s",
                  }}
                  onMouseEnter={(e) => { (e.target as HTMLElement).style.transform = "scale(1.3)"; }}
                  onMouseLeave={(e) => { (e.target as HTMLElement).style.transform = "scale(1)"; }}
                />
              ))}
            </div>
            <div style={{ display: "flex", gap: 14, marginTop: 10, flexWrap: "wrap" }}>
              {[
                { color: "#22c55e", label: "flow" },
                { color: "#eab308", label: "working" },
                { color: "#f97316", label: "frustrated" },
                { color: "#ef4444", label: "fatigued" },
                { color: "#3b82f6", label: "exploring" },
                { color: "#6b7280", label: "idle" },
              ].map((l) => (
                <span key={l.label} style={{ display: "flex", alignItems: "center", gap: 4, fontSize: "0.68rem" }}>
                  <span style={{ width: 10, height: 10, borderRadius: "50%", background: l.color, display: "inline-block" }} />
                  <span style={{ color: "#94a3b8" }}>{l.label}</span>
                </span>
              ))}
            </div>
          </div>
        ) : <EmptyState />}
      </div>

      {/* Bottom 2x2 Grid */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 20 }}>
        {/* Dream Forge */}
        <div style={cardStyle} onClick={() => nav("dreams")} role="button" tabIndex={0}>
          <h3 style={headingStyle}>Dream Forge</h3>
          {dreams ? (
            <div>
              <StatRow label="Status" value={dreams.active_dreams > 0 ? "Active" : "Idle"} />
              <StatRow label="Completed today" value={dreams.completed_today} />
              <StatRow label="Next dream in" value={dreams.next_scheduled} />
              {briefing && briefing.summary && (
                <div style={{ marginTop: 10, padding: 10, background: "rgba(167,139,250,0.05)", borderRadius: 6, border: "1px solid rgba(167,139,250,0.15)" }}>
                  <div style={{ fontSize: "0.72rem", color: "#a78bfa", marginBottom: 4 }}>LAST BRIEFING</div>
                  <div style={{ fontSize: "0.75rem", color: "#94a3b8", fontStyle: "italic", lineHeight: 1.4 }}>
                    {briefing.summary.slice(0, 120)}{briefing.summary.length > 120 ? "..." : ""}
                  </div>
                </div>
              )}
            </div>
          ) : <EmptyState />}
        </div>

        {/* Temporal Engine */}
        <div style={cardStyle} onClick={() => nav("temporal")} role="button" tabIndex={0}>
          <h3 style={headingStyle}>Temporal Engine</h3>
          {temporal ? (
            <div>
              <StatRow label="Active forks" value={temporal.active_forks} />
              <StatRow label="Decisions made" value={temporal.decisions_made} />
              <StatRow label="Best score" value={`${(temporal.best_timeline_score * 10).toFixed(1)}/10`} />
            </div>
          ) : <EmptyState />}
        </div>

        {/* Mesh Status */}
        <div style={cardStyle} onClick={() => nav("identity")} role="button" tabIndex={0}>
          <h3 style={headingStyle}>Mesh Status</h3>
          <StatRow label="Peers" value={`${peers.length} (${peers.length === 0 ? "local only" : "connected"})`} />
          <StatRow label="Agents here" value={agents.length} />
          <StatRow label="Shared knowledge" value={0} />
          {peers.length > 0 && (
            <div style={{ marginTop: 8 }}>
              {peers.slice(0, 3).map((p) => (
                <div key={p.peer_id} style={{ display: "flex", alignItems: "center", gap: 6, padding: "3px 0", fontSize: "0.75rem" }}>
                  <span style={{ width: 6, height: 6, borderRadius: "50%", background: peerStatusColor(p.status), display: "inline-block" }} />
                  <span style={{ color: "#94a3b8" }}>{p.name || p.address}</span>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Recent Activity */}
        <div style={cardStyle}>
          <h3 style={headingStyle}>Recent Activity</h3>
          {auditEvents.length > 0 ? auditEvents.map((evt) => (
            <div key={evt.event_id} style={{ display: "flex", gap: 8, padding: "3px 0", fontSize: "0.75rem" }}>
              <span style={{ color: "#64748b", fontFamily: "monospace", width: 42, flexShrink: 0 }}>
                {formatTime(evt.timestamp)}
              </span>
              <span style={{ color: "#22d3ee" }}>{evt.agent_id.slice(0, 10)}</span>
              <span style={{ color: "#94a3b8", flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {evt.event_type}
              </span>
            </div>
          )) : (
            <div style={{ color: "#64748b", fontSize: "0.82rem" }}>No recent events</div>
          )}
        </div>
      </div>
    </div>
  );
}

/* ================================================================== */
/*  Shared sub-components & styles                                     */
/* ================================================================== */

function StatRow({ label, value }: { label: string; value: string | number }): JSX.Element {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", padding: "3px 0", fontSize: "0.82rem" }}>
      <span style={{ color: "#94a3b8" }}>{label}</span>
      <span style={{ fontFamily: "monospace", color: "#e2e8f0" }}>{value}</span>
    </div>
  );
}

function EmptyState(): JSX.Element {
  return <div style={{ color: "#64748b", fontSize: "0.82rem" }}>Loading...</div>;
}

const cardStyle: React.CSSProperties = {
  background: "rgba(15,23,42,0.7)",
  border: "1px solid #1e293b",
  borderRadius: 10,
  padding: 20,
  backdropFilter: "blur(8px)",
  cursor: "pointer",
  transition: "border-color 0.2s",
};

const headingStyle: React.CSSProperties = {
  fontFamily: "monospace",
  fontSize: "0.95rem",
  color: "#22d3ee",
  marginBottom: 14,
  paddingBottom: 8,
  borderBottom: "1px solid #1e293b",
};

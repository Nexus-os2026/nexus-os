import { useEffect, useMemo, useState } from "react";
import "./audit-timeline.css";
import type { AuditEventRow, AuditChainStatusRow } from "../types";
import { getAuditLog, getAuditChainStatus, hasDesktopRuntime } from "../api/backend";

interface AuditTimelineProps {
  events: AuditEventRow[];
}

const EVENT_TYPE_COLORS: Record<string, string> = {
  StateChange: "#3b82f6",
  ToolCall: "#22c55e",
  LlmCall: "#a855f7",
  Error: "#ef4444",
  UserAction: "#f59e0b",
};

const EVENT_TYPES = ["All", "StateChange", "ToolCall", "LlmCall", "Error", "UserAction"];

function formatTimestamp(ts: number): string {
  const d = new Date(ts * 1000);
  const pad = (n: number): string => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

function payloadSummary(payload: Record<string, unknown>): string {
  const parts: string[] = [];
  for (const [key, val] of Object.entries(payload)) {
    if (key === "previous_hash" || key === "hash") continue;
    const s = typeof val === "object" ? JSON.stringify(val) : String(val);
    parts.push(`${key}: ${s.length > 40 ? s.slice(0, 40) + "..." : s}`);
    if (parts.length >= 3) break;
  }
  return parts.join(" | ") || "no payload";
}

function shortAgent(agentId: string): string {
  return agentId.length > 12 ? agentId.slice(0, 8) + "..." : agentId;
}

export default function AuditTimeline({ events }: AuditTimelineProps): JSX.Element {
  const [agentFilter, setAgentFilter] = useState("All");
  const [typeFilter, setTypeFilter] = useState("All");
  const [chainStatus, setChainStatus] = useState<AuditChainStatusRow | null>(null);
  const [liveEvents, setLiveEvents] = useState<AuditEventRow[]>(events);
  const [refreshing, setRefreshing] = useState(false);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLiveEvents(events);
  }, [events]);

  // Load live audit data on mount when the desktop runtime is available.
  useEffect(() => {
    if (!hasDesktopRuntime()) {
      setLoading(false);
      return;
    }
    Promise.all([getAuditLog(undefined, 500), getAuditChainStatus()])
      .then(([freshEvents, status]) => {
        setLiveEvents(freshEvents);
        setChainStatus(status);
      })
      .catch((e) => { if (import.meta.env.DEV) console.warn("[AuditTimeline]", e); })
      .finally(() => setLoading(false));
  }, []);

  // Auto-refresh every 10 seconds
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    const timer = window.setInterval(() => {
      getAuditLog(undefined, 500).then(setLiveEvents).catch((e) => { if (import.meta.env.DEV) console.warn("[AuditTimeline]", e); });
    }, 10_000);
    return () => window.clearInterval(timer);
  }, []);

  const agents = useMemo(() => {
    const ids = Array.from(new Set(liveEvents.map((e) => e.agent_id)));
    return ["All", ...ids.sort()];
  }, [liveEvents]);

  const filtered = useMemo(() => {
    const sorted = [...liveEvents].sort((a, b) => b.timestamp - a.timestamp);
    return sorted.filter((e) => {
      if (agentFilter !== "All" && e.agent_id !== agentFilter) return false;
      if (typeFilter !== "All" && e.event_type !== typeFilter) return false;
      return true;
    });
  }, [liveEvents, agentFilter, typeFilter]);

  async function handleRefresh(): Promise<void> {
    setRefreshing(true);
    try {
      if (hasDesktopRuntime()) {
        const [fresh, status] = await Promise.all([getAuditLog(undefined, 500), getAuditChainStatus()]);
        setLiveEvents(fresh);
        setChainStatus(status);
      }
    } catch {
      // ignore
    }
    setRefreshing(false);
  }

  if (loading) return (
    <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100%", color: "#64748b", fontSize: 14 }}>
      Loading...
    </div>
  );

  return (
    <section className="at-hub">
      <header className="at-header">
        <div>
          <h2 className="at-title">AUDIT TIMELINE // GOVERNANCE LOG</h2>
          <p className="at-subtitle">
            {filtered.length} events shown
            {chainStatus && (
              <span style={{ marginLeft: "1rem", color: chainStatus.chain_valid ? "#22c55e" : "#ef4444" }}>
                {chainStatus.chain_valid
                  ? `Chain valid (${chainStatus.total_events} events)`
                  : "Chain integrity BROKEN"}
              </span>
            )}
          </p>
        </div>
        <button type="button"
          className="at-select"
          style={{ cursor: "pointer", minWidth: "auto", padding: "0.4rem 1rem" }}
          onClick={() => void handleRefresh()}
          disabled={refreshing}
        >
          {refreshing ? "Refreshing..." : "Refresh"}
        </button>
      </header>

      <div className="at-filters">
        <select className="at-select" value={agentFilter} onChange={(e) => setAgentFilter(e.target.value)}>
          {agents.map((a) => <option key={a} value={a}>{a === "All" ? "All Agents" : shortAgent(a)}</option>)}
        </select>
        <select className="at-select" value={typeFilter} onChange={(e) => setTypeFilter(e.target.value)}>
          {EVENT_TYPES.map((t) => <option key={t} value={t}>{t === "All" ? "All Types" : t}</option>)}
        </select>
      </div>

      {filtered.length === 0 && (
        <div style={{ textAlign: "center", padding: "3rem 1rem", opacity: 0.6 }}>
          <p style={{ fontSize: "1.1rem" }}>No audit events yet. Start an agent to generate events.</p>
        </div>
      )}

      <div className="at-timeline">
        {filtered.map((event) => {
          const color = EVENT_TYPE_COLORS[event.event_type] ?? "#888";
          const expanded = expandedId === event.event_id;
          return (
            <div key={event.event_id} className="at-event" onClick={() => setExpandedId(expanded ? null : event.event_id)} style={{ cursor: "pointer" }}>
              <div className="at-event-line">
                <span className="at-event-dot" style={{ background: color }} />
              </div>
              <div className="at-event-card">
                <div className="at-event-top">
                  <span className="at-event-time">{formatTimestamp(event.timestamp)}</span>
                  <span className="at-event-agent">{shortAgent(event.agent_id)}</span>
                  <span className="at-event-type" style={{ color }}>
                    {event.event_type}
                  </span>
                </div>
                <p className="at-event-summary">{payloadSummary(event.payload)}</p>
                {expanded && (
                  <div style={{ marginTop: "0.5rem", fontSize: "0.8rem", opacity: 0.85 }}>
                    <div style={{ marginBottom: "0.25rem" }}>
                      <strong>Event ID:</strong> <span style={{ fontFamily: "monospace" }}>{event.event_id}</span>
                    </div>
                    <div style={{ marginBottom: "0.25rem" }}>
                      <strong>Hash:</strong> <span style={{ fontFamily: "monospace" }}>{event.hash}</span>
                    </div>
                    <div style={{ marginBottom: "0.25rem" }}>
                      <strong>Previous:</strong> <span style={{ fontFamily: "monospace" }}>{event.previous_hash}</span>
                    </div>
                    <pre style={{ background: "rgba(0,0,0,0.3)", padding: "0.5rem", borderRadius: "4px", overflowX: "auto", fontSize: "0.75rem" }}>
                      {JSON.stringify(event.payload, null, 2)}
                    </pre>
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}

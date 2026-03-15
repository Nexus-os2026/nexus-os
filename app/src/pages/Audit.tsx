import { useCallback, useEffect, useMemo, useState } from "react";
import "./audit.css";
import type { AuditEventRow, AuditChainStatusRow } from "../types";
import { getAuditLog, getAuditChainStatus, hasDesktopRuntime } from "../api/backend";

interface AuditProps {
  events: AuditEventRow[];
  onRefresh?: () => void;
}

const EVENT_TYPE_COLORS: Record<string, string> = {
  StateChange: "#3b82f6",
  ToolCall: "#22c55e",
  LlmCall: "#a855f7",
  Error: "#ef4444",
  UserAction: "#f59e0b",
};

function agentColor(agentId: string): string {
  let hash = 0;
  for (let i = 0; i < agentId.length; i++) {
    hash = agentId.charCodeAt(i) + ((hash << 5) - hash);
  }
  const hue = Math.abs(hash) % 360;
  return `hsl(${hue}, 70%, 55%)`;
}

function shortAgent(agentId: string): string {
  return agentId.length > 12 ? agentId.slice(0, 8) + "..." : agentId;
}

type StatusType = "Success" | "Failed" | "Pending";

function eventStatus(eventType: string): StatusType {
  if (eventType.toLowerCase().includes("error")) return "Failed";
  if (eventType.toLowerCase().includes("approval") && eventType.toLowerCase().includes("required")) return "Pending";
  return "Success";
}

function fuelCost(payload: Record<string, unknown>): number | null {
  if (typeof payload.consumed === "number") return payload.consumed;
  if (typeof payload.tokens === "number") return Math.round((payload.tokens as number) * 0.3);
  if (typeof payload.cost === "number") return Math.round((payload.cost as number) * 10000);
  if (typeof payload.fuel === "number") return payload.fuel as number;
  return null;
}

function formatDateTime(timestamp: number): string {
  const d = new Date(timestamp * 1000);
  const pad = (n: number): string => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

type SortField = "index" | "timestamp" | "agent" | "action" | "status" | "fuel";
type SortDir = "asc" | "desc";

export function Audit({ events, onRefresh }: AuditProps): JSX.Element {
  const [query, setQuery] = useState("");
  const [agentFilter, setAgentFilter] = useState("all");
  const [statusFilter, setStatusFilter] = useState("all");
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [chainStatus, setChainStatus] = useState<AuditChainStatusRow | null>(null);
  const [verifyState, setVerifyState] = useState<"idle" | "running" | "done">("idle");
  const [sortField, setSortField] = useState<SortField>("index");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [liveEvents, setLiveEvents] = useState<AuditEventRow[]>(events);
  const [refreshing, setRefreshing] = useState(false);

  useEffect(() => {
    setLiveEvents(events);
  }, [events]);

  const chronological = useMemo(
    () => [...liveEvents].sort((a, b) => a.timestamp - b.timestamp),
    [liveEvents]
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return chronological.filter((event) => {
      if (agentFilter !== "all" && event.agent_id !== agentFilter) return false;
      if (statusFilter !== "all" && eventStatus(event.event_type) !== statusFilter) return false;
      if (q.length > 0) {
        const text = `${event.event_id} ${event.event_type} ${event.agent_id} ${JSON.stringify(event.payload)}`.toLowerCase();
        if (!text.includes(q)) return false;
      }
      return true;
    });
  }, [chronological, query, agentFilter, statusFilter]);

  const sorted = useMemo(() => {
    const arr = [...filtered];
    arr.sort((a, b) => {
      let cmp = 0;
      if (sortField === "timestamp") cmp = a.timestamp - b.timestamp;
      else if (sortField === "agent") cmp = a.agent_id.localeCompare(b.agent_id);
      else if (sortField === "action") cmp = a.event_type.localeCompare(b.event_type);
      else if (sortField === "status") cmp = eventStatus(a.event_type).localeCompare(eventStatus(b.event_type));
      else if (sortField === "fuel") cmp = (fuelCost(a.payload) ?? 0) - (fuelCost(b.payload) ?? 0);
      else cmp = chronological.indexOf(a) - chronological.indexOf(b);
      return sortDir === "desc" ? -cmp : cmp;
    });
    return arr;
  }, [filtered, sortField, sortDir, chronological]);

  const agents = useMemo(
    () => Array.from(new Set(liveEvents.map((e) => e.agent_id))),
    [liveEvents]
  );

  const handleSort = useCallback((field: SortField) => {
    setSortField((prev) => {
      if (prev === field) {
        setSortDir((d) => (d === "asc" ? "desc" : "asc"));
        return prev;
      }
      setSortDir("asc");
      return field;
    });
  }, []);

  async function verifyChain(): Promise<void> {
    if (verifyState === "running") return;
    setVerifyState("running");
    setChainStatus(null);
    try {
      if (hasDesktopRuntime()) {
        const status = await getAuditChainStatus();
        setChainStatus(status);
      } else {
        // Client-side fallback for mock mode
        let valid = true;
        for (let i = 1; i < chronological.length; i++) {
          if (chronological[i].previous_hash !== chronological[i - 1].hash) {
            valid = false;
            break;
          }
        }
        setChainStatus({
          total_events: chronological.length,
          chain_valid: valid,
          first_hash: chronological[0]?.hash ?? "",
          last_hash: chronological[chronological.length - 1]?.hash ?? "",
        });
      }
    } catch {
      setChainStatus({ total_events: chronological.length, chain_valid: false, first_hash: "", last_hash: "" });
    }
    setVerifyState("done");
  }

  async function handleRefresh(): Promise<void> {
    setRefreshing(true);
    try {
      if (hasDesktopRuntime()) {
        const fresh = await getAuditLog(undefined, 500);
        setLiveEvents(fresh);
      }
      onRefresh?.();
    } catch {
      // ignore
    }
    setRefreshing(false);
  }

  useEffect(() => {
    if (verifyState === "done") {
      const timer = window.setTimeout(() => setVerifyState("idle"), 8000);
      return () => window.clearTimeout(timer);
    }
  }, [verifyState]);

  // Auto-refresh every 10 seconds when in desktop mode
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    const timer = window.setInterval(() => {
      getAuditLog(undefined, 500).then(setLiveEvents).catch(() => {});
    }, 10_000);
    return () => window.clearInterval(timer);
  }, []);

  function sortArrow(field: SortField): string {
    if (sortField !== field) return "";
    return sortDir === "asc" ? " \u25B2" : " \u25BC";
  }

  return (
    <section className="audit-forensic">
      <header className="audit-header">
        <div className="audit-header-left">
          <span className="audit-shield">&#x1F6E1;</span>
          <div>
            <h2 className="audit-title">AUDIT CHAIN // {chainStatus?.chain_valid === false ? "INTEGRITY BROKEN" : "INTEGRITY VERIFIED"}</h2>
            <p className="audit-subtitle">{chronological.length} events in chain</p>
          </div>
        </div>
        <div className="audit-header-right">
          {verifyState === "done" && chainStatus && (
            <span className={`audit-verify-result ${chainStatus.chain_valid ? "valid" : "invalid"}`}>
              {chainStatus.chain_valid
                ? `CHAIN VALID (${chainStatus.total_events} events)`
                : "CHAIN BROKEN"}
            </span>
          )}
          <button type="button" className="audit-verify-btn" onClick={() => void verifyChain()} disabled={verifyState === "running"}>
            {verifyState === "running" ? "Verifying..." : "VERIFY CHAIN"}
          </button>
          <button type="button" className="audit-verify-btn" onClick={() => void handleRefresh()} disabled={refreshing}>
            {refreshing ? "Refreshing..." : "REFRESH"}
          </button>
        </div>
      </header>

      {chronological.length === 0 && (
        <div style={{ textAlign: "center", padding: "3rem 1rem", opacity: 0.6 }}>
          <p style={{ fontSize: "1.1rem" }}>No audit events yet. Start an agent to generate events.</p>
        </div>
      )}

      <div className="audit-filters">
        <input
          className="audit-search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search events, payloads, hashes..."
        />
        <select className="audit-select" value={agentFilter} onChange={(e) => setAgentFilter(e.target.value)}>
          <option value="all">All Agents</option>
          {agents.map((id) => (
            <option key={id} value={id}>{shortAgent(id)}</option>
          ))}
        </select>
        <select className="audit-select" value={statusFilter} onChange={(e) => setStatusFilter(e.target.value)}>
          <option value="all">All Status</option>
          <option value="Success">Success</option>
          <option value="Failed">Failed</option>
          <option value="Pending">Pending</option>
        </select>
        <span className="audit-count">{sorted.length} / {chronological.length} events</span>
      </div>

      <div className="audit-table-wrap">
        <table className="audit-table">
          <thead>
            <tr>
              <th className="audit-th" onClick={() => handleSort("index")}>#{ sortArrow("index")}</th>
              <th className="audit-th" onClick={() => handleSort("timestamp")}>Timestamp{sortArrow("timestamp")}</th>
              <th className="audit-th" onClick={() => handleSort("agent")}>Agent{sortArrow("agent")}</th>
              <th className="audit-th" onClick={() => handleSort("action")}>Action{sortArrow("action")}</th>
              <th className="audit-th" onClick={() => handleSort("status")}>Status{sortArrow("status")}</th>
              <th className="audit-th" onClick={() => handleSort("fuel")}>Fuel Cost{sortArrow("fuel")}</th>
              <th className="audit-th">Hash</th>
            </tr>
          </thead>
          <tbody>
            {sorted.map((event, idx) => {
              const globalIdx = chronological.indexOf(event) + 1;
              const status = eventStatus(event.event_type);
              const color = EVENT_TYPE_COLORS[event.event_type] ?? agentColor(event.agent_id);
              const expanded = expandedId === event.event_id;
              const fuel = fuelCost(event.payload);
              return (
                <tr
                  key={event.event_id}
                  className={`audit-row ${expanded ? "expanded" : ""} ${idx % 2 === 0 ? "even" : "odd"}`}
                  onClick={() => setExpandedId(expanded ? null : event.event_id)}
                >
                  <td className="audit-td audit-td-index">{globalIdx}</td>
                  <td className="audit-td audit-td-time">{formatDateTime(event.timestamp)}</td>
                  <td className="audit-td">
                    <span className="audit-agent-dot" style={{ background: color }} />
                    {shortAgent(event.agent_id)}
                  </td>
                  <td className="audit-td audit-td-mono">{event.event_type}</td>
                  <td className="audit-td">
                    <span className={`audit-status-badge ${status.toLowerCase()}`}>{status}</span>
                  </td>
                  <td className="audit-td audit-td-mono">{fuel !== null ? fuel : "-"}</td>
                  <td className="audit-td audit-td-hash">
                    <span className="audit-hash-text">{event.hash.slice(0, 8)}...</span>
                    <button
                      type="button"
                      className="audit-copy-btn"
                      onClick={(e) => {
                        e.stopPropagation();
                        void navigator.clipboard.writeText(event.hash);
                      }}
                      title="Copy hash"
                    >
                      &#x2398;
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {expandedId && (() => {
        const event = chronological.find((e) => e.event_id === expandedId);
        if (!event) return null;
        return (
          <div className="audit-detail-panel">
            <div className="audit-detail-header">
              <h3>Event Detail: {event.event_id}</h3>
              <button type="button" className="audit-detail-close" onClick={() => setExpandedId(null)}>&#x2715;</button>
            </div>
            <div className="audit-detail-grid">
              <div className="audit-detail-field">
                <span className="audit-detail-label">Full Hash</span>
                <span className="audit-detail-value mono">{event.hash}</span>
              </div>
              <div className="audit-detail-field">
                <span className="audit-detail-label">Previous Hash</span>
                <span className="audit-detail-value mono">{event.previous_hash}</span>
              </div>
              <div className="audit-detail-field">
                <span className="audit-detail-label">Event Type</span>
                <span className="audit-detail-value">{event.event_type}</span>
              </div>
              <div className="audit-detail-field">
                <span className="audit-detail-label">Agent ID</span>
                <span className="audit-detail-value mono">{event.agent_id}</span>
              </div>
              <div className="audit-detail-field">
                <span className="audit-detail-label">Timestamp</span>
                <span className="audit-detail-value">{formatDateTime(event.timestamp)}</span>
              </div>
            </div>
            <div className="audit-detail-payload">
              <span className="audit-detail-label">Payload JSON</span>
              <pre className="audit-detail-json">{JSON.stringify(event.payload, null, 2)}</pre>
            </div>
          </div>
        );
      })()}
    </section>
  );
}

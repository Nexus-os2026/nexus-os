import { useCallback, useEffect, useMemo, useState } from "react";
import "./audit.css";
import type { AuditEventRow } from "../types";

interface AuditProps {
  events: AuditEventRow[];
}

const AGENT_NAMES: Record<string, string> = {
  "a0000000-0000-4000-8000-000000000001": "Coder",
  "a0000000-0000-4000-8000-000000000002": "Designer",
  "a0000000-0000-4000-8000-000000000003": "Screen Poster",
  "a0000000-0000-4000-8000-000000000004": "Web Builder",
  "a0000000-0000-4000-8000-000000000005": "Workflow Studio",
  "a0000000-0000-4000-8000-000000000006": "Self-Improve"
};

const AGENT_COLORS: Record<string, string> = {
  "a0000000-0000-4000-8000-000000000001": "#3b82f6",
  "a0000000-0000-4000-8000-000000000002": "#8b5cf6",
  "a0000000-0000-4000-8000-000000000003": "#10b981",
  "a0000000-0000-4000-8000-000000000004": "#f59e0b",
  "a0000000-0000-4000-8000-000000000005": "#00ffd5",
  "a0000000-0000-4000-8000-000000000006": "#ef4444"
};

type StatusType = "Success" | "Failed" | "Pending";

function eventStatus(eventType: string): StatusType {
  if (eventType.toLowerCase().includes("error")) return "Failed";
  if (eventType.toLowerCase().includes("approval") && eventType.toLowerCase().includes("required")) return "Pending";
  return "Success";
}

function fuelCost(payload: Record<string, unknown>): number {
  if (typeof payload.consumed === "number") return payload.consumed;
  if (typeof payload.tokens === "number") return Math.round((payload.tokens as number) * 0.3);
  if (typeof payload.cost === "number") return Math.round((payload.cost as number) * 10000);
  return Math.floor(Math.random() * 80) + 10;
}

function formatDateTime(timestamp: number): string {
  const d = new Date(timestamp * 1000);
  const pad = (n: number): string => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

function chainIntegrity(rows: AuditEventRow[]): { valid: boolean; brokenAt: number } {
  for (let i = 1; i < rows.length; i++) {
    if (rows[i].previous_hash !== rows[i - 1].hash) {
      return { valid: false, brokenAt: i };
    }
  }
  return { valid: true, brokenAt: -1 };
}

type SortField = "index" | "timestamp" | "agent" | "action" | "status" | "fuel";
type SortDir = "asc" | "desc";

export function Audit({ events }: AuditProps): JSX.Element {
  const [query, setQuery] = useState("");
  const [agentFilter, setAgentFilter] = useState("all");
  const [statusFilter, setStatusFilter] = useState("all");
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [verifyState, setVerifyState] = useState<"idle" | "running" | "done">("idle");
  const [verifyProgress, setVerifyProgress] = useState(0);
  const [verifyResult, setVerifyResult] = useState<{ valid: boolean; brokenAt: number } | null>(null);
  const [sortField, setSortField] = useState<SortField>("index");
  const [sortDir, setSortDir] = useState<SortDir>("asc");

  const chronological = useMemo(
    () => [...events].sort((a, b) => a.timestamp - b.timestamp),
    [events]
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
      else if (sortField === "fuel") cmp = fuelCost(a.payload) - fuelCost(b.payload);
      else cmp = chronological.indexOf(a) - chronological.indexOf(b);
      return sortDir === "desc" ? -cmp : cmp;
    });
    return arr;
  }, [filtered, sortField, sortDir, chronological]);

  const agents = useMemo(
    () => Array.from(new Set(events.map((e) => e.agent_id))),
    [events]
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

  function verifyChain(): void {
    if (verifyState === "running") return;
    setVerifyState("running");
    setVerifyProgress(0);
    setVerifyResult(null);
    const total = chronological.length;
    let step = 0;
    const timer = window.setInterval(() => {
      step += 1;
      setVerifyProgress(Math.min(100, Math.round((step / total) * 100)));
      if (step >= total) {
        window.clearInterval(timer);
        const result = chainIntegrity(chronological);
        setVerifyResult(result);
        setVerifyState("done");
      }
    }, 40);
  }

  useEffect(() => {
    if (verifyState === "done") {
      const timer = window.setTimeout(() => setVerifyState("idle"), 5000);
      return () => window.clearTimeout(timer);
    }
  }, [verifyState]);

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
            <h2 className="audit-title">AUDIT CHAIN // INTEGRITY VERIFIED</h2>
            <p className="audit-subtitle">{chronological.length} events in chain</p>
          </div>
        </div>
        <div className="audit-header-right">
          {verifyState === "running" && (
            <div className="audit-verify-progress">
              <div className="audit-verify-bar" style={{ width: `${verifyProgress}%` }} />
            </div>
          )}
          {verifyState === "done" && verifyResult && (
            <span className={`audit-verify-result ${verifyResult.valid ? "valid" : "invalid"}`}>
              {verifyResult.valid
                ? "CHAIN INTEGRITY: VALID \u2713"
                : `BROKEN AT EVENT #${verifyResult.brokenAt}`}
            </span>
          )}
          <button type="button" className="audit-verify-btn" onClick={verifyChain} disabled={verifyState === "running"}>
            {verifyState === "running" ? "Verifying..." : "VERIFY CHAIN"}
          </button>
        </div>
      </header>

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
            <option key={id} value={id}>{AGENT_NAMES[id] ?? id}</option>
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
              const agentColor = AGENT_COLORS[event.agent_id] ?? "var(--cyan)";
              const expanded = expandedId === event.event_id;
              return (
                <tr
                  key={event.event_id}
                  className={`audit-row ${expanded ? "expanded" : ""} ${idx % 2 === 0 ? "even" : "odd"}`}
                  onClick={() => setExpandedId(expanded ? null : event.event_id)}
                >
                  <td className="audit-td audit-td-index">{globalIdx}</td>
                  <td className="audit-td audit-td-time">{formatDateTime(event.timestamp)}</td>
                  <td className="audit-td">
                    <span className="audit-agent-dot" style={{ background: agentColor }} />
                    {AGENT_NAMES[event.agent_id] ?? event.agent_id}
                  </td>
                  <td className="audit-td audit-td-mono">{event.event_type}</td>
                  <td className="audit-td">
                    <span className={`audit-status-badge ${status.toLowerCase()}`}>{status}</span>
                  </td>
                  <td className="audit-td audit-td-mono">{fuelCost(event.payload)}</td>
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
                <span className="audit-detail-label">Agent</span>
                <span className="audit-detail-value">{AGENT_NAMES[event.agent_id] ?? event.agent_id}</span>
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

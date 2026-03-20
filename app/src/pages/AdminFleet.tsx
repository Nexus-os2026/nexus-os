import { useCallback, useEffect, useState } from "react";
import {
  adminFleetStatus,
  adminAgentStopAll,
  adminAgentBulkUpdate,
} from "../api/backend";
import "./admin.css";

export interface AgentFleetEntry {
  did: string;
  name: string;
  workspace_id: string;
  autonomy_level: number;
  status: "Running" | "Idle" | "Stopped" | "Error" | "AwaitingHitl";
  error_message?: string;
  fuel_remaining: number;
  fuel_budget: number;
  last_active: string;
  uptime_seconds: number;
  version: string;
}

export interface FleetStatusData {
  agents: AgentFleetEntry[];
  total_running: number;
  total_idle: number;
  total_stopped: number;
  total_error: number;
}

const AUTONOMY_LABELS = ["L0 Inert", "L1 Suggest", "L2 Approve", "L3 Report", "L4 Bounded", "L5 Full"];

const EMPTY_FLEET: FleetStatusData = {
  agents: [],
  total_running: 0,
  total_idle: 0,
  total_stopped: 0,
  total_error: 0,
};

function statusDotClass(status: string): string {
  if (status === "Running") return "admin-dot admin-dot--running";
  if (status === "Idle" || status === "AwaitingHitl") return "admin-dot admin-dot--idle";
  if (status === "Error") return "admin-dot admin-dot--error";
  return "admin-dot admin-dot--stopped";
}

function fuelPercent(remaining: number, budget: number): number {
  return budget > 0 ? Math.round((remaining / budget) * 100) : 0;
}

function formatUptime(seconds: number): string {
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)}h`;
  return `${Math.floor(seconds / 86400)}d ${Math.floor((seconds % 86400) / 3600)}h`;
}

export default function AdminFleet() {
  const [fleet, setFleet] = useState<FleetStatusData>(EMPTY_FLEET);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [wsFilter, setWsFilter] = useState("");
  const [statusFilter, setStatusFilter] = useState("");

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const data = await adminFleetStatus();
      setFleet(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load fleet status");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const toggleSelect = (did: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(did)) next.delete(did);
      else next.add(did);
      return next;
    });
  };

  const selectAll = () => {
    if (selected.size === filtered.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(filtered.map((a) => a.did)));
    }
  };

  const handleStopAll = async (workspaceId: string) => {
    try {
      const count = await adminAgentStopAll(workspaceId);
      alert(`Stopped ${count} agents`);
      void refresh();
    } catch {
      /* no-op */
    }
  };

  const handleBulkUpdate = async (action: string) => {
    if (selected.size === 0) return;
    try {
      await adminAgentBulkUpdate(Array.from(selected), { action });
      void refresh();
    } catch {
      /* no-op */
    }
  };

  const workspaces = [...new Set(fleet.agents.map((a) => a.workspace_id))];

  const filtered = fleet.agents.filter((a) => {
    if (wsFilter && a.workspace_id !== wsFilter) return false;
    if (statusFilter && a.status !== statusFilter) return false;
    return true;
  });

  return (
    <div className="admin-shell">
      <h1>Agent Fleet</h1>
      <p className="admin-subtitle">Manage deployed agents across all workspaces</p>

      {error && <div className="admin-error">{error}</div>}

      {/* ── Fleet Summary ── */}
      <div className="admin-metrics">
        <div className="admin-metric">
          <span className="admin-metric__label">Running</span>
          <span className="admin-metric__value">{fleet.total_running}</span>
        </div>
        <div className="admin-metric">
          <span className="admin-metric__label">Idle</span>
          <span className="admin-metric__value" style={{ color: "var(--nexus-amber)" }}>{fleet.total_idle}</span>
        </div>
        <div className="admin-metric">
          <span className="admin-metric__label">Stopped</span>
          <span className="admin-metric__value" style={{ color: "var(--text-muted)" }}>{fleet.total_stopped}</span>
        </div>
        <div className="admin-metric">
          <span className="admin-metric__label">Errors</span>
          <span className="admin-metric__value" style={{ color: fleet.total_error > 0 ? "var(--nexus-danger)" : undefined }}>{fleet.total_error}</span>
        </div>
      </div>

      {/* ── Filters & Bulk Actions ── */}
      <div style={{ display: "flex", gap: "0.6rem", marginBottom: "1rem", flexWrap: "wrap", alignItems: "center" }}>
        <select className="admin-select" value={wsFilter} onChange={(e) => setWsFilter(e.target.value)}>
          <option value="">All Workspaces</option>
          {workspaces.map((w) => <option key={w} value={w}>{w}</option>)}
        </select>
        <select className="admin-select" value={statusFilter} onChange={(e) => setStatusFilter(e.target.value)}>
          <option value="">All Statuses</option>
          <option value="Running">Running</option>
          <option value="Idle">Idle</option>
          <option value="Stopped">Stopped</option>
          <option value="Error">Error</option>
          <option value="AwaitingHitl">Awaiting HITL</option>
        </select>
        <div style={{ flex: 1 }} />
        {selected.size > 0 && (
          <>
            <span style={{ fontSize: "0.78rem", color: "var(--text-secondary)" }}>{selected.size} selected</span>
            <button className="admin-btn admin-btn--accent admin-btn--sm" onClick={() => void handleBulkUpdate("restart")}>Restart</button>
            <button className="admin-btn admin-btn--danger admin-btn--sm" onClick={() => void handleBulkUpdate("stop")}>Stop</button>
          </>
        )}
        <button className="admin-btn admin-btn--danger" onClick={() => void handleStopAll(wsFilter || "all")}>
          Stop All
        </button>
      </div>

      {/* ── Agent Table ── */}
      <div className="admin-card" style={{ overflow: "auto" }}>
        <table className="admin-table">
          <thead>
            <tr>
              <th style={{ width: 32 }}>
                <input type="checkbox" checked={selected.size === filtered.length && filtered.length > 0} onChange={selectAll} />
              </th>
              <th>Agent</th>
              <th>Workspace</th>
              <th>Status</th>
              <th>Autonomy</th>
              <th>Fuel</th>
              <th>Uptime</th>
              <th>Version</th>
            </tr>
          </thead>
          <tbody>
            {loading && <tr><td colSpan={8} style={{ textAlign: "center", padding: "2rem" }}>Loading fleet...</td></tr>}
            {!loading && filtered.length === 0 && <tr><td colSpan={8} className="admin-empty">No agents match filter</td></tr>}
            {filtered.map((a) => {
              const fp = fuelPercent(a.fuel_remaining, a.fuel_budget);
              return (
                <tr key={a.did}>
                  <td><input type="checkbox" checked={selected.has(a.did)} onChange={() => toggleSelect(a.did)} /></td>
                  <td style={{ color: "var(--text-primary)", fontWeight: 500 }}>
                    {a.name}
                    <div style={{ fontSize: "0.68rem", color: "var(--text-muted)", fontFamily: "var(--font-mono)" }}>{a.did}</div>
                  </td>
                  <td>{a.workspace_id}</td>
                  <td>
                    <span className={statusDotClass(a.status)} />
                    {a.status}
                    {a.error_message && <div style={{ fontSize: "0.68rem", color: "var(--nexus-danger)" }}>{a.error_message}</div>}
                  </td>
                  <td>{AUTONOMY_LABELS[a.autonomy_level] ?? `L${a.autonomy_level}`}</td>
                  <td>
                    <div style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
                      <div className="admin-bar" style={{ width: 60 }}>
                        <div
                          className={`admin-bar__fill ${fp < 20 ? "admin-bar__fill--warn" : "admin-bar__fill--ok"}`}
                          style={{ width: `${fp}%` }}
                        />
                      </div>
                      <span style={{ fontSize: "0.72rem" }}>{fp}%</span>
                    </div>
                  </td>
                  <td>{formatUptime(a.uptime_seconds)}</td>
                  <td style={{ fontFamily: "var(--font-mono)", fontSize: "0.72rem" }}>{a.version}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}

import { useCallback, useEffect, useState } from "react";
import {
  adminOverview,
} from "../api/backend";
import "./admin.css";

export interface AdminOverviewData {
  total_agents: number;
  active_agents: number;
  total_users: number;
  active_users: number;
  workspaces: number;
  fuel_consumed_24h: number;
  hitl_pending: number;
  security_events_24h: number;
  system_health: {
    status: string;
    cpu_percent: number;
    memory_percent: number;
    disk_percent: number;
    uptime_seconds: number;
  };
}


function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  return `${days}d ${hours}h`;
}

function formatFuel(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export default function AdminDashboard() {
  const [overview, setOverview] = useState<AdminOverviewData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await adminOverview();
      setOverview(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load admin overview");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  if (loading) {
    return (
      <div className="admin-shell">
        <h1>Admin Console</h1>
        <p className="admin-subtitle">Loading fleet overview…</p>
      </div>
    );
  }

  if (error || !overview) {
    return (
      <div className="admin-shell">
        <h1>Admin Console</h1>
        <p className="admin-subtitle" style={{ color: "var(--nexus-danger)" }}>
          {error ?? "No data available"}
        </p>
        <button className="admin-btn" onClick={refresh}>Retry</button>
      </div>
    );
  }

  const h = overview.system_health;

  return (
    <div className="admin-shell">
      <h1>Admin Console</h1>
      <p className="admin-subtitle">Fleet overview &mdash; live</p>

      {/* ── Top Metrics ── */}
      <div className="admin-metrics">
        <div className="admin-metric">
          <span className="admin-metric__label">Total Agents</span>
          <span className="admin-metric__value">{overview.total_agents}</span>
          <span className="admin-metric__sub">{overview.active_agents} active</span>
        </div>
        <div className="admin-metric">
          <span className="admin-metric__label">Users</span>
          <span className="admin-metric__value">{overview.total_users}</span>
          <span className="admin-metric__sub">{overview.active_users} online</span>
        </div>
        <div className="admin-metric">
          <span className="admin-metric__label">Workspaces</span>
          <span className="admin-metric__value">{overview.workspaces}</span>
        </div>
        <div className="admin-metric">
          <span className="admin-metric__label">Fuel (24h)</span>
          <span className="admin-metric__value">{formatFuel(overview.fuel_consumed_24h)}</span>
        </div>
        <div className="admin-metric">
          <span className="admin-metric__label">HITL Pending</span>
          <span className="admin-metric__value" style={{ color: overview.hitl_pending > 0 ? "var(--nexus-amber)" : undefined }}>
            {overview.hitl_pending}
          </span>
        </div>
        <div className="admin-metric">
          <span className="admin-metric__label">Security Events (24h)</span>
          <span className="admin-metric__value" style={{ color: overview.security_events_24h > 0 ? "var(--nexus-danger)" : undefined }}>
            {overview.security_events_24h}
          </span>
        </div>
      </div>

      <div className="admin-grid-2">
        {/* ── System Health ── */}
        <div className="admin-card">
          <div className="admin-card__title">System Health</div>
          <div style={{ display: "flex", flexDirection: "column", gap: "0.8rem" }}>
            <HealthBar label="CPU" value={h.cpu_percent} />
            <HealthBar label="Memory" value={h.memory_percent} />
            <HealthBar label="Disk" value={h.disk_percent} />
            <div style={{ display: "flex", justifyContent: "space-between", fontSize: "0.78rem", color: "var(--text-secondary)" }}>
              <span>Status: <span style={{ color: h.status === "healthy" ? "var(--nexus-accent)" : "var(--nexus-danger)" }}>{h.status}</span></span>
              <span>Uptime: {formatUptime(h.uptime_seconds)}</span>
            </div>
          </div>
        </div>

        {/* ── Recent Security Events ── */}
        <div className="admin-card">
          <div className="admin-card__title">Security Events (last 24h)</div>
          {overview.security_events_24h === 0 ? (
            <div className="admin-empty">No security events in the last 24 hours</div>
          ) : (
            <div className={`admin-alert ${overview.security_events_24h > 5 ? "admin-alert--danger" : "admin-alert--warn"}`}>
              <span style={{ fontWeight: 600, fontSize: "0.72rem", textTransform: "uppercase" }}>Security Alert</span>
              <span style={{ flex: 1 }}>
                {overview.security_events_24h} security event{overview.security_events_24h !== 1 ? "s" : ""} detected in the last 24 hours
              </span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function HealthBar({ label, value }: { label: string; value: number }) {
  const fillClass = value > 85 ? "admin-bar__fill--warn" : "admin-bar__fill--ok";
  return (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", fontSize: "0.75rem", marginBottom: "0.25rem" }}>
        <span style={{ color: "var(--text-secondary)" }}>{label}</span>
        <span style={{ color: "var(--text-primary)" }}>{value}%</span>
      </div>
      <div className="admin-bar">
        <div className={`admin-bar__fill ${fillClass}`} style={{ width: `${value}%` }} />
      </div>
    </div>
  );
}

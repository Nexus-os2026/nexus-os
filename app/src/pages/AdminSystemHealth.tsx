import { useCallback, useEffect, useState } from "react";
import {
  adminSystemHealth,
  backupCreate,
  backupList,
  backupRestore,
  backupVerify,
  telemetryHealth,
} from "../api/backend";
import "./admin.css";

interface InstanceInfo {
  id: string;
  hostname: string;
  status: "online" | "degraded" | "offline";
  cpu_percent: number;
  memory_percent: number;
  disk_percent: number;
  agent_count: number;
  uptime_seconds: number;
}

interface ProviderHealth {
  name: string;
  status: "healthy" | "degraded" | "down";
  latency_ms: number;
  error_rate: number;
  requests_24h: number;
}

interface SystemHealthData {
  instances: InstanceInfo[];
  providers: ProviderHealth[];
  database: {
    size_mb: number;
    growth_rate_mb_day: number;
    tables: number;
    total_rows: number;
  };
  backup: {
    last_backup: string;
    next_scheduled: string;
    backup_size_mb: number;
    status: "ok" | "overdue" | "failed";
  };
}

interface BackupEntry {
  id: string;
  created_at: string;
  size_mb: number;
  status: string;
}

const EMPTY_HEALTH: SystemHealthData = {
  instances: [],
  providers: [],
  database: { size_mb: 0, growth_rate_mb_day: 0, tables: 0, total_rows: 0 },
  backup: {
    last_backup: new Date().toISOString(),
    next_scheduled: new Date().toISOString(),
    backup_size_mb: 0,
    status: "ok",
  },
};

function instanceStatusColor(s: InstanceInfo["status"]): string {
  if (s === "online") return "var(--nexus-accent)";
  if (s === "degraded") return "var(--nexus-amber)";
  return "var(--nexus-danger)";
}

function providerStatusColor(s: ProviderHealth["status"]): string {
  if (s === "healthy") return "var(--nexus-accent)";
  if (s === "degraded") return "var(--nexus-amber)";
  return "var(--nexus-danger)";
}

function formatUptime(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  return `${days}d ${hours}h`;
}

export default function AdminSystemHealth() {
  const [data, setData] = useState<SystemHealthData>(EMPTY_HEALTH);
  const [loading, setLoading] = useState(true);
  const [backups, setBackups] = useState<BackupEntry[]>([]);
  const [backupBusy, setBackupBusy] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [health, telemetry] = await Promise.allSettled([
        adminSystemHealth(),
        telemetryHealth(),
      ]);
      if (health.status === "fulfilled") {
        const base: SystemHealthData = health.value as SystemHealthData;
        // Merge telemetry provider data if available
        if (telemetry.status === "fulfilled" && telemetry.value) {
          const tel = telemetry.value as Partial<SystemHealthData>;
          setData({
            ...base,
            providers: tel.providers?.length ? tel.providers : base.providers,
          });
        } else {
          setData(base);
        }
      }
    } catch {
      // keep empty state; backend may not be running in web-only mode
    } finally {
      setLoading(false);
    }
  }, []);

  const refreshBackups = useCallback(async () => {
    try {
      const list = await backupList();
      setBackups(Array.isArray(list) ? list : []);
    } catch {
      // backend may not support backup commands yet
    }
  }, []);

  const handleCreateBackup = async () => {
    setBackupBusy("creating");
    try {
      await backupCreate();
      await refreshBackups();
    } catch {
      // ignore
    } finally {
      setBackupBusy(null);
    }
  };

  const handleVerify = async (id: string) => {
    setBackupBusy(`verify-${id}`);
    try {
      await backupVerify(id);
      await refreshBackups();
    } catch {
      // ignore
    } finally {
      setBackupBusy(null);
    }
  };

  const handleRestore = async (id: string) => {
    setBackupBusy(`restore-${id}`);
    try {
      await backupRestore(id);
    } catch {
      // ignore
    } finally {
      setBackupBusy(null);
    }
  };

  useEffect(() => {
    refresh();
    refreshBackups();
    const interval = setInterval(() => void refresh(), 30000);
    return () => clearInterval(interval);
  }, [refresh, refreshBackups]);

  return (
    <div className="admin-shell">
      <h1>System Health</h1>
      <p className="admin-subtitle">Instance monitoring, LLM providers, database, and backups{loading && " — loading..."}</p>

      {/* ── Instances ── */}
      <div className="admin-card">
        <div className="admin-card__title">Instances</div>
        <table className="admin-table">
          <thead>
            <tr>
              <th>Hostname</th>
              <th>Status</th>
              <th>CPU</th>
              <th>Memory</th>
              <th>Disk</th>
              <th>Agents</th>
              <th>Uptime</th>
            </tr>
          </thead>
          <tbody>
            {data.instances.map((inst) => (
              <tr key={inst.id}>
                <td style={{ color: "var(--text-primary)", fontWeight: 500, fontFamily: "var(--font-mono)", fontSize: "0.78rem" }}>{inst.hostname}</td>
                <td>
                  <span style={{ color: instanceStatusColor(inst.status), fontWeight: 600, fontSize: "0.75rem", textTransform: "uppercase" }}>
                    {inst.status}
                  </span>
                </td>
                <td><MiniBar value={inst.cpu_percent} /></td>
                <td><MiniBar value={inst.memory_percent} /></td>
                <td><MiniBar value={inst.disk_percent} /></td>
                <td>{inst.agent_count}</td>
                <td>{formatUptime(inst.uptime_seconds)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="admin-grid-2">
        {/* ── LLM Providers ── */}
        <div className="admin-card">
          <div className="admin-card__title">LLM Providers</div>
          <table className="admin-table">
            <thead>
              <tr>
                <th>Provider</th>
                <th>Status</th>
                <th>Latency</th>
                <th>Error Rate</th>
                <th>Requests (24h)</th>
              </tr>
            </thead>
            <tbody>
              {data.providers.map((p) => (
                <tr key={p.name}>
                  <td style={{ color: "var(--text-primary)", fontWeight: 500 }}>{p.name}</td>
                  <td>
                    <span style={{ color: providerStatusColor(p.status), fontWeight: 600, fontSize: "0.75rem", textTransform: "uppercase" }}>
                      {p.status}
                    </span>
                  </td>
                  <td style={{ fontFamily: "var(--font-mono)", fontSize: "0.78rem" }}>{p.latency_ms}ms</td>
                  <td style={{ color: p.error_rate > 2 ? "var(--nexus-danger)" : "var(--text-secondary)" }}>{p.error_rate}%</td>
                  <td>{p.requests_24h.toLocaleString()}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* ── Database & Backup ── */}
        <div>
          <div className="admin-card">
            <div className="admin-card__title">Database</div>
            <StatRow label="Size" value={`${data.database.size_mb} MB`} />
            <StatRow label="Growth Rate" value={`${data.database.growth_rate_mb_day} MB/day`} />
            <StatRow label="Tables" value={String(data.database.tables)} />
            <StatRow label="Total Rows" value={data.database.total_rows.toLocaleString()} />
          </div>

          <div className="admin-card">
            <div className="admin-card__title">Backup</div>
            <StatRow label="Status" value={data.backup.status.toUpperCase()} color={data.backup.status === "ok" ? "var(--nexus-accent)" : "var(--nexus-danger)"} />
            <StatRow label="Last Backup" value={new Date(data.backup.last_backup).toLocaleString()} />
            <StatRow label="Next Scheduled" value={new Date(data.backup.next_scheduled).toLocaleString()} />
            <StatRow label="Backup Size" value={`${data.backup.backup_size_mb} MB`} />
          </div>
        </div>
      </div>

      {/* ── Backups ── */}
      <div className="admin-card">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div className="admin-card__title">Backups</div>
          <button
            className="admin-btn admin-btn--primary"
            disabled={backupBusy !== null}
            onClick={handleCreateBackup}
          >
            {backupBusy === "creating" ? "Creating..." : "Create Backup"}
          </button>
        </div>
        <table className="admin-table">
          <thead>
            <tr>
              <th>ID</th>
              <th>Created</th>
              <th>Size</th>
              <th>Status</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {backups.length === 0 && (
              <tr>
                <td colSpan={5} style={{ textAlign: "center", color: "var(--text-secondary)" }}>No backups found</td>
              </tr>
            )}
            {backups.map((b) => (
              <tr key={b.id}>
                <td style={{ fontFamily: "var(--font-mono)", fontSize: "0.78rem", color: "var(--text-primary)" }}>{b.id.slice(0, 8)}</td>
                <td>{new Date(b.created_at).toLocaleString()}</td>
                <td style={{ fontFamily: "var(--font-mono)", fontSize: "0.78rem" }}>{b.size_mb} MB</td>
                <td>
                  <span style={{ color: b.status === "ok" ? "var(--nexus-accent)" : "var(--nexus-amber)", fontWeight: 600, fontSize: "0.75rem", textTransform: "uppercase" }}>
                    {b.status}
                  </span>
                </td>
                <td style={{ display: "flex", gap: "0.4rem" }}>
                  <button
                    className="admin-btn admin-btn--secondary"
                    disabled={backupBusy !== null}
                    onClick={() => handleVerify(b.id)}
                  >
                    {backupBusy === `verify-${b.id}` ? "Verifying..." : "Verify"}
                  </button>
                  <button
                    className="admin-btn admin-btn--secondary"
                    disabled={backupBusy !== null}
                    onClick={() => handleRestore(b.id)}
                  >
                    {backupBusy === `restore-${b.id}` ? "Restoring..." : "Restore"}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function MiniBar({ value }: { value: number }) {
  const fillClass = value > 85 ? "admin-bar__fill--warn" : "admin-bar__fill--ok";
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "0.3rem" }}>
      <div className="admin-bar" style={{ width: 50 }}>
        <div className={`admin-bar__fill ${fillClass}`} style={{ width: `${value}%` }} />
      </div>
      <span style={{ fontSize: "0.72rem", fontFamily: "var(--font-mono)" }}>{value}%</span>
    </div>
  );
}

function StatRow({ label, value, color }: { label: string; value: string; color?: string }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", padding: "0.35rem 0", fontSize: "0.82rem", borderBottom: "1px solid rgba(90,142,190,0.08)" }}>
      <span style={{ color: "var(--text-secondary)" }}>{label}</span>
      <span style={{ color: color ?? "var(--text-primary)", fontFamily: "var(--font-mono)" }}>{value}</span>
    </div>
  );
}

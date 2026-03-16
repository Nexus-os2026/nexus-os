import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Area, AreaChart, Bar, BarChart, CartesianGrid, Cell,
  Legend, Line, LineChart, Pie, PieChart,
  ResponsiveContainer, Tooltip, XAxis, YAxis,
} from "recharts";
import { getLiveSystemMetrics, listAgents } from "../api/backend";
import type { AgentSummary } from "../types";
import "./system-monitor.css";

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface LiveMetrics {
  cpu_name: string;
  cpu_cores: number;
  cpu_avg: number;
  per_core_usage: number[];
  total_ram: number;
  used_ram: number;
  available_ram: number;
  uptime_secs: number;
  process_count: number;
  nexus_disk_bytes: number;
  disk_total: number;
  disk_available: number;
  agents: AgentFuel[];
}

interface AgentFuel {
  id: string;
  name: string;
  state: string;
  fuel_budget: number;
  fuel_used: number;
  remaining_fuel: number;
}

interface MetricsSnapshot {
  ts: number;
  cpu: number;
  ram: number;
  disk: number;
}

interface FuelHistoryPoint {
  ts: string;
  [agentName: string]: string | number;
}

interface AlertEntry {
  id: string;
  ts: number;
  severity: "critical" | "warning" | "info";
  agent: string;
  message: string;
  dismissed: boolean;
}

interface AuditEntry {
  ts: number;
  event: string;
  detail: string;
}

type TabView = "overview" | "agents" | "fuel" | "alerts";

/* ================================================================== */
/*  Helpers                                                            */
/* ================================================================== */

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });
}

const CHART_COLORS = {
  cpu: "var(--nexus-accent)",
  ram: "#c084fc",
  disk: "#34d399",
};

const PIE_COLORS = ["var(--nexus-accent)", "#c084fc", "#34d399", "#f59e0b", "#60a5fa", "#fb923c", "#a78bfa", "#f472b6"];

const MAX_DATA_POINTS = 60;

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function SystemMonitor(): JSX.Element {
  const [tab, setTab] = useState<TabView>("overview");
  const [history, setHistory] = useState<MetricsSnapshot[]>([]);
  const [latest, setLatest] = useState<LiveMetrics | null>(null);
  const [allAgents, setAllAgents] = useState<AgentSummary[]>([]);
  const [fuelHistory, setFuelHistory] = useState<FuelHistoryPoint[]>([]);
  const [alerts, setAlerts] = useState<AlertEntry[]>([]);
  const [auditLog, setAuditLog] = useState<AuditEntry[]>([]);
  const [error, setError] = useState<string | null>(null);

  const appendAudit = useCallback((event: string, detail: string) => {
    setAuditLog((prev) => [{ ts: Date.now(), event, detail }, ...prev].slice(0, 100));
  }, []);

  /* ---- Poll real metrics every 2 seconds ---- */
  useEffect(() => {
    let mounted = true;

    async function poll() {
      try {
        const raw = await getLiveSystemMetrics();
        if (!mounted) return;
        const data: LiveMetrics = JSON.parse(raw);
        setLatest(data);
        setError(null);

        const now = Date.now();
        const ramPct = data.total_ram > 0 ? (data.used_ram / data.total_ram) * 100 : 0;
        const diskPct = data.disk_total > 0 ? ((data.disk_total - data.disk_available) / data.disk_total) * 100 : 0;

        setHistory((prev) => [
          ...prev.slice(-(MAX_DATA_POINTS - 1)),
          { ts: now, cpu: data.cpu_avg, ram: Math.round(ramPct * 10) / 10, disk: Math.round(diskPct * 10) / 10 },
        ]);

        // Build fuel history point from agent data
        if (data.agents.length > 0) {
          const timeLabel = new Date(now).toLocaleTimeString("en-US", { minute: "2-digit", second: "2-digit" });
          const point: FuelHistoryPoint = { ts: timeLabel };
          let total = 0;
          for (const agent of data.agents) {
            point[agent.name] = agent.fuel_used;
            total += agent.fuel_used;
          }
          point["total"] = total;
          setFuelHistory((prev) => [...prev.slice(-(MAX_DATA_POINTS - 1)), point]);
        }

        // Generate alerts from real data
        if (data.cpu_avg > 90) {
          setAlerts((prev) => {
            if (prev.some((a) => !a.dismissed && a.message.startsWith("CPU usage critical"))) return prev;
            const entry: AlertEntry = { id: `cpu-${now}`, ts: now, severity: "critical", agent: "System", message: `CPU usage critical: ${data.cpu_avg.toFixed(1)}%`, dismissed: false };
            return [entry, ...prev].slice(0, 50);
          });
        }
        if (ramPct > 85) {
          setAlerts((prev) => {
            if (prev.some((a) => !a.dismissed && a.message.startsWith("RAM usage high"))) return prev;
            const entry: AlertEntry = { id: `ram-${now}`, ts: now, severity: "warning", agent: "System", message: `RAM usage high: ${ramPct.toFixed(1)}%`, dismissed: false };
            return [entry, ...prev].slice(0, 50);
          });
        }
        // Check for agents exceeding 80% fuel budget
        for (const agent of data.agents) {
          if (agent.fuel_budget > 0 && agent.fuel_used / agent.fuel_budget > 0.8) {
            setAlerts((prev) => {
              if (prev.some((a) => !a.dismissed && a.message.includes(agent.name) && a.message.includes("fuel"))) return prev;
              const entry: AlertEntry = { id: `fuel-${agent.id}-${now}`, ts: now, severity: "warning", agent: agent.name, message: `${agent.name} fuel consumption at ${Math.round((agent.fuel_used / agent.fuel_budget) * 100)}% of budget`, dismissed: false };
              return [entry, ...prev].slice(0, 50);
            });
          }
        }
      } catch (err) {
        if (!mounted) return;
        setError(err instanceof Error ? err.message : String(err));
      }
    }

    // Initial poll
    poll();
    const interval = setInterval(poll, 2000);
    return () => {
      mounted = false;
      clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    let mounted = true;

    async function refreshAgents() {
      try {
        const agents = await listAgents();
        if (mounted) {
          setAllAgents(agents);
        }
      } catch (err) {
        console.error("[SystemMonitor] failed to list agents", err);
      }
    }

    void refreshAgents();
    const interval = setInterval(() => {
      void refreshAgents();
    }, 10000);
    return () => {
      mounted = false;
      clearInterval(interval);
    };
  }, []);

  const activeAlerts = alerts.filter((a) => !a.dismissed);

  const chartMetrics = useMemo(() =>
    history.map((m) => ({
      ...m,
      time: new Date(m.ts).toLocaleTimeString("en-US", { minute: "2-digit", second: "2-digit" }),
    })),
  [history]);

  function dismissAlert(id: string): void {
    setAlerts((prev) => prev.map((a) => a.id === id ? { ...a, dismissed: true } : a));
    appendAudit("AlertDismiss", id);
  }

  const TABS: { id: TabView; label: string }[] = [
    { id: "overview", label: "Overview" },
    { id: "agents", label: "Agents" },
    { id: "fuel", label: "Fuel" },
    { id: "alerts", label: `Alerts (${activeAlerts.length})` },
  ];

  // Derived values
  const totalFuel = latest?.agents.reduce((a, b) => a + b.fuel_used, 0) ?? 0;
  const totalBudget = latest?.agents.reduce((a, b) => a + b.fuel_budget, 0) ?? 1;
  const ramPct = latest && latest.total_ram > 0 ? (latest.used_ram / latest.total_ram) * 100 : 0;
  const diskPct = latest && latest.disk_total > 0 ? ((latest.disk_total - latest.disk_available) / latest.disk_total) * 100 : 0;

  const liveAgentMap = new Map((latest?.agents ?? []).map((agent) => [agent.id, agent]));

  const mergedAgents = useMemo(() => {
    const normalized = new Map<string, AgentFuel>();

    for (const agent of allAgents) {
      const live = liveAgentMap.get(agent.id);
      normalized.set(agent.id, {
        id: agent.id,
        name: live?.name ?? agent.name,
        state: live?.state ?? (agent.status === "Running" ? "Running" : agent.status === "Paused" ? "Idle" : "Stopped"),
        fuel_budget: live?.fuel_budget ?? agent.fuel_budget ?? 0,
        fuel_used: live?.fuel_used ?? ((agent.fuel_budget ?? 0) - agent.fuel_remaining),
        remaining_fuel: live?.remaining_fuel ?? agent.fuel_remaining,
      });
    }

    for (const live of latest?.agents ?? []) {
      if (!normalized.has(live.id)) {
        normalized.set(live.id, live);
      }
    }

    return Array.from(normalized.values()).sort((a, b) => a.name.localeCompare(b.name));
  }, [allAgents, latest?.agents]);

  const totalAgentCount = mergedAgents.length;
  const runningAgentCount = mergedAgents.filter((a) => a.state === "Running").length;
  const agentPieData = mergedAgents.filter((a) => a.fuel_used > 0).map((a) => ({ name: a.name, value: a.fuel_used }));
  const agentNames = mergedAgents.map((a) => a.name);

  /* ================================================================ */
  /*  RENDER                                                           */
  /* ================================================================ */
  return (
    <section className="sm-root">
      {/* ---- Header ---- */}
      <header className="sm-header">
        <div className="sm-header-left">
          <h2 className="sm-title">SYSTEM MONITOR</h2>
          <span className="sm-subtitle">real-time system metrics{latest ? ` | ${latest.cpu_name}` : ""}</span>
        </div>
        <div className="sm-header-right">
          {error && <span style={{ color: "#ef4444", fontSize: 11, marginRight: 12 }}>Backend: {error}</span>}
          <div className="sm-live-stats">
            <div className="sm-live-stat">
              <span className="sm-live-label">CPU</span>
              <span className="sm-live-value" style={{ color: (latest?.cpu_avg ?? 0) > 80 ? "#ef4444" : (latest?.cpu_avg ?? 0) > 60 ? "#f59e0b" : "var(--nexus-accent)" }}>{latest?.cpu_avg.toFixed(1) ?? "—"}%</span>
            </div>
            <div className="sm-live-stat">
              <span className="sm-live-label">RAM</span>
              <span className="sm-live-value" style={{ color: ramPct > 80 ? "#ef4444" : ramPct > 60 ? "#f59e0b" : "#c084fc" }}>{ramPct.toFixed(1)}%</span>
            </div>
            <div className="sm-live-stat">
              <span className="sm-live-label">DISK</span>
              <span className="sm-live-value" style={{ color: "#34d399" }}>{diskPct.toFixed(1)}%</span>
            </div>
            <div className="sm-live-stat">
              <span className="sm-live-label">FUEL</span>
              <span className="sm-live-value" style={{ color: totalFuel / totalBudget > 0.8 ? "#ef4444" : "var(--nexus-accent)" }}>{totalBudget > 0 ? Math.round((1 - totalFuel / totalBudget) * 100) : 100}%</span>
            </div>
          </div>
          {activeAlerts.length > 0 && (
            <div className="sm-alert-badge" onClick={() => setTab("alerts")}>
              <span className="sm-alert-dot" />
              {activeAlerts.length}
            </div>
          )}
        </div>
      </header>

      {/* ---- Tabs ---- */}
      <div className="sm-tabs">
        {TABS.map((t) => (
          <button key={t.id} type="button" className={`sm-tab ${tab === t.id ? "sm-tab-active" : ""}`} onClick={() => setTab(t.id)}>
            {t.label}
          </button>
        ))}
      </div>

      {/* ---- Content ---- */}
      <div className="sm-content">

        {/* ======== OVERVIEW ======== */}
        {tab === "overview" && (
          <div className="sm-overview">
            {/* CPU & RAM chart */}
            <div className="sm-chart-card sm-chart-wide">
              <div className="sm-chart-header"><span>CPU & RAM USAGE</span><span className="sm-chart-live">LIVE</span></div>
              <ResponsiveContainer width="100%" height={180}>
                <AreaChart data={chartMetrics}>
                  <CartesianGrid strokeDasharray="3 3" stroke="rgba(56,189,248,0.08)" />
                  <XAxis dataKey="time" tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} />
                  <YAxis domain={[0, 100]} tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} unit="%" />
                  <Tooltip contentStyle={{ background: "#0f172a", border: "1px solid rgba(56,189,248,0.2)", borderRadius: 6, fontSize: 12 }} />
                  <Area type="monotone" dataKey="cpu" stroke={CHART_COLORS.cpu} fill={CHART_COLORS.cpu} fillOpacity={0.15} strokeWidth={2} name="CPU" />
                  <Area type="monotone" dataKey="ram" stroke={CHART_COLORS.ram} fill={CHART_COLORS.ram} fillOpacity={0.1} strokeWidth={2} name="RAM" />
                </AreaChart>
              </ResponsiveContainer>
            </div>

            {/* Disk chart */}
            <div className="sm-chart-card sm-chart-wide">
              <div className="sm-chart-header"><span>DISK USAGE</span></div>
              <ResponsiveContainer width="100%" height={140}>
                <LineChart data={chartMetrics}>
                  <CartesianGrid strokeDasharray="3 3" stroke="rgba(56,189,248,0.08)" />
                  <XAxis dataKey="time" tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} />
                  <YAxis domain={[0, 100]} tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} unit="%" />
                  <Tooltip contentStyle={{ background: "#0f172a", border: "1px solid rgba(56,189,248,0.2)", borderRadius: 6, fontSize: 12 }} />
                  <Line type="monotone" dataKey="disk" stroke={CHART_COLORS.disk} strokeWidth={2} dot={false} name="Disk" />
                </LineChart>
              </ResponsiveContainer>
            </div>

            {/* Summary cards */}
            <div className="sm-summary-row">
              <div className="sm-summary-card">
                <span className="sm-summary-label">CPU Cores</span>
                <span className="sm-summary-value">{latest?.cpu_cores ?? "—"}</span>
              </div>
              <div className="sm-summary-card">
                <span className="sm-summary-label">RAM</span>
                <span className="sm-summary-value">{latest ? `${formatBytes(latest.used_ram)} / ${formatBytes(latest.total_ram)}` : "—"}</span>
              </div>
              <div className="sm-summary-card">
                <span className="sm-summary-label">Uptime</span>
                <span className="sm-summary-value">{latest ? formatUptime(latest.uptime_secs) : "—"}</span>
              </div>
              <div className="sm-summary-card">
                <span className="sm-summary-label">Processes</span>
                <span className="sm-summary-value">{latest?.process_count ?? "—"}</span>
              </div>
              <div className="sm-summary-card">
                <span className="sm-summary-label">Nexus Data</span>
                <span className="sm-summary-value">{latest ? formatBytes(latest.nexus_disk_bytes) : "—"}</span>
              </div>
              <div className="sm-summary-card">
                <span className="sm-summary-label">Agents Active</span>
                <span className="sm-summary-value">{totalAgentCount > 0 ? `${runningAgentCount} / ${totalAgentCount}` : "—"}</span>
              </div>
              <div className="sm-summary-card">
                <span className="sm-summary-label">Total Fuel Used</span>
                <span className="sm-summary-value">{totalFuel.toLocaleString()} / {totalBudget.toLocaleString()}</span>
              </div>
              <div className="sm-summary-card">
                <span className="sm-summary-label">Alerts</span>
                <span className="sm-summary-value" style={{ color: activeAlerts.some((a) => a.severity === "critical") ? "#ef4444" : "var(--nexus-accent)" }}>{activeAlerts.length} active</span>
              </div>
            </div>
          </div>
        )}

        {/* ======== AGENTS ======== */}
        {tab === "agents" && (
          <div className="sm-agents">
            <div className="sm-agents-grid">
              {mergedAgents.map((a) => {
                const fuelPct = a.fuel_budget > 0 ? Math.round((a.fuel_used / a.fuel_budget) * 100) : 0;
                return (
                  <div key={a.id} className={`sm-agent-card sm-agent-${a.state === "Running" ? "running" : a.state === "Idle" ? "idle" : "stopped"}`}>
                    <div className="sm-agent-top">
                      <span className="sm-agent-name">{a.name}</span>
                      <span className={`sm-agent-status sm-status-${a.state === "Running" ? "running" : a.state === "Idle" ? "idle" : "stopped"}`}>{a.state}</span>
                    </div>
                    <div className="sm-agent-metrics">
                      <div className="sm-agent-metric">
                        <span className="sm-ametric-label">Fuel Used</span>
                        <span className="sm-ametric-value">{a.fuel_used.toLocaleString()}</span>
                        <div className="sm-ametric-bar"><div className="sm-ametric-fill" style={{ width: `${fuelPct}%`, background: fuelPct > 80 ? "#ef4444" : fuelPct > 50 ? "#f59e0b" : "#34d399" }} /></div>
                      </div>
                      <div className="sm-agent-metric">
                        <span className="sm-ametric-label">Budget</span>
                        <span className="sm-ametric-value">{a.fuel_budget.toLocaleString()}</span>
                      </div>
                      <div className="sm-agent-metric">
                        <span className="sm-ametric-label">Remaining</span>
                        <span className="sm-ametric-value">{a.remaining_fuel.toLocaleString()}</span>
                      </div>
                    </div>
                    <div className="sm-agent-details">
                      <span className="sm-agent-detail">Fuel: {fuelPct}%</span>
                    </div>
                  </div>
                );
              })}
              {mergedAgents.length === 0 && (
                <div style={{ color: "rgba(165,243,252,0.5)", padding: 24 }}>
                  No agents available yet. This page shows live system usage and the runtime state of every governed agent.
                </div>
              )}
            </div>

            {/* Agent resource pies */}
            {agentPieData.length > 0 && (
              <div className="sm-pie-row">
                <div className="sm-chart-card">
                  <div className="sm-chart-header"><span>FUEL DISTRIBUTION</span></div>
                  <ResponsiveContainer width="100%" height={200}>
                    <PieChart>
                      <Pie data={agentPieData} dataKey="value" nameKey="name" cx="50%" cy="50%" outerRadius={70} label={({ name, percent }: { name?: string; percent?: number }) => `${name ?? ""} ${((percent ?? 0) * 100).toFixed(0)}%`} labelLine={false} fontSize={10}>
                        {agentPieData.map((_, i) => <Cell key={i} fill={PIE_COLORS[i % PIE_COLORS.length]} />)}
                      </Pie>
                      <Tooltip contentStyle={{ background: "#0f172a", border: "1px solid rgba(56,189,248,0.2)", borderRadius: 6, fontSize: 12 }} />
                    </PieChart>
                  </ResponsiveContainer>
                </div>
              </div>
            )}
          </div>
        )}

        {/* ======== FUEL ======== */}
        {tab === "fuel" && (
          <div className="sm-fuel">
            <div className="sm-chart-card sm-chart-wide">
              <div className="sm-chart-header"><span>FUEL CONSUMPTION OVER TIME</span><span className="sm-chart-live">LIVE</span></div>
              <ResponsiveContainer width="100%" height={220}>
                <AreaChart data={fuelHistory}>
                  <CartesianGrid strokeDasharray="3 3" stroke="rgba(56,189,248,0.08)" />
                  <XAxis dataKey="ts" tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} />
                  <YAxis tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} />
                  <Tooltip contentStyle={{ background: "#0f172a", border: "1px solid rgba(56,189,248,0.2)", borderRadius: 6, fontSize: 12 }} />
                  <Legend wrapperStyle={{ fontSize: 11 }} />
                  {agentNames.map((name, i) => (
                    <Area key={name} type="monotone" dataKey={name} stackId="1" stroke={PIE_COLORS[i % PIE_COLORS.length]} fill={PIE_COLORS[i % PIE_COLORS.length]} fillOpacity={0.3} name={name} />
                  ))}
                </AreaChart>
              </ResponsiveContainer>
            </div>

            <div className="sm-chart-card sm-chart-wide">
              <div className="sm-chart-header"><span>FUEL BUDGET USAGE PER AGENT</span></div>
              <ResponsiveContainer width="100%" height={180}>
                <BarChart data={mergedAgents.filter((a) => a.fuel_used > 0)} layout="vertical">
                  <CartesianGrid strokeDasharray="3 3" stroke="rgba(56,189,248,0.08)" />
                  <XAxis type="number" tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} />
                  <YAxis type="category" dataKey="name" tick={{ fill: "rgba(165,243,252,0.5)", fontSize: 11 }} tickLine={false} width={90} />
                  <Tooltip contentStyle={{ background: "#0f172a", border: "1px solid rgba(56,189,248,0.2)", borderRadius: 6, fontSize: 12 }} />
                  <Bar dataKey="fuel_used" name="Used" radius={[0, 4, 4, 0]}>
                    {mergedAgents.filter((a) => a.fuel_used > 0).map((a, i) => (
                      <Cell key={a.id} fill={PIE_COLORS[i % PIE_COLORS.length]} fillOpacity={0.7} />
                    ))}
                  </Bar>
                </BarChart>
              </ResponsiveContainer>
            </div>

            <div className="sm-fuel-totals">
              {mergedAgents.map((a, i) => {
                const pct = a.fuel_budget > 0 ? Math.round((a.fuel_used / a.fuel_budget) * 100) : 0;
                return (
                  <div key={a.id} className="sm-fuel-agent">
                    <div className="sm-fuel-agent-top">
                      <span className="sm-fuel-agent-name">{a.name}</span>
                      <span className="sm-fuel-agent-pct" style={{ color: pct > 80 ? "#ef4444" : pct > 50 ? "#f59e0b" : "#34d399" }}>{pct}%</span>
                    </div>
                    <div className="sm-fuel-agent-bar"><div className="sm-fuel-agent-fill" style={{ width: `${pct}%`, background: pct > 80 ? "#ef4444" : pct > 50 ? "#f59e0b" : "var(--nexus-accent)" }} /></div>
                    <span className="sm-fuel-agent-detail">{a.fuel_used.toLocaleString()} / {a.fuel_budget.toLocaleString()}</span>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* ======== ALERTS ======== */}
        {tab === "alerts" && (
          <div className="sm-alerts">
            <div className="sm-alerts-header">
              <span>GOVERNANCE ALERTS</span>
              <span className="sm-alerts-count">{activeAlerts.length} active / {alerts.length} total</span>
            </div>
            <div className="sm-alerts-list">
              {alerts.length === 0 && (
                <div style={{ color: "rgba(165,243,252,0.5)", padding: 24 }}>No alerts — system nominal</div>
              )}
              {alerts.map((a) => (
                <div key={a.id} className={`sm-alert-item sm-alert-${a.severity} ${a.dismissed ? "sm-alert-dismissed" : ""}`}>
                  <div className="sm-alert-left">
                    <span className={`sm-alert-severity sm-sev-${a.severity}`}>
                      {a.severity === "critical" ? "!!" : a.severity === "warning" ? "!" : "i"}
                    </span>
                    <div className="sm-alert-body">
                      <div className="sm-alert-msg">{a.message}</div>
                      <div className="sm-alert-meta">
                        <span className="sm-alert-agent">{a.agent}</span>
                        <span className="sm-alert-time">{formatTime(a.ts)}</span>
                      </div>
                    </div>
                  </div>
                  {!a.dismissed && (
                    <button type="button" className="sm-alert-dismiss" onClick={() => dismissAlert(a.id)}>Dismiss</button>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

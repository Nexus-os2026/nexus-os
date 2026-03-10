import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Area, AreaChart, Bar, BarChart, CartesianGrid, Cell,
  Legend, Line, LineChart, Pie, PieChart,
  ResponsiveContainer, Tooltip, XAxis, YAxis,
} from "recharts";
import "./system-monitor.css";

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface SystemMetrics {
  ts: number;
  cpu: number;
  ram: number;
  gpu: number;
  disk: number;
  netIn: number;
  netOut: number;
}

interface AgentResource {
  id: string;
  name: string;
  status: "running" | "idle" | "stopped" | "error";
  cpu: number;
  ram: number;
  fuelUsed: number;
  fuelBudget: number;
  netRequests: number;
  uptime: number;
  lastAction: string;
}

interface ProcessEntry {
  pid: number;
  name: string;
  agent: string | null;
  cpu: number;
  ram: number;
  status: "running" | "sleeping" | "zombie";
  started: number;
}

interface NetworkConn {
  id: string;
  agent: string;
  target: string;
  protocol: string;
  bytesIn: number;
  bytesOut: number;
  status: "active" | "idle" | "blocked";
  latency: number;
}

interface FuelHistoryPoint {
  ts: string;
  coder: number;
  designer: number;
  researcher: number;
  selfImprove: number;
  total: number;
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

type TabView = "overview" | "agents" | "processes" | "network" | "fuel" | "alerts";

/* ================================================================== */
/*  Helpers                                                            */
/* ================================================================== */

function formatUptime(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
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

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, v));
}

/* ================================================================== */
/*  Mock data generators                                               */
/* ================================================================== */

function generateMetricsHistory(count: number): SystemMetrics[] {
  const data: SystemMetrics[] = [];
  const now = Date.now();
  let cpu = 35, ram = 42, gpu = 18, disk = 22, netIn = 150, netOut = 80;
  for (let i = count - 1; i >= 0; i--) {
    cpu = clamp(cpu + (Math.random() - 0.45) * 8, 5, 95);
    ram = clamp(ram + (Math.random() - 0.48) * 3, 20, 85);
    gpu = clamp(gpu + (Math.random() - 0.5) * 6, 0, 80);
    disk = clamp(disk + (Math.random() - 0.5) * 2, 10, 60);
    netIn = clamp(netIn + (Math.random() - 0.5) * 40, 10, 500);
    netOut = clamp(netOut + (Math.random() - 0.5) * 25, 5, 300);
    data.push({
      ts: now - i * 2000,
      cpu: Math.round(cpu * 10) / 10,
      ram: Math.round(ram * 10) / 10,
      gpu: Math.round(gpu * 10) / 10,
      disk: Math.round(disk * 10) / 10,
      netIn: Math.round(netIn),
      netOut: Math.round(netOut),
    });
  }
  return data;
}

const INITIAL_AGENTS: AgentResource[] = [
  { id: "a1", name: "Coder", status: "running", cpu: 12.3, ram: 245, fuelUsed: 2800, fuelBudget: 10000, netRequests: 47, uptime: 14520, lastAction: "Refactoring boot sequence" },
  { id: "a2", name: "Designer", status: "idle", cpu: 0.8, ram: 128, fuelUsed: 900, fuelBudget: 10000, netRequests: 12, uptime: 14520, lastAction: "Waiting for input" },
  { id: "a3", name: "Researcher", status: "running", cpu: 8.7, ram: 312, fuelUsed: 3500, fuelBudget: 10000, netRequests: 156, uptime: 12340, lastAction: "Web search: Rust async patterns" },
  { id: "a4", name: "Self-Improve", status: "running", cpu: 5.2, ram: 189, fuelUsed: 4200, fuelBudget: 10000, netRequests: 83, uptime: 14520, lastAction: "Optimizing prompt templates" },
  { id: "a5", name: "Reviewer", status: "stopped", cpu: 0, ram: 0, fuelUsed: 0, fuelBudget: 10000, netRequests: 0, uptime: 0, lastAction: "—" },
];

const INITIAL_PROCESSES: ProcessEntry[] = [
  { pid: 1, name: "nexus-kernel", agent: null, cpu: 2.1, ram: 156, status: "running", started: Date.now() - 14520000 },
  { pid: 42, name: "nexus-supervisor", agent: null, cpu: 3.4, ram: 234, status: "running", started: Date.now() - 14520000 },
  { pid: 101, name: "agent:coder", agent: "Coder", cpu: 12.3, ram: 245, status: "running", started: Date.now() - 14520000 },
  { pid: 102, name: "agent:designer", agent: "Designer", cpu: 0.8, ram: 128, status: "sleeping", started: Date.now() - 14520000 },
  { pid: 103, name: "agent:researcher", agent: "Researcher", cpu: 8.7, ram: 312, status: "running", started: Date.now() - 12340000 },
  { pid: 104, name: "agent:self-improve", agent: "Self-Improve", cpu: 5.2, ram: 189, status: "running", started: Date.now() - 14520000 },
  { pid: 201, name: "nexus-api-server", agent: null, cpu: 1.5, ram: 98, status: "running", started: Date.now() - 14520000 },
  { pid: 202, name: "nexus-audit-writer", agent: null, cpu: 0.3, ram: 45, status: "running", started: Date.now() - 14520000 },
  { pid: 301, name: "ollama-server", agent: null, cpu: 15.6, ram: 2048, status: "running", started: Date.now() - 14520000 },
  { pid: 302, name: "nexus-slm-runner", agent: null, cpu: 4.8, ram: 512, status: "running", started: Date.now() - 10000000 },
];

const INITIAL_CONNECTIONS: NetworkConn[] = [
  { id: "n1", agent: "Researcher", target: "api.duckduckgo.com", protocol: "HTTPS", bytesIn: 245780, bytesOut: 12340, status: "active", latency: 42 },
  { id: "n2", agent: "Coder", target: "crates.io", protocol: "HTTPS", bytesIn: 89450, bytesOut: 3200, status: "idle", latency: 38 },
  { id: "n3", agent: "Self-Improve", target: "localhost:11434", protocol: "HTTP", bytesIn: 1245000, bytesOut: 45600, status: "active", latency: 2 },
  { id: "n4", agent: "Researcher", target: "docs.rs", protocol: "HTTPS", bytesIn: 567800, bytesOut: 8900, status: "active", latency: 55 },
  { id: "n5", agent: "Designer", target: "fonts.google.com", protocol: "HTTPS", bytesIn: 34500, bytesOut: 1200, status: "idle", latency: 67 },
  { id: "n6", agent: "Coder", target: "malware.bad.com", protocol: "HTTPS", bytesIn: 0, bytesOut: 0, status: "blocked", latency: 0 },
];

function generateFuelHistory(): FuelHistoryPoint[] {
  const data: FuelHistoryPoint[] = [];
  let c = 0, d = 0, r = 0, s = 0;
  for (let i = 0; i < 24; i++) {
    c += Math.floor(Math.random() * 180);
    d += Math.floor(Math.random() * 60);
    r += Math.floor(Math.random() * 200);
    s += Math.floor(Math.random() * 250);
    data.push({
      ts: `${String(i).padStart(2, "0")}:00`,
      coder: c, designer: d, researcher: r, selfImprove: s,
      total: c + d + r + s,
    });
  }
  return data;
}

const INITIAL_ALERTS: AlertEntry[] = [
  { id: "al1", ts: Date.now() - 300000, severity: "warning", agent: "Self-Improve", message: "Fuel consumption rate exceeds 80% of budget in 4h window", dismissed: false },
  { id: "al2", ts: Date.now() - 900000, severity: "critical", agent: "Coder", message: "Blocked network request to malware.bad.com", dismissed: false },
  { id: "al3", ts: Date.now() - 1800000, severity: "info", agent: "Researcher", message: "156 network requests in last hour (threshold: 200)", dismissed: false },
  { id: "al4", ts: Date.now() - 3600000, severity: "warning", agent: "Researcher", message: "RAM usage spike: 312MB (threshold: 256MB)", dismissed: true },
  { id: "al5", ts: Date.now() - 7200000, severity: "info", agent: "System", message: "Audit trail checkpoint: 1,247 events, chain verified", dismissed: true },
];

const CHART_COLORS = {
  cpu: "#22d3ee",
  ram: "#c084fc",
  gpu: "#f59e0b",
  disk: "#34d399",
  netIn: "#60a5fa",
  netOut: "#fb923c",
  coder: "#22d3ee",
  designer: "#c084fc",
  researcher: "#34d399",
  selfImprove: "#f59e0b",
};

const PIE_COLORS = ["#22d3ee", "#c084fc", "#34d399", "#f59e0b", "#60a5fa"];

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function SystemMonitor(): JSX.Element {
  const [tab, setTab] = useState<TabView>("overview");
  const [metrics, setMetrics] = useState<SystemMetrics[]>(() => generateMetricsHistory(60));
  const [agents, setAgents] = useState<AgentResource[]>(INITIAL_AGENTS);
  const [processes] = useState<ProcessEntry[]>(INITIAL_PROCESSES);
  const [connections, setConnections] = useState<NetworkConn[]>(INITIAL_CONNECTIONS);
  const [fuelHistory] = useState<FuelHistoryPoint[]>(() => generateFuelHistory());
  const [alerts, setAlerts] = useState<AlertEntry[]>(INITIAL_ALERTS);
  const [auditLog, setAuditLog] = useState<AuditEntry[]>([]);

  const appendAudit = useCallback((event: string, detail: string) => {
    setAuditLog((prev) => [{ ts: Date.now(), event, detail }, ...prev].slice(0, 100));
  }, []);

  /* ---- Live metric simulation ---- */
  useEffect(() => {
    const interval = setInterval(() => {
      setMetrics((prev) => {
        const last = prev[prev.length - 1];
        const next: SystemMetrics = {
          ts: Date.now(),
          cpu: clamp(last.cpu + (Math.random() - 0.45) * 8, 5, 95),
          ram: clamp(last.ram + (Math.random() - 0.48) * 3, 20, 85),
          gpu: clamp(last.gpu + (Math.random() - 0.5) * 6, 0, 80),
          disk: clamp(last.disk + (Math.random() - 0.5) * 2, 10, 60),
          netIn: Math.round(clamp(last.netIn + (Math.random() - 0.5) * 40, 10, 500)),
          netOut: Math.round(clamp(last.netOut + (Math.random() - 0.5) * 25, 5, 300)),
        };
        return [...prev.slice(-59), next];
      });

      // Jitter agent metrics
      setAgents((prev) =>
        prev.map((a) => {
          if (a.status !== "running") return a;
          return {
            ...a,
            cpu: Math.round(clamp(a.cpu + (Math.random() - 0.5) * 3, 0.1, 30) * 10) / 10,
            ram: Math.round(clamp(a.ram + (Math.random() - 0.5) * 20, 50, 500)),
            fuelUsed: Math.min(a.fuelUsed + Math.floor(Math.random() * 8), a.fuelBudget),
            netRequests: a.netRequests + (Math.random() > 0.7 ? 1 : 0),
          };
        })
      );

      // Jitter network latency
      setConnections((prev) =>
        prev.map((c) => c.status === "active" ? {
          ...c,
          latency: Math.round(clamp(c.latency + (Math.random() - 0.5) * 10, 1, 200)),
          bytesIn: c.bytesIn + Math.floor(Math.random() * 5000),
          bytesOut: c.bytesOut + Math.floor(Math.random() * 1000),
        } : c)
      );
    }, 2000);
    return () => clearInterval(interval);
  }, []);

  const latest = metrics[metrics.length - 1];
  const activeAlerts = alerts.filter((a) => !a.dismissed);
  const totalFuel = agents.reduce((a, b) => a + b.fuelUsed, 0);
  const totalBudget = agents.reduce((a, b) => a + b.fuelBudget, 0);

  const agentPieData = agents.filter((a) => a.fuelUsed > 0).map((a) => ({ name: a.name, value: a.fuelUsed }));
  const ramPieData = agents.filter((a) => a.ram > 0).map((a) => ({ name: a.name, value: a.ram }));

  const chartMetrics = useMemo(() =>
    metrics.map((m) => ({
      ...m,
      time: new Date(m.ts).toLocaleTimeString("en-US", { minute: "2-digit", second: "2-digit" }),
    })),
  [metrics]);

  function dismissAlert(id: string): void {
    setAlerts((prev) => prev.map((a) => a.id === id ? { ...a, dismissed: true } : a));
    appendAudit("AlertDismiss", id);
  }

  const TABS: { id: TabView; label: string }[] = [
    { id: "overview", label: "Overview" },
    { id: "agents", label: "Agents" },
    { id: "processes", label: "Processes" },
    { id: "network", label: "Network" },
    { id: "fuel", label: "Fuel" },
    { id: "alerts", label: `Alerts (${activeAlerts.length})` },
  ];

  /* ================================================================ */
  /*  RENDER                                                           */
  /* ================================================================ */
  return (
    <section className="sm-root">
      {/* ---- Header ---- */}
      <header className="sm-header">
        <div className="sm-header-left">
          <h2 className="sm-title">SYSTEM MONITOR</h2>
          <span className="sm-subtitle">real-time governed metrics</span>
        </div>
        <div className="sm-header-right">
          <div className="sm-live-stats">
            <div className="sm-live-stat">
              <span className="sm-live-label">CPU</span>
              <span className="sm-live-value" style={{ color: latest.cpu > 80 ? "#ef4444" : latest.cpu > 60 ? "#f59e0b" : "#22d3ee" }}>{latest.cpu.toFixed(1)}%</span>
            </div>
            <div className="sm-live-stat">
              <span className="sm-live-label">RAM</span>
              <span className="sm-live-value" style={{ color: latest.ram > 80 ? "#ef4444" : latest.ram > 60 ? "#f59e0b" : "#c084fc" }}>{latest.ram.toFixed(1)}%</span>
            </div>
            <div className="sm-live-stat">
              <span className="sm-live-label">GPU</span>
              <span className="sm-live-value" style={{ color: "#f59e0b" }}>{latest.gpu.toFixed(1)}%</span>
            </div>
            <div className="sm-live-stat">
              <span className="sm-live-label">FUEL</span>
              <span className="sm-live-value" style={{ color: totalFuel / totalBudget > 0.8 ? "#ef4444" : "#22d3ee" }}>{Math.round((1 - totalFuel / totalBudget) * 100)}%</span>
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
            {/* CPU + RAM chart */}
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

            {/* GPU + Disk chart */}
            <div className="sm-chart-card sm-chart-wide">
              <div className="sm-chart-header"><span>GPU & DISK</span></div>
              <ResponsiveContainer width="100%" height={140}>
                <LineChart data={chartMetrics}>
                  <CartesianGrid strokeDasharray="3 3" stroke="rgba(56,189,248,0.08)" />
                  <XAxis dataKey="time" tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} />
                  <YAxis domain={[0, 100]} tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} unit="%" />
                  <Tooltip contentStyle={{ background: "#0f172a", border: "1px solid rgba(56,189,248,0.2)", borderRadius: 6, fontSize: 12 }} />
                  <Line type="monotone" dataKey="gpu" stroke={CHART_COLORS.gpu} strokeWidth={2} dot={false} name="GPU" />
                  <Line type="monotone" dataKey="disk" stroke={CHART_COLORS.disk} strokeWidth={2} dot={false} name="Disk" />
                </LineChart>
              </ResponsiveContainer>
            </div>

            {/* Network chart */}
            <div className="sm-chart-card sm-chart-wide">
              <div className="sm-chart-header"><span>NETWORK I/O (KB/s)</span></div>
              <ResponsiveContainer width="100%" height={140}>
                <AreaChart data={chartMetrics}>
                  <CartesianGrid strokeDasharray="3 3" stroke="rgba(56,189,248,0.08)" />
                  <XAxis dataKey="time" tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} />
                  <YAxis tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} />
                  <Tooltip contentStyle={{ background: "#0f172a", border: "1px solid rgba(56,189,248,0.2)", borderRadius: 6, fontSize: 12 }} />
                  <Area type="monotone" dataKey="netIn" stroke={CHART_COLORS.netIn} fill={CHART_COLORS.netIn} fillOpacity={0.12} strokeWidth={2} name="In" />
                  <Area type="monotone" dataKey="netOut" stroke={CHART_COLORS.netOut} fill={CHART_COLORS.netOut} fillOpacity={0.1} strokeWidth={2} name="Out" />
                </AreaChart>
              </ResponsiveContainer>
            </div>

            {/* Summary cards */}
            <div className="sm-summary-row">
              <div className="sm-summary-card">
                <span className="sm-summary-label">Agents Active</span>
                <span className="sm-summary-value">{agents.filter((a) => a.status === "running").length} / {agents.length}</span>
              </div>
              <div className="sm-summary-card">
                <span className="sm-summary-label">Total Fuel Used</span>
                <span className="sm-summary-value">{totalFuel.toLocaleString()} / {totalBudget.toLocaleString()}</span>
              </div>
              <div className="sm-summary-card">
                <span className="sm-summary-label">Network Connections</span>
                <span className="sm-summary-value">{connections.filter((c) => c.status === "active").length} active</span>
              </div>
              <div className="sm-summary-card">
                <span className="sm-summary-label">Alerts</span>
                <span className="sm-summary-value" style={{ color: activeAlerts.some((a) => a.severity === "critical") ? "#ef4444" : "#22d3ee" }}>{activeAlerts.length} active</span>
              </div>
            </div>
          </div>
        )}

        {/* ======== AGENTS ======== */}
        {tab === "agents" && (
          <div className="sm-agents">
            <div className="sm-agents-grid">
              {agents.map((a) => {
                const fuelPct = Math.round((a.fuelUsed / a.fuelBudget) * 100);
                return (
                  <div key={a.id} className={`sm-agent-card sm-agent-${a.status}`}>
                    <div className="sm-agent-top">
                      <span className="sm-agent-name">{a.name}</span>
                      <span className={`sm-agent-status sm-status-${a.status}`}>{a.status}</span>
                    </div>
                    <div className="sm-agent-metrics">
                      <div className="sm-agent-metric">
                        <span className="sm-ametric-label">CPU</span>
                        <span className="sm-ametric-value">{a.cpu.toFixed(1)}%</span>
                        <div className="sm-ametric-bar"><div className="sm-ametric-fill" style={{ width: `${Math.min(a.cpu * 3, 100)}%`, background: CHART_COLORS.cpu }} /></div>
                      </div>
                      <div className="sm-agent-metric">
                        <span className="sm-ametric-label">RAM</span>
                        <span className="sm-ametric-value">{a.ram} MB</span>
                        <div className="sm-ametric-bar"><div className="sm-ametric-fill" style={{ width: `${Math.min(a.ram / 5, 100)}%`, background: CHART_COLORS.ram }} /></div>
                      </div>
                      <div className="sm-agent-metric">
                        <span className="sm-ametric-label">Fuel</span>
                        <span className="sm-ametric-value">{fuelPct}%</span>
                        <div className="sm-ametric-bar"><div className="sm-ametric-fill" style={{ width: `${fuelPct}%`, background: fuelPct > 80 ? "#ef4444" : fuelPct > 50 ? "#f59e0b" : "#34d399" }} /></div>
                      </div>
                    </div>
                    <div className="sm-agent-details">
                      <span className="sm-agent-detail">Net: {a.netRequests} reqs</span>
                      <span className="sm-agent-detail">Up: {formatUptime(a.uptime)}</span>
                    </div>
                    <div className="sm-agent-action">{a.lastAction}</div>
                  </div>
                );
              })}
            </div>

            {/* Agent resource pies */}
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
              <div className="sm-chart-card">
                <div className="sm-chart-header"><span>RAM DISTRIBUTION</span></div>
                <ResponsiveContainer width="100%" height={200}>
                  <PieChart>
                    <Pie data={ramPieData} dataKey="value" nameKey="name" cx="50%" cy="50%" outerRadius={70} label={({ name, value }) => `${name} ${value}MB`} labelLine={false} fontSize={10}>
                      {ramPieData.map((_, i) => <Cell key={i} fill={PIE_COLORS[i % PIE_COLORS.length]} />)}
                    </Pie>
                    <Tooltip contentStyle={{ background: "#0f172a", border: "1px solid rgba(56,189,248,0.2)", borderRadius: 6, fontSize: 12 }} />
                  </PieChart>
                </ResponsiveContainer>
              </div>
            </div>
          </div>
        )}

        {/* ======== PROCESSES ======== */}
        {tab === "processes" && (
          <div className="sm-processes">
            <div className="sm-proc-header">
              <span className="sm-proc-col sm-proc-pid">PID</span>
              <span className="sm-proc-col sm-proc-name">Process</span>
              <span className="sm-proc-col sm-proc-agent">Agent</span>
              <span className="sm-proc-col sm-proc-cpu">CPU %</span>
              <span className="sm-proc-col sm-proc-ram">RAM MB</span>
              <span className="sm-proc-col sm-proc-status">Status</span>
            </div>
            <div className="sm-proc-list">
              {[...processes].sort((a, b) => b.cpu - a.cpu).map((p) => (
                <div key={p.pid} className={`sm-proc-row ${p.agent ? "sm-proc-agent-row" : ""}`}>
                  <span className="sm-proc-col sm-proc-pid">{p.pid}</span>
                  <span className="sm-proc-col sm-proc-name">{p.name}</span>
                  <span className="sm-proc-col sm-proc-agent">{p.agent ?? "—"}</span>
                  <span className="sm-proc-col sm-proc-cpu" style={{ color: p.cpu > 10 ? "#f59e0b" : p.cpu > 5 ? "#22d3ee" : "inherit" }}>
                    {p.cpu.toFixed(1)}
                  </span>
                  <span className="sm-proc-col sm-proc-ram">{p.ram}</span>
                  <span className={`sm-proc-col sm-proc-status sm-proc-st-${p.status}`}>{p.status}</span>
                </div>
              ))}
            </div>
            <div className="sm-proc-summary">
              <span>{processes.length} processes</span>
              <span>Total CPU: {processes.reduce((a, b) => a + b.cpu, 0).toFixed(1)}%</span>
              <span>Total RAM: {processes.reduce((a, b) => a + b.ram, 0).toLocaleString()} MB</span>
              <span>Agent processes: {processes.filter((p) => p.agent).length}</span>
            </div>
          </div>
        )}

        {/* ======== NETWORK ======== */}
        {tab === "network" && (
          <div className="sm-network">
            <div className="sm-net-header">
              <span className="sm-net-col sm-net-agent">Agent</span>
              <span className="sm-net-col sm-net-target">Target</span>
              <span className="sm-net-col sm-net-proto">Protocol</span>
              <span className="sm-net-col sm-net-in">In</span>
              <span className="sm-net-col sm-net-out">Out</span>
              <span className="sm-net-col sm-net-latency">Latency</span>
              <span className="sm-net-col sm-net-status">Status</span>
            </div>
            <div className="sm-net-list">
              {connections.map((c) => (
                <div key={c.id} className={`sm-net-row sm-net-st-${c.status}`}>
                  <span className="sm-net-col sm-net-agent">{c.agent}</span>
                  <span className="sm-net-col sm-net-target">{c.target}</span>
                  <span className="sm-net-col sm-net-proto">{c.protocol}</span>
                  <span className="sm-net-col sm-net-in">{formatBytes(c.bytesIn)}</span>
                  <span className="sm-net-col sm-net-out">{formatBytes(c.bytesOut)}</span>
                  <span className="sm-net-col sm-net-latency" style={{ color: c.latency > 100 ? "#ef4444" : c.latency > 50 ? "#f59e0b" : "#34d399" }}>
                    {c.status === "blocked" ? "—" : `${c.latency}ms`}
                  </span>
                  <span className={`sm-net-col sm-net-status sm-conn-${c.status}`}>{c.status}</span>
                </div>
              ))}
            </div>
            <div className="sm-net-summary">
              <span>{connections.filter((c) => c.status === "active").length} active</span>
              <span>{connections.filter((c) => c.status === "blocked").length} blocked</span>
              <span>Total In: {formatBytes(connections.reduce((a, b) => a + b.bytesIn, 0))}</span>
              <span>Total Out: {formatBytes(connections.reduce((a, b) => a + b.bytesOut, 0))}</span>
            </div>
          </div>
        )}

        {/* ======== FUEL ======== */}
        {tab === "fuel" && (
          <div className="sm-fuel">
            <div className="sm-chart-card sm-chart-wide">
              <div className="sm-chart-header"><span>FUEL CONSUMPTION OVER TIME</span></div>
              <ResponsiveContainer width="100%" height={220}>
                <AreaChart data={fuelHistory}>
                  <CartesianGrid strokeDasharray="3 3" stroke="rgba(56,189,248,0.08)" />
                  <XAxis dataKey="ts" tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} />
                  <YAxis tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} />
                  <Tooltip contentStyle={{ background: "#0f172a", border: "1px solid rgba(56,189,248,0.2)", borderRadius: 6, fontSize: 12 }} />
                  <Legend wrapperStyle={{ fontSize: 11 }} />
                  <Area type="monotone" dataKey="coder" stackId="1" stroke={CHART_COLORS.coder} fill={CHART_COLORS.coder} fillOpacity={0.3} name="Coder" />
                  <Area type="monotone" dataKey="designer" stackId="1" stroke={CHART_COLORS.designer} fill={CHART_COLORS.designer} fillOpacity={0.3} name="Designer" />
                  <Area type="monotone" dataKey="researcher" stackId="1" stroke={CHART_COLORS.researcher} fill={CHART_COLORS.researcher} fillOpacity={0.3} name="Researcher" />
                  <Area type="monotone" dataKey="selfImprove" stackId="1" stroke={CHART_COLORS.selfImprove} fill={CHART_COLORS.selfImprove} fillOpacity={0.3} name="Self-Improve" />
                </AreaChart>
              </ResponsiveContainer>
            </div>

            <div className="sm-chart-card sm-chart-wide">
              <div className="sm-chart-header"><span>FUEL BUDGET USAGE PER AGENT</span></div>
              <ResponsiveContainer width="100%" height={180}>
                <BarChart data={agents.filter((a) => a.fuelUsed > 0)} layout="vertical">
                  <CartesianGrid strokeDasharray="3 3" stroke="rgba(56,189,248,0.08)" />
                  <XAxis type="number" domain={[0, 10000]} tick={{ fill: "rgba(165,243,252,0.3)", fontSize: 10 }} tickLine={false} />
                  <YAxis type="category" dataKey="name" tick={{ fill: "rgba(165,243,252,0.5)", fontSize: 11 }} tickLine={false} width={90} />
                  <Tooltip contentStyle={{ background: "#0f172a", border: "1px solid rgba(56,189,248,0.2)", borderRadius: 6, fontSize: 12 }} />
                  <Bar dataKey="fuelUsed" name="Used" radius={[0, 4, 4, 0]}>
                    {agents.filter((a) => a.fuelUsed > 0).map((a, i) => (
                      <Cell key={a.id} fill={PIE_COLORS[i % PIE_COLORS.length]} fillOpacity={0.7} />
                    ))}
                  </Bar>
                </BarChart>
              </ResponsiveContainer>
            </div>

            <div className="sm-fuel-totals">
              {agents.map((a) => {
                const pct = Math.round((a.fuelUsed / a.fuelBudget) * 100);
                return (
                  <div key={a.id} className="sm-fuel-agent">
                    <div className="sm-fuel-agent-top">
                      <span className="sm-fuel-agent-name">{a.name}</span>
                      <span className="sm-fuel-agent-pct" style={{ color: pct > 80 ? "#ef4444" : pct > 50 ? "#f59e0b" : "#34d399" }}>{pct}%</span>
                    </div>
                    <div className="sm-fuel-agent-bar"><div className="sm-fuel-agent-fill" style={{ width: `${pct}%`, background: pct > 80 ? "#ef4444" : pct > 50 ? "#f59e0b" : "#22d3ee" }} /></div>
                    <span className="sm-fuel-agent-detail">{a.fuelUsed.toLocaleString()} / {a.fuelBudget.toLocaleString()}</span>
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

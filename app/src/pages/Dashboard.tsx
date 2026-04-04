import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getAuditLog, getLiveSystemMetricsJson, getSystemInfo, listAgents } from "../api/backend";
import type { AgentSummary, AuditEventRow, SystemInfo } from "../types";

type SystemMetricsAgent = {
  id?: string;
  name?: string;
  fuel_budget?: number;
  fuel_used?: number;
  remaining_fuel?: number;
  state?: string;
};

type SystemMetrics = {
  cpu_name?: string;
  cpu_avg?: number;
  cpu_cores?: number;
  used_ram?: number;
  total_ram?: number;
  uptime_secs?: number;
  process_count?: number;
  agents?: SystemMetricsAgent[];
};

function formatUptime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return `${hours}h ${minutes}m`;
}

function normalizeMetrics(input: unknown): SystemMetrics | null {
  if (!input || typeof input !== "object") {
    return null;
  }
  const candidate = input as SystemMetrics;
  return {
    ...candidate,
    agents: Array.isArray(candidate.agents) ? candidate.agents : [],
  };
}

function truncateId(id: string): string {
  return id.length > 16 ? id.slice(0, 16) + "…" : id;
}

export default function Dashboard(): JSX.Element {
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [auditEvents, setAuditEvents] = useState<AuditEventRow[]>([]);
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);
  const [sysInfo, setSysInfo] = useState<SystemInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const pollRef = useRef<number>(0);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [agentRows, auditRows, metricRows, sysInfoResult] = await Promise.all([
        listAgents(),
        getAuditLog(undefined, 8),
        getLiveSystemMetricsJson<SystemMetrics>(),
        getSystemInfo().catch(() => null),
      ]);
      setAgents(Array.isArray(agentRows) ? agentRows : []);
      setAuditEvents(Array.isArray(auditRows) ? auditRows : []);
      setMetrics(normalizeMetrics(metricRows));
      if (sysInfoResult) setSysInfo(sysInfoResult);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Poll system info every 3s to match topbar
  useEffect(() => {
    let active = true;
    function poll(): void {
      getSystemInfo()
        .then((info) => { if (active) setSysInfo(info); })
        .catch(() => {});
    }
    poll();
    pollRef.current = window.setInterval(poll, 3000);
    return () => { active = false; clearInterval(pollRef.current); };
  }, []);

  // Active agents: match topbar definition (status === "Running")
  const runningAgents = useMemo(
    () => agents.filter((a) => a.status === "Running").length,
    [agents],
  );

  const fuelSummary = useMemo(() => {
    const source = Array.isArray(metrics?.agents) ? metrics.agents : [];
    return source.reduce(
      (acc, agent) => {
        acc.budget += Number(agent.fuel_budget ?? 0);
        acc.used += Number(agent.fuel_used ?? 0);
        acc.remaining += Number(agent.remaining_fuel ?? 0);
        return acc;
      },
      { budget: 0, used: 0, remaining: 0 },
    );
  }, [metrics]);

  // Live CPU/RAM from getSystemInfo (same source as topbar)
  const cpuPercent = sysInfo?.cpu_usage_percent ?? metrics?.cpu_avg ?? 0;
  const ramUsedGb = sysInfo?.ram_used_gb ?? (metrics?.used_ram ? +(metrics.used_ram / 1024 / 1024 / 1024).toFixed(1) : 0);
  const ramTotalGb = sysInfo?.ram_total_gb ?? (metrics?.total_ram ? +(metrics.total_ram / 1024 / 1024 / 1024).toFixed(1) : 0);
  const cpuName = sysInfo?.cpu_name ?? metrics?.cpu_name ?? "Unknown CPU";

  return (
    <section className="mx-auto flex max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6">
      <header className="nexus-panel rounded-3xl p-6 shadow-sm">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="text-xs uppercase tracking-[0.24em] text-cyan-300/70">Home Overview</p>
            <h2 className="nexus-display mt-2 text-3xl text-cyan-50">Dashboard</h2>
            <p className="mt-2 max-w-2xl text-sm text-cyan-100/65">
              Live runtime status — agents, system resources, fuel accounting, and audit trail.
            </p>
          </div>
          <button
            type="button"
            onClick={() => void refresh()}
            className="rounded-full border border-cyan-400/40 bg-cyan-500/10 px-4 py-2 text-sm text-cyan-100"
          >
            Refresh
          </button>
        </div>
      </header>

      {error ? (
        <div className="rounded-2xl border border-rose-400/40 bg-rose-500/10 p-4 text-sm text-rose-100">
          {error}
        </div>
      ) : null}

      {/* ── KPI Cards ── */}
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <article className="nexus-panel rounded-2xl p-5">
          <p className="text-xs uppercase tracking-[0.2em] text-cyan-300/60">Total Agents</p>
          <p className="mt-3 text-3xl text-cyan-50">{agents.length}</p>
        </article>
        <article className="nexus-panel rounded-2xl p-5">
          <p className="text-xs uppercase tracking-[0.2em] text-cyan-300/60">Running Agents</p>
          <p className="mt-3 text-3xl text-cyan-50">{runningAgents}</p>
        </article>
        <article className="nexus-panel rounded-2xl p-5">
          <p className="text-xs uppercase tracking-[0.2em] text-cyan-300/60">Fuel Used</p>
          <p className="mt-3 text-3xl text-cyan-50">{fuelSummary.used.toLocaleString()}</p>
          <p className="mt-1 text-xs text-cyan-100/60">
            {fuelSummary.remaining.toLocaleString()} remaining of {fuelSummary.budget.toLocaleString()}
          </p>
        </article>
        <article className="nexus-panel rounded-2xl p-5">
          <p className="text-xs uppercase tracking-[0.2em] text-cyan-300/60">CPU / RAM</p>
          <p className="mt-3 text-2xl text-cyan-50">
            {Math.round(cpuPercent)}%
          </p>
          <p className="mt-1 text-xs text-cyan-100/60">
            {ramUsedGb} GB / {ramTotalGb} GB
          </p>
        </article>
      </div>

      {/* ── System + Agents + Audit ── */}
      <div className="grid gap-4 xl:grid-cols-[1.1fr_0.9fr]">
        {/* Left: System Metrics */}
        <section className="nexus-panel rounded-2xl p-5">
          <h3 className="text-lg text-cyan-50">System Metrics</h3>
          <p className="text-sm text-cyan-100/60">Live hardware telemetry — polls every 3s.</p>
          {loading && !metrics ? (
            <p className="mt-4 text-sm text-cyan-100/60">Loading metrics...</p>
          ) : (
            <div className="mt-5 grid gap-4 md:grid-cols-2">
              <div className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4">
                <p className="text-xs uppercase tracking-[0.16em] text-cyan-300/50">CPU</p>
                <p className="mt-2 text-xl text-cyan-50">{cpuName}</p>
                <p className="mt-1 text-sm text-cyan-100/60">
                  {metrics?.cpu_cores ?? 0} cores at {Math.round(cpuPercent)}% load
                </p>
              </div>
              <div className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4">
                <p className="text-xs uppercase tracking-[0.16em] text-cyan-300/50">Runtime</p>
                <p className="mt-2 text-xl text-cyan-50">
                  {metrics ? formatUptime(metrics.uptime_secs ?? 0) : "--"}
                </p>
                <p className="mt-1 text-sm text-cyan-100/60">
                  {metrics?.process_count ?? 0} processes visible to the runtime
                </p>
              </div>
              <div className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4 md:col-span-2">
                <p className="text-xs uppercase tracking-[0.16em] text-cyan-300/50">Fuel Summary</p>
                <div className="mt-3 h-3 overflow-hidden rounded-full bg-slate-900">
                  <div
                    className="h-full rounded-full bg-gradient-to-r from-cyan-400 via-sky-400 to-emerald-400"
                    style={{
                      width: `${
                        fuelSummary.budget > 0
                          ? Math.min(100, (fuelSummary.used / fuelSummary.budget) * 100)
                          : 0
                      }%`,
                    }}
                  />
                </div>
                <div className="mt-3 flex flex-wrap gap-4 text-sm text-cyan-100/65">
                  <span>Used: {fuelSummary.used.toLocaleString()}</span>
                  <span>Remaining: {fuelSummary.remaining.toLocaleString()}</span>
                  <span>Budget: {fuelSummary.budget.toLocaleString()}</span>
                </div>
              </div>
            </div>
          )}
        </section>

        {/* Right: Recent Audit Events (compact) */}
        <section className="nexus-panel rounded-2xl p-5">
          <h3 className="text-lg text-cyan-50">Recent Audit Events</h3>
          <p className="text-sm text-cyan-100/60">Hash-chained governance trail — last 8 events.</p>
          <div className="mt-4 space-y-2">
            {auditEvents.length === 0 ? (
              <p className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4 text-sm text-cyan-100/60">
                No audit events recorded yet.
              </p>
            ) : (
              auditEvents.map((event) => {
                const payloadStr = typeof event.payload === "object" && event.payload !== null
                  ? Object.entries(event.payload as Record<string, unknown>)
                      .filter(([k]) => !["manifest", "instructions", "goal"].includes(k.toLowerCase()))
                      .map(([k, v]) => {
                        const val = typeof v === "string" && v.length > 60 ? v.slice(0, 60) + "…" : String(v);
                        return `${k}: ${val}`;
                      })
                      .slice(0, 3)
                      .join(" · ")
                  : "";
                return (
                  <article
                    key={event.event_id}
                    className="rounded-xl border border-cyan-500/10 bg-slate-950/40 px-4 py-3"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <span className="text-sm font-semibold text-cyan-50">{event.event_type}</span>
                      <span className="whitespace-nowrap font-mono text-[0.65rem] text-cyan-100/45">
                        {new Date(event.timestamp * 1000).toLocaleString()}
                      </span>
                    </div>
                    <p className="mt-1 font-mono text-[0.65rem] text-cyan-100/40">
                      {truncateId(event.agent_id)}
                    </p>
                    {payloadStr && (
                      <p className="mt-1 text-xs text-cyan-100/50">
                        {payloadStr}
                      </p>
                    )}
                  </article>
                );
              })
            )}
          </div>
        </section>
      </div>
    </section>
  );
}

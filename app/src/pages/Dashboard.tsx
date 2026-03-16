import { useCallback, useEffect, useMemo, useState } from "react";
import { getAuditLog, getLiveSystemMetricsJson, listAgents } from "../api/backend";
import type { AgentSummary, AuditEventRow } from "../types";

type SystemMetrics = {
  cpu_name?: string;
  cpu_avg?: number;
  cpu_cores?: number;
  used_ram?: number;
  total_ram?: number;
  uptime_secs?: number;
  process_count?: number;
  agents?: Array<{
    id: string;
    name: string;
    fuel_budget?: number;
    fuel_used?: number;
    remaining_fuel?: number;
    state?: string;
  }>;
};

function formatPercent(value: number): string {
  return `${Math.round(value)}%`;
}

function formatBytesToGiB(value: number): string {
  return `${(value / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function formatUptime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return `${hours}h ${minutes}m`;
}

export default function Dashboard(): JSX.Element {
  const [agents, setAgents] = useState<AgentSummary[]>([]);
  const [auditEvents, setAuditEvents] = useState<AuditEventRow[]>([]);
  const [metrics, setMetrics] = useState<SystemMetrics | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [agentRows, auditRows, metricRows] = await Promise.all([
        listAgents(),
        getAuditLog(undefined, 12),
        getLiveSystemMetricsJson<SystemMetrics>(),
      ]);
      setAgents(agentRows);
      setAuditEvents(auditRows);
      setMetrics(metricRows);
    } catch (loadError) {
      setError(loadError instanceof Error ? loadError.message : String(loadError));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const activeAgents = useMemo(
    () =>
      agents.filter((agent) =>
        ["running", "starting", "paused"].includes(agent.status.toLowerCase()),
      ).length,
    [agents],
  );

  const fuelSummary = useMemo(() => {
    const source = metrics?.agents ?? [];
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

  return (
    <section className="mx-auto flex max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6">
      <header className="nexus-panel rounded-3xl p-6 shadow-sm">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="text-xs uppercase tracking-[0.24em] text-cyan-300/70">Home Overview</p>
            <h2 className="nexus-display mt-2 text-3xl text-cyan-50">Dashboard</h2>
            <p className="mt-2 max-w-2xl text-sm text-cyan-100/65">
              Real runtime status from registered agents, live system metrics, and the latest audit trail.
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

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        <article className="nexus-panel rounded-2xl p-5">
          <p className="text-xs uppercase tracking-[0.2em] text-cyan-300/60">Total Agents</p>
          <p className="mt-3 text-3xl text-cyan-50">{agents.length}</p>
        </article>
        <article className="nexus-panel rounded-2xl p-5">
          <p className="text-xs uppercase tracking-[0.2em] text-cyan-300/60">Active Agents</p>
          <p className="mt-3 text-3xl text-cyan-50">{activeAgents}</p>
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
            {metrics ? formatPercent(metrics.cpu_avg ?? 0) : "--"}
          </p>
          <p className="mt-1 text-xs text-cyan-100/60">
            {metrics
              ? `${formatBytesToGiB(metrics.used_ram ?? 0)} / ${formatBytesToGiB(metrics.total_ram ?? 0)}`
              : "Loading metrics"}
          </p>
        </article>
      </div>

      <div className="grid gap-4 xl:grid-cols-[1.1fr_0.9fr]">
        <section className="nexus-panel rounded-2xl p-5">
          <div className="flex items-center justify-between gap-4">
            <div>
              <h3 className="text-lg text-cyan-50">System Metrics</h3>
              <p className="text-sm text-cyan-100/60">Backed by `get_live_system_metrics`.</p>
            </div>
          </div>
          {loading && !metrics ? (
            <p className="mt-4 text-sm text-cyan-100/60">Loading metrics...</p>
          ) : (
            <div className="mt-5 grid gap-4 md:grid-cols-2">
              <div className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4">
                <p className="text-xs uppercase tracking-[0.16em] text-cyan-300/50">CPU</p>
                <p className="mt-2 text-xl text-cyan-50">{metrics?.cpu_name ?? "Unknown CPU"}</p>
                <p className="mt-1 text-sm text-cyan-100/60">
                  {metrics?.cpu_cores ?? 0} cores at {formatPercent(metrics?.cpu_avg ?? 0)} average load
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

        <section className="nexus-panel rounded-2xl p-5">
          <h3 className="text-lg text-cyan-50">Recent Audit Events</h3>
          <p className="text-sm text-cyan-100/60">Backed by `get_audit_log`.</p>
          <div className="mt-4 space-y-3">
            {auditEvents.length === 0 ? (
              <p className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4 text-sm text-cyan-100/60">
                No audit events found.
              </p>
            ) : (
              auditEvents.map((event) => (
                <article
                  key={event.event_id}
                  className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4"
                >
                  <div className="flex items-center justify-between gap-3">
                    <strong className="text-sm text-cyan-50">{event.event_type}</strong>
                    <span className="text-xs text-cyan-100/50">
                      {new Date(event.timestamp * 1000).toLocaleString()}
                    </span>
                  </div>
                  <p className="mt-2 text-xs text-cyan-100/55">{event.agent_id}</p>
                  <pre className="mt-3 overflow-x-auto whitespace-pre-wrap text-xs text-cyan-100/65">
                    {JSON.stringify(event.payload, null, 2)}
                  </pre>
                </article>
              ))
            )}
          </div>
        </section>
      </div>
    </section>
  );
}

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

/* ── UX Fix 1: Human-readable audit action labels ── */

const ACTION_LABELS: Record<string, string> = {
  file_manager_list: "Listed files",
  file_manager_read: "Read file",
  file_manager_write: "Wrote file",
  file_manager_delete: "Deleted file",
  file_manager_create_dir: "Created folder",
  file_manager_rename: "Renamed file",
  email_list: "Listed emails",
  email_oauth_status: "Checked email connection",
  email_start_oauth: "Started email sign-in",
  email_send: "Sent email",
  web_search: "Searched the web",
  web_fetch: "Fetched web page",
  process_exec: "Executed command",
  agent_spawn: "Started an agent",
  agent_stop: "Stopped an agent",
  agent_pause: "Paused an agent",
  agent_resume: "Resumed an agent",
  llm_query: "Queried LLM",
  chat_send: "Sent chat message",
  model_pull: "Downloaded model",
  model_delete: "Deleted model",
  config_save: "Saved configuration",
  scheduler_create: "Created schedule",
  deploy_start: "Started deployment",
  deploy_complete: "Deployment completed",
};

function getActionLabel(action: string): string {
  if (ACTION_LABELS[action]) return ACTION_LABELS[action];
  // Extract the inner part from Rust debug format like "UserAction" or "StateChange"
  const cleaned = action.replace(/^(UserAction|StateChange|SecurityEvent|SystemEvent)\s*/, "");
  if (cleaned && ACTION_LABELS[cleaned]) return ACTION_LABELS[cleaned];
  // Fallback: convert snake_case / PascalCase to Title Case
  return action
    .replace(/([A-Z])/g, " $1")
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase())
    .trim();
}

/* ── Bug 1: Well-known system UUID to detect system-initiated events ── */
const SYSTEM_UUID_PREFIX = "4e585359-532d-0001";

function formatAgentId(id: string): string {
  if (!id || id === "00000000-0000-0000-0000-000000000000") return "system";
  if (id.startsWith(SYSTEM_UUID_PREFIX)) return "system";
  return id.slice(0, 8) + "...";
}

/* ── Helpers ── */

function formatUptime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return `${hours}h ${minutes}m`;
}

function normalizeMetrics(input: unknown): SystemMetrics | null {
  if (!input || typeof input !== "object") return null;
  const candidate = input as SystemMetrics;
  return { ...candidate, agents: Array.isArray(candidate.agents) ? candidate.agents : [] };
}

/* ── Component ── */

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

  useEffect(() => { void refresh(); }, [refresh]);

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

  const runningAgents = useMemo(
    () => agents.filter((a) => a.status === "Running").length,
    [agents],
  );
  const dormantAgents = agents.length - runningAgents;

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

  const cpuPercent = sysInfo?.cpu_usage_percent ?? metrics?.cpu_avg ?? 0;
  const ramUsedGb = sysInfo?.ram_used_gb ?? (metrics?.used_ram ? +(metrics.used_ram / 1024 / 1024 / 1024).toFixed(1) : 0);
  const ramTotalGb = sysInfo?.ram_total_gb ?? (metrics?.total_ram ? +(metrics.total_ram / 1024 / 1024 / 1024).toFixed(1) : 0);
  const cpuName = sysInfo?.cpu_name ?? metrics?.cpu_name ?? "Awaiting runtime...";

  return (
    <section className="mx-auto flex max-w-7xl flex-col gap-6 px-4 py-6 sm:px-6" style={{ paddingBottom: 80 }}>
      <div className="flex items-center justify-between">
        <h2 className="nexus-display text-2xl text-cyan-50">Runtime Overview</h2>
        <button
          type="button"
          onClick={() => void refresh()}
          className="rounded-full border border-cyan-400/40 bg-cyan-500/10 px-4 py-2 text-sm text-cyan-100 transition hover:bg-cyan-500/20"
        >
          Refresh
        </button>
      </div>

      {error ? (
        <div className="rounded-2xl border border-rose-400/40 bg-rose-500/10 p-4 text-sm text-rose-100">
          {error}
        </div>
      ) : null}

      {/* ── KPI Cards (UX Fix 2: wrap on narrow screens) ── */}
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4" style={{ flexWrap: "wrap" }}>
        {/* UX Fix 2: "Available Agents" with dormant/active breakdown */}
        <article className="nexus-panel rounded-2xl p-5">
          <p className="text-xs uppercase tracking-[0.2em] text-cyan-300/60">Available Agents</p>
          <p className="mt-3 text-3xl text-cyan-50">{agents.length}</p>
          <p className="mt-1 text-xs text-cyan-100/50">
            {runningAgents} active {"\u00B7"} {dormantAgents} dormant
          </p>
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
          <p className="mt-3 text-2xl text-cyan-50">{Math.round(cpuPercent)}%</p>
          <p className="mt-1 text-xs text-cyan-100/60">{ramUsedGb} GB / {ramTotalGb} GB</p>
        </article>
      </div>

      {/* ── System + Audit (Bug 2: responsive breakpoint) ── */}
      <div className="grid gap-4 lg:grid-cols-[1.1fr_0.9fr]">
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

        {/* Right: Recent Audit Events (Bug 2: scrollable, UX Fix 1: readable labels) */}
        <section className="nexus-panel rounded-2xl p-5">
          <h3 className="text-lg text-cyan-50">Recent Audit Events</h3>
          <p className="text-sm text-cyan-100/60">Hash-chained governance trail — last 8 events.</p>
          <div className="mt-4 space-y-2" style={{ maxHeight: 500, overflowY: "auto", scrollbarWidth: "thin", scrollbarColor: "#06b6d4 #111827" }}>
            {auditEvents.length === 0 ? (
              <p className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4 text-sm text-cyan-100/60">
                No audit events recorded yet.
              </p>
            ) : (
              auditEvents.map((event) => {
                /* Extract action from payload for human-readable label */
                const rawAction = typeof event.payload === "object" && event.payload !== null
                  ? String((event.payload as Record<string, unknown>).action ?? "")
                  : "";
                const rawPath = typeof event.payload === "object" && event.payload !== null
                  ? String((event.payload as Record<string, unknown>).path ?? (event.payload as Record<string, unknown>).url ?? "")
                  : "";

                const humanLabel = rawAction ? getActionLabel(rawAction) : getActionLabel(event.event_type);

                const payloadStr = typeof event.payload === "object" && event.payload !== null
                  ? Object.entries(event.payload as Record<string, unknown>)
                      .filter(([k]) => !["manifest", "instructions", "goal", "action"].includes(k.toLowerCase()))
                      .map(([k, v]) => {
                        const val = typeof v === "string" && v.length > 60 ? v.slice(0, 60) + "\u2026" : String(v);
                        return `${k}: ${val}`;
                      })
                      .slice(0, 3)
                      .join(" \u00B7 ")
                  : "";

                return (
                  <article
                    key={event.event_id}
                    className="rounded-xl border border-cyan-500/10 bg-slate-950/40 px-4 py-3"
                  >
                    <div className="flex items-center justify-between gap-3">
                      {/* UX Fix 1: Human-readable label as primary text */}
                      <span className="text-sm font-semibold text-cyan-50">{humanLabel}</span>
                      <span className="whitespace-nowrap font-mono text-[0.65rem] text-cyan-100/45">
                        {new Date(event.timestamp * 1000).toLocaleString()}
                      </span>
                    </div>
                    {/* Raw event type + agent ID in muted line */}
                    <p className="mt-1 font-mono text-[0.65rem] text-cyan-100/35">
                      {event.event_type}
                      {rawPath ? ` \u2014 ${rawPath.length > 40 ? rawPath.slice(0, 40) + "\u2026" : rawPath}` : ""}
                      {" \u00B7 "}
                      {/* Bug 1: Show readable identity instead of null UUID */}
                      <span className="text-cyan-100/30">{formatAgentId(event.agent_id)}</span>
                    </p>
                    {payloadStr && (
                      <p className="mt-1 text-xs text-cyan-100/50">{payloadStr}</p>
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

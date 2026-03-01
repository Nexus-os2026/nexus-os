import { useMemo, useState } from "react";
import type { AuditEventRow } from "../types";

interface AuditProps {
  events: AuditEventRow[];
}

export function Audit({ events }: AuditProps): JSX.Element {
  const [query, setQuery] = useState("");
  const [agentFilter, setAgentFilter] = useState("all");
  const [eventTypeFilter, setEventTypeFilter] = useState("all");

  const filtered = useMemo(() => {
    const lowered = query.toLowerCase();
    return events.filter((event) => {
      const matchesQuery =
        lowered.length === 0 ||
        event.event_id.toLowerCase().includes(lowered) ||
        JSON.stringify(event.payload).toLowerCase().includes(lowered);
      const matchesAgent = agentFilter === "all" || event.agent_id === agentFilter;
      const matchesType = eventTypeFilter === "all" || event.event_type === eventTypeFilter;
      return matchesQuery && matchesAgent && matchesType;
    });
  }, [agentFilter, eventTypeFilter, events, query]);

  const integrity = useMemo(() => {
    for (let index = 1; index < filtered.length; index += 1) {
      if (filtered[index].previous_hash !== filtered[index - 1].hash) {
        return false;
      }
    }
    return true;
  }, [filtered]);

  const agents = Array.from(new Set(events.map((event) => event.agent_id)));
  const eventTypes = Array.from(new Set(events.map((event) => event.event_type)));

  return (
    <section className="soft-card rounded-2xl p-6 shadow-sm">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="font-display text-2xl text-ink">Audit Explorer</h2>
        <span
          className={`rounded-full px-3 py-1 text-xs font-semibold ${
            integrity ? "bg-emerald-100 text-emerald-700" : "bg-rose-100 text-rose-700"
          }`}
        >
          {integrity ? "Integrity Valid" : "Integrity Tampered"}
        </span>
      </div>

      <div className="mt-4 grid gap-2 sm:grid-cols-3">
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search events"
          className="rounded-lg border border-slate-300 px-3 py-2 text-sm"
        />
        <select
          value={agentFilter}
          onChange={(event) => setAgentFilter(event.target.value)}
          className="rounded-lg border border-slate-300 px-3 py-2 text-sm"
        >
          <option value="all">All agents</option>
          {agents.map((agentId) => (
            <option key={agentId} value={agentId}>{agentId}</option>
          ))}
        </select>
        <select
          value={eventTypeFilter}
          onChange={(event) => setEventTypeFilter(event.target.value)}
          className="rounded-lg border border-slate-300 px-3 py-2 text-sm"
        >
          <option value="all">All event types</option>
          {eventTypes.map((eventType) => (
            <option key={eventType} value={eventType}>{eventType}</option>
          ))}
        </select>
      </div>

      <div className="mt-4 max-h-80 overflow-auto rounded-xl border border-slate-200 bg-white/80">
        <table className="min-w-full text-left text-xs">
          <thead className="sticky top-0 bg-mist text-ink">
            <tr>
              <th className="px-3 py-2">Time</th>
              <th className="px-3 py-2">Agent</th>
              <th className="px-3 py-2">Type</th>
              <th className="px-3 py-2">Event</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((event) => (
              <tr key={event.event_id} className="border-t border-slate-100">
                <td className="px-3 py-2">{new Date(event.timestamp * 1000).toLocaleString()}</td>
                <td className="px-3 py-2">{event.agent_id}</td>
                <td className="px-3 py-2">{event.event_type}</td>
                <td className="px-3 py-2">{event.event_id}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

import { useMemo, useState } from "react";
import type { AuditEventRow } from "../types";

interface AuditProps {
  events: AuditEventRow[];
}

export function Audit({ events }: AuditProps): JSX.Element {
  const [query, setQuery] = useState("");
  const [agentFilter, setAgentFilter] = useState("all");
  const [eventTypeFilter, setEventTypeFilter] = useState("all");
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const [selectedEventId, setSelectedEventId] = useState<string | null>(events[0]?.event_id ?? null);

  const filtered = useMemo(() => {
    const lowered = query.toLowerCase();
    return events.filter((event) => {
      const eventDate = new Date(event.timestamp * 1000);
      const fromMatches = dateFrom.length === 0 || eventDate >= new Date(`${dateFrom}T00:00:00`);
      const toMatches = dateTo.length === 0 || eventDate <= new Date(`${dateTo}T23:59:59`);
      const matchesQuery =
        lowered.length === 0 ||
        event.event_id.toLowerCase().includes(lowered) ||
        JSON.stringify(event.payload).toLowerCase().includes(lowered);
      const matchesAgent = agentFilter === "all" || event.agent_id === agentFilter;
      const matchesType = eventTypeFilter === "all" || event.event_type === eventTypeFilter;
      return matchesQuery && matchesAgent && matchesType && fromMatches && toMatches;
    });
  }, [agentFilter, dateFrom, dateTo, eventTypeFilter, events, query]);

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
  const selectedEvent = filtered.find((event) => event.event_id === selectedEventId) ?? filtered[0] ?? null;

  return (
    <section className="grid h-[calc(100vh-10rem)] grid-cols-1 gap-4 lg:grid-cols-[1.25fr_1fr]">
      <div className="rounded-2xl border border-zinc-800 bg-zinc-900/80 p-6 shadow-sm">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="font-display text-2xl text-zinc-100">Audit Explorer</h2>
        <span
          className={`rounded-full px-3 py-1 text-xs font-semibold ${
            integrity ? "bg-emerald-500/20 text-emerald-300" : "bg-rose-500/20 text-rose-300"
          }`}
        >
          {integrity ? "Chain: ✓ valid" : "Chain: invalid"}
        </span>
      </div>

      <div className="mt-4 grid gap-2 sm:grid-cols-5">
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search events"
          className="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
        />
        <select
          value={agentFilter}
          onChange={(event) => setAgentFilter(event.target.value)}
          className="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
        >
          <option value="all">All agents</option>
          {agents.map((agentId) => (
            <option key={agentId} value={agentId}>{agentId}</option>
          ))}
        </select>
        <select
          value={eventTypeFilter}
          onChange={(event) => setEventTypeFilter(event.target.value)}
          className="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
        >
          <option value="all">All event types</option>
          {eventTypes.map((eventType) => (
            <option key={eventType} value={eventType}>{eventType}</option>
          ))}
        </select>
        <input
          type="date"
          value={dateFrom}
          onChange={(event) => setDateFrom(event.target.value)}
          className="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
        />
        <input
          type="date"
          value={dateTo}
          onChange={(event) => setDateTo(event.target.value)}
          className="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
        />
      </div>

      <div className="mt-4 max-h-[32rem] overflow-auto rounded-xl border border-zinc-800 bg-zinc-950/70">
        <table className="min-w-full text-left text-xs text-zinc-200">
          <thead className="sticky top-0 bg-zinc-900 text-zinc-300">
            <tr>
              <th className="px-3 py-2">Time</th>
              <th className="px-3 py-2">Agent</th>
              <th className="px-3 py-2">Type</th>
              <th className="px-3 py-2">Summary</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((event) => (
              <tr
                key={event.event_id}
                onClick={() => setSelectedEventId(event.event_id)}
                className={`cursor-pointer border-t border-zinc-800 ${
                  selectedEvent?.event_id === event.event_id ? "bg-zinc-800/80" : "hover:bg-zinc-900"
                }`}
              >
                <td className="px-3 py-2">{new Date(event.timestamp * 1000).toLocaleString()}</td>
                <td className="px-3 py-2">{event.agent_id}</td>
                <td className="px-3 py-2">{event.event_type}</td>
                <td className="px-3 py-2">{summarizePayload(event.payload)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      </div>

      <aside className="rounded-2xl border border-zinc-800 bg-zinc-900/80 p-6 shadow-sm">
        <h3 className="font-display text-lg text-zinc-100">Event Details</h3>
        {selectedEvent ? (
          <div className="mt-3 space-y-3 text-xs">
            <div className="rounded-lg border border-zinc-800 bg-zinc-950 p-3">
              <p className="text-zinc-300">Timestamp</p>
              <p className="mt-1 text-zinc-100">{new Date(selectedEvent.timestamp * 1000).toLocaleString()}</p>
            </div>
            <div className="rounded-lg border border-zinc-800 bg-zinc-950 p-3">
              <p className="text-zinc-300">Agent</p>
              <p className="mt-1 text-zinc-100">{selectedEvent.agent_id}</p>
            </div>
            <div className="rounded-lg border border-zinc-800 bg-zinc-950 p-3">
              <p className="text-zinc-300">Event Type</p>
              <p className="mt-1 text-zinc-100">{selectedEvent.event_type}</p>
            </div>
            <div className="rounded-lg border border-zinc-800 bg-zinc-950 p-3">
              <p className="text-zinc-300">Payload</p>
              <pre className="mt-1 whitespace-pre-wrap text-zinc-100">{JSON.stringify(selectedEvent.payload, null, 2)}</pre>
            </div>
          </div>
        ) : (
          <p className="mt-3 text-sm text-zinc-400">Select an event to inspect payload details.</p>
        )}
      </aside>
    </section>
  );
}

function summarizePayload(payload: Record<string, unknown>): string {
  const compact = JSON.stringify(payload);
  if (compact.length <= 120) {
    return compact;
  }
  return `${compact.slice(0, 117)}...`;
}

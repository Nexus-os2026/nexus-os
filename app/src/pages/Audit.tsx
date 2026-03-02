import { useEffect, useMemo, useState } from "react";
import "./audit.css";
import type { AuditEventRow } from "../types";

interface AuditProps {
  events: AuditEventRow[];
}

type EventCategory = "StateChange" | "ToolCall" | "LlmCall" | "Error" | "UserAction" | "Other";

const EVENT_TYPES: EventCategory[] = ["StateChange", "ToolCall", "LlmCall", "Error", "UserAction", "Other"];

function eventCategory(eventType: string): EventCategory {
  const lowered = eventType.toLowerCase();
  if (lowered.includes("state")) {
    return "StateChange";
  }
  if (lowered.includes("tool")) {
    return "ToolCall";
  }
  if (lowered.includes("llm")) {
    return "LlmCall";
  }
  if (lowered.includes("error")) {
    return "Error";
  }
  if (lowered.includes("user")) {
    return "UserAction";
  }
  return "Other";
}

function categoryClass(category: EventCategory): string {
  if (category === "StateChange") {
    return "audit-color-statechange";
  }
  if (category === "ToolCall") {
    return "audit-color-toolcall";
  }
  if (category === "LlmCall") {
    return "audit-color-llmcall";
  }
  if (category === "Error") {
    return "audit-color-error";
  }
  if (category === "UserAction") {
    return "audit-color-useraction";
  }
  return "audit-color-statechange";
}

function categoryIcon(category: EventCategory): string {
  if (category === "StateChange") {
    return "◉";
  }
  if (category === "ToolCall") {
    return "⬢";
  }
  if (category === "LlmCall") {
    return "✦";
  }
  if (category === "Error") {
    return "✕";
  }
  if (category === "UserAction") {
    return "✓";
  }
  return "◌";
}

function summarizePayload(payload: Record<string, unknown>): string {
  const compact = JSON.stringify(payload);
  if (compact.length <= 140) {
    return compact;
  }
  return `${compact.slice(0, 137)}...`;
}

function formatDateTime(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleString("en-GB", {
    hour12: false,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  });
}

function chainIntegrity(rows: AuditEventRow[]): boolean {
  for (let index = 1; index < rows.length; index += 1) {
    if (rows[index].previous_hash !== rows[index - 1].hash) {
      return false;
    }
  }
  return true;
}

export function Audit({ events }: AuditProps): JSX.Element {
  const [query, setQuery] = useState("");
  const [agentFilter, setAgentFilter] = useState("all");
  const [selectedTypes, setSelectedTypes] = useState<EventCategory[]>(EVENT_TYPES);
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");
  const [expandedEventId, setExpandedEventId] = useState<string | null>(null);
  const [replayMode, setReplayMode] = useState(false);
  const [replayIndex, setReplayIndex] = useState(0);

  const chronological = useMemo(
    () => [...events].sort((left, right) => left.timestamp - right.timestamp),
    [events]
  );

  const filtered = useMemo(() => {
    const lowered = query.trim().toLowerCase();
    return chronological.filter((event) => {
      const eventDate = new Date(event.timestamp * 1000);
      const fromMatches = dateFrom.length === 0 || eventDate >= new Date(`${dateFrom}T00:00:00`);
      const toMatches = dateTo.length === 0 || eventDate <= new Date(`${dateTo}T23:59:59`);
      const category = eventCategory(event.event_type);
      const typeMatches = selectedTypes.includes(category);
      const agentMatches = agentFilter === "all" || event.agent_id === agentFilter;
      const textMatches =
        lowered.length === 0 ||
        event.event_id.toLowerCase().includes(lowered) ||
        event.event_type.toLowerCase().includes(lowered) ||
        JSON.stringify(event.payload).toLowerCase().includes(lowered);
      return fromMatches && toMatches && typeMatches && agentMatches && textMatches;
    });
  }, [agentFilter, chronological, dateFrom, dateTo, query, selectedTypes]);

  const integrity = useMemo(() => chainIntegrity(filtered), [filtered]);

  const visibleEvents = useMemo(() => {
    if (!replayMode) {
      return filtered;
    }
    return filtered.slice(0, replayIndex + 1);
  }, [filtered, replayIndex, replayMode]);

  useEffect(() => {
    if (!replayMode) {
      return;
    }
    if (filtered.length === 0) {
      setReplayMode(false);
      setReplayIndex(0);
      return;
    }
    const timer = window.setInterval(() => {
      setReplayIndex((current) => {
        if (current >= filtered.length - 1) {
          window.clearInterval(timer);
          setReplayMode(false);
          return current;
        }
        return current + 1;
      });
    }, 520);
    return () => {
      window.clearInterval(timer);
    };
  }, [filtered, replayMode]);

  const agents = useMemo(
    () => Array.from(new Set(events.map((event) => event.agent_id))),
    [events]
  );

  function toggleType(category: EventCategory): void {
    setSelectedTypes((previous) => {
      if (previous.includes(category)) {
        const next = previous.filter((item) => item !== category);
        return next.length > 0 ? next : previous;
      }
      return [...previous, category];
    });
  }

  function startReplay(): void {
    if (filtered.length === 0) {
      return;
    }
    setReplayIndex(0);
    setReplayMode(true);
  }

  return (
    <section className="audit-forensic">
      <header className="audit-header">
        <h2 className="audit-title">FORENSIC REPLAY // AUDIT TRAIL</h2>
        <span className={`audit-integrity ${integrity ? "valid" : "invalid"}`}>
          <span>{integrity ? "⬢✓" : "⛓✕"}</span>
          {integrity ? "Hash chain intact" : "Chain integrity warning"}
        </span>
      </header>

      <div className="audit-controls">
        <input
          className="audit-input"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search payloads, type, id..."
        />

        <select
          className="audit-select"
          value={agentFilter}
          onChange={(event) => setAgentFilter(event.target.value)}
        >
          <option value="all">All agents</option>
          {agents.map((agentId) => (
            <option key={agentId} value={agentId}>
              {agentId}
            </option>
          ))}
        </select>

        <select
          className="audit-select"
          multiple
          value={selectedTypes}
          onChange={(event) => {
            const values = Array.from(event.target.selectedOptions).map(
              (option) => option.value as EventCategory
            );
            if (values.length > 0) {
              setSelectedTypes(values);
            }
          }}
          aria-label="Event types"
        >
          {EVENT_TYPES.map((type) => (
            <option key={type} value={type}>
              {type}
            </option>
          ))}
        </select>

        <input
          type="date"
          className="audit-input"
          value={dateFrom}
          onChange={(event) => setDateFrom(event.target.value)}
        />

        <input
          type="date"
          className="audit-input"
          value={dateTo}
          onChange={(event) => setDateTo(event.target.value)}
        />

        <button
          type="button"
          className="audit-replay-btn"
          onClick={() => {
            if (replayMode) {
              setReplayMode(false);
              return;
            }
            startReplay();
          }}
        >
          {replayMode ? "Stop Replay" : "Replay"}
        </button>
      </div>

      <div className="audit-controls !grid-cols-1 !pt-0">
        <div className="flex flex-wrap items-center gap-2">
          {EVENT_TYPES.map((type) => (
            <button
              key={type}
              type="button"
              onClick={() => toggleType(type)}
              className={`audit-event-toggle ${selectedTypes.includes(type) ? "audit-color-statechange" : ""}`}
            >
              {selectedTypes.includes(type) ? "✓" : "○"} {type}
            </button>
          ))}
        </div>
      </div>

      <main className="audit-timeline">
        {visibleEvents.length === 0 ? (
          <p className="audit-empty">No events match the current forensic filters.</p>
        ) : (
          visibleEvents.map((event) => {
            const category = eventCategory(event.event_type);
            const colorClass = categoryClass(category);
            const expanded = expandedEventId === event.event_id;
            return (
              <article key={event.event_id} className="audit-event fade-slide-up">
                <span className={`audit-event-node ${colorClass}`} />
                <div className="audit-event-head">
                  <span className={`audit-event-type ${colorClass}`}>
                    {categoryIcon(category)} {event.event_type}
                  </span>
                  <span className="audit-event-time">{formatDateTime(event.timestamp)}</span>
                </div>
                <p className="audit-event-agent">Agent: {event.agent_id}</p>
                <p className="audit-event-summary">{summarizePayload(event.payload)}</p>
                <button
                  type="button"
                  className="audit-event-toggle"
                  onClick={() => setExpandedEventId(expanded ? null : event.event_id)}
                >
                  {expanded ? "Hide Payload" : "View Payload JSON"}
                </button>
                {expanded ? (
                  <pre className="audit-event-code">{JSON.stringify(event.payload, null, 2)}</pre>
                ) : null}
              </article>
            );
          })
        )}
      </main>
    </section>
  );
}

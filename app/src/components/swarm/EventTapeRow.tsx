/**
 * EventTapeRow — single event in the tape. Category pill colors + a
 * one-line summary derived from the variant. Click the row to expand
 * the raw JSON payload in-place.
 */

import { useState } from "react";
import { eventCategory, type EventCategory } from "../../lib/swarm/event_category";
import { isFailureEvent } from "../../lib/swarm/selectors";
import type { SwarmEvent } from "../../lib/swarm/types";

const PILL_COLORS: Record<EventCategory, { bg: string; fg: string; border: string }> = {
  plan: {
    bg: "rgba(147,51,234,0.18)",
    fg: "#d8b4fe",
    border: "rgba(192,132,252,0.45)",
  },
  node: {
    bg: "rgba(22,163,74,0.18)",
    fg: "#86efac",
    border: "rgba(74,222,128,0.45)",
  },
  oracle: {
    bg: "rgba(234,179,8,0.18)",
    fg: "#fcd34d",
    border: "rgba(251,191,36,0.45)",
  },
  provider: {
    bg: "rgba(59,130,246,0.18)",
    fg: "#93c5fd",
    border: "rgba(96,165,250,0.45)",
  },
  swarm: {
    bg: "rgba(100,116,139,0.18)",
    fg: "#cbd5e1",
    border: "rgba(148,163,184,0.45)",
  },
};

function summarize(ev: SwarmEvent): string {
  switch (ev.event) {
    case "plan_proposed":
      return `plan proposed · run ${ev.run_id.slice(0, 8)}…`;
    case "plan_approved":
      return `plan approved · run ${ev.run_id.slice(0, 8)}…`;
    case "plan_rejected":
      return `plan rejected · ${ev.reason}`;
    case "node_started":
      return `node started · ${ev.ref.node_id} → ${ev.provider_id}/${ev.model_id}`;
    case "node_event":
      return `node event · ${ev.ref.node_id} · ${ev.phase}`;
    case "node_completed":
      return `node completed · ${ev.ref.node_id}`;
    case "node_failed":
      return `node failed · ${ev.ref.node_id} · ${ev.reason}`;
    case "route_denied":
      return `route denied · ${ev.ref.node_id} · ${ev.denied.reasons.join(", ")}`;
    case "budget_update":
      return `budget · tokens ${ev.tokens_remaining} / cents ${ev.cents_remaining}`;
    case "provider_health_update":
      return `providers · ${ev.providers.map((p) => `${p.provider_id}:${p.status}`).join(", ")}`;
    case "swarm_completed":
      return `swarm completed · run ${ev.run_id.slice(0, 8)}…`;
    case "swarm_cancelled":
      return `swarm cancelled · run ${ev.run_id.slice(0, 8)}…`;
    case "oracle_ticket_issued":
      return `ticket issued · ${ev.ticket_id.slice(0, 8)}…`;
    case "oracle_runtime_check":
      return `oracle ${ev.decision.approved ? "approved" : "denied"} · ${ev.highrisk_event.kind}`;
    case "oracle_runtime_denial":
      return `oracle denial · node ${ev.node_id}`;
    default: {
      const _exhaustive: never = ev;
      void _exhaustive;
      return "unknown event";
    }
  }
}

export interface EventTapeRowProps {
  event: SwarmEvent;
  /**
   * Sequence number fed by EventTape — stable, 1-based within the
   * current tape window. Used purely for the DOM test id.
   */
  seq: number;
  /** HH:MM:SS rendered by the parent; we render whatever it hands us. */
  timestamp: string;
}

export function EventTapeRow({ event, seq, timestamp }: EventTapeRowProps): JSX.Element {
  const [expanded, setExpanded] = useState(false);
  const cat = eventCategory(event);
  const pill = PILL_COLORS[cat];
  const failure = isFailureEvent(event);

  return (
    <div
      data-testid={`event-tape-row-${seq}`}
      data-event-kind={event.event}
      data-category={cat}
      data-failure={failure ? "true" : "false"}
      onClick={(): void => setExpanded((x) => !x)}
      style={{
        padding: "6px 10px",
        borderLeft: failure ? "3px solid #f87171" : "3px solid transparent",
        background: expanded ? "rgba(30,41,59,0.55)" : "rgba(15,23,42,0.35)",
        borderBottom: "1px solid rgba(100,116,139,0.15)",
        cursor: "pointer",
        color: "#cbd5e1",
        fontSize: 11,
        fontFamily: "var(--font-mono, monospace)",
      }}
    >
      <div style={{ display: "flex", gap: 8, alignItems: "baseline" }}>
        <span style={{ color: "#64748b", width: 64, flexShrink: 0 }}>{timestamp}</span>
        <span
          data-testid={`event-tape-pill-${seq}`}
          style={{
            padding: "1px 6px",
            borderRadius: 999,
            background: pill.bg,
            color: pill.fg,
            border: `1px solid ${pill.border}`,
            fontSize: 9,
            fontWeight: 700,
            letterSpacing: "0.08em",
            textTransform: "uppercase",
            flexShrink: 0,
          }}
        >
          {cat}
        </span>
        <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {summarize(event)}
        </span>
      </div>
      {expanded && (
        <pre
          data-testid={`event-tape-json-${seq}`}
          style={{
            margin: "6px 0 0 72px",
            padding: "6px 8px",
            background: "rgba(0,0,0,0.45)",
            border: "1px solid rgba(100,116,139,0.2)",
            borderRadius: 6,
            color: "#94a3b8",
            fontSize: 10,
            whiteSpace: "pre-wrap",
            wordBreak: "break-all",
          }}
        >
          {JSON.stringify(event, null, 2)}
        </pre>
      )}
    </div>
  );
}

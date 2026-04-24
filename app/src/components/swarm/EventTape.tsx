/**
 * EventTape — chronological event log with filters.
 *
 * Newest-first. Auto-scroll-to-top on new events is suppressed when
 * the user has manually scrolled away from the top (a tiny UX nicety
 * so reading a past event isn't disrupted). Bounded by the store's
 * 100-event cap — no virtualization needed.
 */

import { useEffect, useMemo, useRef } from "react";
import {
  selectEventFilter,
  selectFilteredEvents,
  selectRecentEvents,
} from "../../lib/swarm/selectors";
import { useSwarmStore } from "../../lib/swarm/store";
import { EventTapeRow } from "./EventTapeRow";
import { EventTapeFilters } from "./EventTapeFilters";

function formatHMS(date: Date): string {
  return date.toTimeString().slice(0, 8);
}

export function EventTape(): JSX.Element {
  const filter = useSwarmStore(selectEventFilter);
  const filterSelector = useMemo(() => selectFilteredEvents(filter), [filter]);
  const filtered = useSwarmStore(filterSelector);
  const total = useSwarmStore(selectRecentEvents).length;

  // Newest first.
  const reversed = useMemo(() => {
    const out = [...filtered];
    out.reverse();
    return out;
  }, [filtered]);

  const scrollRef = useRef<HTMLDivElement | null>(null);
  const userScrolledRef = useRef<boolean>(false);
  const prevCountRef = useRef<number>(reversed.length);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    if (reversed.length > prevCountRef.current && !userScrolledRef.current) {
      el.scrollTop = 0;
    }
    prevCountRef.current = reversed.length;
  }, [reversed.length]);

  const hasFilter =
    filter.byAgent !== null || filter.byEventType !== null || filter.severityOnly;

  // Arrival time is not carried on the wire for every variant — we stamp
  // on ingestion in the DOM. Stable across re-renders because seq is
  // monotonic within the tape window.
  const nowStamp = useMemo(() => formatHMS(new Date()), []);

  return (
    <section
      data-testid="event-tape"
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        width: "100%",
        minHeight: 0,
        minWidth: 0,
        border: "1px solid rgba(100,116,139,0.25)",
        borderRadius: 10,
        background: "rgba(10,14,26,0.6)",
        overflow: "hidden",
      }}
    >
      <EventTapeFilters />
      <div
        ref={scrollRef}
        onScroll={(e): void => {
          userScrolledRef.current = (e.currentTarget.scrollTop ?? 0) > 0;
        }}
        style={{ flex: 1, minHeight: 0, overflow: "auto" }}
      >
        {reversed.length === 0 && (
          <div
            data-testid="event-tape-empty"
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              height: "100%",
              color: "#475569",
              fontSize: 12,
              fontFamily: "var(--font-mono, monospace)",
            }}
          >
            {total === 0 ? "No events yet." : "No events match the current filter."}
          </div>
        )}
        {reversed.map((ev, i) => (
          <EventTapeRow
            // key uses event discriminator + offset; events are append-only
            // in the store so offset within the reversed slice is stable
            // within a single render window.
            key={`${ev.event}-${total - i}`}
            seq={total - i}
            timestamp={nowStamp}
            event={ev}
          />
        ))}
      </div>
      <footer
        data-testid="event-tape-footer"
        style={{
          padding: "4px 10px",
          borderTop: "1px solid rgba(100,116,139,0.2)",
          background: "rgba(10,14,26,0.5)",
          color: "#94a3b8",
          fontSize: 10,
          display: "flex",
          gap: 8,
          alignItems: "center",
          fontFamily: "var(--font-mono, monospace)",
        }}
      >
        <span>
          showing {reversed.length} of {total} event{total === 1 ? "" : "s"}
        </span>
        {hasFilter && (
          <span style={{ marginLeft: "auto", color: "#cbd5e1" }}>filter active</span>
        )}
      </footer>
    </section>
  );
}

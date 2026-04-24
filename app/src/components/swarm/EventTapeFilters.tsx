/**
 * EventTapeFilters — chip row sitting above the tape. Three dimensions:
 *   - by capability (derived from the current plan/run's DAG)
 *   - by event category (fixed 5-value vocab)
 *   - failures-only toggle
 *
 * Clicking the active chip in a dimension clears that dimension.
 */

import { useMemo } from "react";
import {
  clearEventFilter,
  setEventFilter,
  useSwarmStore,
} from "../../lib/swarm/store";
import {
  selectActiveRun,
  selectCurrentPlan,
  selectEventFilter,
} from "../../lib/swarm/selectors";
import { EVENT_CATEGORIES, type EventCategory } from "../../lib/swarm/event_category";

function Chip({
  active,
  onClick,
  children,
  testId,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
  testId: string;
}): JSX.Element {
  return (
    <button
      type="button"
      data-testid={testId}
      data-active={active ? "true" : "false"}
      onClick={onClick}
      style={{
        padding: "2px 8px",
        borderRadius: 999,
        background: active ? "rgba(59,130,246,0.22)" : "rgba(30,41,59,0.4)",
        border: active
          ? "1px solid rgba(96,165,250,0.6)"
          : "1px solid rgba(100,116,139,0.3)",
        color: active ? "#bfdbfe" : "#cbd5e1",
        fontSize: 10,
        fontWeight: 600,
        letterSpacing: "0.04em",
        cursor: "pointer",
        textTransform: "lowercase",
      }}
    >
      {children}
    </button>
  );
}

export function EventTapeFilters(): JSX.Element {
  const filter = useSwarmStore(selectEventFilter);
  const activeRun = useSwarmStore(selectActiveRun);
  const currentPlan = useSwarmStore(selectCurrentPlan);

  const capabilities = useMemo<string[]>(() => {
    const source = activeRun?.dag.nodes ?? currentPlan?.dag.nodes ?? [];
    return Array.from(new Set(source.map((n) => n.capability_id))).sort();
  }, [activeRun, currentPlan]);

  return (
    <div
      data-testid="event-tape-filters"
      style={{
        display: "flex",
        flexWrap: "wrap",
        gap: 6,
        padding: "6px 8px",
        borderBottom: "1px solid rgba(100,116,139,0.2)",
        background: "rgba(10,14,26,0.5)",
        alignItems: "center",
      }}
    >
      <span
        style={{
          fontSize: 9,
          color: "#64748b",
          textTransform: "uppercase",
          letterSpacing: "0.12em",
          marginRight: 4,
        }}
      >
        filter
      </span>

      <Chip
        testId="filter-agent-all"
        active={filter.byAgent === null}
        onClick={(): void => setEventFilter({ byAgent: null })}
      >
        all agents
      </Chip>
      {capabilities.map((cap) => (
        <Chip
          key={`cap-${cap}`}
          testId={`filter-agent-${cap}`}
          active={filter.byAgent === cap}
          onClick={(): void =>
            setEventFilter({ byAgent: filter.byAgent === cap ? null : cap })
          }
        >
          {cap}
        </Chip>
      ))}

      <span style={{ width: 8 }} />

      <Chip
        testId="filter-type-all"
        active={filter.byEventType === null}
        onClick={(): void => setEventFilter({ byEventType: null })}
      >
        all types
      </Chip>
      {EVENT_CATEGORIES.map((cat: EventCategory) => (
        <Chip
          key={`cat-${cat}`}
          testId={`filter-type-${cat}`}
          active={filter.byEventType === cat}
          onClick={(): void =>
            setEventFilter({ byEventType: filter.byEventType === cat ? null : cat })
          }
        >
          {cat}
        </Chip>
      ))}

      <span style={{ width: 8 }} />

      <Chip
        testId="filter-severity-only"
        active={filter.severityOnly}
        onClick={(): void => setEventFilter({ severityOnly: !filter.severityOnly })}
      >
        failures only
      </Chip>

      {(filter.byAgent !== null || filter.byEventType !== null || filter.severityOnly) && (
        <button
          type="button"
          data-testid="filter-clear"
          onClick={clearEventFilter}
          style={{
            marginLeft: "auto",
            background: "transparent",
            border: "none",
            color: "#94a3b8",
            fontSize: 10,
            cursor: "pointer",
            textDecoration: "underline",
          }}
        >
          clear filters
        </button>
      )}
    </div>
  );
}

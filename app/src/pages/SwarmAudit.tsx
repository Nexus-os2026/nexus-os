/**
 * Track C #1: Swarm Audit Viewer.
 *
 * Read-only display of `swarm_audit_tail`. Single-column layout
 * modelled on `Audit.tsx`. v1 scope: filter chips on event_kind +
 * node_id substring, client-side time-range filter, manual refresh.
 *
 * Data path: one-shot `getSwarmAuditTail(runId)` on mount and on
 * explicit refresh. No live-append (Bug AM tracks live updates from
 * the swarm:event channel). Audit data is in-memory per-process —
 * tails are lost on desktop restart (Bug AL tracks SQLite-backed
 * persistence).
 */

import { useEffect, useMemo, useState } from "react";
import {
  AUDIT_EVENT_KINDS,
  auditKindCategory,
} from "../lib/swarm/audit_kind_category";
import { getSwarmAuditTail } from "../lib/swarm/commands";
import type { EventCategory } from "../lib/swarm/event_category";
import { selectActiveRun } from "../lib/swarm/selectors";
import { useSwarmStore } from "../lib/swarm/store";
import type {
  AuditEntry,
  AuditEventKind,
  AuditTimestamp,
} from "../lib/swarm/types";

type TimeRange = "all" | "1m" | "5m" | "15m" | "1h";

const TIME_RANGE_OPTIONS: ReadonlyArray<{
  readonly value: TimeRange;
  readonly label: string;
  readonly windowMs: number | null;
}> = Object.freeze([
  { value: "all", label: "All", windowMs: null },
  { value: "1m", label: "Last 1m", windowMs: 60_000 },
  { value: "5m", label: "Last 5m", windowMs: 5 * 60_000 },
  { value: "15m", label: "Last 15m", windowMs: 15 * 60_000 },
  { value: "1h", label: "Last hour", windowMs: 60 * 60_000 },
]);

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

function timestampToMs(t: AuditTimestamp): number {
  return t.secs_since_epoch * 1000 + Math.floor(t.nanos_since_epoch / 1_000_000);
}

function formatTimestamp(t: AuditTimestamp): string {
  const d = new Date(timestampToMs(t));
  return d.toISOString().replace("T", " ").slice(0, 23);
}

function isUuidLike(s: string): boolean {
  // Accept anything that looks like a UUID; the backend will reject
  // truly malformed input. We're not doing validation — we're
  // gating the fetch button.
  return /^[0-9a-fA-F-]{8,}$/.test(s.trim());
}

interface FilterState {
  readonly kinds: ReadonlySet<AuditEventKind>;
  readonly nodeSubstring: string;
  readonly timeRange: TimeRange;
}

function filterEntries(
  entries: readonly AuditEntry[],
  filters: FilterState,
  nowMs: number,
): readonly AuditEntry[] {
  const cutoffMs = (() => {
    const opt = TIME_RANGE_OPTIONS.find((o) => o.value === filters.timeRange);
    return opt && opt.windowMs !== null ? nowMs - opt.windowMs : null;
  })();
  const nodeNeedle = filters.nodeSubstring.trim().toLowerCase();
  const kindsActive = filters.kinds.size > 0;
  return entries.filter((e) => {
    if (kindsActive && !filters.kinds.has(e.event_kind)) return false;
    if (nodeNeedle.length > 0) {
      if (e.node_id === null) return false;
      if (!e.node_id.toLowerCase().includes(nodeNeedle)) return false;
    }
    if (cutoffMs !== null && timestampToMs(e.timestamp) < cutoffMs) return false;
    return true;
  });
}

function AuditRow({ entry, seq }: { readonly entry: AuditEntry; readonly seq: number }): JSX.Element {
  const [expanded, setExpanded] = useState(false);
  const cat = auditKindCategory(entry.event_kind);
  const pill = PILL_COLORS[cat];
  return (
    <div
      data-testid={`swarm-audit-row-${seq}`}
      data-event-kind={entry.event_kind}
      data-category={cat}
      onClick={(): void => setExpanded((x) => !x)}
      style={{
        padding: "6px 10px",
        background: expanded ? "rgba(30,41,59,0.55)" : "rgba(15,23,42,0.35)",
        borderBottom: "1px solid rgba(100,116,139,0.15)",
        cursor: "pointer",
        color: "#cbd5e1",
        fontSize: 11,
        fontFamily: "var(--font-mono, monospace)",
      }}
    >
      <div style={{ display: "flex", gap: 8, alignItems: "baseline" }}>
        <span style={{ color: "#64748b", width: 168, flexShrink: 0 }}>
          {formatTimestamp(entry.timestamp)}
        </span>
        <span
          data-testid={`swarm-audit-pill-${seq}`}
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
          {entry.event_kind}
        </span>
        <span
          style={{
            color: entry.node_id !== null ? "#bfdbfe" : "#475569",
            width: 120,
            flexShrink: 0,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {entry.node_id ?? "—"}
        </span>
        <span
          style={{
            flex: 1,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {entry.payload_summary}
        </span>
        <span style={{ color: "#475569", flexShrink: 0 }}>
          {entry.ticket_nonce.slice(0, 8)}…
        </span>
      </div>
      {expanded && (
        <pre
          data-testid={`swarm-audit-json-${seq}`}
          style={{
            margin: "6px 0 0 176px",
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
          {JSON.stringify(entry, null, 2)}
        </pre>
      )}
    </div>
  );
}

export default function SwarmAudit(): JSX.Element {
  const activeRun = useSwarmStore(selectActiveRun);
  const activeRunId = activeRun?.run_id ?? null;

  const [runIdInput, setRunIdInput] = useState<string>(activeRunId ?? "");
  const [entries, setEntries] = useState<readonly AuditEntry[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [lastFetchedRunId, setLastFetchedRunId] = useState<string | null>(null);

  const [selectedKinds, setSelectedKinds] = useState<ReadonlySet<AuditEventKind>>(
    () => new Set<AuditEventKind>(),
  );
  const [nodeSubstring, setNodeSubstring] = useState<string>("");
  const [timeRange, setTimeRange] = useState<TimeRange>("all");

  const fetchTail = async (runId: string): Promise<void> => {
    const trimmed = runId.trim();
    if (!isUuidLike(trimmed)) {
      setError("Enter a valid run UUID");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const result = await getSwarmAuditTail(trimmed);
      setEntries(result);
      setLastFetchedRunId(trimmed);
    } catch (e) {
      setError(typeof e === "string" ? e : (e as Error).message ?? String(e));
      setEntries([]);
    } finally {
      setLoading(false);
    }
  };

  // One-shot fetch on mount when an active run is available.
  useEffect(() => {
    if (activeRunId !== null && lastFetchedRunId === null) {
      void fetchTail(activeRunId);
    }
    // We deliberately do NOT depend on activeRunId — the user picks
    // when to refresh; auto-refetch on run-change would defeat the
    // "read-only snapshot" UX.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const filters: FilterState = useMemo(
    () => ({ kinds: selectedKinds, nodeSubstring, timeRange }),
    [selectedKinds, nodeSubstring, timeRange],
  );

  const filtered = useMemo(
    () => filterEntries(entries, filters, Date.now()),
    [entries, filters],
  );

  const filtersActive =
    selectedKinds.size > 0 || nodeSubstring.trim().length > 0 || timeRange !== "all";

  const toggleKind = (kind: AuditEventKind): void => {
    setSelectedKinds((prev) => {
      const next = new Set(prev);
      if (next.has(kind)) {
        next.delete(kind);
      } else {
        next.add(kind);
      }
      return next;
    });
  };

  return (
    <div
      data-testid="swarm-audit-page"
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 12,
        padding: "16px 20px",
        height: "100%",
        minHeight: 0,
        color: "#e2e8f0",
        fontFamily: "var(--font-sans, system-ui)",
      }}
    >
      <header>
        <h1 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>Swarm Audit</h1>
        <p style={{ fontSize: 12, color: "#94a3b8", margin: "4px 0 0 0" }}>
          Session-only audit tail. Persists across runs in v2 (Bug AL).
        </p>
      </header>

      {/* Run-id row */}
      <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
        <label
          htmlFor="swarm-audit-run-id"
          style={{ fontSize: 11, color: "#94a3b8" }}
        >
          Run ID
        </label>
        <input
          id="swarm-audit-run-id"
          data-testid="swarm-audit-run-input"
          type="text"
          value={runIdInput}
          onChange={(e): void => setRunIdInput(e.target.value)}
          placeholder={activeRunId ?? "Paste a swarm run UUID"}
          style={{
            flex: 1,
            minWidth: 280,
            padding: "6px 10px",
            background: "rgba(15,23,42,0.6)",
            border: "1px solid rgba(100,116,139,0.45)",
            borderRadius: 4,
            color: "#e2e8f0",
            fontFamily: "var(--font-mono, monospace)",
            fontSize: 12,
          }}
        />
        <button
          data-testid="swarm-audit-use-active"
          type="button"
          disabled={activeRunId === null}
          onClick={(): void => {
            if (activeRunId !== null) setRunIdInput(activeRunId);
          }}
          style={buttonStyle(activeRunId === null)}
        >
          Use active run
        </button>
        <button
          data-testid="swarm-audit-refresh"
          type="button"
          disabled={loading || runIdInput.trim().length === 0}
          onClick={(): void => {
            void fetchTail(runIdInput);
          }}
          style={buttonStyle(loading || runIdInput.trim().length === 0)}
        >
          {loading ? "Loading…" : "Refresh"}
        </button>
      </div>

      {/* Filter chips row */}
      <div
        style={{
          display: "flex",
          gap: 8,
          alignItems: "center",
          flexWrap: "wrap",
          padding: "8px 10px",
          background: "rgba(15,23,42,0.45)",
          border: "1px solid rgba(100,116,139,0.2)",
          borderRadius: 4,
        }}
      >
        <span style={{ fontSize: 11, color: "#94a3b8" }}>Kind:</span>
        {AUDIT_EVENT_KINDS.map((kind) => {
          const active = selectedKinds.has(kind);
          const cat = auditKindCategory(kind);
          const pill = PILL_COLORS[cat];
          return (
            <button
              key={kind}
              data-testid={`swarm-audit-chip-${kind}`}
              data-active={active}
              type="button"
              onClick={(): void => toggleKind(kind)}
              style={{
                padding: "2px 8px",
                borderRadius: 999,
                fontSize: 10,
                fontWeight: 600,
                letterSpacing: "0.04em",
                textTransform: "uppercase",
                cursor: "pointer",
                background: active ? pill.bg : "transparent",
                color: active ? pill.fg : "#64748b",
                border: `1px solid ${active ? pill.border : "rgba(100,116,139,0.3)"}`,
              }}
            >
              {kind}
            </button>
          );
        })}
        <span style={{ width: 1, height: 18, background: "rgba(100,116,139,0.3)", margin: "0 4px" }} />
        <label
          htmlFor="swarm-audit-node-filter"
          style={{ fontSize: 11, color: "#94a3b8" }}
        >
          Node:
        </label>
        <input
          id="swarm-audit-node-filter"
          data-testid="swarm-audit-node-filter"
          type="text"
          value={nodeSubstring}
          onChange={(e): void => setNodeSubstring(e.target.value)}
          placeholder="substring"
          style={{
            padding: "4px 8px",
            background: "rgba(15,23,42,0.6)",
            border: "1px solid rgba(100,116,139,0.45)",
            borderRadius: 4,
            color: "#e2e8f0",
            fontFamily: "var(--font-mono, monospace)",
            fontSize: 11,
            width: 160,
          }}
        />
        <span style={{ width: 1, height: 18, background: "rgba(100,116,139,0.3)", margin: "0 4px" }} />
        <label
          htmlFor="swarm-audit-time-range"
          style={{ fontSize: 11, color: "#94a3b8" }}
        >
          Window:
        </label>
        <select
          id="swarm-audit-time-range"
          data-testid="swarm-audit-time-range"
          value={timeRange}
          onChange={(e): void => setTimeRange(e.target.value as TimeRange)}
          style={{
            padding: "4px 8px",
            background: "rgba(15,23,42,0.6)",
            border: "1px solid rgba(100,116,139,0.45)",
            borderRadius: 4,
            color: "#e2e8f0",
            fontSize: 11,
          }}
        >
          {TIME_RANGE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </div>

      {/* Status strip */}
      <div style={{ display: "flex", gap: 12, fontSize: 11, color: "#64748b" }}>
        <span data-testid="swarm-audit-count-total">total: {entries.length}</span>
        <span data-testid="swarm-audit-count-filtered">visible: {filtered.length}</span>
        {lastFetchedRunId !== null && (
          <span data-testid="swarm-audit-last-fetch">
            run: {lastFetchedRunId.slice(0, 8)}…
          </span>
        )}
        {error !== null && (
          <span data-testid="swarm-audit-error" style={{ color: "#fca5a5" }}>
            error: {error}
          </span>
        )}
      </div>

      {/* Body */}
      <div
        data-testid="swarm-audit-list"
        style={{
          flex: 1,
          minHeight: 0,
          overflowY: "auto",
          background: "rgba(0,0,0,0.25)",
          border: "1px solid rgba(100,116,139,0.2)",
          borderRadius: 4,
        }}
      >
        {filtered.length === 0 ? (
          <div
            data-testid="swarm-audit-empty"
            style={{ padding: 20, color: "#64748b", fontSize: 12, textAlign: "center" }}
          >
            {entries.length === 0
              ? "No audit entries for this run."
              : filtersActive
                ? "No matching entries."
                : "No audit entries for this run."}
          </div>
        ) : (
          filtered.map((entry) => (
            <AuditRow key={entry.seq} entry={entry} seq={entry.seq} />
          ))
        )}
      </div>
    </div>
  );
}

function buttonStyle(disabled: boolean): React.CSSProperties {
  return {
    padding: "6px 12px",
    fontSize: 11,
    fontWeight: 500,
    border: "1px solid rgba(100,116,139,0.45)",
    background: disabled ? "rgba(15,23,42,0.3)" : "rgba(15,23,42,0.6)",
    color: disabled ? "#475569" : "#e2e8f0",
    borderRadius: 4,
    cursor: disabled ? "not-allowed" : "pointer",
  };
}

// Re-export so tests can drive `filterEntries` without a render.
export { filterEntries };
export type { FilterState, TimeRange };

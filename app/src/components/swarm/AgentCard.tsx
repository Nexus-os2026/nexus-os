/**
 * AgentCard — one card per currently-Running swarm node.
 *
 * The card subscribes to two store slices: the per-node event slice
 * (for the current phase + latest payload summary) and provider health
 * (for the latency fed into the SlmStatusBadge adapter). Mounting and
 * unmounting is driven entirely by `selectActiveNodes` upstream: when
 * a node transitions to Done / Failed / Skipped, SwarmGrid drops this
 * card and the elapsed-time interval is torn down in cleanup.
 *
 * SlmStatusBadge re-home (Phase 3b): the badge wants an `SlmStatus`
 * shape (loaded / model_id / latency / routing / ram / queries). We
 * derive what we can from the swarm domain — model_id + provider_id
 * from the latest node_started event, latency from providerHealth — and
 * pass 0 for ram/queries which the badge hides. The CSS for the badge
 * lives in the legacy `pages/agents.css`; we side-effect import it
 * here to keep the stylesheet alive now that the old Agents page no
 * longer imports it.
 */

import { useEffect, useMemo, useRef, useState } from "react";
import { cancelSwarmNode } from "../../lib/swarm/commands";
import { useSwarmStore } from "../../lib/swarm/store";
import {
  selectActiveRun,
  selectEventsByNode,
  selectFocusedNodeId,
  selectProviderHealth,
} from "../../lib/swarm/selectors";
import type {
  DagNode,
  ProviderHealth,
  SwarmEvent,
} from "../../lib/swarm/types";
import { SlmStatusBadge } from "../agents/SlmStatusBadge";
import type { GovernanceRouting, SlmStatus } from "../../types";
import "../../pages/agents.css"; // revive the .slm-* classes used by SlmStatusBadge

export interface AgentCardProps {
  node: DagNode;
}

interface NodeProvenance {
  provider_id: string | null;
  model_id: string | null;
}

interface NodeActivity {
  phase: string | null;
  summary: string | null;
}

function latestNodeStarted(events: readonly SwarmEvent[]): NodeProvenance {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    const ev = events[i];
    if (ev.event === "node_started") {
      return { provider_id: ev.provider_id, model_id: ev.model_id };
    }
  }
  return { provider_id: null, model_id: null };
}

function summarizePayload(payload: unknown): string {
  if (payload === null || payload === undefined) return "—";
  if (typeof payload === "string") return truncate(payload, 60);
  if (typeof payload !== "object") return truncate(String(payload), 60);
  // Prefer a tool-call shape if present.
  const obj = payload as Record<string, unknown>;
  if (typeof obj.tool === "string") return `tool: ${truncate(obj.tool, 50)}`;
  const firstKey = Object.keys(obj)[0];
  if (firstKey === undefined) return "{}";
  return truncate(`${firstKey}: ${stringify(obj[firstKey])}`, 60);
}

function stringify(value: unknown): string {
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return `${s.slice(0, max - 1)}…`;
}

function latestActivity(events: readonly SwarmEvent[]): NodeActivity {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    const ev = events[i];
    if (ev.event === "node_event") {
      return { phase: ev.phase, summary: summarizePayload(ev.payload) };
    }
  }
  return { phase: null, summary: null };
}

function countTokens(events: readonly SwarmEvent[]): number | null {
  // No per-node token accounting exists in the event vocabulary today;
  // BudgetUpdate is run-scoped. Surfacing "—" until Phase 4 threads
  // per-node token deltas through NodeEvent.
  void events;
  return null;
}

function formatElapsed(ms: number): string {
  if (ms < 0) return "0s";
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  const rem = s % 60;
  return `${m}m ${rem}s`;
}

const LOCAL_PROVIDERS: ReadonlySet<string> = new Set(["ollama", "codex-cli"]);

function routingFor(providerId: string | null): GovernanceRouting {
  if (providerId === null) return "cloud";
  return LOCAL_PROVIDERS.has(providerId) ? "local" : "cloud";
}

function adaptToSlmStatus(
  providerId: string | null,
  modelId: string | null,
  health: readonly ProviderHealth[],
): SlmStatus {
  const ph = providerId ? health.find((h) => h.provider_id === providerId) : undefined;
  const latency = ph && ph.latency_ms !== null ? ph.latency_ms : 0;
  return {
    loaded: modelId !== null,
    model_id: modelId,
    ram_usage_mb: 0,
    avg_latency_ms: latency,
    total_queries: 0,
    governance_routing: routingFor(providerId),
  };
}

function useElapsed(startedMsRef: React.MutableRefObject<number>): number {
  const [now, setNow] = useState<number>(() => Date.now());
  useEffect(() => {
    const handle = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(handle);
  }, []);
  return Math.max(0, now - startedMsRef.current);
}

export function AgentCard({ node }: AgentCardProps): JSX.Element {
  // Memoize the factory selector so its internal cache survives across
  // renders — a fresh closure per render would bust the cache and the
  // array identity on every tick, causing useSyncExternalStore to loop.
  const eventsByNodeSelector = useMemo(() => selectEventsByNode(node.id), [node.id]);
  const nodeEvents = useSwarmStore(eventsByNodeSelector);
  const providerHealth = useSwarmStore(selectProviderHealth);
  const activeRun = useSwarmStore(selectActiveRun);
  const focusedId = useSwarmStore(selectFocusedNodeId);

  const provenance = useMemo(() => latestNodeStarted(nodeEvents), [nodeEvents]);
  const activity = useMemo(() => latestActivity(nodeEvents), [nodeEvents]);
  const slmStatus = useMemo(
    () => adaptToSlmStatus(provenance.provider_id, provenance.model_id, providerHealth),
    [provenance, providerHealth],
  );
  const tokens = useMemo(() => countTokens(nodeEvents), [nodeEvents]);

  const startedAtRef = useRef<number>(activeRun?.started_at_ms ?? Date.now());
  const elapsedMs = useElapsed(startedAtRef);

  const [cancelling, setCancelling] = useState(false);
  const runId = activeRun?.run_id ?? null;

  const onCancel = (): void => {
    if (!runId || cancelling) return;
    setCancelling(true);
    void cancelSwarmNode(runId, node.id).finally(() => setCancelling(false));
  };

  const focused = focusedId === node.id;

  return (
    <section
      data-testid={`agent-card-${node.id}`}
      data-focused={focused ? "true" : "false"}
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 8,
        padding: 12,
        minWidth: 260,
        minHeight: 180,
        borderRadius: 10,
        border: focused ? "1px solid rgba(59,130,246,0.9)" : "1px solid rgba(100,116,139,0.25)",
        background: "rgba(15,23,42,0.55)",
        color: "#e2e8f0",
      }}
    >
      <header style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", gap: 8 }}>
        <span
          style={{
            fontSize: 12,
            fontWeight: 700,
            textTransform: "uppercase",
            letterSpacing: "0.08em",
            color: "#cbd5e1",
          }}
          title={node.capability_id}
        >
          {node.capability_id}
        </span>
        <span
          style={{
            fontSize: 10,
            color: "#64748b",
            fontFamily: "var(--font-mono, monospace)",
          }}
        >
          {node.id}
        </span>
      </header>

      <div
        data-testid={`agent-card-phase-${node.id}`}
        style={{
          fontSize: 11,
          color: "#94a3b8",
          fontFamily: "var(--font-mono, monospace)",
          minHeight: 14,
        }}
      >
        phase: <span style={{ color: activity.phase ? "#bfdbfe" : "#64748b" }}>{activity.phase ?? "…"}</span>
      </div>

      <div
        data-testid={`agent-card-summary-${node.id}`}
        style={{
          fontSize: 11,
          color: "#cbd5e1",
          fontFamily: "var(--font-mono, monospace)",
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
          minHeight: 14,
        }}
        title={activity.summary ?? ""}
      >
        {activity.summary ?? "—"}
      </div>

      <div data-testid={`agent-card-slm-${node.id}`}>
        <SlmStatusBadge status={slmStatus} />
      </div>

      <footer
        style={{
          display: "flex",
          gap: 10,
          alignItems: "center",
          justifyContent: "space-between",
          fontSize: 11,
          color: "#94a3b8",
          marginTop: "auto",
        }}
      >
        <span data-testid={`agent-card-tokens-${node.id}`}>
          tokens: {tokens === null ? "—" : tokens}
        </span>
        <span data-testid={`agent-card-elapsed-${node.id}`}>{formatElapsed(elapsedMs)}</span>
        <button
          type="button"
          data-testid={`agent-card-cancel-${node.id}`}
          onClick={onCancel}
          disabled={cancelling || !runId}
          style={{
            padding: "4px 10px",
            borderRadius: 6,
            background: "rgba(220,38,38,0.18)",
            border: "1px solid rgba(248,113,113,0.45)",
            color: "#fca5a5",
            fontSize: 11,
            fontWeight: 600,
            cursor: cancelling || !runId ? "not-allowed" : "pointer",
            opacity: cancelling || !runId ? 0.55 : 1,
          }}
        >
          {cancelling ? "cancelling…" : "cancel"}
        </button>
      </footer>
    </section>
  );
}

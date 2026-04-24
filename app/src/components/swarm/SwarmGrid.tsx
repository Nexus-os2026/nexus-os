/**
 * SwarmGrid — auto-fill grid of AgentCards, one per Running node.
 *
 * Three visual states:
 *   1. Idle     — no plan and no active run. Invitation to submit an intent.
 *   2. Staging  — plan proposed or approved, but no node has reported
 *                  Running yet (brief gap between PlanApproved and first
 *                  node_started).
 *   3. Active   — at least one Running node. Cards render in the grid.
 */

import { useSwarmStore } from "../../lib/swarm/store";
import {
  selectActiveNodes,
  selectActiveRun,
  selectCurrentPlan,
} from "../../lib/swarm/selectors";
import { AgentCard } from "./AgentCard";

function EmptyState({ label, testId }: { label: string; testId: string }): JSX.Element {
  return (
    <div
      data-testid={testId}
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        height: "100%",
        width: "100%",
        color: "#475569",
        fontSize: 13,
        fontFamily: "var(--font-mono, monospace)",
      }}
    >
      {label}
    </div>
  );
}

export function SwarmGrid(): JSX.Element {
  const activeNodes = useSwarmStore(selectActiveNodes);
  const activeRun = useSwarmStore(selectActiveRun);
  const currentPlan = useSwarmStore(selectCurrentPlan);

  const hasAnyState = activeRun !== null || currentPlan !== null;

  return (
    <div
      data-testid="swarm-grid"
      style={{
        width: "100%",
        height: "100%",
        minHeight: 0,
        minWidth: 0,
        padding: 0,
        border: "1px solid rgba(100,116,139,0.25)",
        borderRadius: 10,
        background: "rgba(10,14,26,0.6)",
        overflow: "auto",
      }}
    >
      {!hasAnyState && (
        <EmptyState
          label="No agents running. Submit a plan to begin."
          testId="swarm-grid-empty"
        />
      )}
      {hasAnyState && activeNodes.length === 0 && (
        <EmptyState
          label="Waiting for first node to start…"
          testId="swarm-grid-waiting"
        />
      )}
      {activeNodes.length > 0 && (
        <div
          style={{
            display: "grid",
            gridTemplateColumns: "repeat(auto-fill, minmax(260px, 1fr))",
            gap: 12,
            padding: 12,
          }}
        >
          {activeNodes.map((n) => (
            <AgentCard key={n.id} node={n} />
          ))}
        </div>
      )}
    </div>
  );
}

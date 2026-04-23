/**
 * PlanApprovalCard — the approve/reject surface for a proposed swarm.
 *
 * Visibility is controlled by the parent (Agents.tsx) which couples it to
 * `selectIsPlanPending`. The card content comes from the full
 * PlannedSwarmJson returned by `planSwarm` (lifted up from DirectorConsole
 * via `onPlanReady`) because the `plan_proposed` event alone doesn't
 * carry ticket_id / budget_hash / privacy_envelope.
 */

import { useState } from "react";
import { approveSwarm, rejectSwarm } from "../../lib/swarm/commands";
import type { PlannedSwarmJson, PrivacyClass } from "../../lib/swarm/types";

export interface PlanApprovalCardProps {
  plan: PlannedSwarmJson;
  onDecisionSent: () => void;
}

const PRIVACY_COLORS: Record<PrivacyClass, { bg: string; fg: string; border: string }> = {
  Public: {
    bg: "rgba(22,163,74,0.18)",
    fg: "#4ade80",
    border: "rgba(74,222,128,0.45)",
  },
  Sensitive: {
    bg: "rgba(234,179,8,0.18)",
    fg: "#fbbf24",
    border: "rgba(251,191,36,0.45)",
  },
  StrictLocal: {
    bg: "rgba(220,38,38,0.18)",
    fg: "#f87171",
    border: "rgba(248,113,113,0.45)",
  },
};

function short(id: string, n = 8): string {
  return id.length > n ? id.slice(0, n) : id;
}

export function PlanApprovalCard({ plan, onDecisionSent }: PlanApprovalCardProps): JSX.Element {
  const [acting, setActing] = useState<"approve" | "reject" | null>(null);
  const privacy = PRIVACY_COLORS[plan.privacy_envelope];
  const nodeCount = plan.dag.nodes.length;
  const edgeCount = plan.dag.edges.length;

  const approveTooltip = `Execute ${nodeCount} node${nodeCount === 1 ? "" : "s"} across the DAG (${edgeCount} edge${edgeCount === 1 ? "" : "s"}).`;
  const rejectTooltip = "Discard this plan. No providers are contacted.";

  const onApprove = async (): Promise<void> => {
    if (acting) return;
    setActing("approve");
    try {
      await approveSwarm(plan.ticket_id);
      onDecisionSent();
    } finally {
      setActing(null);
    }
  };

  const onReject = async (): Promise<void> => {
    if (acting) return;
    setActing("reject");
    try {
      await rejectSwarm(plan.ticket_id, undefined);
      onDecisionSent();
    } finally {
      setActing(null);
    }
  };

  return (
    <section
      data-testid="plan-approval-card"
      role="dialog"
      aria-label="Plan approval"
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 10,
        padding: 12,
        borderRadius: 10,
        background: "rgba(15,23,42,0.7)",
        border: "1px solid rgba(100,116,139,0.35)",
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 10 }}>
        <span
          style={{
            fontSize: 10,
            textTransform: "uppercase",
            letterSpacing: "0.12em",
            color: "#94a3b8",
          }}
        >
          Plan pending approval
        </span>
        <span
          data-testid="plan-privacy-badge"
          data-privacy={plan.privacy_envelope}
          style={{
            padding: "2px 8px",
            borderRadius: 999,
            background: privacy.bg,
            color: privacy.fg,
            border: `1px solid ${privacy.border}`,
            fontSize: 10,
            fontWeight: 700,
            letterSpacing: "0.06em",
          }}
        >
          {plan.privacy_envelope}
        </span>
      </div>

      <dl
        style={{
          display: "grid",
          gridTemplateColumns: "auto 1fr",
          columnGap: 10,
          rowGap: 4,
          margin: 0,
          fontSize: 12,
          color: "#cbd5e1",
          fontFamily: "var(--font-mono, monospace)",
        }}
      >
        <dt style={{ color: "#64748b" }}>ticket</dt>
        <dd data-testid="plan-ticket-id-short" style={{ margin: 0 }} title={plan.ticket_id}>
          {short(plan.ticket_id)}…
        </dd>
        <dt style={{ color: "#64748b" }}>budget</dt>
        <dd style={{ margin: 0 }} title={plan.budget_hash}>
          {short(plan.budget_hash)}…
        </dd>
        <dt style={{ color: "#64748b" }}>nodes</dt>
        <dd style={{ margin: 0 }}>{nodeCount}</dd>
        <dt style={{ color: "#64748b" }}>edges</dt>
        <dd style={{ margin: 0 }}>{edgeCount}</dd>
      </dl>

      <div style={{ display: "flex", gap: 8 }}>
        <button
          type="button"
          data-testid="plan-approve"
          onClick={(): void => {
            void onApprove();
          }}
          disabled={acting !== null}
          title={approveTooltip}
          style={{
            padding: "6px 14px",
            borderRadius: 8,
            background: "rgba(22,163,74,0.18)",
            border: "1px solid rgba(74,222,128,0.45)",
            color: "#86efac",
            fontSize: 12,
            fontWeight: 700,
            cursor: acting === null ? "pointer" : "not-allowed",
            opacity: acting === null ? 1 : 0.55,
          }}
        >
          {acting === "approve" ? "approving…" : "Approve"}
        </button>
        <button
          type="button"
          data-testid="plan-reject"
          onClick={(): void => {
            void onReject();
          }}
          disabled={acting !== null}
          title={rejectTooltip}
          style={{
            padding: "6px 14px",
            borderRadius: 8,
            background: "rgba(220,38,38,0.18)",
            border: "1px solid rgba(248,113,113,0.45)",
            color: "#fca5a5",
            fontSize: 12,
            fontWeight: 700,
            cursor: acting === null ? "pointer" : "not-allowed",
            opacity: acting === null ? 1 : 0.55,
          }}
        >
          {acting === "reject" ? "rejecting…" : "Reject"}
        </button>
      </div>
    </section>
  );
}

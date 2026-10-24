import { useState } from "react";
import { ShieldAlert, X } from "lucide-react";
import type { P3Commitment } from "../api/backend";
import {
  denyCommitment,
  dismissResult,
  reviewInNativeWindow,
  stopRun,
  useOwnerActions,
  usePendingOwnerApprovals,
  type ActionResult,
  type PendingOwnerApprovals,
} from "../hooks/usePendingOwnerApprovals";

// A contextual presenter for Phase Three R2 commitments. Every text shown is
// backend data rendered as plain text; nothing here is authority. The Review
// button sends only the listed commitment id to the existing `p3_approve`,
// and the backend's native window is the only place the effect is allowed.

export const PREVIEW_CAPTION = "Preview only. The Nexus native approval window contains the authoritative details.";

export const LEGACY_CONSENT_LABEL =
  "Desktop consent (unverified) — does not authorize governed real-world effects.";

/** Legacy consent records carry this label wherever they are shown. */
export function LegacyConsentLabel({ style }: { style?: React.CSSProperties }) {
  return (
    <div data-testid="legacy-consent-label" style={{ fontSize: 11, color: "#fbbf24", ...style }}>
      {LEGACY_CONSENT_LABEL}
    </div>
  );
}

/**
 * Pages that already show these commitments with their own controls: the
 * global card stays out of them so a request never appears twice.
 */
export const GLOBAL_CARD_SUPPRESSED_PAGES: readonly string[] = ["governed-control", "approvals"];

const RED = "#ef4444";
const MUTED = "#94a3b8";
const SUMMARY_LINES_SHOWN = 6;
const UNCONSUMED = new Set(["prepared", "authorized", "executing"]);

const STATE_LABELS: Record<string, string> = {
  prepared: "Still waiting (prepared)",
  authorized: "In progress (authorized)",
  executing: "In progress (executing)",
  succeeded: "Succeeded",
  failed: "Failed",
  cancelled: "Cancelled",
  denied: "Denied",
  expired: "Expired",
  revoked: "Revoked",
};

const REFUSALS: Record<string, string> = {
  expired: "Expired",
  not_pending: "No longer pending",
  declined: "Declined in the Nexus approval window",
  run_cancelled: "Run stopped",
  run_not_active: "Run stopped",
  stale_policy_generation: "Revoked: the policy changed",
  grant_not_live: "Revoked: its grant is no longer live",
  no_covering_grant: "Revoked: no grant covers it",
  target_changed: "Changed since it was requested",
  parameters_changed: "Changed since it was requested",
  approval_mismatch: "Changed since it was requested",
  emergency_stopped: "Emergency stop is active",
  unknown_commitment: "Unknown to Nexus",
};

/** A plain label for a backend refusal; the backend's text is always shown beside it. */
export function refusalLabel(error: string): string {
  const refused = /refused: ([a-z_]+)/.exec(error)?.[1];
  if (refused && REFUSALS[refused]) return REFUSALS[refused];
  if (/unknown commitment/.test(error)) return REFUSALS.unknown_commitment;
  return "Refused or failed";
}

function stateLabel(state: string): string {
  return STATE_LABELS[state] ?? state;
}

function buttonStyle(color: string, disabled: boolean): React.CSSProperties {
  return {
    borderRadius: 6,
    padding: "5px 10px",
    fontSize: 12,
    fontWeight: 600,
    cursor: disabled ? "not-allowed" : "pointer",
    opacity: disabled ? 0.5 : 1,
    background: `${color}22`,
    color,
    border: `1px solid ${color}66`,
  };
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ display: "flex", gap: 6, fontSize: 12, minWidth: 0 }}>
      <span style={{ color: MUTED, flexShrink: 0, width: 64 }}>{label}</span>
      <span style={{ color: "#e2e8f0", fontFamily: "monospace", wordBreak: "break-all" }}>{value}</span>
    </div>
  );
}

export interface ContextualApprovalCardProps {
  commitment: P3Commitment;
  /** This commitment's review is outstanding in the Nexus window. */
  reviewing: boolean;
  /** That review is still being checked against a fresh read, not yet sent. */
  verifying?: boolean;
  /** Another review is outstanding: one native window at a time. */
  reviewBlocked: boolean;
  denying: boolean;
  onReview: () => void;
  onDeny: () => void;
  onStop: () => void;
}

/** One pending R2 commitment, as the backend listed it. */
export function ContextualApprovalCard({
  commitment,
  reviewing,
  verifying = false,
  reviewBlocked,
  denying,
  onReview,
  onDeny,
  onStop,
}: ContextualApprovalCardProps) {
  const waiting = commitment.state === "prepared";
  const shown = commitment.summary.slice(0, SUMMARY_LINES_SHOWN);
  const more = commitment.summary.length - shown.length;
  return (
    <div
      data-testid="governed-approval-card"
      data-commitment={commitment.id}
      style={{
        background: "rgba(10, 22, 40, 0.97)",
        border: `1px solid ${RED}55`,
        borderRadius: 10,
        padding: "12px 14px",
        marginBottom: 8,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
        <ShieldAlert size={15} style={{ color: RED, flexShrink: 0 }} />
        <span style={{ fontWeight: 700, fontSize: 13, color: "#e2e8f0" }}>Governed approval waiting</span>
        <span
          style={{ marginLeft: "auto", fontSize: 11, fontWeight: 700, color: RED, border: `1px solid ${RED}88`, borderRadius: 4, padding: "0 6px" }}
        >
          {commitment.class}
        </span>
      </div>
      <Field label="Agent" value={commitment.agent} />
      <Field label="Run" value={commitment.run} />
      <Field label="Effect" value={`${commitment.class} · ${commitment.kind}`} />
      <Field label="Operation" value={commitment.operation} />
      <Field label="Target" value={commitment.target} />
      <Field label="Binding" value={commitment.binding} />
      {shown.length > 0 && (
        <div
          data-testid="governed-approval-summary"
          style={{ marginTop: 6, padding: "6px 8px", background: "rgba(0,0,0,0.3)", borderRadius: 6 }}
        >
          {shown.map((line, index) => (
            <div key={index} style={{ fontSize: 11, color: "#d1d5db", fontFamily: "monospace", whiteSpace: "pre-wrap", wordBreak: "break-all" }}>
              {line}
            </div>
          ))}
          {more > 0 && <div style={{ fontSize: 11, color: MUTED }}>{more} more lines in the Nexus approval window</div>}
        </div>
      )}
      <div style={{ fontSize: 11, color: "#fbbf24", marginTop: 6 }}>{PREVIEW_CAPTION}</div>
      <div style={{ fontSize: 11, color: MUTED, marginTop: 2 }}>
        Nexus checks this request again when you review it; it may already have expired (a request lasts at most 15
        minutes).
      </div>
      {reviewing && waiting && verifying && (
        <div style={{ fontSize: 12, color: "#e2e8f0", marginTop: 6 }}>
          Checking with Nexus that this request is still waiting…
        </div>
      )}
      {reviewing && waiting && !verifying && (
        <div style={{ fontSize: 12, color: "#e2e8f0", marginTop: 6 }}>
          Waiting for your answer in the Nexus approval window…
        </div>
      )}
      {/* Once the backend no longer lists it prepared, its state is shown, never an assumed answer. */}
      {!waiting && (
        <div style={{ fontSize: 12, color: MUTED, marginTop: 6 }}>Nexus lists this request as {commitment.state}.</div>
      )}
      <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginTop: 8 }}>
        <button
          type="button"
          style={buttonStyle(RED, !waiting || reviewing || reviewBlocked)}
          disabled={!waiting || reviewing || reviewBlocked}
          onClick={onReview}
        >
          Review in Nexus approval window
        </button>
        <button type="button" style={buttonStyle(MUTED, !waiting || denying)} disabled={!waiting || denying} onClick={onDeny}>
          Deny
        </button>
        {/* Stops are never held by a review in flight. */}
        <button type="button" style={buttonStyle("#f97316", false)} onClick={onStop}>
          Stop this run
        </button>
      </div>
    </div>
  );
}

const ACTION_NAMES: Record<ActionResult["action"], string> = {
  review: "Review",
  deny: "Deny",
  stop: "Stop this run",
};

/** What the backend reported after one of the owner's actions. */
function ResultNotice({ result }: { result: ActionResult }) {
  const c = result.commitment;
  return (
    <div
      data-testid="governed-approval-result"
      style={{ background: "rgba(10, 22, 40, 0.97)", border: `1px solid ${MUTED}44`, borderRadius: 8, padding: "8px 12px", marginBottom: 8, fontSize: 12 }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
        <span style={{ fontWeight: 600, color: "#e2e8f0" }}>
          {ACTION_NAMES[result.action]}: {c.operation} {c.target}
        </span>
        <button
          type="button"
          aria-label="Dismiss"
          onClick={() => dismissResult(result.key)}
          style={{ marginLeft: "auto", background: "none", border: "none", color: MUTED, cursor: "pointer", padding: 0 }}
        >
          <X size={12} />
        </button>
      </div>
      {result.error !== null && (
        <div style={{ color: "#f87171", marginTop: 2 }}>
          Nexus refused: {refusalLabel(result.error)} <span style={{ fontFamily: "monospace" }}>({result.error})</span>
        </div>
      )}
      {result.notSent !== null ? (
        <div data-testid="governed-approval-not-sent" style={{ color: "#fbbf24", marginTop: 2 }}>
          Review not sent: {result.notSent}. The Nexus approval window was not requested.
          {result.reread === "listed" && result.backendState !== null
            ? ` Nexus now reports: ${stateLabel(result.backendState)}.`
            : ""}
        </div>
      ) : (
        <div style={{ color: "#cbd5e1", marginTop: 2 }}>
          {result.reread === "listed" && result.backendState !== null
            ? `Nexus now reports: ${stateLabel(result.backendState)}.`
            : result.reread === "unlisted"
              ? "Nexus no longer lists this request; its outcome is not known here. See Governed Control."
              : "Nexus could not be read again; the outcome is unknown here. See Governed Control."}
        </div>
      )}
      {result.runCancelled !== null && (
        <div style={{ color: "#cbd5e1" }}>Nexus lists the run as {result.runCancelled ? "cancelled" : "not cancelled"}.</div>
      )}
      {result.action === "review" && result.outputText !== null && (
        <pre style={{ whiteSpace: "pre-wrap", wordBreak: "break-all", fontSize: 11, maxHeight: 120, overflow: "auto", margin: "4px 0 0", color: "#d1d5db" }}>
          {result.outputText.slice(0, 400)}
        </pre>
      )}
      {result.action === "review" && result.outputBytes !== null && (
        <div style={{ color: MUTED }}>{result.outputBytes} bytes of data</div>
      )}
      {result.action === "review" && result.error === null && result.notSent === null && (
        <div style={{ color: MUTED, marginTop: 2 }}>
          This result is shown here; Nexus does not report that the requesting agent received it.
        </div>
      )}
    </div>
  );
}

/**
 * The cards for the backend-listed pending commitments, plus the outcomes of
 * the owner's recent actions. A commitment under review stays shown while the
 * backend still lists it unconsumed, so its run can still be stopped.
 */
export function GovernedApprovalsInbox({
  approvals,
  variant,
}: {
  approvals: PendingOwnerApprovals;
  variant: "global" | "page";
}) {
  const actions = useOwnerActions();
  const { availability, snapshot } = approvals;
  const shown = snapshot ? [...snapshot.pending] : [];
  if (snapshot && actions.reviewing !== null && !shown.some((c) => c.id === actions.reviewing)) {
    const underReview = snapshot.byId.get(actions.reviewing);
    if (underReview && UNCONSUMED.has(underReview.state)) shown.unshift(underReview);
  }
  return (
    <div data-testid={`governed-approvals-${variant}`}>
      {availability === "unavailable" && (
        <div
          data-testid="governed-approvals-unavailable"
          title={approvals.reason ?? undefined}
          style={{ fontSize: 12, color: "#fbbf24", background: "rgba(10, 22, 40, 0.92)", border: "1px solid #fbbf2455", borderRadius: 8, padding: "6px 10px", marginBottom: 8 }}
        >
          Governed approvals: status UNAVAILABLE — Nexus could not be read, so waiting requests cannot be shown.
          {variant === "page" && approvals.reason ? ` (${approvals.reason})` : ""}
        </div>
      )}
      {variant === "page" && availability === "loading" && (
        <div style={{ fontSize: 12, color: MUTED }}>Checking Nexus for governed requests…</div>
      )}
      {/* p3_status returns recent runs and their commitments only: what it lists is a
          bounded observation, never proof that nothing else is waiting. */}
      {variant === "page" && availability === "available" && snapshot && (
        <div data-testid="governed-approvals-scope" style={{ fontSize: 12, color: MUTED, marginBottom: 6 }}>
          {shown.length === 0 ? "No governed request is waiting in" : "Shown from"} the status Nexus returned at{" "}
          {new Date(snapshot.readAt).toLocaleTimeString()} ({snapshot.runs.size} recent runs, {snapshot.byId.size}{" "}
          commitments). That status covers recent runs only and may not include every waiting request.
        </div>
      )}
      {actions.results.map((result) => (
        <ResultNotice key={result.key} result={result} />
      ))}
      {shown.map((commitment) => (
        <ContextualApprovalCard
          key={commitment.id}
          commitment={commitment}
          reviewing={actions.reviewing === commitment.id}
          verifying={actions.reviewing === commitment.id && !actions.reviewSent}
          reviewBlocked={actions.reviewing !== null}
          denying={actions.denying.has(commitment.id)}
          onReview={() => void reviewInNativeWindow(commitment, approvals)}
          onDeny={() => void denyCommitment(commitment, approvals)}
          onStop={() => void stopRun(commitment, approvals)}
        />
      ))}
    </div>
  );
}

function GlobalPresenter() {
  const approvals = usePendingOwnerApprovals();
  const actions = useOwnerActions();
  const [collapsedFor, setCollapsedFor] = useState<ReadonlySet<string> | null>(null);
  const pendingIds = approvals.snapshot?.pending.map((c) => c.id) ?? [];
  const nothingToShow =
    approvals.availability === "loading" ||
    (approvals.availability === "available" && pendingIds.length === 0 && actions.reviewing === null && actions.results.length === 0);
  if (nothingToShow) return null;
  // Collapsed until a request appears that was not waiting when the owner collapsed it.
  const collapsed = collapsedFor !== null && pendingIds.every((id) => collapsedFor.has(id));
  return (
    <div
      data-testid="governed-approvals-global-slot"
      role="region"
      aria-label="Governed approvals"
      style={{
        position: "fixed",
        top: 12,
        right: 12,
        zIndex: 9998,
        width: "min(420px, calc(100vw - 24px))",
        maxHeight: "calc(100vh - 24px)",
        overflowY: "auto",
      }}
    >
      {collapsed ? (
        <button
          type="button"
          onClick={() => setCollapsedFor(null)}
          style={{ ...buttonStyle(RED, false), width: "100%", background: "rgba(10, 22, 40, 0.97)" }}
        >
          Governed approval waiting ({pendingIds.length} in the recent status) — show
        </button>
      ) : (
        <>
          {pendingIds.length > 0 && (
            <div style={{ display: "flex", justifyContent: "flex-end", marginBottom: 4 }}>
              <button
                type="button"
                onClick={() => setCollapsedFor(new Set(pendingIds))}
                style={{ background: "rgba(10, 22, 40, 0.9)", border: `1px solid ${MUTED}44`, borderRadius: 6, color: MUTED, fontSize: 11, padding: "2px 8px", cursor: "pointer" }}
              >
                Hide
              </button>
            </div>
          )}
          <GovernedApprovalsInbox approvals={approvals} variant="global" />
        </>
      )}
    </div>
  );
}

/**
 * The one global place pending R2 commitments appear, on every page the owner
 * works in (Chat, Agents, Files and the rest), without changing page. It
 * shows the backend's agent and run identity; it never claims a request
 * belongs to a particular conversation.
 */
export function GlobalGovernedApprovals({ currentPage }: { currentPage?: string }) {
  if (currentPage && GLOBAL_CARD_SUPPRESSED_PAGES.includes(currentPage)) return null;
  return <GlobalPresenter />;
}

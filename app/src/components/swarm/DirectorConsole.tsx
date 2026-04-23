/**
 * DirectorConsole — intent textarea + submit for the swarm plan flow.
 *
 * Flow:
 *   1. User types an intent, hits Submit (or ⌘/Ctrl+Enter).
 *   2. `planSwarm(intent)` is called; the component enters a local
 *      "planning..." state until the store reports `selectIsPlanPending`
 *      (i.e. the `plan_proposed` event has landed).
 *   3. On success the textarea is cleared and the full PlannedSwarmJson
 *      is handed to the parent via `onPlanReady` so the approval card
 *      can render with the correct ticket_id (the plan_proposed event
 *      alone doesn't carry ticket_id — that comes back as the Tauri
 *      command's return value).
 *   4. Escape cancels a pending plan by calling `rejectSwarm(ticket_id)`
 *      — no confirmation prompt, the gesture is the confirmation.
 *
 * The component is disabled while a plan is pending approval OR a run is
 * active. That's the single "don't submit another intent" guard.
 */

import { useEffect, useRef, useState, useCallback } from "react";
import type { KeyboardEvent, ChangeEvent } from "react";
import { planSwarm, rejectSwarm } from "../../lib/swarm/commands";
import type { PlannedSwarmJson } from "../../lib/swarm/types";
import { useSwarmStore } from "../../lib/swarm/store";
import { selectActiveRun, selectIsPlanPending } from "../../lib/swarm/selectors";

export interface DirectorConsoleProps {
  /**
   * Fires once `planSwarm` resolves. Parent lifts this so
   * `PlanApprovalCard` can read the full response (ticket_id,
   * budget_hash, privacy_envelope) which the store doesn't receive
   * through `plan_proposed` alone.
   */
  onPlanReady: (planned: PlannedSwarmJson) => void;
  /**
   * The ticket the parent currently treats as pending — used by the
   * Escape-to-reject gesture. null when no plan is outstanding.
   */
  pendingTicketId: string | null;
  /** Clears the parent-owned pending plan after a successful reject. */
  onPlanCleared: () => void;
}

const MIN_ROWS = 3;
const MAX_ROWS = 8;
const LINE_HEIGHT_PX = 20;

function computeRows(value: string): number {
  const lines = value.split("\n").length;
  return Math.min(MAX_ROWS, Math.max(MIN_ROWS, lines));
}

export function DirectorConsole({
  onPlanReady,
  pendingTicketId,
  onPlanCleared,
}: DirectorConsoleProps): JSX.Element {
  const [text, setText] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  const planPending = useSwarmStore(selectIsPlanPending);
  const activeRun = useSwarmStore(selectActiveRun);
  const disabled = planPending || activeRun !== null;

  // Once the plan_proposed event lands in the store, drop our local
  // "submitting" spinner. The parent still owns the ticket id.
  useEffect(() => {
    if (submitting && planPending) setSubmitting(false);
  }, [submitting, planPending]);

  const handleSubmit = useCallback(async (): Promise<void> => {
    const intent = text.trim();
    if (intent.length === 0 || disabled || submitting) return;
    setSubmitting(true);
    setError(null);
    try {
      const planned = await planSwarm(intent);
      setText("");
      onPlanReady(planned);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setSubmitting(false);
    }
  }, [text, disabled, submitting, onPlanReady]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>): void => {
      if (e.key === "Escape" && pendingTicketId) {
        e.preventDefault();
        void rejectSwarm(pendingTicketId).finally(() => onPlanCleared());
        return;
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        void handleSubmit();
      }
    },
    [pendingTicketId, onPlanCleared, handleSubmit],
  );

  const handleChange = useCallback((e: ChangeEvent<HTMLTextAreaElement>): void => {
    setText(e.target.value);
    if (error) setError(null);
  }, [error]);

  const rows = computeRows(text);

  return (
    <section
      data-testid="director-console"
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 8,
        padding: 12,
        borderRadius: 10,
        background: "rgba(15,23,42,0.55)",
        border: "1px solid rgba(100,116,139,0.25)",
      }}
      aria-label="Director console"
    >
      <label
        htmlFor="director-intent"
        style={{
          fontSize: 10,
          textTransform: "uppercase",
          letterSpacing: "0.12em",
          color: "#94a3b8",
        }}
      >
        Director
      </label>
      <textarea
        id="director-intent"
        ref={textareaRef}
        data-testid="director-textarea"
        value={text}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        rows={rows}
        placeholder="Describe the outcome. e.g. Research the top 3 quantum error-correction papers and summarise each."
        disabled={disabled || submitting}
        style={{
          resize: "none",
          padding: "8px 10px",
          fontSize: 13,
          lineHeight: `${LINE_HEIGHT_PX}px`,
          color: "#e2e8f0",
          background: "rgba(10,14,26,0.65)",
          border: "1px solid rgba(100,116,139,0.35)",
          borderRadius: 8,
          outline: "none",
          fontFamily: "var(--font-mono, monospace)",
          opacity: disabled ? 0.55 : 1,
        }}
      />
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <button
          type="button"
          data-testid="director-submit"
          onClick={(): void => {
            void handleSubmit();
          }}
          disabled={disabled || submitting || text.trim().length === 0}
          style={{
            padding: "6px 14px",
            borderRadius: 8,
            background: "rgba(37,99,235,0.18)",
            border: "1px solid rgba(59,130,246,0.45)",
            color: "#bfdbfe",
            fontSize: 12,
            fontWeight: 600,
            cursor: (disabled || submitting || text.trim().length === 0) ? "not-allowed" : "pointer",
            opacity: (disabled || submitting || text.trim().length === 0) ? 0.55 : 1,
          }}
        >
          {submitting ? "planning…" : "Submit"}
        </button>
        {submitting && (
          <span
            data-testid="director-spinner"
            style={{ fontSize: 11, color: "#94a3b8" }}
          >
            awaiting PlanProposed…
          </span>
        )}
        {error && (
          <span
            data-testid="director-error"
            role="alert"
            style={{ fontSize: 11, color: "#f87171" }}
          >
            {error}
          </span>
        )}
      </div>
    </section>
  );
}

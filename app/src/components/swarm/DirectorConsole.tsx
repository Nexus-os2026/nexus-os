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
import { voicePipelineHealth } from "../../api/backend";
import type { VoicePipelineHealth } from "../../api/backend";
import { planSwarm, rejectSwarm } from "../../lib/swarm/commands";
import type { PlannedSwarmJson } from "../../lib/swarm/types";
import { useSwarmStore } from "../../lib/swarm/store";
import { selectActiveRun, selectIsPlanPending } from "../../lib/swarm/selectors";
import { PushToTalk } from "../../voice/PushToTalk";

/**
 * Track C #3 commit 2: append a new transcript chunk to the existing
 * textarea value with the documented separator semantics. Pure helper;
 * exported for unit testing.
 *
 *   - empty existing → return transcript unchanged
 *   - existing ends in `\n` → return existing + transcript (no double)
 *   - else → return existing + "\n" + transcript
 */
export function appendTranscript(currentText: string, transcript: string): string {
  if (currentText.length === 0) return transcript;
  if (currentText.endsWith("\n")) return currentText + transcript;
  return currentText + "\n" + transcript;
}

const MAX_RECORDING_MS = 30_000;

function prefersReducedMotion(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

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

  // Track C #3 commit 2: mic state + pipeline-health probe.
  const [pipelineHealth, setPipelineHealth] = useState<VoicePipelineHealth | null>(
    null,
  );
  const [isRecording, setIsRecording] = useState(false);
  const pushToTalkRef = useRef<PushToTalk | null>(null);
  const recordingCapTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const planPending = useSwarmStore(selectIsPlanPending);
  const activeRun = useSwarmStore(selectActiveRun);
  const disabled = planPending || activeRun !== null;

  // Once the plan_proposed event lands in the store, drop our local
  // "submitting" spinner. The parent still owns the ticket id.
  useEffect(() => {
    if (submitting && planPending) setSubmitting(false);
  }, [submitting, planPending]);

  // Track C #3 commit 2: pre-flight pipeline-health probe on mount.
  // Fires once; the mic button stays disabled until the probe lands.
  // Failures land in synthetic { reachable: false } so the tooltip
  // surfaces the cause instead of leaving the button mysterious.
  useEffect(() => {
    let cancelled = false;
    void (async (): Promise<void> => {
      try {
        const health = await voicePipelineHealth();
        if (!cancelled) setPipelineHealth(health);
      } catch (e) {
        if (!cancelled) {
          setPipelineHealth({
            reachable: false,
            model: null,
            last_error: e instanceof Error ? e.message : "Pipeline check failed",
          });
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Track C #3 commit 2: cleanup the recording cap timer on unmount.
  useEffect(() => {
    return () => {
      if (recordingCapTimerRef.current !== null) {
        clearTimeout(recordingCapTimerRef.current);
      }
    };
  }, []);

  const performStop = useCallback(
    async (autoCapped: boolean): Promise<void> => {
      const recorder = pushToTalkRef.current;
      if (!recorder || !isRecording) return;
      if (recordingCapTimerRef.current !== null) {
        clearTimeout(recordingCapTimerRef.current);
        recordingCapTimerRef.current = null;
      }
      setIsRecording(false);
      try {
        const result = await recorder.stopAndTranscribe();
        if (autoCapped) {
          setError("Recording auto-stopped at 30s");
        }
        const transcript = result.transcript.trim();
        if (transcript.length > 0) {
          setText((prev) => {
            const next = appendTranscript(prev, transcript);
            // Schedule cursor restore for after the controlled-input
            // re-render lands in the DOM.
            queueMicrotask(() => {
              const el = textareaRef.current;
              if (el !== null) {
                el.focus();
                el.setSelectionRange(next.length, next.length);
              }
            });
            return next;
          });
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [isRecording],
  );

  const handleMicToggle = useCallback((): void => {
    if (isRecording) {
      void performStop(false);
      return;
    }
    if (disabled) return;
    if (pipelineHealth === null || !pipelineHealth.reachable) return;
    setError(null);
    if (pushToTalkRef.current === null) {
      pushToTalkRef.current = new PushToTalk();
    }
    pushToTalkRef.current.startRecording();
    setIsRecording(true);
    recordingCapTimerRef.current = setTimeout(() => {
      void performStop(true);
    }, MAX_RECORDING_MS);
  }, [isRecording, disabled, pipelineHealth, performStop]);

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
          data-testid="director-mic"
          aria-label={isRecording ? "Stop voice input" : "Start voice input"}
          aria-pressed={isRecording}
          title={
            pipelineHealth === null
              ? "Checking voice pipeline…"
              : pipelineHealth.reachable
                ? "Push to talk"
                : `Voice unavailable: ${pipelineHealth.last_error ?? "Unknown error"}`
          }
          disabled={
            (disabled && !isRecording) ||
            (!isRecording && (pipelineHealth === null || !pipelineHealth.reachable))
          }
          onClick={handleMicToggle}
          style={{
            padding: "6px 14px",
            borderRadius: 8,
            background: isRecording
              ? "rgba(255,109,122,0.18)"
              : "rgba(15,23,42,0.6)",
            border: `1px solid ${
              isRecording ? "rgba(255,109,122,0.65)" : "rgba(100,116,139,0.45)"
            }`,
            color: isRecording ? "var(--nexus-danger, #ff6d7a)" : "#cbd5e1",
            fontSize: 12,
            fontWeight: 600,
            cursor:
              ((disabled && !isRecording) ||
                (!isRecording && (pipelineHealth === null || !pipelineHealth.reachable)))
                ? "not-allowed"
                : "pointer",
            opacity:
              ((disabled && !isRecording) ||
                (!isRecording && (pipelineHealth === null || !pipelineHealth.reachable)))
                ? 0.55
                : 1,
            animation:
              isRecording && !prefersReducedMotion()
                ? "swarm-node-pulse 1.5s ease-out infinite"
                : undefined,
          }}
        >
          {isRecording ? "■ Stop" : "🎙️ Mic"}
        </button>
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

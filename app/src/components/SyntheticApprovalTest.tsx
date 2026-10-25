import { useCallback, useEffect, useRef, useState } from "react";
import { p3Status, p3Submit } from "../api/backend";
import { readSnapshot } from "../hooks/usePendingOwnerApprovals";

// P4-UAT05, a Phase Four amendment the owner adopted: the owner's synthetic
// approval test, started from the page the owner works on. It appears on Chat
// and Agents only while Nexus lists a live synthetic grant, and only the
// owner's own click sends the one constant command. The backend then prepares
// an R2 request that the existing approval card shows and the Nexus approval
// window alone decides. This launcher never approves, denies, grants or stops
// anything, never shows a request as waiting, and reads nothing from the
// chat. Showing it is display gating only: the backend checks everything.

/** The one command it sends, verbatim. */
export const SYNTHETIC_COMMAND = "synthetic-receipt";

/** The pages it appears on: Chat and Agents. */
export const SYNTHETIC_LAUNCHER_PAGES: readonly string[] = ["ai-chat-hub", "agents"];

export const SYNTHETIC_LAUNCHER_LABEL = "Start synthetic approval test";
export const SYNTHETIC_LAUNCHER_NOTE = "Your own test command. No external effect.";

const MUTED = "#94a3b8";
const ACCENT = "#06b6d4";

/**
 * Whether a `p3_status` answer is one this page can read, says governed
 * control is supported here, and lists a live synthetic grant. Any part it
 * cannot read makes the whole answer unreadable: a malformed status never
 * shows the launcher.
 */
export function liveSyntheticGrant(raw: unknown): boolean {
  if (!readSnapshot(raw, 0).ok) return false;
  const grants = (raw as Record<string, unknown>).grants;
  if (!Array.isArray(grants)) return false;
  let live = false;
  for (const entry of grants) {
    if (!entry || typeof entry !== "object") return false;
    const grant = entry as Record<string, unknown>;
    if (typeof grant.id !== "string" || grant.id.length === 0) return false;
    if (typeof grant.kind !== "string" || grant.kind.length === 0) return false;
    if (typeof grant.live !== "boolean") return false;
    if (!Array.isArray(grant.lines) || !grant.lines.every((line) => typeof line === "string")) return false;
    if (grant.kind === "synthetic" && grant.live) live = true;
  }
  return live;
}

type Answer =
  | { kind: "committed"; commitment: string; state: string }
  | { kind: "not-understood"; reason: string }
  | { kind: "refused"; error: string };

const UNREADABLE = "Nexus returned an answer this test cannot read";

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** The backend's answer to the command, as it gave it. */
function answerOf(submitted: unknown): Answer {
  const s = submitted && typeof submitted === "object" ? (submitted as Record<string, unknown>) : null;
  if (s?.understood === true) {
    const c = s.commitment as Record<string, unknown> | undefined;
    if (c && typeof c.id === "string" && c.id.length > 0 && typeof c.state === "string") {
      return { kind: "committed", commitment: c.id, state: c.state };
    }
    return { kind: "refused", error: UNREADABLE };
  }
  if (s?.understood === false) {
    return { kind: "not-understood", reason: typeof s.reason === "string" ? s.reason : "not understood" };
  }
  return { kind: "refused", error: UNREADABLE };
}

// One command in flight across the whole interface (a page change that
// remounts the launcher does not start a second).
let submitting = false;

/** Clear the in-flight mark (between isolated test cases). */
export function resetSyntheticLauncher(): void {
  submitting = false;
}

/**
 * The owner's synthetic approval test on Chat and Agents. Elsewhere it
 * renders nothing. It reads `p3_status` when the owner enters one of those
 * pages and after each click; it does not poll.
 */
export function SyntheticApprovalTest({ currentPage }: { currentPage?: string }) {
  if (!currentPage || !SYNTHETIC_LAUNCHER_PAGES.includes(currentPage)) return null;
  return <Launcher key={currentPage} />;
}

function Launcher() {
  const [available, setAvailable] = useState(false);
  const [sending, setSending] = useState(false);
  const [answer, setAnswer] = useState<Answer | null>(null);
  const mounted = useRef(false);

  const read = useCallback(async () => {
    let live = false;
    try {
      live = liveSyntheticGrant(await p3Status());
    } catch {
      live = false;
    }
    if (mounted.current) setAvailable(live);
  }, []);

  useEffect(() => {
    mounted.current = true;
    void read();
    return () => {
      mounted.current = false;
    };
  }, [read]);

  // The owner's click only: the one constant command, one at a time.
  const start = async () => {
    if (submitting) return;
    submitting = true;
    setSending(true);
    setAnswer(null);
    let next: Answer;
    try {
      next = answerOf(await p3Submit(SYNTHETIC_COMMAND));
    } catch (error) {
      next = { kind: "refused", error: errorText(error) };
    }
    if (mounted.current) setAnswer(next);
    try {
      await read();
    } finally {
      submitting = false;
      if (mounted.current) setSending(false);
    }
  };

  if (!available && answer === null) return null;
  return (
    <div
      data-testid="synthetic-approval-test"
      role="region"
      aria-label="Synthetic approval test"
      style={{
        position: "fixed",
        top: 12,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 9997,
        width: "min(360px, calc(100vw - 24px))",
        background: "rgba(10, 22, 40, 0.97)",
        border: `1px solid ${MUTED}44`,
        borderRadius: 8,
        padding: "8px 12px",
        fontSize: 12,
      }}
    >
      {available && (
        <>
          <button
            type="button"
            disabled={sending}
            onClick={() => void start()}
            style={{
              borderRadius: 6,
              padding: "5px 10px",
              fontSize: 12,
              fontWeight: 600,
              cursor: sending ? "not-allowed" : "pointer",
              opacity: sending ? 0.5 : 1,
              background: `${ACCENT}22`,
              color: ACCENT,
              border: `1px solid ${ACCENT}66`,
            }}
          >
            {SYNTHETIC_LAUNCHER_LABEL}
          </button>
          <div style={{ color: MUTED, marginTop: 4 }}>{SYNTHETIC_LAUNCHER_NOTE}</div>
        </>
      )}
      {sending && <div style={{ color: "#e2e8f0", marginTop: 4 }}>Sending your test command to Nexus…</div>}
      {answer && (
        <div data-testid="synthetic-approval-answer" style={{ marginTop: 6 }}>
          {answer.kind === "committed" && (
            <div style={{ color: "#e2e8f0" }}>
              Nexus answered: request <span style={{ fontFamily: "monospace" }}>{answer.commitment}</span> is{" "}
              {answer.state}. Only the Nexus approval window can allow it; its approval card shows what Nexus lists.
            </div>
          )}
          {answer.kind === "not-understood" && (
            <div style={{ color: "#fbbf24" }}>Nexus did not take the command: {answer.reason}</div>
          )}
          {answer.kind === "refused" && (
            <div style={{ color: "#f87171" }}>
              Nexus refused: <span style={{ fontFamily: "monospace" }}>{answer.error}</span>
            </div>
          )}
          <button
            type="button"
            onClick={() => setAnswer(null)}
            style={{ marginTop: 4, background: "none", border: "none", color: MUTED, cursor: "pointer", padding: 0, fontSize: 11 }}
          >
            Dismiss
          </button>
        </div>
      )}
    </div>
  );
}

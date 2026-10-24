import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { p3Approve, p3CancelRun, p3Deny, p3Status, type P3Commitment } from "../api/backend";

// ── Phase Three R2 commitments waiting for the owner, wherever the owner is ──
//
// Display only. This module reads the backend's `p3_status`; it never
// approves anything. The one way it reaches an approval is the owner's own
// click, which sends a backend-listed commitment id to the existing
// `p3_approve`; the backend then shows its native window, and only the
// owner's answer there authorizes the effect. Every outcome shown is the
// backend's state as read again after the action, never an assumption.

/** The fastest automatic re-read: one `p3_status` per two seconds at most. */
export const POLL_INTERVAL_MS = 2000;

const CLASSES = new Set(["R0", "R1", "R2"]);

export interface GovernedRun {
  id: string;
  cancelled: boolean;
  finished: boolean;
}

export interface GovernedSnapshot {
  /** Every commitment the backend listed, in any state. */
  byId: ReadonlyMap<string, P3Commitment>;
  runs: ReadonlyMap<string, GovernedRun>;
  /** R2 commitments still prepared that need the owner's native approval. */
  pending: P3Commitment[];
  emergencyStopped: boolean;
  readAt: number;
}

export type SnapshotReading = { ok: true; snapshot: GovernedSnapshot } | { ok: false; reason: string };

function text(value: unknown): value is string {
  return typeof value === "string" && value.length > 0;
}

function readCommitment(raw: unknown): P3Commitment | null {
  if (!raw || typeof raw !== "object") return null;
  const c = raw as Record<string, unknown>;
  if (!text(c.id) || !text(c.agent) || !text(c.run) || !text(c.kind)) return null;
  if (!text(c.operation) || !text(c.state) || !text(c.binding) || typeof c.target !== "string") return null;
  if (typeof c.class !== "string" || !CLASSES.has(c.class)) return null;
  // Only R2 needs the owner's native approval: any other pairing is not a
  // shape this presenter trusts.
  if (typeof c.requires_approval !== "boolean" || c.requires_approval !== (c.class === "R2")) return null;
  if (!Array.isArray(c.summary) || !c.summary.every((line) => typeof line === "string")) return null;
  return {
    id: c.id,
    agent: c.agent,
    run: c.run,
    kind: c.kind,
    class: c.class as P3Commitment["class"],
    operation: c.operation,
    target: c.target,
    summary: [...(c.summary as string[])],
    state: c.state,
    requires_approval: c.requires_approval,
    binding: c.binding,
  };
}

/**
 * The backend's status as a snapshot, or the reason it cannot be one. Any
 * entry this presenter cannot read makes the whole answer unreadable: a
 * dropped entry could hide a request, and "nothing waiting" must never be
 * claimed from an answer that was not understood.
 */
export function readSnapshot(raw: unknown, now: number): SnapshotReading {
  const unreadable: SnapshotReading = { ok: false, reason: "Nexus returned a governed status this page cannot read" };
  if (!raw || typeof raw !== "object") return unreadable;
  const s = raw as Record<string, unknown>;
  const status = s.status as Record<string, unknown> | undefined;
  if (!status || typeof status !== "object" || !Array.isArray(s.commitments) || !Array.isArray(s.runs)) {
    return unreadable;
  }
  // The safety fields are explicit booleans or the answer is not understood:
  // a missing or malformed one is never read as permission.
  if (typeof status.platform_supported !== "boolean" || typeof status.emergency_stopped !== "boolean") {
    return unreadable;
  }
  if (!status.platform_supported) {
    return { ok: false, reason: "governed real-world control is available on Linux only" };
  }
  const byId = new Map<string, P3Commitment>();
  for (const entry of s.commitments) {
    const commitment = readCommitment(entry);
    if (!commitment || byId.has(commitment.id)) return unreadable;
    byId.set(commitment.id, commitment);
  }
  const runs = new Map<string, GovernedRun>();
  for (const entry of s.runs) {
    const run = entry as Record<string, unknown> | null;
    if (!run || !text(run.id) || typeof run.cancelled !== "boolean" || typeof run.finished !== "boolean") {
      return unreadable;
    }
    // One entry per run: a second could contradict whether it was stopped.
    if (runs.has(run.id)) return unreadable;
    runs.set(run.id, { id: run.id, cancelled: run.cancelled, finished: run.finished });
  }
  const pending = [...byId.values()].filter(
    (c) => c.state === "prepared" && c.class === "R2" && c.requires_approval,
  );
  return {
    ok: true,
    snapshot: { byId, runs, pending, emergencyStopped: status.emergency_stopped, readAt: now },
  };
}

export type Availability = "loading" | "available" | "unavailable";

export interface PendingOwnerApprovals {
  availability: Availability;
  /** Why the backend's state is unknown (unavailable only). */
  reason: string | null;
  /** The last snapshot read, while the backend is available. */
  snapshot: GovernedSnapshot | null;
  /**
   * Read the backend again now: the snapshot, or null when it is unknown,
   * including when a newer read began before this one answered.
   */
  refresh: () => Promise<GovernedSnapshot | null>;
}

interface ReadState {
  availability: Availability;
  reason: string | null;
  snapshot: GovernedSnapshot | null;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Pending R2 commitments, re-read from the backend at most every
 * {@link POLL_INTERVAL_MS} while the window is visible. A failed read shows
 * the state as unavailable and drops every card: a request the backend did
 * not just list is never actionable.
 */
export function usePendingOwnerApprovals(): PendingOwnerApprovals {
  const [state, setState] = useState<ReadState>({ availability: "loading", reason: null, snapshot: null });
  const latest = useRef(0);
  const mounted = useRef(false);

  const refresh = useCallback(async (): Promise<GovernedSnapshot | null> => {
    const mine = ++latest.current;
    let next: ReadState;
    try {
      const reading = readSnapshot(await p3Status(), Date.now());
      next = reading.ok
        ? { availability: "available", reason: null, snapshot: reading.snapshot }
        : { availability: "unavailable", reason: reading.reason, snapshot: null };
    } catch (error) {
      next = { availability: "unavailable", reason: errorText(error), snapshot: null };
    }
    // A newer read began after this one: an older answer never replaces it,
    // and is never returned as current (a review is never sent on it).
    const current = mine === latest.current;
    if (mounted.current && current) setState(next);
    return current ? next.snapshot : null;
  }, []);

  useEffect(() => {
    mounted.current = true;
    let stopped = false;
    let running = false;
    let lastStart = 0;
    let timer: number | undefined;
    const visible = () => typeof document === "undefined" || document.visibilityState !== "hidden";
    const schedule = (delay: number) => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => void tick(), Math.max(0, delay));
    };
    const tick = async () => {
      if (stopped || running || !visible()) return;
      running = true;
      lastStart = Date.now();
      try {
        await refresh();
      } finally {
        running = false;
        if (!stopped && visible()) schedule(POLL_INTERVAL_MS);
      }
    };
    const onVisibility = () => {
      if (stopped) return;
      if (!visible()) {
        window.clearTimeout(timer);
      } else if (!running) {
        schedule(lastStart + POLL_INTERVAL_MS - Date.now());
      }
    };
    document.addEventListener("visibilitychange", onVisibility);
    void tick();
    return () => {
      stopped = true;
      mounted.current = false;
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [refresh]);

  return { ...state, refresh };
}

// ── The owner's actions, shared by every presenter ─────────────────────────
//
// One review at a time across the whole interface: a second native window is
// never requested while one is outstanding. Stops are never held by it.

export type OwnerAction = "review" | "deny" | "stop";

export interface ActionResult {
  key: number;
  action: OwnerAction;
  commitment: P3Commitment;
  /** The backend's refusal or failure, verbatim. */
  error: string | null;
  /** Why a review was not sent to the backend at all (review only). */
  notSent: string | null;
  /** How the backend answered the re-read after the action. */
  reread: "listed" | "unlisted" | "unavailable";
  /** The commitment's state in that re-read (listed only). */
  backendState: string | null;
  /** Whether that re-read lists the run as cancelled (stop only). */
  runCancelled: boolean | null;
  outputText: string | null;
  outputBytes: number | null;
}

interface ActionStore {
  reviewing: string | null;
  /** Whether that review has been sent to `p3_approve` (false while its fresh read is checked). */
  reviewSent: boolean;
  denying: ReadonlySet<string>;
  results: ActionResult[];
}

const MAX_RESULTS = 5;
let store: ActionStore = { reviewing: null, reviewSent: false, denying: new Set(), results: [] };
let resultKey = 0;
const listeners = new Set<() => void>();

function update(next: Partial<ActionStore>) {
  store = { ...store, ...next };
  listeners.forEach((listener) => listener());
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The shared action state: the review in flight, denials in flight, recent results. */
export function useOwnerActions(): ActionStore {
  return useSyncExternalStore(subscribe, () => store, () => store);
}

/** Clear the shared action state (between isolated test cases). */
export function resetOwnerActions(): void {
  update({ reviewing: null, reviewSent: false, denying: new Set(), results: [] });
}

export function dismissResult(key: number): void {
  update({ results: store.results.filter((result) => result.key !== key) });
}

function record(
  action: OwnerAction,
  commitment: P3Commitment,
  error: string | null,
  snapshot: GovernedSnapshot | null,
  output: { text: string | null; bytes: number | null } | null,
  notSent: string | null = null,
) {
  const listed = snapshot?.byId.get(commitment.id) ?? null;
  const run = snapshot?.runs.get(commitment.run) ?? null;
  const result: ActionResult = {
    key: ++resultKey,
    action,
    commitment,
    error,
    notSent,
    reread: snapshot === null ? "unavailable" : listed ? "listed" : "unlisted",
    backendState: listed?.state ?? null,
    runCancelled: action === "stop" && run ? run.cancelled : null,
    outputText: typeof output?.text === "string" ? output.text : null,
    outputBytes: typeof output?.bytes === "number" ? output.bytes : null,
  };
  update({ results: [result, ...store.results].slice(0, MAX_RESULTS) });
}

/**
 * Whether `commitment` is one the backend listed as waiting in `snapshot`:
 * the same id, still prepared R2 needing approval. Anything else (a forged
 * card, a stale one, an R0/R1 commitment `p3_approve` would simply run) is
 * never sent for review.
 */
export function isListedPending(snapshot: GovernedSnapshot | null, commitment: P3Commitment): boolean {
  return snapshot?.pending.some((c) => c.id === commitment.id && c.run === commitment.run) ?? false;
}

/**
 * The longest a click waits for its fresh `p3_status`: an answer that comes
 * later never opens the native window long after the owner's click.
 */
export const FRESH_READ_TIMEOUT_MS = 5000;

/**
 * Why `commitment` must not be sent for review according to `fresh`, the
 * status read after the owner's click, or null when that read still lists it
 * as the same waiting request: the same id, run, agent, effect and binding,
 * R2, still prepared, needing the owner's native approval, its run not
 * stopped or finished. A read that a newer one superseded arrives as null.
 * This supplements the backend's own checks; it never replaces them.
 */
export function freshReviewRefusal(fresh: GovernedSnapshot | null, commitment: P3Commitment): string | null {
  if (fresh === null) {
    return "Nexus could not be read just now, or a newer status read replaced this check, so this request could not be confirmed as still waiting";
  }
  if (fresh.emergencyStopped) return "Emergency stop is active";
  const listed = fresh.byId.get(commitment.id);
  if (!listed) return "Nexus no longer lists this request in the status it returned";
  if (
    listed.run !== commitment.run ||
    listed.agent !== commitment.agent ||
    listed.kind !== commitment.kind ||
    listed.operation !== commitment.operation ||
    listed.target !== commitment.target ||
    listed.binding !== commitment.binding
  ) {
    return "Nexus now lists this request with a different run or details";
  }
  if (listed.class !== "R2" || listed.requires_approval !== true) {
    return "Nexus no longer lists this request as needing your native approval";
  }
  if (listed.state !== "prepared") return `Nexus lists this request as ${listed.state}`;
  const run = fresh.runs.get(commitment.run);
  if (!run) return "Nexus does not list this request's run";
  if (run.cancelled) return "Nexus lists this request's run as stopped";
  // A finished run's waiting commitments are revoked, and its approval is
  // refused (run_not_active): never sent, even in the moment between the two.
  if (run.finished) return "Nexus lists this request's run as finished";
  return null;
}

/** `work`'s answer, or null when it fails or does not answer within `ms`. */
function withinDeadline<T>(work: Promise<T>, ms: number): Promise<T | null> {
  return new Promise((resolve) => {
    const timer = window.setTimeout(() => resolve(null), ms);
    work.then(
      (value) => {
        window.clearTimeout(timer);
        resolve(value);
      },
      () => {
        window.clearTimeout(timer);
        resolve(null);
      },
    );
  });
}

/**
 * The owner's click on "Review in Nexus approval window". It reads the
 * backend once more and sends only the commitment id to `p3_approve` if that
 * read still lists the same waiting request; the backend's native window then
 * decides. Call it from a click handler only.
 */
export async function reviewInNativeWindow(
  commitment: P3Commitment,
  approvals: Pick<PendingOwnerApprovals, "snapshot" | "refresh">,
): Promise<void> {
  // Taken before the first await: a second click, card or presenter finds it taken.
  if (store.reviewing !== null || !isListedPending(approvals.snapshot, commitment)) return;
  update({ reviewing: commitment.id, reviewSent: false });
  try {
    const fresh = await withinDeadline(approvals.refresh(), FRESH_READ_TIMEOUT_MS);
    const refusal = freshReviewRefusal(fresh, commitment);
    if (refusal !== null) {
      record("review", commitment, null, fresh, null, refusal);
      return;
    }
    update({ reviewSent: true });
    let error: string | null = null;
    let output: { text: string | null; bytes: number | null } | null = null;
    try {
      output = await p3Approve(commitment.id);
    } catch (e) {
      error = errorText(e);
    }
    record("review", commitment, error, await approvals.refresh(), output);
  } finally {
    update({ reviewing: null, reviewSent: false });
  }
}

/** The owner's denial: refusal only, by the listed commitment id. */
export async function denyCommitment(
  commitment: P3Commitment,
  approvals: Pick<PendingOwnerApprovals, "snapshot" | "refresh">,
): Promise<void> {
  if (store.denying.has(commitment.id) || !isListedPending(approvals.snapshot, commitment)) return;
  update({ denying: new Set([...store.denying, commitment.id]) });
  let error: string | null = null;
  try {
    await p3Deny(commitment.id);
  } catch (e) {
    error = errorText(e);
  }
  try {
    record("deny", commitment, error, await approvals.refresh(), null);
  } finally {
    const denying = new Set(store.denying);
    denying.delete(commitment.id);
    update({ denying });
  }
}

/** Whether the backend listed `commitment` (any state) in `snapshot`, with the same run. */
export function isListed(snapshot: GovernedSnapshot | null, commitment: P3Commitment): boolean {
  return snapshot?.byId.get(commitment.id)?.run === commitment.run;
}

/** The owner's stop of the commitment's run: never waits for, or is held by, a review. */
export async function stopRun(
  commitment: P3Commitment,
  approvals: Pick<PendingOwnerApprovals, "snapshot" | "refresh">,
): Promise<void> {
  if (!isListed(approvals.snapshot, commitment)) return;
  let error: string | null = null;
  try {
    await p3CancelRun(commitment.run);
  } catch (e) {
    error = errorText(e);
  }
  record("stop", commitment, error, await approvals.refresh(), null);
}

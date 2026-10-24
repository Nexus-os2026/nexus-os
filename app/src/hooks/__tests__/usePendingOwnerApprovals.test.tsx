import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { P3Commitment } from "../../api/backend";
import { mockInvoke } from "../../test/setup";
import {
  FRESH_READ_TIMEOUT_MS,
  POLL_INTERVAL_MS,
  freshReviewRefusal,
  readSnapshot,
  resetOwnerActions,
  reviewInNativeWindow,
  stopRun,
  useOwnerActions,
  usePendingOwnerApprovals,
} from "../usePendingOwnerApprovals";

// Synthetic fixtures only: these tests prove what the presenter reads, shows
// and sends, never a kernel effect.

const R2: P3Commitment = {
  id: "cmt-0123456789abcdef0123456789abcdef",
  agent: "agent-7f3c",
  run: "run-0123456789abcdef0123456789abcdef",
  kind: "egress",
  class: "R2",
  operation: "egress.request",
  target: "https://example.com:443",
  summary: ["POST https://example.com/x"],
  state: "prepared",
  requires_approval: true,
  binding: "0a1b2c3d4e5f",
};

function status(commitments: unknown[], extra: Record<string, unknown> = {}) {
  return {
    status: { platform_supported: true, emergency_stopped: false, policy_generation: 0, display: null, tools: [], connector_operations: [], ...extra },
    commitments,
    grants: [],
    runs: [{ id: R2.run, agent: R2.agent, cancelled: false, finished: false }],
  };
}

function calls(command: string): unknown[][] {
  return mockInvoke.mock.calls.filter((call: unknown[]) => call[0] === command);
}

function setVisibility(state: "visible" | "hidden") {
  Object.defineProperty(document, "visibilityState", { value: state, configurable: true });
  document.dispatchEvent(new Event("visibilitychange"));
}

async function flush(ms = 0) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

describe("usePendingOwnerApprovals", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    resetOwnerActions();
  });
  afterEach(() => {
    setVisibility("visible");
    vi.useRealTimers();
  });

  it("re-reads p3_status no more often than every two seconds and never approves", async () => {
    mockInvoke.mockImplementation((cmd: string) => Promise.resolve(cmd === "p3_status" ? status([R2]) : {}));
    const { result } = renderHook(() => usePendingOwnerApprovals());
    await flush();
    expect(calls("p3_status")).toHaveLength(1);
    expect(result.current.availability).toBe("available");
    expect(result.current.snapshot?.pending.map((c) => c.id)).toEqual([R2.id]);
    await flush(POLL_INTERVAL_MS - 1);
    expect(calls("p3_status")).toHaveLength(1);
    await flush(1);
    expect(calls("p3_status")).toHaveLength(2);
    await flush(10 * POLL_INTERVAL_MS);
    expect(calls("p3_status")).toHaveLength(12);
    expect(calls("p3_approve")).toHaveLength(0);
    expect(mockInvoke.mock.calls.every((call: unknown[]) => call[0] === "p3_status")).toBe(true);
  });

  it("stops reading while the window is hidden and resumes no sooner than the interval", async () => {
    mockInvoke.mockImplementation((cmd: string) => Promise.resolve(cmd === "p3_status" ? status([R2]) : {}));
    renderHook(() => usePendingOwnerApprovals());
    await flush();
    expect(calls("p3_status")).toHaveLength(1);
    setVisibility("hidden");
    await flush(30_000);
    expect(calls("p3_status")).toHaveLength(1);
    setVisibility("visible");
    await flush();
    // The last read was long ago: one read now, then the interval again.
    expect(calls("p3_status")).toHaveLength(2);
    setVisibility("hidden");
    setVisibility("visible");
    await flush(POLL_INTERVAL_MS - 1);
    expect(calls("p3_status")).toHaveLength(2);
    await flush(1);
    expect(calls("p3_status")).toHaveLength(3);
  });

  it("shows a failed read as unavailable, never as nothing pending", async () => {
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === "p3_status" ? Promise.reject(new Error("governed real-world control is available on Linux only")) : Promise.resolve({}),
    );
    const { result } = renderHook(() => usePendingOwnerApprovals());
    await flush();
    expect(result.current.availability).toBe("unavailable");
    expect(result.current.snapshot).toBeNull();
    expect(result.current.reason).toContain("Linux only");
  });

  it("drops every card when a later read fails", async () => {
    let fail = false;
    mockInvoke.mockImplementation((cmd: string) =>
      cmd === "p3_status" ? (fail ? Promise.reject(new Error("backend gone")) : Promise.resolve(status([R2]))) : Promise.resolve({}),
    );
    const { result } = renderHook(() => usePendingOwnerApprovals());
    await flush();
    expect(result.current.snapshot?.pending).toHaveLength(1);
    fail = true;
    await flush(POLL_INTERVAL_MS);
    expect(result.current.availability).toBe("unavailable");
    expect(result.current.snapshot).toBeNull();
  });

  it("treats any answer it cannot read as unavailable", () => {
    const unreadable: unknown[] = [
      null,
      {},
      "prepared",
      { status: {}, commitments: [R2] },
      { status: {}, runs: [] },
      status([{ ...R2, id: 7 }]),
      status([{ ...R2, summary: "<b>one string</b>" }]),
      status([{ ...R2, class: "R3" }]),
      status([{ ...R2, class: "R1" }]),
      status([{ ...R2, requires_approval: false }]),
      status([{ ...R2, requires_approval: "true" }]),
      status([R2, { ...R2, state: "succeeded" }]),
      { ...status([R2]), runs: [{ id: R2.run }] },
    ];
    for (const raw of unreadable) {
      const reading = readSnapshot(raw, 0);
      expect(reading.ok, JSON.stringify(raw)).toBe(false);
    }
    expect(readSnapshot(status([R2], { platform_supported: false }), 0)).toEqual({
      ok: false,
      reason: "governed real-world control is available on Linux only",
    });
  });

  it("lists only prepared R2 commitments that need approval as pending", () => {
    const r1 = { ...R2, id: "cmt-r1", class: "R1", requires_approval: false };
    const r0 = { ...R2, id: "cmt-r0", class: "R0", requires_approval: false };
    const done = { ...R2, id: "cmt-done", state: "succeeded" };
    const authorized = { ...R2, id: "cmt-auth", state: "authorized" };
    const reading = readSnapshot(status([r1, r0, done, authorized, R2]), 0);
    expect(reading.ok).toBe(true);
    if (!reading.ok) return;
    expect(reading.snapshot.pending.map((c) => c.id)).toEqual([R2.id]);
    expect(reading.snapshot.byId.size).toBe(5);
    // Extra fields from the backend are not carried into the presenter.
    const extra = readSnapshot(status([{ ...R2, html: "<script>x</script>" }]), 0);
    expect(extra.ok && Object.keys(extra.snapshot.pending[0]).sort()).toEqual(Object.keys(R2).sort());
  });

  it("never lets an older answer replace a newer one", async () => {
    let releaseFirst: (value: unknown) => void = () => {};
    let first = true;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd !== "p3_status") return Promise.resolve({});
      if (first) {
        first = false;
        return new Promise((resolve) => {
          releaseFirst = resolve;
        });
      }
      return Promise.resolve(status([{ ...R2, state: "denied" }]));
    });
    const { result } = renderHook(() => usePendingOwnerApprovals());
    await act(async () => {
      await result.current.refresh();
    });
    expect(result.current.snapshot?.pending).toHaveLength(0);
    await act(async () => {
      releaseFirst(status([R2]));
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(result.current.snapshot?.pending).toHaveLength(0);
  });

  it("sends a review only for a commitment the backend lists as pending R2", async () => {
    vi.useRealTimers();
    const r1: P3Commitment = { ...R2, id: "cmt-r1", class: "R1", requires_approval: false };
    const reading = readSnapshot(status([r1, R2]), 0);
    if (!reading.ok) throw new Error("fixture unreadable");
    const approvals = { snapshot: reading.snapshot, refresh: async () => reading.snapshot };
    mockInvoke.mockResolvedValue({});
    // p3_approve would simply run an R0/R1 commitment: it is never sent one.
    await reviewInNativeWindow(r1, approvals);
    await reviewInNativeWindow({ ...R2, id: "cmt-forged" }, approvals);
    await reviewInNativeWindow({ ...R2, run: "run-other" }, approvals);
    await reviewInNativeWindow(R2, { snapshot: null, refresh: async () => null });
    expect(calls("p3_approve")).toHaveLength(0);
    await reviewInNativeWindow(R2, approvals);
    expect(calls("p3_approve")).toEqual([["p3_approve", { commitment: R2.id }]]);
  });

  it("issues one review even when clicks arrive before the interface can disable the button", async () => {
    vi.useRealTimers();
    const other: P3Commitment = { ...R2, id: "cmt-other", run: "run-other" };
    const reading = readSnapshot(
      { ...status([R2, other]), runs: [{ id: R2.run, cancelled: false, finished: false }, { id: other.run, cancelled: false, finished: false }] },
      0,
    );
    if (!reading.ok) throw new Error("fixture unreadable");
    const refresh = vi.fn(async () => reading.snapshot);
    const approvals = { snapshot: reading.snapshot, refresh };
    // The native window stays open for the whole test.
    mockInvoke.mockImplementation((cmd: string) => (cmd === "p3_approve" ? new Promise(() => {}) : Promise.resolve({})));
    void reviewInNativeWindow(R2, approvals);
    void reviewInNativeWindow(R2, approvals);
    void reviewInNativeWindow(other, approvals);
    // Let the one fresh read answer and the review be sent.
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(calls("p3_approve")).toEqual([["p3_approve", { commitment: R2.id }]]);
  });
});

// ── Click-time freshness (closure 02) ──────────────────────────────────────
//
// A click re-reads the backend once and sends a review only if that read still
// lists the same waiting R2 request. These checks supplement the backend's own
// authority; they never replace it.

type Snapshot = NonNullable<ReturnType<typeof usePendingOwnerApprovals>["snapshot"]>;

function snapshotOf(raw: unknown): Snapshot {
  const reading = readSnapshot(raw, 0);
  if (!reading.ok) throw new Error("fixture unreadable");
  return reading.snapshot;
}

/** The shared action state as it is now. */
function actionsNow() {
  const { result, unmount } = renderHook(() => useOwnerActions());
  const now = result.current;
  unmount();
  return now;
}

describe("click-time freshness", () => {
  beforeEach(() => {
    resetOwnerActions();
    mockInvoke.mockImplementation(() => Promise.resolve({ text: "done", bytes: null, meta: [] }));
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("H1: refuses unless the fresh read lists the same prepared R2 request on a live run", () => {
    expect(freshReviewRefusal(snapshotOf(status([R2])), R2)).toBeNull();
    const refused: [string, Snapshot | null][] = [
      ["unreadable", null],
      ["emergency stop", snapshotOf(status([R2], { emergency_stopped: true }))],
      ["withdrawn", snapshotOf(status([]))],
      ["other run", snapshotOf({ ...status([{ ...R2, run: "run-other" }]), runs: [{ id: "run-other", cancelled: false, finished: false }] })],
      ["other agent", snapshotOf(status([{ ...R2, agent: "agent-other" }]))],
      ["other kind", snapshotOf(status([{ ...R2, kind: "connector" }]))],
      ["other operation", snapshotOf(status([{ ...R2, operation: "egress.other" }]))],
      ["other target", snapshotOf(status([{ ...R2, target: "https://evil.example:443" }]))],
      ["other binding", snapshotOf(status([{ ...R2, binding: "ffffffffffff" }]))],
      ["now R1", snapshotOf(status([{ ...R2, class: "R1", requires_approval: false }]))],
      ["now R0", snapshotOf(status([{ ...R2, class: "R0", requires_approval: false }]))],
      ["authorized", snapshotOf(status([{ ...R2, state: "authorized" }]))],
      ["expired", snapshotOf(status([{ ...R2, state: "expired" }]))],
      ["run not listed", snapshotOf({ ...status([R2]), runs: [] })],
      ["run stopped", snapshotOf({ ...status([R2]), runs: [{ id: R2.run, cancelled: true, finished: false }] })],
    ];
    for (const [label, fresh] of refused) {
      expect(freshReviewRefusal(fresh, R2), label).not.toBeNull();
    }
  });

  it("H2: a request withdrawn or changed after the card was drawn is never sent", async () => {
    const drawn = snapshotOf(status([R2]));
    const later = [
      snapshotOf(status([])),
      snapshotOf(status([{ ...R2, state: "denied" }])),
      snapshotOf(status([{ ...R2, binding: "ffffffffffff" }])),
    ];
    for (const fresh of later) {
      resetOwnerActions();
      await reviewInNativeWindow(R2, { snapshot: drawn, refresh: async () => fresh });
      const now = actionsNow();
      expect(now.reviewing).toBeNull();
      expect(now.results[0].notSent).toBeTruthy();
      expect(now.results[0].error).toBeNull();
    }
    expect(calls("p3_approve")).toHaveLength(0);
  });

  it("H3: a fresh read that fails or cannot be understood is never sent", async () => {
    const drawn = snapshotOf(status([R2]));
    await reviewInNativeWindow(R2, { snapshot: drawn, refresh: async () => null });
    await reviewInNativeWindow(R2, { snapshot: drawn, refresh: () => Promise.reject(new Error("backend gone")) });
    expect(calls("p3_approve")).toHaveLength(0);
    const now = actionsNow();
    expect(now.results.map((r) => r.reread)).toEqual(["unavailable", "unavailable"]);
    expect(now.results.every((r) => r.notSent !== null)).toBe(true);
    expect(now.reviewing).toBeNull();
  });

  it("H4: a fresh read that answers after the deadline never opens the window", async () => {
    vi.useFakeTimers();
    const drawn = snapshotOf(status([R2]));
    let answer: (value: Snapshot) => void = () => {};
    const late = new Promise<Snapshot>((resolve) => {
      answer = resolve;
    });
    const done = reviewInNativeWindow(R2, { snapshot: drawn, refresh: () => late });
    await vi.advanceTimersByTimeAsync(FRESH_READ_TIMEOUT_MS - 1);
    expect(actionsNow().reviewing).toBe(R2.id);
    await vi.advanceTimersByTimeAsync(1);
    await done;
    expect(actionsNow().reviewing).toBeNull();
    expect(actionsNow().results[0].notSent).toMatch(/could not be read/);
    answer(drawn);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(calls("p3_approve")).toHaveLength(0);
  });

  it("H6: a card the drawn status never listed is refused before any read, whatever a fresh read would say", async () => {
    const refresh = vi.fn(async () => snapshotOf(status([R2])));
    await reviewInNativeWindow(R2, { snapshot: snapshotOf(status([])), refresh });
    await reviewInNativeWindow(R2, { snapshot: null, refresh });
    expect(refresh).not.toHaveBeenCalled();
    expect(calls("p3_approve")).toHaveLength(0);
    expect(actionsNow().results).toHaveLength(0);
  });

  it("H5: clicks during the fresh read cause one read and at most one review", async () => {
    const other: P3Commitment = { ...R2, id: "cmt-other", run: "run-other" };
    const drawn = snapshotOf({
      ...status([R2, other]),
      runs: [{ id: R2.run, cancelled: false, finished: false }, { id: other.run, cancelled: false, finished: false }],
    });
    let answer: (value: Snapshot) => void = () => {};
    const refresh = vi.fn(
      () =>
        new Promise<Snapshot>((resolve) => {
          answer = resolve;
        }),
    );
    mockInvoke.mockImplementation((cmd: string) => (cmd === "p3_approve" ? new Promise(() => {}) : Promise.resolve({})));
    void reviewInNativeWindow(R2, { snapshot: drawn, refresh });
    void reviewInNativeWindow(R2, { snapshot: drawn, refresh });
    void reviewInNativeWindow(other, { snapshot: drawn, refresh });
    expect(actionsNow().reviewSent).toBe(false);
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(calls("p3_approve")).toHaveLength(0);
    answer(drawn);
    await new Promise((resolve) => setTimeout(resolve, 20));
    // The window is open now: still no second read and no second review.
    void reviewInNativeWindow(R2, { snapshot: drawn, refresh });
    void reviewInNativeWindow(other, { snapshot: drawn, refresh });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(calls("p3_approve")).toEqual([["p3_approve", { commitment: R2.id }]]);
    expect(actionsNow().reviewSent).toBe(true);
  });
});

// ── Strict status and superseded reads (integration prep 01) ────────────────
//
// A safety field the presenter cannot read makes the whole status unavailable,
// never permission; and a click-time read that a newer read superseded never
// sends a review. These use the hook's own refresh against a scripted
// p3_status, so supersession is the hook's, not a stand-in's.

const UNREADABLE = { ok: false, reason: "Nexus returned a governed status this page cannot read" };

function statusWithout(field: string): Record<string, unknown> {
  const raw = status([R2]);
  const safety: Record<string, unknown> = { ...raw.status };
  delete safety[field];
  return { ...raw, status: safety };
}

/** p3_status answers only when the test says so; every other command resolves. */
function scriptedReads(): ((value: unknown) => void)[] {
  const reads: ((value: unknown) => void)[] = [];
  mockInvoke.mockImplementation((cmd: string) =>
    cmd === "p3_status"
      ? new Promise((resolve) => {
          reads.push(resolve);
        })
      : Promise.resolve(cmd === "p3_approve" ? { text: "done", bytes: null, meta: [] } : null),
  );
  return reads;
}

async function answer(reads: ((value: unknown) => void)[], index: number, value: unknown) {
  await act(async () => {
    reads[index](value);
    await vi.advanceTimersByTimeAsync(0);
  });
}

/** The hook mounted with R2 waiting in its first read. */
async function mountedWithR2(reads: ((value: unknown) => void)[]) {
  const hook = renderHook(() => usePendingOwnerApprovals());
  await flush();
  await answer(reads, 0, status([R2]));
  expect(hook.result.current.snapshot?.pending.map((c) => c.id)).toEqual([R2.id]);
  return hook;
}

describe("strict status and superseded click-time reads", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    resetOwnerActions();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("I1: a status without platform_supported is unavailable, never permission", async () => {
    expect(readSnapshot(statusWithout("platform_supported"), 0)).toEqual(UNREADABLE);
    mockInvoke.mockImplementation((cmd: string) => Promise.resolve(cmd === "p3_status" ? statusWithout("platform_supported") : {}));
    const { result } = renderHook(() => usePendingOwnerApprovals());
    await flush();
    expect(result.current.availability).toBe("unavailable");
    expect(result.current.snapshot).toBeNull();
    await reviewInNativeWindow(R2, result.current);
    expect(calls("p3_approve")).toHaveLength(0);
  });

  it("I2: a status without emergency_stopped is unavailable, never 'not stopped'", async () => {
    expect(readSnapshot(statusWithout("emergency_stopped"), 0)).toEqual(UNREADABLE);
    mockInvoke.mockImplementation((cmd: string) => Promise.resolve(cmd === "p3_status" ? statusWithout("emergency_stopped") : {}));
    const { result } = renderHook(() => usePendingOwnerApprovals());
    await flush();
    expect(result.current.availability).toBe("unavailable");
    expect(result.current.snapshot).toBeNull();
    await reviewInNativeWindow(R2, result.current);
    expect(calls("p3_approve")).toHaveLength(0);
  });

  it("I3: a malformed or contradictory safety field is unavailable; explicit booleans are read as given", () => {
    const malformed: unknown[] = ["true", "false", 1, 0, null, {}, [], "yes"];
    for (const value of malformed) {
      expect(readSnapshot(status([R2], { platform_supported: value }), 0), `platform_supported ${JSON.stringify(value)}`).toEqual(UNREADABLE);
      expect(readSnapshot(status([R2], { emergency_stopped: value }), 0), `emergency_stopped ${JSON.stringify(value)}`).toEqual(UNREADABLE);
    }
    for (const safety of [[], null, "ok"]) {
      expect(readSnapshot({ ...status([R2]), status: safety }, 0).ok, JSON.stringify(safety)).toBe(false);
    }
    // A malformed commitment still makes the whole answer unreadable.
    expect(readSnapshot(status([R2, { ...R2, id: "cmt-other", class: "R9" }]), 0)).toEqual(UNREADABLE);
    expect(readSnapshot(status([R2], { platform_supported: false }), 0)).toEqual({
      ok: false,
      reason: "governed real-world control is available on Linux only",
    });
    const stopped = readSnapshot(status([R2], { emergency_stopped: true }), 0);
    expect(stopped.ok && stopped.snapshot.emergencyStopped).toBe(true);
    const running = readSnapshot(status([R2]), 0);
    expect(running.ok && running.snapshot.emergencyStopped).toBe(false);
  });

  it("I4: duplicate run identifiers make the status unavailable, agreeing or not", () => {
    const live = { id: R2.run, agent: R2.agent, cancelled: false, finished: false };
    const duplicates = [
      [live, live],
      [live, { ...live, cancelled: true }],
      [{ ...live, cancelled: true }, live],
      [live, { ...live, finished: true }],
    ];
    for (const runs of duplicates) {
      expect(readSnapshot({ ...status([R2]), runs }, 0), JSON.stringify(runs)).toEqual(UNREADABLE);
    }
    const distinct = readSnapshot({ ...status([R2]), runs: [live, { ...live, id: "run-other" }] }, 0);
    expect(distinct.ok && distinct.snapshot.runs.size).toBe(2);
  });

  it("I5: a click-time read superseded by a newer read never sends, even when both list the request", async () => {
    const reads = scriptedReads();
    const { result } = await mountedWithR2(reads);
    const review = reviewInNativeWindow(R2, result.current);
    await flush();
    expect(reads).toHaveLength(2);
    // A newer read begins after the click's and answers first.
    await act(async () => {
      void result.current.refresh();
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(reads).toHaveLength(3);
    await answer(reads, 2, status([R2]));
    // The click's own read answers last, still listing the same waiting request.
    await act(async () => {
      reads[1](status([R2]));
      await review;
    });
    expect(calls("p3_approve")).toHaveLength(0);
    const now = actionsNow();
    expect(now.reviewing).toBeNull();
    expect(now.results[0].notSent).toMatch(/a newer status read replaced this check/);
    expect(now.results[0].reread).toBe("unavailable");
    // The newer answer is the one shown.
    expect(result.current.snapshot?.pending.map((c) => c.id)).toEqual([R2.id]);
  });

  it("I6: a click-time read superseded by a read still under way never sends", async () => {
    const reads = scriptedReads();
    const { result } = await mountedWithR2(reads);
    const review = reviewInNativeWindow(R2, result.current);
    await flush();
    await act(async () => {
      void result.current.refresh();
      await vi.advanceTimersByTimeAsync(0);
    });
    await act(async () => {
      reads[1](status([R2]));
      await review;
    });
    expect(calls("p3_approve")).toHaveLength(0);
    expect(actionsNow().results[0].notSent).toMatch(/a newer status read replaced this check/);
  });

  it("I7: a late click-time answer after the automatic read reported the request withdrawn never sends", async () => {
    const reads = scriptedReads();
    const { result } = await mountedWithR2(reads);
    const review = reviewInNativeWindow(R2, result.current);
    await flush();
    expect(reads).toHaveLength(2);
    // The automatic read comes due while the click's read is outstanding.
    await flush(POLL_INTERVAL_MS);
    expect(reads).toHaveLength(3);
    await answer(reads, 2, status([{ ...R2, state: "denied" }]));
    expect(result.current.snapshot?.pending).toHaveLength(0);
    // The click's read answers late (within the deadline) with the older state.
    await act(async () => {
      reads[1](status([R2]));
      await review;
    });
    expect(calls("p3_approve")).toHaveLength(0);
    expect(actionsNow().results[0].notSent).toMatch(/a newer status read replaced this check/);
    // The older answer replaced nothing, and nothing is sent later.
    expect(result.current.snapshot?.byId.get(R2.id)?.state).toBe("denied");
    await flush(FRESH_READ_TIMEOUT_MS * 4);
    expect(calls("p3_approve")).toHaveLength(0);
  });

  it("I8: one valid fresh R2 review is sent once, and an older read answering later does not block it", async () => {
    const reads = scriptedReads();
    const { result } = await mountedWithR2(reads);
    // An automatic read is under way when the owner clicks.
    await flush(POLL_INTERVAL_MS);
    expect(reads).toHaveLength(2);
    const review = reviewInNativeWindow(R2, result.current);
    await flush();
    expect(reads).toHaveLength(3);
    await answer(reads, 2, status([R2]));
    expect(calls("p3_approve")).toEqual([["p3_approve", { commitment: R2.id }]]);
    // The older automatic read answers afterwards: no second review, no change.
    await answer(reads, 1, status([R2]));
    expect(calls("p3_approve")).toHaveLength(1);
    // The re-read after the review settles it.
    expect(reads).toHaveLength(4);
    await act(async () => {
      reads[3](status([{ ...R2, state: "succeeded" }]));
      await review;
    });
    const now = actionsNow();
    expect(now.results[0]).toMatchObject({ action: "review", error: null, notSent: null, reread: "listed", backendState: "succeeded" });
    expect(now.reviewing).toBeNull();
    expect(calls("p3_approve")).toHaveLength(1);
  });

  it("I9: R0 and R1 commitments are never sent for review, listed so when drawn or at click time", async () => {
    const r0: P3Commitment = { ...R2, id: "cmt-r0", class: "R0", requires_approval: false };
    const r1: P3Commitment = { ...R2, id: "cmt-r1", class: "R1", requires_approval: false };
    const reads = scriptedReads();
    const { result } = renderHook(() => usePendingOwnerApprovals());
    await flush();
    await answer(reads, 0, status([r0, r1]));
    expect(result.current.availability).toBe("available");
    expect(result.current.snapshot?.pending).toHaveLength(0);
    await reviewInNativeWindow(r0, result.current);
    await reviewInNativeWindow(r1, result.current);
    expect(reads).toHaveLength(1);
    // Drawn as R2, listed as R1 by the click's read.
    await flush(POLL_INTERVAL_MS);
    await answer(reads, 1, status([R2]));
    const review = reviewInNativeWindow(R2, result.current);
    await flush();
    await act(async () => {
      reads[2](status([{ ...R2, class: "R1", requires_approval: false }]));
      await review;
    });
    expect(actionsNow().results[0].notSent).toBe("Nexus no longer lists this request as needing your native approval");
    expect(calls("p3_approve")).toHaveLength(0);
  });

  it("I10: no native window is requested without a click, whatever the reads say", async () => {
    let answerNo = 0;
    const answers = [
      status([R2]),
      statusWithout("emergency_stopped"),
      { ...status([R2]), runs: [{ id: R2.run, cancelled: false, finished: false }, { id: R2.run, cancelled: true, finished: false }] },
      status([R2], { platform_supported: "true" }),
      status([R2]),
    ];
    mockInvoke.mockImplementation((cmd: string) =>
      Promise.resolve(cmd === "p3_status" ? answers[answerNo++ % answers.length] : { text: "done", bytes: null, meta: [] }),
    );
    const { result } = renderHook(() => usePendingOwnerApprovals());
    const seen = new Set<string>();
    for (let i = 0; i < 20; i++) {
      await flush(i === 0 ? 0 : POLL_INTERVAL_MS);
      seen.add(result.current.availability);
    }
    expect([...seen].sort()).toEqual(["available", "unavailable"]);
    expect(calls("p3_status").length).toBeGreaterThanOrEqual(20);
    expect(calls("p3_approve")).toHaveLength(0);
    expect(mockInvoke.mock.calls.every((call: unknown[]) => call[0] === "p3_status")).toBe(true);
  });

  it("I11: Stop stays usable while a click-time read is outstanding, and supersedes it", async () => {
    const reads = scriptedReads();
    const { result } = await mountedWithR2(reads);
    const review = reviewInNativeWindow(R2, result.current);
    await flush();
    expect(actionsNow().reviewing).toBe(R2.id);
    // Stop goes out at once: it is not held by the review being checked.
    const stop = stopRun(R2, result.current);
    await flush();
    expect(calls("p3_cancel_run")).toEqual([["p3_cancel_run", { run: R2.run }]]);
    expect(reads).toHaveLength(3);
    // The click's read answers as if from before the stop: still not sent.
    await act(async () => {
      reads[1](status([R2]));
      await review;
    });
    expect(calls("p3_approve")).toHaveLength(0);
    expect(actionsNow().results[0].notSent).toMatch(/a newer status read replaced this check/);
    await act(async () => {
      reads[2]({ ...status([{ ...R2, state: "revoked" }]), runs: [{ id: R2.run, cancelled: true, finished: false }] });
      await stop;
    });
    expect(actionsNow().results[0]).toMatchObject({ action: "stop", error: null, reread: "listed", runCancelled: true });
    expect(calls("p3_approve")).toHaveLength(0);
  });

  it("I12: a request whose run the backend lists as finished is never sent for review", async () => {
    const finished = { ...status([R2]), runs: [{ id: R2.run, cancelled: false, finished: true }] };
    expect(freshReviewRefusal(snapshotOf(finished), R2)).toBe("Nexus lists this request's run as finished");
    vi.useRealTimers();
    await reviewInNativeWindow(R2, { snapshot: snapshotOf(status([R2])), refresh: async () => snapshotOf(finished) });
    expect(actionsNow().results[0].notSent).toBe("Nexus lists this request's run as finished");
    expect(calls("p3_approve")).toHaveLength(0);
  });
});

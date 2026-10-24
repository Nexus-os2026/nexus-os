import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { listen } from "@tauri-apps/api/event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { P3Commitment } from "../../api/backend";
import { mockInvoke } from "../../test/setup";
import ApprovalCenter from "../../pages/ApprovalCenter";
import appSource from "../../App.tsx?raw";
import cardSource from "../ContextualApprovalCard.tsx?raw";
import hookSource from "../../hooks/usePendingOwnerApprovals.ts?raw";
import {
  ContextualApprovalCard,
  GLOBAL_CARD_SUPPRESSED_PAGES,
  GlobalGovernedApprovals,
  LEGACY_CONSENT_LABEL,
  PREVIEW_CAPTION,
} from "../ContextualApprovalCard";
import { InlineApprovalBanner } from "../InlineApprovalBanner";
import {
  FRESH_READ_TIMEOUT_MS,
  readSnapshot,
  resetOwnerActions,
  reviewInNativeWindow,
} from "../../hooks/usePendingOwnerApprovals";

// Synthetic fixtures only. These tests prove what the webview shows and
// sends; they are not evidence of any kernel effect. The backend below is a
// scripted stand-in for p3_status / p3_approve / p3_deny / p3_cancel_run.

const R2: P3Commitment = {
  id: "cmt-0123456789abcdef0123456789abcdef",
  agent: "agent-7f3c",
  run: "run-0123456789abcdef0123456789abcdef",
  kind: "egress",
  class: "R2",
  operation: "egress.request",
  target: "https://example.com:443",
  summary: ["POST https://example.com/x", "Body: 2 bytes (digest 0a1b2c3d4e5f)"],
  state: "prepared",
  requires_approval: true,
  binding: "0a1b2c3d4e5f",
};

const OTHER: P3Commitment = {
  ...R2,
  id: "cmt-fedcba9876543210fedcba9876543210",
  run: "run-fedcba9876543210fedcba9876543210",
  agent: "agent-91aa",
  target: "https://other.example:443",
};

const CONSENT = {
  consent_id: "consent-1",
  agent_id: "agent-legacy-0001",
  agent_name: "Legacy Agent",
  operation_type: "web.post",
  operation_summary: "post a message",
  risk_level: "High",
  fuel_cost_estimate: 0,
  side_effects_preview: [],
  requested_at: new Date().toISOString(),
  auto_deny_at: new Date(Date.now() + 600_000).toISOString(),
  batch_actions: [],
  status: "pending",
  source_surface: "chat",
};

type Reply = (args: Record<string, unknown>) => Promise<unknown>;

interface Backend {
  commitments: P3Commitment[];
  runs: { id: string; agent: string; cancelled: boolean; finished: boolean }[];
  statusFails: boolean;
  /** When set, p3_status answers with this instead (a held or late read). */
  statusOverride: (() => Promise<unknown>) | null;
  approve: Reply;
  deny: Reply;
  cancel: Reply;
  consents: unknown[];
}

let backend: Backend;

function setState(id: string, state: string) {
  backend.commitments = backend.commitments.map((c) => (c.id === id ? { ...c, state } : c));
}

function statusReply() {
  return {
    status: { platform_supported: true, emergency_stopped: false, policy_generation: 0, display: null, tools: [], connector_operations: [] },
    commitments: backend.commitments,
    grants: [],
    runs: backend.runs,
  };
}

function install(commitments: P3Commitment[]) {
  backend = {
    commitments,
    runs: commitments.map((c) => ({ id: c.run, agent: c.agent, cancelled: false, finished: false })),
    statusFails: false,
    statusOverride: null,
    approve: async () => ({ text: "done", bytes: null, meta: [] }),
    deny: async (args) => {
      setState(String(args.commitment), "denied");
      return null;
    },
    cancel: async (args) => {
      backend.runs = backend.runs.map((r) => (r.id === args.run ? { ...r, cancelled: true } : r));
      backend.commitments = backend.commitments.map((c) =>
        c.run === args.run && c.state === "prepared" ? { ...c, state: "revoked" } : c,
      );
      return null;
    },
    consents: [],
  };
  mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
    switch (cmd) {
      case "p3_status":
        if (backend.statusOverride) return backend.statusOverride();
        if (backend.statusFails) return Promise.reject(new Error("governed control: unavailable: backend gone"));
        return Promise.resolve(statusReply());
      case "p3_evidence":
        return Promise.resolve([]);
      case "p3_approve":
        return backend.approve(args ?? {});
      case "p3_deny":
        return backend.deny(args ?? {});
      case "p3_cancel_run":
        return backend.cancel(args ?? {});
      case "list_pending_consents":
        return Promise.resolve(backend.consents);
      case "get_consent_history":
        return Promise.resolve([]);
      case "hitl_stats":
        return Promise.resolve('{"pending_count":0,"approval_rate":1.0,"avg_response_time_ms":0,"total_decisions_today":0}');
      default:
        return Promise.resolve(null);
    }
  });
}

function argsOf(command: string): Record<string, unknown>[] {
  return mockInvoke.mock.calls
    .filter((call: unknown[]) => call[0] === command)
    .map((call: unknown[]) => (call[1] ?? {}) as Record<string, unknown>);
}

function card(id = R2.id): HTMLElement {
  const found = document.querySelector(`[data-testid="governed-approval-card"][data-commitment="${id}"]`);
  if (!found) throw new Error(`no card for ${id}`);
  return found as HTMLElement;
}

function button(scope: HTMLElement, name: string): HTMLButtonElement {
  return within(scope).getByRole("button", { name }) as HTMLButtonElement;
}

const REVIEW = "Review in Nexus approval window";

function never(): Promise<unknown> {
  return new Promise(() => {});
}

describe("Contextual approval card (Phase Three R2, presentation only)", () => {
  beforeEach(() => {
    resetOwnerActions();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("T1: shows a pending R2 on the current page with the backend's identity and the preview caption", async () => {
    install([R2]);
    render(<GlobalGovernedApprovals currentPage="ai-chat-hub" />);
    await waitFor(() => card());
    const text = card().textContent ?? "";
    for (const value of [R2.agent, R2.run, "R2", R2.kind, R2.operation, R2.target, R2.binding, ...R2.summary]) {
      expect(text).toContain(value);
    }
    expect(screen.getByText(PREVIEW_CAPTION)).toBeTruthy();
    expect(PREVIEW_CAPTION).toBe("Preview only. The Nexus native approval window contains the authoritative details.");
    expect(argsOf("p3_approve")).toHaveLength(0);
  });

  it("T2: Review sends exactly the listed commitment id and nothing else", async () => {
    install([R2]);
    render(<GlobalGovernedApprovals currentPage="agents" />);
    await waitFor(() => card());
    fireEvent.click(button(card(), REVIEW));
    await waitFor(() => expect(argsOf("p3_approve")).toHaveLength(1));
    expect(argsOf("p3_approve")).toEqual([{ commitment: R2.id }]);
    expect(Object.keys(argsOf("p3_approve")[0])).toEqual(["commitment"]);
    for (const [cmd, sent] of mockInvoke.mock.calls.map((call: unknown[]) => [call[0], JSON.stringify(call[1] ?? {})])) {
      expect(String(cmd)).toMatch(/^p3_(status|approve)$/);
      expect(sent).not.toMatch(/approv|confirm|binding|grant|allow|true/i);
    }
  });

  it("T3: a double click, a second card or a second presenter never issues a concurrent review", async () => {
    install([R2, OTHER]);
    backend.approve = () => never();
    render(
      <>
        <GlobalGovernedApprovals currentPage="file-manager" />
        <ApprovalCenter />
      </>,
    );
    await waitFor(() => expect(document.querySelectorAll(`[data-commitment="${OTHER.id}"]`)).toHaveLength(2));
    const [globalCard, pageCard] = Array.from(document.querySelectorAll(`[data-commitment="${R2.id}"]`)) as HTMLElement[];
    const review = button(globalCard, REVIEW);
    fireEvent.click(review);
    fireEvent.click(review);
    await waitFor(() => expect(argsOf("p3_approve")).toHaveLength(1));
    // Every other Review, in either presenter, waits for the outstanding one.
    for (const el of Array.from(document.querySelectorAll('[data-testid="governed-approval-card"]')) as HTMLElement[]) {
      expect(button(el, REVIEW).disabled).toBe(true);
      fireEvent.click(button(el, REVIEW));
    }
    fireEvent.click(button(pageCard, REVIEW));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });
    expect(argsOf("p3_approve")).toEqual([{ commitment: R2.id }]);
    expect(within(globalCard).getByText(/Waiting for your answer in the Nexus approval window/)).toBeTruthy();
  });

  it("T4: nothing calls p3_approve on mount, polling, focus, events or route changes", async () => {
    vi.useFakeTimers();
    install([R2, OTHER]);
    const { rerender } = render(<GlobalGovernedApprovals currentPage="ai-chat-hub" />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(document.querySelectorAll('[data-testid="governed-approval-card"]')).toHaveLength(2);
    for (const page of ["chat", "agents", "file-manager", "dashboard", "governed-coding", "settings", "ai-chat-hub"]) {
      rerender(<GlobalGovernedApprovals currentPage={page} />);
      window.dispatchEvent(new Event("focus"));
      window.dispatchEvent(new Event("blur"));
      window.dispatchEvent(new PopStateEvent("popstate"));
      window.dispatchEvent(new HashChangeEvent("hashchange"));
      document.dispatchEvent(new Event("visibilitychange"));
      window.dispatchEvent(new MessageEvent("message", { data: { type: "p3-approval-pending", commitment: R2.id } }));
      window.dispatchEvent(new CustomEvent("p3-approval-pending", { detail: { commitment: R2.id } }));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(17_000);
      });
    }
    // 7 pages × 17 s ≈ 119 s: at most one read per two seconds, no approval.
    expect(argsOf("p3_approve")).toHaveLength(0);
    expect(argsOf("p3_deny")).toHaveLength(0);
    expect(argsOf("p3_cancel_run")).toHaveLength(0);
    const reads = argsOf("p3_status").length;
    expect(reads).toBeGreaterThan(1);
    expect(reads).toBeLessThanOrEqual(1 + Math.floor((7 * 17_000) / 2000));
  });

  it("T5: Deny invokes only p3_deny for that commitment and shows the backend's state", async () => {
    install([R2]);
    render(<GlobalGovernedApprovals currentPage="file-manager" />);
    await waitFor(() => card());
    fireEvent.click(button(card(), "Deny"));
    await waitFor(() => expect(screen.getByTestId("governed-approval-result").textContent).toContain("Nexus now reports: Denied."));
    expect(argsOf("p3_deny")).toEqual([{ commitment: R2.id }]);
    expect(argsOf("p3_approve")).toHaveLength(0);
    expect(argsOf("p3_cancel_run")).toHaveLength(0);
    expect(document.querySelector('[data-testid="governed-approval-card"]')).toBeNull();
    expect(document.body.textContent).not.toMatch(/approved/i);
  });

  it("T6: Stop this run sends the exact run id and stays usable while a review is in flight", async () => {
    install([R2]);
    backend.approve = () => {
      // The owner allowed it; the effect is running and never finishes here.
      setState(R2.id, "executing");
      return never();
    };
    render(<GlobalGovernedApprovals currentPage="agents" />);
    await waitFor(() => card());
    fireEvent.click(button(card(), REVIEW));
    await waitFor(() => expect(argsOf("p3_approve")).toHaveLength(1));
    // The next read lists it executing: the card stays, so its run can be stopped.
    const readsAtClick = argsOf("p3_status").length;
    await waitFor(() => expect(argsOf("p3_status").length).toBeGreaterThan(readsAtClick), { timeout: 4000 });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    // The owner answered: the card shows the backend's state, not "waiting for your answer".
    expect(card().textContent).toContain("Nexus lists this request as executing.");
    expect(card().textContent).not.toContain("Waiting for your answer");
    const stop = button(card(), "Stop this run");
    expect(stop.disabled).toBe(false);
    expect(button(card(), REVIEW).disabled).toBe(true);
    fireEvent.click(stop);
    await waitFor(() => expect(argsOf("p3_cancel_run")).toEqual([{ run: R2.run }]));
    await waitFor(() => expect(screen.getByTestId("governed-approval-result").textContent).toContain("Nexus lists the run as cancelled."));
    expect(argsOf("p3_approve")).toHaveLength(1);
  }, 20_000);

  it("T7: stale and expired requests fail closed", async () => {
    install([R2, OTHER]);
    backend.approve = async (args) => {
      setState(String(args.commitment), "expired");
      throw new Error("governed control: refused: expired");
    };
    render(<GlobalGovernedApprovals currentPage="file-manager" />);
    await waitFor(() => card());
    fireEvent.click(button(card(), REVIEW));
    await waitFor(() => screen.getByTestId("governed-approval-result"));
    const result = screen.getByTestId("governed-approval-result").textContent ?? "";
    expect(result).toContain("Nexus refused: Expired");
    expect(result).toContain("governed control: refused: expired");
    expect(result).toContain("Nexus now reports: Expired.");
    expect(result).not.toMatch(/succeeded|approved/i);
    expect(document.querySelector(`[data-commitment="${R2.id}"]`)).toBeNull();
    expect(argsOf("p3_approve")).toHaveLength(1);
    // A request the backend stops listing as waiting loses its card on the next read, unclicked.
    setState(OTHER.id, "expired");
    await waitFor(() => expect(document.querySelector(`[data-commitment="${OTHER.id}"]`)).toBeNull(), { timeout: 4000 });
    // And a backend that cannot be read leaves no actionable card at all.
    backend.commitments = [{ ...R2, id: "cmt-new0000000000000000000000000000" }];
    await waitFor(() => card("cmt-new0000000000000000000000000000"), { timeout: 4000 });
    backend.statusFails = true;
    await waitFor(() => screen.getByTestId("governed-approvals-unavailable"), { timeout: 4000 });
    expect(document.querySelector('[data-testid="governed-approval-card"]')).toBeNull();
    expect(argsOf("p3_approve")).toHaveLength(1);
  }, 20_000);

  it("T8: changed and replayed requests show the backend's refusal", async () => {
    install([R2, OTHER]);
    backend.approve = async (args) => {
      if (args.commitment === R2.id) {
        setState(R2.id, "failed");
        throw new Error("governed control: refused: parameters_changed");
      }
      // OTHER: the backend still lists it prepared (expiry is applied lazily), but it was consumed.
      throw new Error("governed control: refused: not_pending");
    };
    render(<GlobalGovernedApprovals currentPage="ai-chat-hub" />);
    await waitFor(() => card());
    fireEvent.click(button(card(), REVIEW));
    await waitFor(() => screen.getByTestId("governed-approval-result"));
    let result = screen.getAllByTestId("governed-approval-result")[0].textContent ?? "";
    expect(result).toContain("Changed since it was requested");
    expect(result).toContain("governed control: refused: parameters_changed");
    expect(result).toContain("Nexus now reports: Failed.");
    fireEvent.click(button(card(OTHER.id), REVIEW));
    await waitFor(() => expect(screen.getAllByTestId("governed-approval-result")).toHaveLength(2));
    result = screen.getAllByTestId("governed-approval-result")[0].textContent ?? "";
    expect(result).toContain("No longer pending");
    expect(result).toContain("governed control: refused: not_pending");
    expect(result).toContain("Nexus now reports: Still waiting (prepared).");
    expect(document.body.textContent).not.toMatch(/approved|succeeded/i);
  });

  it("T8b: a reviewed request that succeeded offers no second review", async () => {
    install([R2]);
    backend.approve = async () => {
      setState(R2.id, "succeeded");
      return { text: "done", bytes: null, meta: [] };
    };
    render(<GlobalGovernedApprovals currentPage="agents" />);
    await waitFor(() => card());
    fireEvent.click(button(card(), REVIEW));
    await waitFor(() => screen.getByTestId("governed-approval-result"));
    expect(screen.getByTestId("governed-approval-result").textContent).toContain("Nexus now reports: Succeeded.");
    expect(screen.queryByRole("button", { name: REVIEW })).toBeNull();
    expect(argsOf("p3_approve")).toHaveLength(1);
  });

  it("T9: legacy consent is labelled unverified and its clicks never reach Phase Three", async () => {
    install([R2]);
    backend.consents = [CONSENT];
    render(
      <>
        <InlineApprovalBanner currentPage="file-manager" />
        <GlobalGovernedApprovals currentPage="file-manager" />
      </>,
    );
    await waitFor(() => card());
    await waitFor(() => screen.getByText("post a message"));
    expect(screen.getAllByTestId("legacy-consent-label")[0].textContent).toBe(LEGACY_CONSENT_LABEL);
    expect(LEGACY_CONSENT_LABEL).toBe("Desktop consent (unverified) — does not authorize governed real-world effects.");
    fireEvent.click(screen.getByRole("button", { name: /^Approve$/ }));
    await waitFor(() => expect(argsOf("approve_consent_request")).toHaveLength(1));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });
    expect(argsOf("p3_approve")).toHaveLength(0);
    expect(argsOf("p3_deny")).toHaveLength(0);
    expect(argsOf("p3_cancel_run")).toHaveLength(0);
    // The R2 still waits for the owner's native approval.
    expect(button(card(), REVIEW).disabled).toBe(false);
  });

  it("T9b: the Approvals page separates Phase Three from desktop consent", async () => {
    install([R2]);
    backend.consents = [CONSENT];
    render(<ApprovalCenter />);
    await waitFor(() => card());
    expect(screen.getByText("Governed approvals — Phase Three")).toBeTruthy();
    expect(screen.getByTestId("governed-approvals-section").textContent).not.toContain("post a message");
    expect(screen.getAllByTestId("legacy-consent-label").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText(/not the complete historical ledger/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Approve$/ }));
    await waitFor(() => expect(argsOf("approve_consent_request")).toHaveLength(1));
    expect(argsOf("p3_approve")).toHaveLength(0);
  });

  it("T10: spoofed events and forged cards never become actionable; markup stays text", async () => {
    install([]);
    const listensBefore = vi.mocked(listen).mock.calls.length;
    render(<GlobalGovernedApprovals currentPage="file-manager" />);
    await waitFor(() => expect(argsOf("p3_status").length).toBeGreaterThan(0));
    window.dispatchEvent(new CustomEvent("p3-approval-pending", { detail: R2 }));
    window.dispatchEvent(new MessageEvent("message", { data: R2 }));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });
    // The presenter subscribes to no event at all: only the backend's own list counts.
    const legacy = new Set(["consent-request-pending", "consent-resolved"]);
    expect(vi.mocked(listen).mock.calls.slice(listensBefore).filter((call) => !legacy.has(String(call[0])))).toEqual([]);
    expect(hookSource).not.toMatch(/@tauri-apps\/api\/event|listen\(|addEventListener\("(message|focus)/);
    expect(cardSource).not.toMatch(/@tauri-apps\/api\/event|listen\(|addEventListener/);
    expect(document.querySelector('[data-testid="governed-approval-card"]')).toBeNull();
    // A forged card wired to the real action refuses an id the backend did not list.
    const reading = readSnapshot({ status: { platform_supported: true, emergency_stopped: false }, commitments: [], runs: [] }, 0);
    if (!reading.ok) throw new Error("fixture unreadable");
    const approvals = { snapshot: reading.snapshot, refresh: async () => reading.snapshot };
    render(
      <ContextualApprovalCard
        commitment={OTHER}
        reviewing={false}
        reviewBlocked={false}
        denying={false}
        onReview={() => void reviewInNativeWindow(OTHER, approvals)}
        onDeny={() => {}}
        onStop={() => {}}
      />,
    );
    fireEvent.click(button(card(OTHER.id), REVIEW));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(argsOf("p3_approve")).toHaveLength(0);
    // An R0/R1 commitment waiting to be run never gets a card: p3_approve would just run it.
    backend.commitments = [{ ...R2, id: "cmt-r1", class: "R1", requires_approval: false }];
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 2200));
    });
    expect(document.querySelector('[data-commitment="cmt-r1"]')).toBeNull();
    // Backend text is shown as text, never as markup.
    const hostile = "<img src=x onerror=\"window.__pwned=1\"><script>window.__pwned=1</script><b>bold</b>";
    backend.commitments = [{ ...R2, agent: hostile, target: hostile, operation: hostile, summary: [hostile] }];
    await waitFor(() => card(), { timeout: 4000 });
    const el = card();
    expect(el.querySelector("img, script, b")).toBeNull();
    expect(el.textContent).toContain(hostile);
    expect((window as unknown as { __pwned?: number }).__pwned).toBeUndefined();
    expect(cardSource).not.toContain("dangerouslySetInnerHTML");
    expect(cardSource).not.toMatch(/innerHTML/);
  }, 20_000);

  it("T11: the outcome shown is the backend's state after the action, not the click", async () => {
    install([R2, OTHER]);
    backend.approve = async (args) => {
      // The call resolves, yet the backend records the effect as failed.
      setState(String(args.commitment), args.commitment === R2.id ? "failed" : "prepared");
      if (args.commitment === OTHER.id) {
        backend.commitments = backend.commitments.filter((c) => c.id !== OTHER.id);
      }
      return { text: "done", bytes: null, meta: [] };
    };
    render(<GlobalGovernedApprovals currentPage="file-manager" />);
    await waitFor(() => card());
    fireEvent.click(button(card(), REVIEW));
    await waitFor(() => screen.getByTestId("governed-approval-result"));
    const first = screen.getByTestId("governed-approval-result").textContent ?? "";
    expect(first).toContain("Nexus now reports: Failed.");
    expect(first).not.toMatch(/succeeded|approved/i);
    expect(first).toContain("Nexus does not report that the requesting agent received it.");
    fireEvent.click(button(card(OTHER.id), REVIEW));
    await waitFor(() => expect(screen.getAllByTestId("governed-approval-result")).toHaveLength(2));
    expect(screen.getAllByTestId("governed-approval-result")[0].textContent).toContain(
      "Nexus no longer lists this request; its outcome is not known here.",
    );
    // A re-read that fails reports the outcome as unknown.
    backend.commitments = [{ ...R2, id: "cmt-third000000000000000000000000000" }];
    backend.runs = [{ id: R2.run, agent: R2.agent, cancelled: false, finished: false }];
    await waitFor(() => card("cmt-third000000000000000000000000000"), { timeout: 4000 });
    backend.approve = async () => {
      backend.statusFails = true;
      return { text: "done", bytes: null, meta: [] };
    };
    fireEvent.click(button(card("cmt-third000000000000000000000000000"), REVIEW));
    await waitFor(() => expect(screen.getAllByTestId("governed-approval-result")).toHaveLength(3));
    expect(screen.getAllByTestId("governed-approval-result")[0].textContent).toContain(
      "Nexus could not be read again; the outcome is unknown here.",
    );
    expect(document.body.textContent).not.toMatch(/approved/i);
  }, 20_000);

  it("T12: the Approvals page stays usable when Phase Three cannot be read", async () => {
    // The existing ApprovalCenter fixtures answer every unknown command with {}.
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "list_pending_consents" || cmd === "get_consent_history") return Promise.resolve([]);
      return Promise.resolve({});
    });
    render(<ApprovalCenter />);
    await waitFor(() => screen.getByTestId("governed-approvals-unavailable"));
    await waitFor(() => screen.getByTestId("governed-evidence-unavailable"));
    expect(screen.getByTestId("governed-approvals-section").textContent).not.toMatch(/lists no governed request/);
    expect(argsOf("p3_approve")).toHaveLength(0);
  });

  it("T13: the card is visible on Chat, Agents and Files without changing page", async () => {
    install([R2]);
    const { rerender } = render(<GlobalGovernedApprovals currentPage="ai-chat-hub" />);
    await waitFor(() => card());
    for (const page of ["chat", "agents", "file-manager", "dashboard", "governed-coding"]) {
      rerender(<GlobalGovernedApprovals currentPage={page} />);
      await waitFor(() => card());
      expect(screen.getByTestId("governed-approvals-global-slot")).toBeTruthy();
    }
    // Pages that list these commitments with their own controls show no second copy.
    expect([...GLOBAL_CARD_SUPPRESSED_PAGES]).toEqual(["governed-control", "approvals"]);
    for (const page of GLOBAL_CARD_SUPPRESSED_PAGES) {
      rerender(<GlobalGovernedApprovals currentPage={page} />);
      expect(document.querySelector('[data-testid="governed-approval-card"]')).toBeNull();
    }
    expect(argsOf("p3_approve")).toHaveLength(0);
    // App mounts the one global slot in the shell, outside the routed page.
    const mounts = appSource.split("<GlobalGovernedApprovals currentPage={page} />").length - 1;
    expect(mounts).toBe(1);
    expect(appSource.indexOf("<GlobalGovernedApprovals currentPage={page} />")).toBeGreaterThan(appSource.indexOf("</main>"));
  });
});

// ── Closure 02: bounded-status honesty and click-time freshness ────────────
//
// p3_status returns recent runs and their commitments, not necessarily every
// waiting request; and a click re-reads it once before any review is sent.
// These frontend checks supplement the backend's authority, never replace it.

const UNCONDITIONAL = /Nexus lists no governed request|no governed request waiting for you|Nothing is waiting/i;

async function tick(ms = 0) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

function hold(): { promise: Promise<unknown>; release: (value: unknown) => void } {
  let release: (value: unknown) => void = () => {};
  const promise = new Promise((resolve) => {
    release = resolve;
  });
  return { promise, release };
}

function cards(): HTMLElement[] {
  return Array.from(document.querySelectorAll('[data-testid="governed-approval-card"]')) as HTMLElement[];
}

function notSent(): string {
  return screen.getAllByTestId("governed-approval-not-sent")[0].textContent ?? "";
}

function listRun(c: P3Commitment) {
  return { id: c.run, agent: c.agent, cancelled: false, finished: false };
}

describe("Closure 02: bounded status and click-time freshness", () => {
  beforeEach(() => {
    resetOwnerActions();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("C1: the returned status is described as a bounded recent observation, empty or not", async () => {
    install([{ ...R2, state: "succeeded" }, { ...OTHER, state: "denied" }]);
    render(<ApprovalCenter />);
    const scope = await waitFor(() => screen.getByTestId("governed-approvals-scope"));
    expect(scope.textContent).toMatch(
      /^No governed request is waiting in the status Nexus returned at .+ \(2 recent runs, 2 commitments\)\. That status covers recent runs only and may not include every waiting request\.$/,
    );
    expect(screen.getByTestId("governed-approvals-section").textContent).not.toMatch(UNCONDITIONAL);
    backend.commitments = [R2, { ...OTHER, state: "denied" }];
    await waitFor(() => card(), { timeout: 4000 });
    expect(screen.getByTestId("governed-approvals-scope").textContent).toMatch(
      /^Shown from the status Nexus returned at .+ \(2 recent runs, 2 commitments\)\. That status covers recent runs only and may not include every waiting request\.$/,
    );
    expect(argsOf("p3_approve")).toHaveLength(0);
  });

  it("C2: a waiting request older than the returned window never yields a claim that nothing is waiting", async () => {
    // The backend returns only its most recent runs and their commitments
    // (RealWorld::status). OLDER is still prepared there, outside that window,
    // so the answer below never contains it.
    const OLDER: P3Commitment = { ...R2, id: "cmt-older000000000000000000000000000", run: "run-older" };
    const recent = Array.from({ length: 32 }, (_, i) => ({ ...R2, id: `cmt-recent-${i}`, run: `run-recent-${i}`, state: "succeeded" }));
    install(recent);
    render(
      <>
        <GlobalGovernedApprovals currentPage="ai-chat-hub" />
        <ApprovalCenter />
      </>,
    );
    const scope = await waitFor(() => screen.getByTestId("governed-approvals-scope"));
    expect(scope.textContent).toContain("(32 recent runs, 32 commitments)");
    expect(scope.textContent).toContain("may not include every waiting request");
    // The global slot shows nothing at all rather than "nothing is waiting".
    expect(screen.queryByTestId("governed-approvals-global-slot")).toBeNull();
    expect(document.querySelector(`[data-commitment="${OLDER.id}"]`)).toBeNull();
    expect(document.body.textContent).not.toMatch(UNCONDITIONAL);
    // A hidden global card keeps the same bound in its count.
    backend.commitments = [...recent, R2];
    backend.runs = [...backend.runs, listRun(R2)];
    await waitFor(() => screen.getByTestId("governed-approvals-global-slot"), { timeout: 4000 });
    fireEvent.click(within(screen.getByTestId("governed-approvals-global-slot")).getByRole("button", { name: "Hide" }));
    expect(screen.getByRole("button", { name: "Governed approval waiting (1 in the recent status) — show" })).toBeTruthy();
    expect(argsOf("p3_approve")).toHaveLength(0);
  });

  it("C3: a request withdrawn or settled between render and click cannot start a review", async () => {
    vi.useFakeTimers();
    install([R2]);
    render(<GlobalGovernedApprovals currentPage="agents" />);
    await tick();
    card();
    // Withdrawn after the card was drawn; the next automatic read has not run.
    backend.commitments = [];
    fireEvent.click(button(card(), REVIEW));
    await tick();
    expect(argsOf("p3_approve")).toHaveLength(0);
    expect(notSent()).toBe(
      "Review not sent: Nexus no longer lists this request in the status it returned. The Nexus approval window was not requested.",
    );
    expect(document.querySelector('[data-testid="governed-approval-card"]')).toBeNull();
    // Denied elsewhere after the card was drawn.
    backend.commitments = [OTHER];
    backend.runs = [listRun(OTHER)];
    await tick(2000);
    card(OTHER.id);
    setState(OTHER.id, "denied");
    fireEvent.click(button(card(OTHER.id), REVIEW));
    await tick();
    expect(argsOf("p3_approve")).toHaveLength(0);
    expect(notSent()).toContain("Review not sent: Nexus lists this request as denied.");
    expect(notSent()).toContain("Nexus now reports: Denied.");
    expect(document.body.textContent).not.toMatch(/approved|succeeded|Waiting for your answer/i);
  });

  it("C4: a request whose run or details changed cannot start a review", async () => {
    vi.useFakeTimers();
    install([R2]);
    render(<GlobalGovernedApprovals currentPage="file-manager" />);
    await tick();
    const changes: P3Commitment[] = [
      { ...R2, run: "run-changed" },
      { ...R2, binding: "ffffffffffff" },
      { ...R2, target: "https://changed.example:443" },
      { ...R2, agent: "agent-changed" },
    ];
    for (const changed of changes) {
      backend.commitments = [R2];
      backend.runs = [listRun(R2)];
      await tick(2000);
      expect(card().textContent).toContain(R2.run);
      backend.commitments = [changed];
      backend.runs = [listRun(changed)];
      fireEvent.click(button(card(), REVIEW));
      await tick();
      expect(notSent()).toContain("Review not sent: Nexus now lists this request with a different run or details.");
    }
    expect(argsOf("p3_approve")).toHaveLength(0);
  });

  it("C5: a fresh read that fails, cannot be read or answers late blocks the review", async () => {
    vi.useFakeTimers();
    install([R2]);
    render(<GlobalGovernedApprovals currentPage="agents" />);
    await tick();
    backend.statusFails = true;
    fireEvent.click(button(card(), REVIEW));
    await tick();
    expect(notSent()).toContain("Review not sent: Nexus could not be read just now");
    expect(document.querySelector('[data-testid="governed-approval-card"]')).toBeNull();
    backend.statusFails = false;
    await tick(2000);
    backend.statusOverride = () => Promise.resolve({ ...statusReply(), commitments: [{ ...R2, class: "R3" }] });
    fireEvent.click(button(card(), REVIEW));
    await tick();
    expect(screen.getAllByTestId("governed-approval-not-sent")).toHaveLength(2);
    backend.statusOverride = null;
    await tick(2000);
    // Held past the deadline, then answered: still never sent.
    const late = hold();
    backend.statusOverride = () => late.promise;
    fireEvent.click(button(card(), REVIEW));
    expect(card().textContent).toContain("Checking with Nexus that this request is still waiting…");
    await tick(FRESH_READ_TIMEOUT_MS - 1);
    expect(screen.getAllByTestId("governed-approval-not-sent")).toHaveLength(2);
    await tick(1);
    expect(screen.getAllByTestId("governed-approval-not-sent")).toHaveLength(3);
    expect(notSent()).toContain("Nexus could not be read just now");
    backend.statusOverride = null;
    late.release(statusReply());
    await tick(60_000);
    expect(argsOf("p3_approve")).toHaveLength(0);
  });

  it("C6: R0 and R1 commitments are never offered native review, before or at click time", async () => {
    vi.useFakeTimers();
    const r0: P3Commitment = { ...R2, id: "cmt-r0", class: "R0", requires_approval: false };
    const r1: P3Commitment = { ...OTHER, id: "cmt-r1", class: "R1", requires_approval: false };
    install([r0, r1]);
    render(<GlobalGovernedApprovals currentPage="chat" />);
    await tick();
    expect(argsOf("p3_status").length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: REVIEW })).toBeNull();
    expect(cards()).toHaveLength(0);
    // Drawn as R2, listed as R1 at click time: p3_approve would simply run it.
    backend.commitments = [R2];
    backend.runs = [listRun(R2)];
    await tick(2000);
    backend.commitments = [{ ...R2, class: "R1", requires_approval: false }];
    fireEvent.click(button(card(), REVIEW));
    await tick();
    expect(notSent()).toContain("Review not sent: Nexus no longer lists this request as needing your native approval.");
    expect(argsOf("p3_approve")).toHaveLength(0);
  });

  it("C7: clicks while the fresh read is outstanding cause one read and at most one review", async () => {
    vi.useFakeTimers();
    install([R2, OTHER]);
    backend.approve = () => never();
    render(
      <>
        <GlobalGovernedApprovals currentPage="agents" />
        <GlobalGovernedApprovals currentPage="chat" />
      </>,
    );
    await tick();
    expect(cards()).toHaveLength(4);
    const held = hold();
    backend.statusOverride = () => held.promise;
    const readsBefore = argsOf("p3_status").length;
    const review = button(card(), REVIEW);
    fireEvent.click(review);
    fireEvent.click(review);
    for (const el of cards()) fireEvent.click(button(el, REVIEW));
    expect(argsOf("p3_status")).toHaveLength(readsBefore + 1);
    expect(argsOf("p3_approve")).toHaveLength(0);
    expect(card().textContent).toContain("Checking with Nexus that this request is still waiting…");
    for (const el of cards()) expect(button(el, REVIEW).disabled).toBe(true);
    backend.statusOverride = null;
    held.release(statusReply());
    await tick();
    expect(argsOf("p3_approve")).toEqual([{ commitment: R2.id }]);
    expect(card().textContent).toContain("Waiting for your answer in the Nexus approval window…");
    for (const el of cards()) fireEvent.click(button(el, REVIEW));
    await tick(2000);
    expect(argsOf("p3_approve")).toEqual([{ commitment: R2.id }]);
  });

  it("C8: Deny and Stop stay scoped, and Stop works while Review is being checked", async () => {
    vi.useFakeTimers();
    install([R2, OTHER]);
    render(<GlobalGovernedApprovals currentPage="file-manager" />);
    await tick();
    const held = hold();
    backend.statusOverride = () => held.promise;
    fireEvent.click(button(card(R2.id), REVIEW));
    expect(card(R2.id).textContent).toContain("Checking with Nexus that this request is still waiting…");
    const stop = button(card(R2.id), "Stop this run");
    expect(stop.disabled).toBe(false);
    fireEvent.click(stop);
    fireEvent.click(button(card(OTHER.id), "Deny"));
    await tick();
    expect(argsOf("p3_cancel_run")).toEqual([{ run: R2.run }]);
    expect(argsOf("p3_deny")).toEqual([{ commitment: OTHER.id }]);
    expect(argsOf("p3_approve")).toHaveLength(0);
    // The run was stopped before the fresh read answered, and Stop's own read
    // superseded that fresh read: the review is not sent.
    backend.statusOverride = null;
    held.release(statusReply());
    await tick();
    expect(notSent()).toBe(
      "Review not sent: Nexus could not be read just now, or a newer status read replaced this check, so this request could not be confirmed as still waiting. The Nexus approval window was not requested.",
    );
    expect(document.querySelector(`[data-testid="governed-approval-card"][data-commitment="${R2.id}"]`)).toBeNull();
    expect(argsOf("p3_approve")).toHaveLength(0);
    expect(argsOf("p3_cancel_run")).toEqual([{ run: R2.run }]);
    expect(argsOf("p3_deny")).toEqual([{ commitment: OTHER.id }]);
  });

  it("C9: a blocked review is never retried, and no native window opens without a click", async () => {
    vi.useFakeTimers();
    install([R2]);
    render(<GlobalGovernedApprovals currentPage="ai-chat-hub" />);
    await tick();
    backend.statusFails = true;
    fireEvent.click(button(card(), REVIEW));
    await tick();
    expect(notSent()).toContain("Nexus could not be read just now");
    // Readable again, the same request waiting: nothing is sent on its own.
    backend.statusFails = false;
    for (let i = 0; i < 30; i++) {
      window.dispatchEvent(new Event("focus"));
      document.dispatchEvent(new Event("visibilitychange"));
      await tick(2000);
    }
    card();
    expect(argsOf("p3_approve")).toHaveLength(0);
    // Only a new click sends it, after its own fresh read.
    fireEvent.click(button(card(), REVIEW));
    await tick();
    expect(argsOf("p3_approve")).toEqual([{ commitment: R2.id }]);
  });

  it("C10: legacy consent stays separate from the governed review and its fresh read", async () => {
    install([R2]);
    backend.consents = [CONSENT];
    render(
      <>
        <InlineApprovalBanner currentPage="file-manager" />
        <GlobalGovernedApprovals currentPage="file-manager" />
      </>,
    );
    await waitFor(() => card());
    await waitFor(() => screen.getByText("post a message"));
    fireEvent.click(screen.getByRole("button", { name: /^Approve$/ }));
    await waitFor(() => expect(argsOf("approve_consent_request")).toHaveLength(1));
    // The legacy approval neither starts nor stands in for a governed review.
    expect(card().textContent).not.toContain("Checking with Nexus");
    expect(button(card(), REVIEW).disabled).toBe(false);
    expect(argsOf("p3_approve")).toHaveLength(0);
    for (const command of ["p3_approve", "p3_deny", "p3_cancel_run"]) {
      expect(JSON.stringify(argsOf(command))).not.toContain(CONSENT.consent_id);
    }
    fireEvent.click(button(card(), REVIEW));
    await waitFor(() => expect(argsOf("p3_approve")).toEqual([{ commitment: R2.id }]));
    expect(JSON.stringify(argsOf("approve_consent_request"))).not.toContain(R2.id);
  });
});

// ── Integration prep 01: strict status in the presenter ────────────────────

describe("Integration prep 01: strict status", () => {
  beforeEach(() => {
    resetOwnerActions();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("I13: a status missing a safety field shows UNAVAILABLE and offers no review until a complete one", async () => {
    vi.useFakeTimers();
    install([R2]);
    for (const missing of ["platform_supported", "emergency_stopped"]) {
      backend.statusOverride = () => {
        const reply = statusReply();
        const safety: Record<string, unknown> = { ...reply.status };
        delete safety[missing];
        return Promise.resolve({ ...reply, status: safety });
      };
      const { unmount } = render(<GlobalGovernedApprovals currentPage="agents" />);
      await tick();
      expect(screen.getByTestId("governed-approvals-unavailable").textContent).toContain("status UNAVAILABLE");
      expect(cards()).toHaveLength(0);
      expect(screen.queryByRole("button", { name: REVIEW })).toBeNull();
      // A complete status lists the request again; nothing was sent meanwhile.
      backend.statusOverride = null;
      await tick(2000);
      card();
      expect(screen.queryByTestId("governed-approvals-unavailable")).toBeNull();
      unmount();
    }
    expect(argsOf("p3_approve")).toHaveLength(0);
  });
});

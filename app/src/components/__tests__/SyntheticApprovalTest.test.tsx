import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { P3Commitment } from "../../api/backend";
import { mockInvoke } from "../../test/setup";
import appSource from "../../App.tsx?raw";
import launcherSource from "../SyntheticApprovalTest.tsx?raw";
import { GlobalGovernedApprovals } from "../ContextualApprovalCard";
import {
  SYNTHETIC_COMMAND,
  SYNTHETIC_LAUNCHER_LABEL,
  SYNTHETIC_LAUNCHER_NOTE,
  SYNTHETIC_LAUNCHER_PAGES,
  SyntheticApprovalTest,
  liveSyntheticGrant,
  resetSyntheticLauncher,
} from "../SyntheticApprovalTest";
import { resetOwnerActions } from "../../hooks/usePendingOwnerApprovals";

// P4-UAT05 (N-7 and the launcher's contract). Scripted backend only: these
// tests prove what the webview shows and sends, never a native window, a
// real agent's request or a real-world effect.

const SYNTHETIC_GRANT = {
  id: "grant-0123456789abcdef0123456789abcdef",
  kind: "synthetic",
  lines: [
    "Synthetic test receipts only: Nexus performs no external effect",
    "Usable only by your own commands, never by an agent",
    "Each receipt also needs your approval in a separate Nexus approval window",
    "A receipt is never evidence that a real-world action happened",
  ],
  live: true,
};

const RECEIPT = "rcpt-0123456789abcdef0123456789abcdef";

const PROBE: P3Commitment = {
  id: "cmt-fedcba9876543210fedcba9876543210",
  agent: "owner-session",
  run: "run-fedcba9876543210fedcba9876543210",
  kind: "synthetic",
  class: "R2",
  operation: "synthetic.receipt",
  target: "none (synthetic test receipt; no external target)",
  summary: [
    "SYNTHETIC TEST: Nexus performs no external effect",
    "Allow issues one test receipt; nothing is written outside Nexus's own records",
    "No network, process, browser, connector, display or input action",
    `Receipt: ${RECEIPT}`,
    "A receipt is never evidence that a real-world action happened",
  ],
  state: "prepared",
  requires_approval: true,
  binding: "0a1b2c3d4e5f",
};

interface Backend {
  grants: unknown[];
  commitments: P3Commitment[];
  /** When set, p3_status answers with this instead. */
  status: (() => Promise<unknown>) | null;
  submit: () => Promise<unknown>;
}

let backend: Backend;

function statusReply() {
  return {
    status: { platform_supported: true, emergency_stopped: false, policy_generation: 0, display: null, tools: [], connector_operations: [] },
    commitments: backend.commitments,
    grants: backend.grants,
    runs: backend.commitments.map((c) => ({ id: c.run, agent: c.agent, cancelled: false, finished: false })),
  };
}

function install(grants: unknown[] = [SYNTHETIC_GRANT]) {
  backend = {
    grants,
    commitments: [],
    status: null,
    submit: async () => {
      backend.commitments = [PROBE];
      return { understood: true, commitment: PROBE };
    },
  };
  mockInvoke.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "p3_status":
        return backend.status ? backend.status() : Promise.resolve(statusReply());
      case "p3_submit":
        return backend.submit();
      case "p3_approve":
        return Promise.resolve({ text: `SYNTHETIC TEST RECEIPT ${RECEIPT} for ${PROBE.id}: no external effect was performed.`, bytes: null, meta: [] });
      default:
        return Promise.resolve(null);
    }
  });
}

function calls(command: string): Record<string, unknown>[] {
  return mockInvoke.mock.calls
    .filter((call: unknown[]) => call[0] === command)
    .map((call: unknown[]) => (call[1] ?? {}) as Record<string, unknown>);
}

/** Every command the webview sent other than reads. */
function mutations(): string[] {
  return mockInvoke.mock.calls.map((call: unknown[]) => String(call[0])).filter((cmd: string) => cmd !== "p3_status");
}

function launcher(): HTMLButtonElement | null {
  return screen.queryByRole("button", { name: SYNTHETIC_LAUNCHER_LABEL }) as HTMLButtonElement | null;
}

async function settle(ms = 0) {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, ms));
  });
}

function never(): Promise<unknown> {
  return new Promise(() => {});
}

const OTHER_PAGES = [
  "chat",
  "dashboard",
  "file-manager",
  "governed-control",
  "approvals",
  "governed-coding",
  "settings",
  "terminal",
  "browser",
];

describe("Synthetic approval test launcher (P4-UAT05, N-7)", () => {
  beforeEach(() => {
    resetSyntheticLauncher();
    resetOwnerActions();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("appears only on Chat and Agents, with its wording, while a synthetic grant is live", async () => {
    expect([...SYNTHETIC_LAUNCHER_PAGES]).toEqual(["ai-chat-hub", "agents"]);
    expect(SYNTHETIC_LAUNCHER_LABEL).toBe("Start synthetic approval test");
    expect(SYNTHETIC_LAUNCHER_NOTE).toBe("Your own test command. No external effect.");
    install();
    for (const page of SYNTHETIC_LAUNCHER_PAGES) {
      const { unmount } = render(<SyntheticApprovalTest currentPage={page} />);
      await waitFor(() => expect(launcher()).not.toBeNull());
      expect(screen.getByText(SYNTHETIC_LAUNCHER_NOTE)).toBeTruthy();
      unmount();
    }
    for (const page of [...OTHER_PAGES, undefined]) {
      const { unmount } = render(<SyntheticApprovalTest currentPage={page} />);
      await settle();
      expect(launcher()).toBeNull();
      expect(screen.queryByTestId("synthetic-approval-test")).toBeNull();
      unmount();
    }
    expect(mutations()).toEqual([]);
  });

  it("does not appear without a live synthetic grant", async () => {
    for (const grants of [
      [],
      [{ ...SYNTHETIC_GRANT, live: false }],
      [{ ...SYNTHETIC_GRANT, kind: "egress" }],
      [{ ...SYNTHETIC_GRANT, kind: "perception" }],
    ]) {
      install(grants);
      const { unmount } = render(<SyntheticApprovalTest currentPage="ai-chat-hub" />);
      await settle();
      expect(launcher()).toBeNull();
      unmount();
    }
    expect(calls("p3_submit")).toHaveLength(0);
  });

  it("refuses to appear on a status it cannot read, or one that says governed control is unsupported", async () => {
    const reply = () => statusReply() as Record<string, unknown>;
    const malformed: (() => unknown)[] = [
      () => ({ ...reply(), status: { ...(reply().status as object), platform_supported: undefined } }),
      () => ({ ...reply(), status: { ...(reply().status as object), platform_supported: false } }),
      () => ({ ...reply(), status: { ...(reply().status as object), emergency_stopped: "no" } }),
      () => ({ ...reply(), grants: undefined }),
      () => ({ ...reply(), grants: "synthetic" }),
      () => ({ ...reply(), grants: [{ ...SYNTHETIC_GRANT, live: "true" }] }),
      () => ({ ...reply(), grants: [{ ...SYNTHETIC_GRANT, lines: "one line" }] }),
      () => ({ ...reply(), grants: [{ ...SYNTHETIC_GRANT, id: "" }] }),
      () => ({ ...reply(), grants: [SYNTHETIC_GRANT, null] }),
      () => ({ ...reply(), commitments: [{ ...PROBE, requires_approval: false }] }),
      () => null,
      () => "status",
    ];
    for (const answer of malformed) {
      install();
      backend.status = () => Promise.resolve(answer());
      const { unmount } = render(<SyntheticApprovalTest currentPage="agents" />);
      await settle();
      expect(launcher()).toBeNull();
      unmount();
    }
    install();
    backend.status = () => Promise.reject(new Error("governed control: unavailable"));
    const { unmount } = render(<SyntheticApprovalTest currentPage="agents" />);
    await settle();
    expect(launcher()).toBeNull();
    unmount();
    expect(liveSyntheticGrant(statusReply())).toBe(true);
    expect(calls("p3_submit")).toHaveLength(0);
  });

  it("sends nothing on mount, re-render, page changes, focus, events or time; it does not poll", async () => {
    vi.useFakeTimers();
    install();
    const { rerender } = render(<SyntheticApprovalTest currentPage="ai-chat-hub" />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(launcher()).not.toBeNull();
    const pages = ["agents", "ai-chat-hub", "dashboard", "agents", "agents", "ai-chat-hub"];
    for (const page of pages) {
      rerender(<SyntheticApprovalTest currentPage={page} />);
      window.dispatchEvent(new Event("focus"));
      document.dispatchEvent(new Event("visibilitychange"));
      window.dispatchEvent(new MessageEvent("message", { data: { type: "synthetic-receipt" } }));
      window.dispatchEvent(new CustomEvent("p3-submit", { detail: { text: SYNTHETIC_COMMAND } }));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(30_000);
      });
    }
    expect(mutations()).toEqual([]);
    // One read per entry to Chat or Agents, none in between: the first page,
    // then each change of page that lands on one of them.
    expect(calls("p3_status")).toHaveLength(5);
  });

  it("sends exactly the one constant command on the owner's click, and nothing else", async () => {
    expect(SYNTHETIC_COMMAND).toBe("synthetic-receipt");
    install();
    render(<SyntheticApprovalTest currentPage="ai-chat-hub" />);
    await waitFor(() => expect(launcher()).not.toBeNull());
    fireEvent.click(launcher() as HTMLButtonElement);
    await waitFor(() => expect(screen.getByTestId("synthetic-approval-answer")).toBeTruthy());
    expect(calls("p3_submit")).toEqual([{ envelope: { text: "synthetic-receipt" } }]);
    expect(Object.keys(calls("p3_submit")[0])).toEqual(["envelope"]);
    expect(Object.keys(calls("p3_submit")[0].envelope as object)).toEqual(["text"]);
    // No grant, approval, denial, stop or resume is ever sent by it.
    expect(mutations()).toEqual(["p3_submit"]);
    // It shows the backend's answer: the request's identity and state.
    const answer = screen.getByTestId("synthetic-approval-answer").textContent ?? "";
    expect(answer).toContain(PROBE.id);
    expect(answer).toContain("is prepared");
  });

  it("sends one command for a double click, and none while one is in flight, even across a page change", async () => {
    install();
    backend.submit = () => never();
    const { rerender } = render(<SyntheticApprovalTest currentPage="ai-chat-hub" />);
    await waitFor(() => expect(launcher()).not.toBeNull());
    const button = launcher() as HTMLButtonElement;
    fireEvent.click(button);
    fireEvent.click(button);
    fireEvent.click(button);
    await settle();
    expect(calls("p3_submit")).toHaveLength(1);
    expect((launcher() as HTMLButtonElement).disabled).toBe(true);
    // A page change remounts the launcher; the command in flight still holds.
    rerender(<SyntheticApprovalTest currentPage="agents" />);
    await waitFor(() => expect(launcher()).not.toBeNull());
    fireEvent.click(launcher() as HTMLButtonElement);
    await settle();
    expect(calls("p3_submit")).toHaveLength(1);
  });

  it("shows the backend's refusal truthfully, and never claims a request is waiting", async () => {
    install();
    backend.submit = () => Promise.reject(new Error("governed control: refused: no_covering_grant"));
    const { unmount } = render(<SyntheticApprovalTest currentPage="agents" />);
    await waitFor(() => expect(launcher()).not.toBeNull());
    fireEvent.click(launcher() as HTMLButtonElement);
    await waitFor(() => expect(screen.getByTestId("synthetic-approval-answer")).toBeTruthy());
    let answer = screen.getByTestId("synthetic-approval-answer").textContent ?? "";
    expect(answer).toContain("Nexus refused: governed control: refused: no_covering_grant");
    expect(answer).not.toMatch(/waiting|prepared|approved|succeeded/i);
    unmount();
    install();
    backend.submit = async () => ({ understood: false, reason: "synthetic-receipt takes nothing" });
    render(<SyntheticApprovalTest currentPage="agents" />);
    await waitFor(() => expect(launcher()).not.toBeNull());
    fireEvent.click(launcher() as HTMLButtonElement);
    await waitFor(() => expect(screen.getByTestId("synthetic-approval-answer")).toBeTruthy());
    answer = screen.getByTestId("synthetic-approval-answer").textContent ?? "";
    expect(answer).toContain("Nexus did not take the command: synthetic-receipt takes nothing");
    expect(answer).not.toMatch(/waiting/i);
    expect(document.querySelector('[data-testid="governed-approval-card"]')).toBeNull();
  });

  it("never makes a card itself: the card shows only what Nexus lists", async () => {
    install();
    // The command is accepted, but the status Nexus returns lists nothing.
    backend.submit = async () => ({ understood: true, commitment: PROBE });
    render(
      <>
        <SyntheticApprovalTest currentPage="ai-chat-hub" />
        <GlobalGovernedApprovals currentPage="ai-chat-hub" />
      </>,
    );
    await waitFor(() => expect(launcher()).not.toBeNull());
    fireEvent.click(launcher() as HTMLButtonElement);
    await waitFor(() => expect(screen.getByTestId("synthetic-approval-answer")).toBeTruthy());
    await settle(50);
    expect(document.querySelector('[data-testid="governed-approval-card"]')).toBeNull();
    expect(calls("p3_approve")).toHaveLength(0);
  });

  it("keeps the owner on the page with the chat draft through test, review and result", async () => {
    install();
    function ChatPage() {
      const [draft, setDraft] = useState("");
      return (
        <div>
          <textarea aria-label="chat draft" value={draft} onChange={(e) => setDraft(e.target.value)} />
          <SyntheticApprovalTest currentPage="ai-chat-hub" />
          <GlobalGovernedApprovals currentPage="ai-chat-hub" />
        </div>
      );
    }
    render(<ChatPage />);
    const draft = screen.getByLabelText("chat draft") as HTMLTextAreaElement;
    fireEvent.change(draft, { target: { value: "draft - do not send" } });
    await waitFor(() => expect(launcher()).not.toBeNull());
    fireEvent.click(launcher() as HTMLButtonElement);
    // The card appears from the backend's own listing, on this page.
    await waitFor(() => expect(document.querySelector(`[data-commitment="${PROBE.id}"]`)).not.toBeNull(), { timeout: 4000 });
    expect((screen.getByLabelText("chat draft") as HTMLTextAreaElement).value).toBe("draft - do not send");
    fireEvent.click(screen.getByRole("button", { name: "Review in Nexus approval window" }));
    await waitFor(() => expect(calls("p3_approve")).toEqual([{ commitment: PROBE.id }]));
    await waitFor(() => expect(screen.getByTestId("governed-approval-result")).toBeTruthy());
    expect((screen.getByLabelText("chat draft") as HTMLTextAreaElement).value).toBe("draft - do not send");
    expect(screen.getByLabelText("chat draft")).toBe(draft);
    // Only the owner's two clicks reached the backend: one command, one review.
    expect(mutations()).toEqual(["p3_submit", "p3_approve"]);
  });
});

describe("Synthetic approval test launcher: its sources", () => {
  /** A glob key (relative to this test) as a path under `src/`. */
  const srcPath = (key: string) => new URL(key, "file:///app/src/components/__tests__/here").pathname.replace(/^\/app\//, "");
  const sources = import.meta.glob(["../../**/*.ts", "../../**/*.tsx", "!../../**/__tests__/**", "!../../**/*.test.*"], {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>;

  it("no other p3_submit call site exists: only Governed Control and the launcher, with its constant", () => {
    const files = Object.keys(sources);
    expect(files.length).toBeGreaterThan(50);
    const callers = files.filter((file) => /\bp3Submit\s*\(/.test(sources[file])).map((file) => srcPath(file));
    expect(callers.sort()).toEqual(["src/api/backend.ts", "src/components/SyntheticApprovalTest.tsx", "src/pages/GovernedControl.tsx"]);
    const definitions = sources["../../api/backend.ts"].match(/\bp3Submit\s*\(/g) ?? [];
    expect(definitions).toHaveLength(1);
    expect(sources["../../api/backend.ts"]).toContain("export function p3Submit(text: string)");
    const named = files.filter((file) => sources[file].includes('"p3_submit"')).map((file) => srcPath(file));
    expect(named).toEqual(["src/api/backend.ts"]);
    // In the launcher: one call, with the constant only.
    expect(launcherSource.match(/\bp3Submit\s*\(/g)).toEqual(["p3Submit("]);
    expect(launcherSource).toContain("p3Submit(SYNTHETIC_COMMAND)");
    expect(launcherSource).toContain('export const SYNTHETIC_COMMAND = "synthetic-receipt";');
  });

  it("the launcher reaches nothing but the status read and the one command", () => {
    const imports = launcherSource.match(/^import .*$/gm) ?? [];
    expect(imports).toEqual([
      'import { useCallback, useEffect, useRef, useState } from "react";',
      'import { p3Status, p3Submit } from "../api/backend";',
      'import { readSnapshot } from "../hooks/usePendingOwnerApprovals";',
    ]);
    for (const absent of [
      "p3Approve",
      "p3Deny",
      "p3CancelRun",
      "p3RequestGrant",
      "p3RevokeGrant",
      "p3EmergencyStop",
      "p3Resume",
      "invoke",
      "localStorage",
      "sessionStorage",
      "setInterval",
      "dangerouslySetInnerHTML",
      "listen(",
      "draft",
    ]) {
      expect(launcherSource).not.toContain(absent);
    }
    // App mounts it once, in the shell beside the card, outside the routed page.
    const mount = "<SyntheticApprovalTest currentPage={page} />";
    expect(appSource.split(mount).length - 1).toBe(1);
    expect(appSource.indexOf(mount)).toBeGreaterThan(appSource.indexOf("<GlobalGovernedApprovals currentPage={page} />"));
    expect(appSource.indexOf(mount)).toBeGreaterThan(appSource.indexOf("</main>"));
  });
});

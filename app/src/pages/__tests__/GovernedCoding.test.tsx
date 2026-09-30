import { render, screen, waitFor, fireEvent, act } from "@testing-library/react";
import { describe, it, expect, vi, afterEach } from "vitest";
import {
  mockInvoke,
  mockCommands,
  mockCommandError,
  expectInvoked,
  expectInvokedWith,
} from "../../test/setup";
import GovernedCoding from "../GovernedCoding";
import type { CodingRunStatus } from "../../api/backend";

function status(overrides: Partial<CodingRunStatus> = {}): CodingRunStatus {
  return {
    run_id: "run-1",
    project_id: "proj-1",
    project_name: "demo-project",
    model: "qwen2.5-coder:7b",
    stage: "review",
    message: null,
    run_state: "completed",
    apply_state: "pending",
    worker: { turns: 3, files_read: 4, accepted: ["src/a.ts"], rejected: 1 },
    verification: { passed: true, candidate_short: "cand1234", base_short: "base5678", violations: [] },
    review: {
      binding_short: "bind9999",
      verification_short: null,
      changes: [
        {
          path: "src/a.ts",
          kind: "replace",
          old_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          new_sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
          old_size: 10,
          new_size: 20,
          diff: "--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1 +1 @@\n-old line\n+<script>alert(1)</script>",
          diff_note: null,
          truncated: false,
        },
      ],
    },
    sandbox_verification: null,
    can_apply: true,
    can_restore: false,
    can_discard: true,
    can_verify: false,
    can_retry_verification_cleanup: false,
    ...overrides,
  };
}

const BASE = {
  coding_list_projects: [{ id: "proj-1", name: "demo-project" }],
  coding_list_local_models: ["qwen2.5-coder:7b", "llama3:8b"],
  coding_list_runs: [],
  coding_select_project: { id: "proj-2", name: "picked-project" },
  coding_start_run: { run_id: "run-1" },
  coding_status: status(),
  coding_approve_apply: status({ stage: "applied", can_apply: false, can_restore: true, can_discard: false, message: "Applied 1 file." }),
  coding_restore_run: status({ stage: "restored", can_apply: false, can_restore: false, can_discard: false }),
  coding_discard_run: status({ stage: "discarded", can_apply: false, can_restore: false, can_discard: false }),
};

function lastArgs(command: string): Record<string, unknown> | undefined {
  const calls = mockInvoke.mock.calls.filter((c: unknown[]) => c[0] === command);
  return calls.length ? (calls[calls.length - 1][1] as Record<string, unknown> | undefined) : undefined;
}

function assertNoForbiddenKeys(args: Record<string, unknown> | undefined, forbidden: RegExp) {
  for (const key of Object.keys(args ?? {})) {
    expect(key).not.toMatch(forbidden);
  }
}

async function startRun() {
  await waitFor(() => expect(screen.getByLabelText("Local model")).toHaveValue("qwen2.5-coder:7b"));
  await waitFor(() => expect(screen.getByLabelText("Project")).toHaveValue("proj-1"));
  fireEvent.change(screen.getByLabelText("Task description"), { target: { value: "Add a helper" } });
  fireEvent.click(screen.getByRole("button", { name: "Run" }));
  await waitFor(() => expectInvoked("coding_start_run"));
}

afterEach(() => {
  vi.useRealTimers();
});

describe("GovernedCoding", () => {
  it("renders the numbered steps", async () => {
    mockCommands(BASE);
    render(<GovernedCoding />);
    expect(screen.getByText("Governed Coding")).toBeInTheDocument();
    for (const title of ["Select Project", "Choose Scope", "Describe Task", "Choose Local Model", "Run"]) {
      expect(screen.getByRole("region", { name: new RegExp(`Step \\d: ${title}$`) })).toBeInTheDocument();
    }
    await waitFor(() => expectInvoked("coding_list_local_models"));
  });

  it("Select Project invokes the backend picker without sending any path", async () => {
    mockCommands(BASE);
    render(<GovernedCoding />);
    fireEvent.click(screen.getByRole("button", { name: /Select Project/ }));
    await waitFor(() => expectInvoked("coding_select_project"));
    const args = lastArgs("coding_select_project");
    expect(args === undefined || Object.keys(args).length === 0).toBe(true);
    await waitFor(() => expect(screen.getByLabelText("Project")).toHaveValue("proj-2"));
  });

  it("shows the picker error when no folder is selected", async () => {
    mockCommandError("coding_select_project", "no folder was selected", BASE);
    render(<GovernedCoding />);
    fireEvent.click(screen.getByRole("button", { name: /Select Project/ }));
    await waitFor(() => expect(screen.getByText("no folder was selected")).toBeInTheDocument());
  });

  it("Run is disabled until project, task and model are chosen", async () => {
    mockCommands(BASE);
    render(<GovernedCoding />);
    await waitFor(() => expect(screen.getByLabelText("Local model")).toHaveValue("qwen2.5-coder:7b"));
    expect(screen.getByRole("button", { name: "Run" })).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Task description"), { target: { value: "Do it" } });
    await waitFor(() => expect(screen.getByRole("button", { name: "Run" })).toBeEnabled());
  });

  it("Run sends project id, relative scope, task and model — never a path", async () => {
    mockCommands(BASE);
    render(<GovernedCoding />);
    fireEvent.change(screen.getByPlaceholderText(/leave empty for the whole project/), {
      target: { value: "src, docs " },
    });
    await startRun();
    expectInvokedWith("coding_start_run", {
      projectId: "proj-1",
      project_id: "proj-1",
      writeScope: ["src", "docs"],
      protectedScope: ["tests"],
      task: "Add a helper",
      model: "qwen2.5-coder:7b",
    });
    assertNoForbiddenKeys(lastArgs("coding_start_run"), /path|dir|folder|approv|confirm/i);
  });

  it("blocks absolute or parent paths in scope", async () => {
    mockCommands(BASE);
    render(<GovernedCoding />);
    fireEvent.change(screen.getByPlaceholderText(/leave empty for the whole project/), {
      target: { value: "/etc" },
    });
    fireEvent.change(screen.getByLabelText("Task description"), { target: { value: "Do it" } });
    await waitFor(() => expect(screen.getByText(/is not a relative folder name/)).toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Run" })).toBeDisabled();
  });

  it("renders review changes and diff text literally, not as HTML", async () => {
    mockCommands(BASE);
    const { container } = render(<GovernedCoding />);
    await startRun();
    await waitFor(() => expect(screen.getByText("Ready for your review")).toBeInTheDocument());
    const diff = screen.getByTestId("coding-diff");
    expect(diff.tagName).toBe("PRE");
    expect(diff.textContent).toContain("+<script>alert(1)</script>");
    expect(container.querySelector("script")).toBeNull();
    expect(screen.getByText(/Structural verification: passed/)).toBeInTheDocument();
    expect(screen.getByText("src/a.ts")).toBeInTheDocument();
    expect(screen.getByText(/aaaaaaaaaaaa → bbbbbbbbbbbb/)).toBeInTheDocument();
    expect(screen.getByText(/took 3 turns, read 4 files/)).toBeInTheDocument();
  });

  it("Approve & Apply sends only the run id (no approval flag, no path)", async () => {
    mockCommands(BASE);
    render(<GovernedCoding />);
    await startRun();
    expect(await screen.findByText(/Nexus will ask you to confirm in a system dialog/)).toBeInTheDocument();
    fireEvent.click(await screen.findByRole("button", { name: "Approve & Apply" }));
    await waitFor(() => expectInvoked("coding_approve_apply"));
    const args = lastArgs("coding_approve_apply") ?? {};
    expect(args).toEqual({ runId: "run-1", run_id: "run-1" });
    assertNoForbiddenKeys(args, /approv|confirm|path/i);
    await waitFor(() => expect(screen.getByText("Applied 1 file.")).toBeInTheDocument());
  });

  it("shows Restore This Run only when can_restore", async () => {
    mockCommands(BASE);
    render(<GovernedCoding />);
    await startRun();
    await screen.findByRole("button", { name: "Approve & Apply" });
    expect(screen.queryByRole("button", { name: "Restore This Run" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Approve & Apply" }));
    const restore = await screen.findByRole("button", { name: "Restore This Run" });
    fireEvent.click(restore);
    await waitFor(() => expectInvokedWith("coding_restore_run", { runId: "run-1" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Restore This Run" })).toBeNull());
  });

  it("Discard calls coding_discard_run", async () => {
    mockCommands(BASE);
    render(<GovernedCoding />);
    await startRun();
    fireEvent.click(await screen.findByRole("button", { name: "Discard" }));
    await waitFor(() => expectInvokedWith("coding_discard_run", { runId: "run-1" }));
    await waitFor(() => expect(screen.getAllByText("Run discarded").length).toBeGreaterThan(0));
  });

  it("shows the failure reason of a failed run", async () => {
    mockCommands({
      ...BASE,
      coding_status: status({
        stage: "failed",
        message: "worker exceeded turn budget",
        review: null,
        verification: null,
        worker: null,
        can_apply: false,
        can_discard: false,
      }),
    });
    render(<GovernedCoding />);
    await startRun();
    await waitFor(() => expect(screen.getAllByText("worker exceeded turn budget").length).toBeGreaterThan(0));
    expect(screen.getByRole("region", { name: "Step 9: Result" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Approve & Apply" })).toBeNull();
  });

  it("shows model unavailable error with a Retry button", async () => {
    mockCommandError("coding_list_local_models", "local model unavailable (is Ollama running?)", BASE);
    render(<GovernedCoding />);
    await waitFor(() =>
      expect(screen.getByText("local model unavailable (is Ollama running?)")).toBeInTheDocument(),
    );
    mockCommands(BASE);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(screen.getByLabelText("Local model")).toHaveValue("qwen2.5-coder:7b"));
  });

  it("shows start errors", async () => {
    mockCommandError("coding_start_run", "project is not registered", BASE);
    render(<GovernedCoding />);
    await startRun();
    await waitFor(() => expect(screen.getByText("project is not registered")).toBeInTheDocument());
  });

  it("polls while active and stops once the run settles", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    let calls = 0;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "coding_status") {
        calls += 1;
        return Promise.resolve(calls < 3 ? status({ stage: "working", review: null }) : status());
      }
      if (cmd in BASE) return Promise.resolve((BASE as Record<string, unknown>)[cmd]);
      return Promise.resolve({});
    });
    const { unmount } = render(<GovernedCoding />);
    await startRun();
    await waitFor(() => expect(screen.getByText(/Model is working/)).toBeInTheDocument());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1600);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1600);
    });
    await waitFor(() => expect(screen.getByText("Ready for your review")).toBeInTheDocument());
    const settled = calls;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(calls).toBe(settled);
    unmount();
  });

  // ── Phase Two: sandboxed verification ────────────────────────────────────

  const PROFILE = {
    name: "rust.cargo-test.offline.v1",
    display_name: "Rust library tests (offline)",
    applicable: true,
    reason: null,
  };

  const VERIFIED = {
    phase: "idle",
    result: {
      generation: 1,
      profile: "rust.cargo-test.offline.v1",
      exit: "failed",
      passed: false,
      exit_code: 101,
      signal: null,
      duration_ms: 2400,
      stdout_bytes: 120,
      stdout_truncated: false,
      stderr_bytes: 40,
      stderr_truncated: true,
      cleanup: "confirmed" as const,
      result_short: "res123456789",
      stdout_excerpt: "test tests::answers ... FAILED\n<img src=x onerror=alert(1)>",
      stderr_excerpt: null,
    },
  };

  it("starts sandboxed verification with only the run id and a profile name", async () => {
    mockCommands({
      ...BASE,
      coding_status: status({ can_verify: true }),
      coding_verification_profiles: [PROFILE],
      coding_start_verification: status({
        stage: "verifying",
        can_apply: false,
        can_verify: false,
        sandbox_verification: { phase: "running", result: null },
      }),
    });
    render(<GovernedCoding />);
    await startRun();
    await waitFor(() =>
      expectInvokedWith("coding_verification_profiles", { runId: "run-1", run_id: "run-1" }),
    );
    fireEvent.click(await screen.findByRole("button", { name: "Run Rust library tests (offline)" }));
    await waitFor(() => expectInvoked("coding_start_verification"));
    const args = lastArgs("coding_start_verification") ?? {};
    expect(args).toEqual({ runId: "run-1", run_id: "run-1", profile: "rust.cargo-test.offline.v1" });
    assertNoForbiddenKeys(args, /path|dir|cmd|command|arg|exec|env|network|sandbox|approv|confirm/i);
    await waitFor(() => expect(screen.getByText("Verification: running")).toBeInTheDocument());
  });

  it("shows why a profile does not apply and offers no run", async () => {
    mockCommands({
      ...BASE,
      coding_status: status({ can_verify: true }),
      coding_verification_profiles: [
        { ...PROFILE, applicable: false, reason: "This profile does not apply to the candidate (Dependencies)." },
      ],
    });
    render(<GovernedCoding />);
    await startRun();
    await waitFor(() =>
      expect(screen.getByText(/does not apply to the candidate \(Dependencies\)/)).toBeInTheDocument(),
    );
    expect(screen.queryByRole("button", { name: /^Run Rust/ })).toBeNull();
  });

  it("shows an advisory result with its output as text and keeps Apply available", async () => {
    mockCommands({
      ...BASE,
      coding_status: status({
        sandbox_verification: VERIFIED,
        review: { ...status().review!, verification_short: "res123456789" },
      }),
    });
    const { container } = render(<GovernedCoding />);
    await startRun();
    await waitFor(() => expect(screen.getByText(/Result: failed \(exit status 101\)/)).toBeInTheDocument());
    expect(screen.getByText(/verification res123456789/)).toBeInTheDocument();
    expect(screen.getByText(/errors 40 bytes\s*\(truncated\)/)).toBeInTheDocument();
    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toContain("<img src=x onerror=alert(1)>");
    expect(screen.getByRole("button", { name: "Approve & Apply" })).toBeInTheDocument();
  });

  it("offers a cleanup retry that sends only the run id", async () => {
    mockCommands({
      ...BASE,
      coding_status: status({
        can_apply: false,
        can_retry_verification_cleanup: true,
        sandbox_verification: {
          phase: "cleanup_failed",
          result: { ...VERIFIED.result, exit: "cleanup_failed", exit_code: null, cleanup: "failed" as const },
        },
      }),
      coding_retry_verification_cleanup: status({ message: "The verification's cleanup is now confirmed." }),
    });
    render(<GovernedCoding />);
    await startRun();
    expect(await screen.findByText("Verification: cleanup not confirmed")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Approve & Apply" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Retry verification cleanup" }));
    await waitFor(() =>
      expectInvokedWith("coding_retry_verification_cleanup", { runId: "run-1", run_id: "run-1" }),
    );
    const args = lastArgs("coding_retry_verification_cleanup") ?? {};
    expect(Object.keys(args).sort()).toEqual(["runId", "run_id"]);
  });
});

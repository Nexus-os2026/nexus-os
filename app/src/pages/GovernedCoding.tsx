import { useCallback, useEffect, useState, type ReactNode } from "react";
import {
  codingApproveApply,
  codingDiscardRun,
  codingListLocalModels,
  codingListProjects,
  codingListRuns,
  codingRestoreRun,
  codingRunStatus,
  codingSelectProject,
  codingStartRun,
  type CodingChange,
  type CodingProject,
  type CodingRunStage,
  type CodingRunStatus,
} from "../api/backend";

// Governed Coding — Phase One. The owner picks a project through a native
// folder picker opened by the backend (the UI only ever holds an opaque
// project id), chooses scope as relative folder names, describes a task,
// picks a local model, reviews the proposed changes and approves apply.
// Approval is confirmed by a native OS dialog in the backend; this page never
// sends an approval flag or a filesystem path.

const MAX_TASK_CHARS = 16000;
const POLL_MS = 1500;
const ACTIVE_STAGES: ReadonlySet<CodingRunStage> = new Set<CodingRunStage>([
  "preparing",
  "working",
  "verifying",
  "applying",
  "restoring",
]);
const RESULT_STAGES: ReadonlySet<CodingRunStage> = new Set<CodingRunStage>([
  "no_changes",
  "applied",
  "rolled_back",
  "restored",
  "failed",
  "recovery_required",
  "discarded",
]);

const STAGE_INFO: Record<CodingRunStage, { label: string; tone: string }> = {
  preparing: { label: "Preparing workspace", tone: "text-cyan-200" },
  working: { label: "Model is working", tone: "text-cyan-200" },
  verifying: { label: "Verifying changes", tone: "text-cyan-200" },
  review: { label: "Ready for your review", tone: "text-amber-200" },
  no_changes: { label: "No changes proposed", tone: "text-cyan-100/70" },
  applying: { label: "Applying changes", tone: "text-cyan-200" },
  restoring: { label: "Restoring files", tone: "text-cyan-200" },
  applied: { label: "Changes applied", tone: "text-emerald-300" },
  rolled_back: { label: "Apply rolled back", tone: "text-amber-300" },
  restored: { label: "Run restored (changes undone)", tone: "text-emerald-300" },
  failed: { label: "Run failed", tone: "text-rose-300" },
  recovery_required: { label: "Recovery required", tone: "text-rose-300" },
  discarded: { label: "Run discarded", tone: "text-cyan-100/70" },
};

const PANEL = "nexus-panel rounded-2xl p-5";
const LABEL = "text-xs uppercase tracking-[0.18em] text-cyan-300/60";
const INPUT =
  "w-full rounded-xl border border-cyan-500/20 bg-slate-950/60 px-3 py-2 text-sm text-cyan-50 placeholder:text-cyan-100/30 focus:border-cyan-400/50 focus:outline-none";
const BTN =
  "rounded-full border border-cyan-400/40 bg-cyan-500/10 px-4 py-2 text-sm text-cyan-100 transition hover:bg-cyan-500/20 disabled:cursor-not-allowed disabled:opacity-40";
const BTN_GO =
  "rounded-full border border-emerald-400/40 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-100 transition hover:bg-emerald-500/20 disabled:cursor-not-allowed disabled:opacity-40";
const BTN_WARN =
  "rounded-full border border-rose-400/40 bg-rose-500/10 px-4 py-2 text-sm text-rose-100 transition hover:bg-rose-500/20 disabled:cursor-not-allowed disabled:opacity-40";
const ERROR_BOX = "rounded-xl border border-rose-400/40 bg-rose-500/10 px-3 py-2 text-sm text-rose-100";

function isRunStatus(v: unknown): v is CodingRunStatus {
  return !!v && typeof v === "object" && typeof (v as CodingRunStatus).run_id === "string" &&
    typeof (v as CodingRunStatus).stage === "string";
}

function errorText(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

/** Parse a comma-separated list of relative folder names typed by the owner. */
function parseScope(text: string): { folders: string[]; problem: string | null } {
  const folders = text
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
  for (const f of folders) {
    if (
      f.startsWith("/") ||
      f.startsWith("\\") ||
      f.startsWith("~") ||
      /^[A-Za-z]:/.test(f) ||
      f.split(/[\\/]/).includes("..")
    ) {
      return { folders, problem: `"${f}" is not a relative folder name inside the project (e.g. src, tests).` };
    }
  }
  return { folders, problem: null };
}

function shortHash(h: string | null): string {
  return h ? h.slice(0, 12) : "—";
}

function sizeText(n: number | null): string {
  if (n === null || n === undefined) return "—";
  if (n < 1024) return `${n} B`;
  return `${(n / 1024).toFixed(1)} KB`;
}

function diffLineClass(line: string): string {
  const c = line.charAt(0);
  if (c === "+") return "text-emerald-300";
  if (c === "-") return "text-rose-300";
  if (c === "@") return "text-cyan-300";
  return "text-cyan-100/60";
}

/** Renders diff text as plain text only; each line is a React text node. */
function DiffText({ diff }: { diff: string }) {
  const lines = diff.split("\n");
  return (
    <pre
      data-testid="coding-diff"
      className="mt-2 max-h-[360px] overflow-auto whitespace-pre rounded-xl border border-cyan-500/10 bg-slate-950/70 p-3 font-mono text-xs"
    >
      {lines.map((line, i) => (
        <span key={i} className={`block ${diffLineClass(line)}`}>
          {line.length > 0 ? line : " "}
        </span>
      ))}
    </pre>
  );
}

function ChangeCard({ change }: { change: CodingChange }) {
  return (
    <li className="rounded-2xl border border-cyan-500/15 bg-slate-950/50 p-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span className="break-all font-mono text-sm text-cyan-50">{change.path}</span>
        <span className="rounded-full border border-cyan-400/20 bg-cyan-500/10 px-3 py-1 text-xs uppercase text-cyan-200">
          {change.kind}
        </span>
      </div>
      <div className="mt-2 flex flex-wrap gap-x-6 gap-y-1 font-mono text-xs text-cyan-100/60">
        <span>
          size: {sizeText(change.old_size)} → {sizeText(change.new_size)}
        </span>
        <span>
          sha256: {shortHash(change.old_sha256)} → {shortHash(change.new_sha256)}
        </span>
      </div>
      {change.diff_note ? <p className="mt-2 text-xs text-amber-200/80">{change.diff_note}</p> : null}
      {change.diff ? <DiffText diff={change.diff} /> : null}
      {change.truncated ? <p className="mt-1 text-xs text-amber-200/80">Diff truncated for display.</p> : null}
    </li>
  );
}

function Step({ n, title, children }: { n: number; title: string; children: ReactNode }) {
  return (
    <section className={PANEL} aria-label={`Step ${n}: ${title}`}>
      <h3 className="flex items-center gap-3 text-lg text-cyan-50">
        <span className="flex h-7 w-7 items-center justify-center rounded-full border border-cyan-400/40 text-sm text-cyan-200">
          {n}
        </span>
        {title}
      </h3>
      <div className="mt-4 space-y-3">{children}</div>
    </section>
  );
}

export default function GovernedCoding() {
  const [projects, setProjects] = useState<CodingProject[]>([]);
  const [projectId, setProjectId] = useState("");
  const [projectError, setProjectError] = useState<string | null>(null);
  const [writeScopeText, setWriteScopeText] = useState("");
  const [protectedScopeText, setProtectedScopeText] = useState("tests");
  const [task, setTask] = useState("");
  const [models, setModels] = useState<string[]>([]);
  const [model, setModel] = useState("");
  const [modelsError, setModelsError] = useState<string | null>(null);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [runId, setRunId] = useState<string | null>(null);
  const [run, setRun] = useState<CodingRunStatus | null>(null);
  const [recentRuns, setRecentRuns] = useState<CodingRunStatus[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const loadModels = useCallback(async () => {
    setModelsLoading(true);
    setModelsError(null);
    try {
      const list = await codingListLocalModels();
      const safe = Array.isArray(list) ? list.filter((m): m is string => typeof m === "string") : [];
      setModels(safe);
      setModel((current) => (current && safe.includes(current) ? current : safe[0] ?? ""));
      if (safe.length === 0) setModelsError("No local models were found.");
    } catch (e) {
      setModels([]);
      setModel("");
      setModelsError(errorText(e));
    } finally {
      setModelsLoading(false);
    }
  }, []);

  const loadRecentRuns = useCallback(async () => {
    try {
      const list = await codingListRuns();
      setRecentRuns(Array.isArray(list) ? list.filter(isRunStatus) : []);
    } catch {
      setRecentRuns([]);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    codingListProjects()
      .then((list) => {
        if (cancelled) return;
        const safe = Array.isArray(list) ? list : [];
        setProjects(safe);
        if (safe.length > 0) setProjectId((current) => current || safe[0].id);
      })
      .catch((e) => {
        if (!cancelled) setProjectError(errorText(e));
      });
    void loadModels();
    void loadRecentRuns();
    return () => {
      cancelled = true;
    };
  }, [loadModels, loadRecentRuns]);

  // Poll run status while the run is in an active stage. Stops on any other
  // stage and on unmount.
  const stage = run?.stage;
  useEffect(() => {
    if (!runId) return;
    if (stage && !ACTIVE_STAGES.has(stage)) return;
    let cancelled = false;
    const timer = setInterval(() => {
      codingRunStatus(runId)
        .then((status) => {
          if (!cancelled && isRunStatus(status)) setRun(status);
        })
        .catch((e) => {
          if (!cancelled) setActionError(errorText(e));
        });
    }, POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [runId, stage]);

  // Refresh the recent-runs list whenever a run reaches a settled stage.
  useEffect(() => {
    if (stage && !ACTIVE_STAGES.has(stage)) void loadRecentRuns();
  }, [stage, loadRecentRuns]);

  const selectProject = async () => {
    setProjectError(null);
    setBusy("select");
    try {
      const project = await codingSelectProject();
      if (project && typeof project.id === "string") {
        setProjects((prev) => (prev.some((p) => p.id === project.id) ? prev : [...prev, project]));
        setProjectId(project.id);
      }
    } catch (e) {
      setProjectError(errorText(e));
    } finally {
      setBusy(null);
    }
  };

  const writeScope = parseScope(writeScopeText);
  const protectedScope = parseScope(protectedScopeText);
  const scopeProblem = writeScope.problem ?? protectedScope.problem;
  const runActive = !!runId && (!stage || ACTIVE_STAGES.has(stage));
  const canRun =
    !!projectId && task.trim().length > 0 && !!model && !scopeProblem && !runActive && busy === null;

  const startRun = async () => {
    if (!canRun) return;
    setActionError(null);
    setBusy("start");
    try {
      const res = await codingStartRun({
        projectId,
        writeScope: writeScope.folders,
        protectedScope: protectedScope.folders,
        task,
        model,
      });
      if (!res || typeof res.run_id !== "string") throw new Error("backend did not return a run id");
      setRun(null);
      setRunId(res.run_id);
      try {
        const status = await codingRunStatus(res.run_id);
        if (isRunStatus(status)) setRun(status);
      } catch (e) {
        setActionError(errorText(e));
      }
    } catch (e) {
      setActionError(errorText(e));
    } finally {
      setBusy(null);
    }
  };

  const runAction = async (name: string, fn: (id: string) => Promise<CodingRunStatus>) => {
    if (!runId) return;
    setActionError(null);
    setBusy(name);
    try {
      const status = await fn(runId);
      if (isRunStatus(status)) setRun(status);
    } catch (e) {
      setActionError(errorText(e));
    } finally {
      setBusy(null);
    }
  };

  const openRun = (r: CodingRunStatus) => {
    setActionError(null);
    setRunId(r.run_id);
    setRun(r);
  };

  const stageInfo = run ? STAGE_INFO[run.stage] ?? { label: run.stage, tone: "text-cyan-100" } : null;
  const changes = run?.review?.changes ?? [];

  return (
    <section className="mx-auto flex max-w-5xl flex-col gap-6 px-4 py-6 sm:px-6" style={{ paddingBottom: 80 }}>
      <header>
        <h2 className="nexus-display text-2xl text-cyan-50">Governed Coding</h2>
        <p className="mt-2 text-sm text-cyan-100/60">
          Let a local model propose changes to one of your projects. Nothing is written to your project until you
          review the changes and confirm in a system dialog. Every applied run can be restored.
        </p>
      </header>

      <Step n={1} title="Select Project">
        <div className="flex flex-wrap items-center gap-3">
          <button type="button" className={BTN} onClick={() => void selectProject()} disabled={busy !== null}>
            {busy === "select" ? "Waiting for folder picker…" : "Select Project…"}
          </button>
          {projects.length > 0 ? (
            <select
              aria-label="Project"
              className={`${INPUT} max-w-xs`}
              value={projectId}
              onChange={(e) => setProjectId(e.target.value)}
            >
              {projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          ) : (
            <span className="text-sm text-cyan-100/50">No project selected yet.</span>
          )}
        </div>
        <p className="text-xs text-cyan-100/50">A system folder picker opens so you can choose the project folder.</p>
        {projectError ? <p className={ERROR_BOX}>{projectError}</p> : null}
      </Step>

      <Step n={2} title="Choose Scope">
        <label className="block">
          <span className={LABEL}>Folders the model may change</span>
          <input
            className={`${INPUT} mt-1`}
            value={writeScopeText}
            onChange={(e) => setWriteScopeText(e.target.value)}
            placeholder="e.g. src, docs (leave empty for the whole project)"
          />
        </label>
        <label className="block">
          <span className={LABEL}>Protected folders (never changed)</span>
          <input
            className={`${INPUT} mt-1`}
            value={protectedScopeText}
            onChange={(e) => setProtectedScopeText(e.target.value)}
            placeholder="e.g. tests"
          />
        </label>
        <p className="text-xs text-cyan-100/50">Comma-separated folder names relative to the project.</p>
        {scopeProblem ? <p className={ERROR_BOX}>{scopeProblem}</p> : null}
      </Step>

      <Step n={3} title="Describe Task">
        <textarea
          aria-label="Task description"
          className={`${INPUT} min-h-[120px]`}
          value={task}
          maxLength={MAX_TASK_CHARS}
          onChange={(e) => setTask(e.target.value.slice(0, MAX_TASK_CHARS))}
          placeholder="Describe what you want changed, in plain language."
        />
        <p className="text-right text-xs text-cyan-100/40">
          {task.length} / {MAX_TASK_CHARS}
        </p>
      </Step>

      <Step n={4} title="Choose Local Model">
        {models.length > 0 ? (
          <select aria-label="Local model" className={`${INPUT} max-w-sm`} value={model} onChange={(e) => setModel(e.target.value)}>
            {models.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        ) : null}
        {modelsLoading ? <p className="text-sm text-cyan-100/50">Looking for local models…</p> : null}
        {modelsError ? (
          <div className="flex flex-wrap items-center gap-3">
            <p className={ERROR_BOX}>{modelsError}</p>
            <button type="button" className={BTN} onClick={() => void loadModels()} disabled={modelsLoading}>
              Retry
            </button>
          </div>
        ) : null}
      </Step>

      <Step n={5} title="Run">
        <div className="flex flex-wrap items-center gap-3">
          <button type="button" className={BTN_GO} onClick={() => void startRun()} disabled={!canRun}>
            {busy === "start" ? "Starting…" : "Run"}
          </button>
          {!canRun && !runActive ? (
            <span className="text-xs text-cyan-100/50">Choose a project, describe a task and pick a model first.</span>
          ) : null}
        </div>
        {run && stageInfo ? (
          <div className="rounded-xl border border-cyan-500/15 bg-slate-950/50 px-4 py-3" data-testid="coding-stage">
            <p className={`text-sm ${stageInfo.tone}`}>
              {stageInfo.label}
              {ACTIVE_STAGES.has(run.stage) ? "…" : ""}
            </p>
            <p className="mt-1 font-mono text-xs text-cyan-100/50">
              run {run.run_id} · {run.project_name} · {run.model}
            </p>
            {run.message && !RESULT_STAGES.has(run.stage) ? (
              <p className="mt-2 text-sm text-cyan-100/80">{run.message}</p>
            ) : null}
          </div>
        ) : runId ? (
          <p className="text-sm text-cyan-100/60">Run {runId} started…</p>
        ) : null}
        {actionError ? <p className={ERROR_BOX}>{actionError}</p> : null}
      </Step>

      {run && (run.review || run.verification || run.worker) ? (
        <Step n={6} title="Review Changes">
          {run.worker ? (
            <p className="text-sm text-cyan-100/70">
              The model took {run.worker.turns} turns, read {run.worker.files_read} files, proposed{" "}
              {run.worker.accepted.length} accepted change(s) and had {run.worker.rejected} rejected.
            </p>
          ) : null}
          {run.verification ? (
            <div className="rounded-xl border border-cyan-500/15 bg-slate-950/50 px-4 py-3">
              <p className={`text-sm ${run.verification.passed ? "text-emerald-300" : "text-rose-300"}`}>
                Structural verification: {run.verification.passed ? "passed" : "failed"}
              </p>
              <p className="mt-1 font-mono text-xs text-cyan-100/50">
                base {run.verification.base_short} → candidate {run.verification.candidate_short}
              </p>
              {run.verification.violations.length > 0 ? (
                <ul className="mt-2 list-disc space-y-1 pl-5 text-sm text-rose-200">
                  {run.verification.violations.map((v, i) => (
                    <li key={i}>{v}</li>
                  ))}
                </ul>
              ) : null}
            </div>
          ) : null}
          {run.review ? (
            <>
              <p className="font-mono text-xs text-cyan-100/50">review binding {run.review.binding_short}</p>
              {changes.length === 0 ? (
                <p className="text-sm text-cyan-100/60">No file changes in this review.</p>
              ) : (
                <ul className="space-y-3">
                  {changes.map((c, i) => (
                    <ChangeCard key={`${c.path}-${i}`} change={c} />
                  ))}
                </ul>
              )}
            </>
          ) : null}
          {run.can_discard ? (
            <button
              type="button"
              className={BTN_WARN}
              onClick={() => void runAction("discard", codingDiscardRun)}
              disabled={busy !== null}
            >
              {busy === "discard" ? "Discarding…" : "Discard"}
            </button>
          ) : null}
        </Step>
      ) : null}

      {run?.can_apply ? (
        <Step n={7} title="Approve & Apply">
          <p className="text-sm text-cyan-100/70">
            Nexus will ask you to confirm in a system dialog before anything is written to your project.
          </p>
          <button
            type="button"
            className={BTN_GO}
            onClick={() => void runAction("apply", codingApproveApply)}
            disabled={busy !== null}
          >
            {busy === "apply" ? "Waiting for your confirmation…" : "Approve & Apply"}
          </button>
        </Step>
      ) : null}

      {run && RESULT_STAGES.has(run.stage) ? (
        <Step n={8} title="Result">
          <p className={`text-sm ${stageInfo?.tone ?? ""}`}>{stageInfo?.label ?? run.stage}</p>
          {run.message ? <p className="text-sm text-cyan-100/80">{run.message}</p> : null}
          <p className="font-mono text-xs text-cyan-100/40">
            run state: {run.run_state} · apply state: {run.apply_state}
          </p>
        </Step>
      ) : null}

      {run?.can_restore ? (
        <Step n={9} title="Restore This Run">
          <p className="text-sm text-cyan-100/70">Undo the changes this run applied to your project.</p>
          <button
            type="button"
            className={BTN_WARN}
            onClick={() => void runAction("restore", codingRestoreRun)}
            disabled={busy !== null}
          >
            {busy === "restore" ? "Restoring…" : "Restore This Run"}
          </button>
        </Step>
      ) : null}

      {recentRuns.length > 0 ? (
        <section className={PANEL} aria-label="Recent runs">
          <h3 className="text-lg text-cyan-50">Recent Runs</h3>
          <ul className="mt-3 space-y-2">
            {recentRuns.map((r) => (
              <li
                key={r.run_id}
                className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-cyan-500/10 bg-slate-950/40 px-4 py-2"
              >
                <span className="text-sm text-cyan-100/80">
                  {r.project_name} · {STAGE_INFO[r.stage]?.label ?? r.stage}
                  <span className="ml-2 font-mono text-xs text-cyan-100/40">{r.run_id}</span>
                </span>
                <button type="button" className={BTN} onClick={() => openRun(r)} disabled={busy !== null}>
                  Open
                </button>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </section>
  );
}

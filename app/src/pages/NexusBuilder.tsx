import { useState, useCallback, useRef, useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  conductBuildStreaming,
  builderIterate,
  builderReadPreview,
  builderInitCheckpoint,
  builderGetBudget,
  builderSetBudget,
  builderSetRemaining,
  builderListCheckpoints,
  builderListProjects,
  builderLoadProject,
  builderDeleteProject,
  builderRollback,
  builderRecordBuild,
  builderGeneratePlan,
  builderArchiveProject,
  builderUnarchiveProject,
  builderExportProject,
  hasDesktopRuntime,
} from "../api/backend";
import BuildPlanCard, { type ProductBrief, type AcceptanceCriteria } from "../components/BuildPlanCard";

/* === Design tokens === */
const C = {
  bg: "#0a0e14",
  surface: "#111820",
  surfaceAlt: "#0d1219",
  border: "#1a2332",
  borderFocus: "#2d6a5a",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentBright: "#00f0c0",
  accentDim: "rgba(0,212,170,0.10)",
  accentGlow: "rgba(0,212,170,0.25)",
  err: "#f85149",
  errDim: "rgba(248,81,73,0.08)",
  ok: "#3fb950",
  warn: "#f0c040",
  mono: "'JetBrains Mono','Fira Code','Cascadia Code',monospace",
  sans: "system-ui,-apple-system,sans-serif",
};

/* === Phase definitions === */
const PHASES = ["Analyzing", "Scaffolding", "Styling", "Building", "Scripting", "Finalizing"] as const;

const PHASE_META: Record<string, { label: string; detail: string }> = {
  Analyzing:   { label: "Analyzing your request",    detail: "Parsing prompt, selecting palette and layout" },
  Scaffolding: { label: "Scaffolding HTML structure", detail: "DOCTYPE, head, meta tags, semantic markup" },
  Styling:     { label: "Applying styles",            detail: "CSS variables, dark theme, responsive layout" },
  Building:    { label: "Generating components",      detail: "Sections, content blocks, interactive elements" },
  Scripting:   { label: "Adding interactivity",       detail: "Event handlers, animations, scroll effects" },
  Finalizing:  { label: "Finalizing output",          detail: "Closing tags, validation, governance scan" },
};

/* === Quick start / iteration data === */
const SUGGESTIONS = [
  { label: "Dark portfolio", desc: "Hero, gallery, contact form", accent: "#00d4aa" },
  { label: "SaaS landing page", desc: "Pricing tiers, testimonials", accent: "#f0c040" },
  { label: "Restaurant site", desc: "Menu, reservations", accent: "#60a5fa" },
  { label: "Personal blog", desc: "Dark mode, code blocks", accent: "#a78bfa" },
];
const SUGGESTION_PROMPTS = [
  "Build a dark portfolio website with animated hero, project gallery with hover effects, skills section, and contact form. Ocean blue accent color.",
  "Create a SaaS landing page with hero, feature grid, 3-tier pricing table, testimonials carousel, and CTA. Dark theme with gradient accents.",
  "Build a restaurant website with hero image, menu sections with prices, online reservation form, location map, and hours. Warm dark theme.",
  "Create a personal blog with article cards, tag filtering, dark mode toggle, code syntax highlighting, and about page. Minimal dark design.",
];

const QUICK_ITER = [
  "Change the color scheme",
  "Add a testimonials section",
  "Make it fully responsive",
  "Add scroll animations",
];

/* === Narrative entry type === */
interface NarrativeEntry {
  id: string;
  type: "phase" | "result" | "checkpoint" | "error" | "header" | "user_message";
  label: string;
  detail?: string;
  status: "active" | "complete" | "pending";
  elapsed?: number;
  cost?: number;
  tokens?: number;
}

/* === Derive a short project name from the prompt === */
function deriveProjectName(prompt: string): string {
  const lower = prompt.toLowerCase();
  const w = prompt.trim().split(/\s+/).slice(0, 6).join(" ");
  if (w.length > 40) return w.slice(0, 37) + "...";
  return w;
}

/* ======================================================================= */

export default function NexusBuilder() {
  /* --- core state --- */
  const [prompt, setPrompt] = useState("");
  const [phase, setPhase] = useState<"idle" | "building" | "done" | "error">("idle");
  const [outDir, setOutDir] = useState("");
  const [ver, setVer] = useState(0);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [projectName, setProjectName] = useState("");
  const [projects, setProjects] = useState<any[]>([]);
  const [showProjectList, setShowProjectList] = useState(false);

  const [html, setHtml] = useState("");
  const [viewMode, setViewMode] = useState<"preview" | "code">("preview");
  const [vp, setVp] = useState<"desktop" | "tablet" | "mobile">("desktop");
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);
  const logRef = useRef<HTMLDivElement>(null);
  const busyRef = useRef(false); // synchronous guard against double-fire

  /* --- streaming state --- */
  const [sPct, setSPct] = useState(0);
  const [sTok, setSTok] = useState(0);
  const [sTime, setSTime] = useState(0);
  const [res, setRes] = useState<any>(null);

  /* --- narrative log --- */
  const [narrative, setNarrative] = useState<NarrativeEntry[]>([]);
  const currentPhaseRef = useRef<string>("");

  /* --- iteration --- */
  const [iterTxt, setIterTxt] = useState("");
  const [itering, setItering] = useState(false);

  /* --- plan --- */
  const [planPhase, setPlanPhase] = useState<"idle" | "planning" | "planned" | "approved">("idle");
  const [planData, setPlanData] = useState<{ brief: ProductBrief; criteria: AcceptanceCriteria } | null>(null);
  const [planCost, setPlanCost] = useState(0);
  const [planTime, setPlanTime] = useState(0);
  const [planModel, setPlanModel] = useState("");
  const [planProjectId, setPlanProjectId] = useState("");

  /* --- checkpoints --- */
  const [cps, setCps] = useState<any[]>([]);
  const [curCp, setCurCp] = useState<string | null>(null);

  /* --- budget --- */
  const [budget, setBudget] = useState<any>(null);
  const [showBudgetEdit, setShowBudgetEdit] = useState(false);
  const [editAnthRemaining, setEditAnthRemaining] = useState("");
  const [editAnthInitial, setEditAnthInitial] = useState("");
  const [editOaiRemaining, setEditOaiRemaining] = useState("");
  const [editOaiInitial, setEditOaiInitial] = useState("");

  /* --- helpers --- */
  const rPreview = useCallback(async (d: string) => { try { setHtml(await builderReadPreview(d)); } catch { /* */ } }, []);
  const rCps = useCallback(async (d: string) => { try { const l = await builderListCheckpoints(d); setCps(l ?? []); if (l?.length) setCurCp(l[l.length - 1]?.id ?? null); } catch { /* */ } }, []);
  const rBudget = useCallback(async () => { try { setBudget(await builderGetBudget()); } catch { /* */ } }, []);

  /* Auto-scroll narrative log */
  useEffect(() => {
    if (logRef.current) logRef.current.scrollTop = logRef.current.scrollHeight;
  }, [narrative]);

  /* --- stream listener (stable — mounts once, uses refs for mutable state) --- */
  const iterTxtRef = useRef(iterTxt);
  useEffect(() => { iterTxtRef.current = iterTxt; }, [iterTxt]);

  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    let cancelled = false;
    let ul: UnlistenFn | null = null;
    listen("build-stream", (ev: any) => {
      if (cancelled) return;
      const p = ev.payload;
      if (p.type === "BuildStarted") {
        setSPct(0); setSTok(0); setSTime(0);
        currentPhaseRef.current = "";
        const name = p.project_name || "your site";
        setProjectName(prev => prev || name);
        setNarrative(prev => {
          // If iterating, keep existing narrative and add a separator
          if (prev.length > 0 && prev.some(e => e.type === "result")) {
            return [...prev, {
              id: `header-iter-${Date.now()}`,
              type: "header" as const,
              label: `Iterating: ${iterTxtRef.current || "updating..."}`,
              status: "active" as const,
            }];
          }
          // Plan-approved build — keep user prompt + planning entries, append build header
          if (prev.length > 0 && prev.some(e => e.type === "user_message" || e.id === "phase-planning")) {
            return [...prev, {
              id: "header-build",
              type: "header" as const,
              label: `Building "${name}"`,
              status: "active" as const,
            }];
          }
          // Fresh build (no plan) — start clean
          return [{
            id: "header-build",
            type: "header" as const,
            label: `Building "${name}"`,
            status: "active" as const,
          }];
        });
      } else if (p.type === "GenerationProgress") {
        const phaseKey = p.phase ?? "Building";
        const pct = p.estimated_total_tokens > 0 ? Math.min(99, Math.round((p.tokens_generated / p.estimated_total_tokens) * 100)) : 0;
        setSPct(pct);
        setSTok(p.tokens_generated ?? 0);
        setSTime(p.elapsed_seconds ?? 0);

        // New phase detected
        if (phaseKey !== currentPhaseRef.current) {
          if (currentPhaseRef.current) {
            const prevPhase = currentPhaseRef.current;
            const elapsed = p.elapsed_seconds ?? 0;
            setNarrative(prev => prev.map(e =>
              e.id === `phase-${prevPhase}` && e.status === "active"
                ? { ...e, status: "complete" as const, elapsed }
                : e
            ));
          }
          currentPhaseRef.current = phaseKey;
          const meta = PHASE_META[phaseKey];
          setNarrative(prev => [...prev, {
            id: `phase-${phaseKey}`,
            type: "phase" as const,
            label: meta?.label ?? phaseKey,
            detail: meta?.detail,
            status: "active" as const,
            elapsed: p.elapsed_seconds ?? 0,
            tokens: p.tokens_generated ?? 0,
          }]);
        } else {
          const elapsed = p.elapsed_seconds ?? 0;
          const tokens = p.tokens_generated ?? 0;
          setNarrative(prev => prev.map(e =>
            e.status === "active" && e.type === "phase"
              ? { ...e, elapsed, tokens }
              : e
          ));
        }
      } else if (p.type === "BuildCompleted") {
        if (currentPhaseRef.current) {
          const prevPhase = currentPhaseRef.current;
          const elapsed = p.elapsed_seconds ?? 0;
          setNarrative(prev => prev.map(e =>
            e.id === `phase-${prevPhase}` && e.status === "active"
              ? { ...e, status: "complete" as const, elapsed }
              : e
          ));
        }
        currentPhaseRef.current = "";

        const d = p.output_dir ?? "";
        setPhase("done"); setOutDir(d); setSPct(100); setRes(p); setBusy(false); setItering(false);
        busyRef.current = false;

        setNarrative(prev => {
          const withResult = [...prev, {
            id: `result-${Date.now()}`,
            type: "result" as const,
            label: "Build complete",
            status: "complete" as const,
            elapsed: p.elapsed_seconds ?? 0,
            cost: p.actual_cost ?? 0,
          }];
          const withCp = p.checkpoint_id
            ? [...withResult, {
                id: `cp-${p.checkpoint_id}`,
                type: "checkpoint" as const,
                label: `Checkpoint ${p.checkpoint_id} saved`,
                status: "complete" as const,
              }]
            : withResult;
          return withCp.map(e =>
            e.type === "header" && e.status === "active"
              ? { ...e, status: "complete" as const }
              : e
          );
        });

        if (d) {
          const isIteration = (p.project_name ?? "").startsWith("Iteration:");
          if (isIteration) {
            // Iteration already wrote to current/ and saved its own checkpoint.
            // Do NOT call initCheckpoint — it would copy the original build's
            // index.html back into current/, overwriting the iteration result.
            rPreview(d); rCps(d);
          } else {
            // Initial build — initialize checkpoint structure from build output.
            builderInitCheckpoint(d, d, p.actual_cost ?? 0).then(() => {
              rPreview(d); rCps(d);
            }).catch(() => { rPreview(d); rCps(d); });
          }
        }
        rBudget();
      } else if (p.type === "BuildFailed") {
        if (currentPhaseRef.current) {
          const prevPhase = currentPhaseRef.current;
          setNarrative(prev => prev.map(e =>
            e.id === `phase-${prevPhase}` && e.status === "active"
              ? { ...e, status: "complete" as const }
              : e
          ));
        }
        currentPhaseRef.current = "";
        setPhase("error"); setErr(p.error ?? "Build failed"); setBusy(false); setItering(false);
        busyRef.current = false;
        setNarrative(prev => [...prev, {
          id: `error-${Date.now()}`,
          type: "error" as const,
          label: p.error ?? "Build failed",
          status: "complete" as const,
        }]);
      }
    }).then(fn => { ul = fn; });
    rBudget();
    return () => { cancelled = true; if (ul) ul(); };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /* --- actions --- */

  // Step 1: Generate plan via Haiku
  const doBuild = useCallback(async () => {
    if (!prompt.trim() || busyRef.current) return;
    busyRef.current = true;
    const name = deriveProjectName(prompt);
    setProjectName(name);
    setBusy(true); setErr(""); setHtml("");
    setSPct(0); setSTok(0); setSTime(0);
    setRes(null); setCps([]); setCurCp(null); setVer(0);
    setShowProjectList(false);

    // Create a project ID for artefact storage
    const timestamp = Math.floor(Date.now() / 1000);
    const projId = String(timestamp);
    setPlanProjectId(projId);

    // Show planning phase in narrative
    setPlanPhase("planning");
    setPhase("building");
    setNarrative([
      {
        id: `user-${Date.now()}`,
        type: "user_message" as const,
        label: prompt.trim(),
        status: "complete" as const,
      },
      {
        id: "phase-planning",
        type: "phase" as const,
        label: "Planning your build",
        detail: "Haiku 4.5 analyzing prompt, generating structured plan",
        status: "active" as const,
      },
    ]);

    try {
      const result = await builderGeneratePlan(prompt, projId);
      const plan = result.plan;
      setPlanData({ brief: plan.product_brief, criteria: plan.acceptance_criteria });
      setPlanCost(result.cost_usd ?? 0);
      setPlanTime(result.elapsed_seconds ?? 0);
      setPlanModel(result.model ?? "Haiku 4.5");
      setPlanProjectId(result.project_dir ?? "");
      setPlanPhase("planned");
      setPhase("idle"); // Show plan card, not building spinner
      setBusy(false);
      busyRef.current = false;

      // Update narrative: mark planning complete with model info
      const modelLabel = result.is_local ? `${result.model} (local)` : result.model;
      setNarrative(prev => prev.map(e =>
        e.id === "phase-planning"
          ? { ...e, status: "complete" as const, elapsed: result.elapsed_seconds ?? 0, cost: result.cost_usd ?? 0,
              detail: `${modelLabel} \u00B7 ${(result.elapsed_seconds ?? 0).toFixed(1)}s \u00B7 $${(result.cost_usd ?? 0).toFixed(4)}` }
          : e
      ));
    } catch (e: any) {
      setPlanPhase("idle");
      setPhase("error");
      setErr(e?.message ?? String(e));
      setBusy(false);
      busyRef.current = false;
      setNarrative(prev => [
        ...prev.map(e =>
          e.id === "phase-planning" ? { ...e, status: "complete" as const } : e
        ),
        {
          id: `error-${Date.now()}`,
          type: "error" as const,
          label: `Plan failed: ${e?.message ?? String(e)}`,
          status: "complete" as const,
        },
      ]);
    }
  }, [prompt]);

  // Step 2: Approve plan and start Sonnet build
  const doApprove = useCallback(async (approvedBrief: ProductBrief, approvedCriteria: AcceptanceCriteria) => {
    if (busyRef.current) return;
    busyRef.current = true;
    setPlanPhase("approved");
    setBusy(true);
    setPhase("building");

    // Add approval to narrative
    setNarrative(prev => [...prev, {
      id: `phase-approved-${Date.now()}`,
      type: "phase" as const,
      label: "Plan approved — starting build",
      detail: `Building "${approvedBrief.project_name}" with Sonnet 4.6`,
      status: "complete" as const,
    }]);

    // Use the same project directory where plan artefacts were saved
    const outputDir = planProjectId || undefined;
    try {
      await conductBuildStreaming(
        prompt,
        outputDir,
        "anthropic/claude-sonnet-4-6",
        JSON.stringify(approvedBrief),
        JSON.stringify(approvedCriteria),
      );
    } catch (e: any) {
      setPhase("error");
      setErr(e?.message ?? String(e));
      setBusy(false);
      busyRef.current = false;
    }
  }, [prompt, planProjectId]);

  // Cancel plan: return to idle
  const doCancelPlan = useCallback(() => {
    setPlanPhase("idle");
    setPlanData(null);
    setPlanCost(0);
    setPlanTime(0);
    setPhase("idle");
    setBusy(false);
    busyRef.current = false;
    setNarrative([]);
  }, []);

  const doIter = useCallback(async () => {
    if (!iterTxt.trim() || !outDir || busyRef.current) return;
    busyRef.current = true;
    const changeText = iterTxt.trim();
    // Add user message to narrative before starting
    setNarrative(prev => [...prev, {
      id: `user-${Date.now()}`,
      type: "user_message" as const,
      label: changeText,
      status: "complete" as const,
    }]);
    setItering(true); setBusy(true); setPhase("building"); setErr("");
    setSPct(0); setSTok(0); setSTime(0);
    try {
      await builderIterate(outDir, changeText, "anthropic/claude-sonnet-4-6");
      await rPreview(outDir); await rCps(outDir); setVer(v => v + 1); await rBudget(); setIterTxt("");
    } catch (e: any) {
      setErr(e?.message ?? String(e)); setPhase("error");
    } finally {
      setBusy(false); setItering(false); busyRef.current = false;
    }
  }, [iterTxt, outDir, rPreview, rCps, rBudget]);

  const doRollback = useCallback(async (id: string) => {
    if (!outDir) return;
    try { await builderRollback(outDir, id); await rPreview(outDir); await rCps(outDir); setCurCp(id); }
    catch (e: any) { setErr(e?.message ?? String(e)); }
  }, [outDir, rPreview, rCps]);

  const refreshProjects = useCallback(async (autoShow?: boolean) => {
    try {
      const list = await builderListProjects() ?? [];
      setProjects(list);
      if (autoShow && list.length > 0) setShowProjectList(true);
    } catch { /* */ }
  }, []);

  const doNew = useCallback(async () => {
    busyRef.current = false;
    setPrompt(""); setPhase("idle"); setOutDir(""); setVer(0); setBusy(false); setErr("");
    setHtml(""); setViewMode("preview"); setVp("desktop"); setSPct(0); setSTok(0); setSTime(0);
    setIterTxt(""); setItering(false); setCps([]); setCurCp(null); setRes(null);
    setNarrative([]); setProjectName(""); currentPhaseRef.current = "";
    setPlanPhase("idle"); setPlanData(null); setPlanCost(0); setPlanTime(0); setPlanModel(""); setPlanProjectId("");
    await refreshProjects();
    setShowProjectList(true);
  }, [refreshProjects]);

  const loadProject = useCallback(async (projectId: string) => {
    try {
      const data = await builderLoadProject(projectId);
      if (!data) return;
      setOutDir(data.project_dir ?? "");
      setHtml(data.html ?? "");
      setProjectName(data.meta?.name ?? "");
      setPrompt(data.meta?.prompt ?? "");
      setCps(data.checkpoints ?? []);
      const cpList = data.checkpoints ?? [];
      if (cpList.length) setCurCp(cpList[cpList.length - 1]?.id ?? null);
      setVer(cpList.length > 0 ? cpList.length - 1 : 0);
      setShowProjectList(false);
      setNarrative([]);

      // Restore UI state based on builder_state status
      const state = data.state;
      const status = state?.status;
      if (status === "Planned" && data.plan) {
        // Show the plan card for approval
        setPlanData({ brief: data.plan.product_brief, criteria: data.plan.acceptance_criteria });
        setPlanPhase("planned");
        setPlanProjectId(data.project_dir ?? "");
        setPhase("idle");
        setRes(null);
      } else if (status === "Draft" || status === "PlanFailed") {
        // Show prompt input, pre-fill error if PlanFailed
        setPhase("idle");
        setPlanPhase("idle");
        setRes(null);
        if (status === "PlanFailed" && state?.error_message) {
          setErr(state.error_message);
          setPhase("error");
        }
      } else if (status === "GenerationFailed") {
        // Show error with option to retry
        setPhase("error");
        setErr(state?.error_message ?? "Build failed");
      } else if (status === "IterationFailed") {
        // Show last checkpoint with error
        setPhase("done");
        setRes({ total_lines: data.meta?.lines ?? 0, actual_cost: data.meta?.total_cost ?? 0 });
        setErr(state?.error_message ?? "Iteration failed");
      } else {
        // Generated, Exported, Iterating (interrupted) — show preview
        setPhase("done");
        setRes({ total_lines: data.meta?.lines ?? 0, actual_cost: data.meta?.total_cost ?? 0 });
      }
    } catch { /* */ }
  }, []);

  const deleteProject = useCallback(async (projectId: string) => {
    try { await builderDeleteProject(projectId); await refreshProjects(); } catch { /* */ }
  }, [refreshProjects]);

  const [exportStatus, setExportStatus] = useState<{ msg: string; path?: string } | null>(null);

  const exportProject = useCallback(async (projectId: string) => {
    try {
      const result = await builderExportProject(projectId);
      setExportStatus({ msg: `Exported: ${result.filename}`, path: result.path });
      await refreshProjects();
      setTimeout(() => setExportStatus(null), 8000);
    } catch (e: any) { setExportStatus({ msg: `Export failed: ${e?.message ?? String(e)}` }); setTimeout(() => setExportStatus(null), 5000); }
  }, [refreshProjects]);

  const archiveProject = useCallback(async (projectId: string) => {
    try { await builderArchiveProject(projectId); await refreshProjects(); } catch { /* */ }
  }, [refreshProjects]);

  const unarchiveProject = useCallback(async (projectId: string) => {
    try { await builderUnarchiveProject(projectId); await refreshProjects(); } catch { /* */ }
  }, [refreshProjects]);

  // Load project list on mount
  // On mount, load projects and auto-show list if any exist
  useEffect(() => { refreshProjects(true); }, [refreshProjects]);

  const doDownload = useCallback(() => {
    if (!html) return;
    const b = new Blob([html], { type: "text/html" }); const u = URL.createObjectURL(b);
    const a = document.createElement("a"); a.href = u; a.download = "nexus-build.html";
    document.body.appendChild(a); a.click(); document.body.removeChild(a); URL.revokeObjectURL(u);
  }, [html]);

  const openBudgetEdit = useCallback(() => {
    const ai = budget?.anthropic_initial ?? 5;
    const ar = budget?.anthropic_remaining ?? ai;
    const oi = budget?.openai_initial ?? 10;
    const or_ = budget?.openai_remaining ?? oi;
    setEditAnthInitial(String(ai));
    setEditAnthRemaining(String(Number(ar.toFixed(2))));
    setEditOaiInitial(String(oi));
    setEditOaiRemaining(String(Number(or_.toFixed(2))));
    setShowBudgetEdit(true);
  }, [budget]);

  const saveBudgetEdit = useCallback(async () => {
    const ai = parseFloat(editAnthInitial);
    const ar = parseFloat(editAnthRemaining);
    const oi = parseFloat(editOaiInitial);
    const or_ = parseFloat(editOaiRemaining);
    try {
      if (ai > 0) await builderSetBudget("anthropic", ai);
      if (!isNaN(ar) && ar >= 0) await builderSetRemaining("anthropic", ar);
      if (oi > 0) await builderSetBudget("openai", oi);
      if (!isNaN(or_) && or_ >= 0) await builderSetRemaining("openai", or_);
      await rBudget();
      setShowBudgetEdit(false);
    } catch { /* */ }
  }, [editAnthInitial, editAnthRemaining, editOaiInitial, editOaiRemaining, rBudget]);

  /* --- derived --- */
  const anthSpent = budget?.anthropic_spent ?? 0;
  const anthTotal = budget?.anthropic_initial ?? 0;
  const anthRem = Math.max(0, anthTotal - anthSpent);
  const oaiSpent = budget?.openai_spent ?? 0;
  const oaiTotal = budget?.openai_initial ?? 0;
  const oaiRem = Math.max(0, oaiTotal - oaiSpent);
  const noBudget = anthTotal <= 0 && oaiTotal <= 0;
  const lineCount = html ? html.split("\n").length : 0;
  const r = res;
  const vpMax = vp === "mobile" ? "375px" : vp === "tablet" ? "768px" : "100%";

  const isPreBuild = (phase === "idle" && planPhase === "idle") || (phase === "error" && narrative.length === 0);
  const isPlanned = planPhase === "planned" && planData !== null;
  const isBuilding = phase === "building";
  const isPostBuild = phase === "done" && r;
  const hasNarrative = narrative.length > 0;

  /* === RENDER === */
  return (
    <div style={{ width: "100%", height: "100vh", display: "flex", flexDirection: "column" as const, background: C.bg, color: C.text, fontFamily: C.sans, overflow: "hidden" }}>

      {/* Global animation styles */}
      <style>{`
        @keyframes nbspin{to{transform:rotate(360deg)}}
        @keyframes nbpulse{0%,100%{opacity:1}50%{opacity:0.4}}
        @keyframes nbfadein{from{opacity:0;transform:translateY(4px)}to{opacity:1;transform:translateY(0)}}
      `}</style>

      {/* ==== TOOLBAR ==== */}
      <div style={{ height: 42, minHeight: 42, display: "flex", alignItems: "center", gap: 10, padding: "0 16px", borderBottom: `1px solid ${C.border}`, background: C.surface }}>
        {!showProjectList && (
          <button onClick={() => { refreshProjects(true); }} style={{ background: C.accentDim, color: C.accent, border: `1px solid ${C.border}`, borderRadius: 4, padding: "2px 10px", fontSize: 11, cursor: "pointer", fontWeight: 600 }} title="Back to projects">{"\u2190"} Projects</button>
        )}
        <span style={{ fontWeight: 700, fontSize: 13, color: C.accent, letterSpacing: 0.5 }}>{"\u2726"} NEXUS BUILDER</span>
        {projectName && !showProjectList && (
          <span style={{ fontSize: 11, color: C.muted, maxWidth: 200, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" as const }}>
            {"\u00B7"} {projectName}
          </span>
        )}
        {!showProjectList && (phase !== "idle" || planPhase !== "idle") && (
          <button onClick={doNew} style={{ background: C.accentDim, color: C.accent, border: `1px solid ${C.border}`, borderRadius: 4, padding: "2px 10px", fontSize: 11, cursor: "pointer", fontWeight: 600 }}>+ New</button>
        )}
        {ver > 0 && <span style={{ background: C.accentDim, color: C.accent, padding: "1px 7px", borderRadius: 3, fontSize: 10, fontWeight: 700 }}>v{ver + 1}</span>}
        <div style={{ flex: 1 }} />
        <span style={{ fontSize: 11, color: C.dim }}>Claude Sonnet 4.6</span>
      </div>

      {/* ==== MAIN ==== */}
      <div style={{ display: "flex", flex: 1, overflow: "hidden" }}>

        {/* ---- LEFT PANEL ---- */}
        <div style={{
          width: 400, minWidth: 400, maxWidth: 400,
          borderRight: `1px solid ${C.border}`, background: C.surface,
          display: "flex", flexDirection: "column" as const,
          overflow: "hidden",
        }}>

          {/* Scrollable content area */}
          <div ref={logRef} style={{ flex: 1, overflowY: "auto" as const, padding: "14px 16px", display: "flex", flexDirection: "column" as const }}>

            {/* == PROJECT LIST == */}
            {showProjectList && projects.length > 0 && (
              <div style={{ flexShrink: 0, marginBottom: 12 }}>
                <div style={{ fontSize: 10, fontWeight: 600, color: C.dim, textTransform: "uppercase" as const, letterSpacing: 1.2, marginBottom: 8 }}>Your projects ({projects.length})</div>
                {exportStatus && (
                  <div style={{ fontSize: 10, padding: "6px 8px", borderRadius: 5, marginBottom: 6, background: exportStatus.path ? "rgba(63,185,80,0.08)" : C.errDim, color: exportStatus.path ? C.ok : C.err, border: `1px solid ${exportStatus.path ? "rgba(63,185,80,0.2)" : "rgba(248,81,73,0.2)"}` }}>
                    {exportStatus.msg}
                    {exportStatus.path && <div style={{ fontSize: 9, color: C.muted, marginTop: 2, fontFamily: C.mono, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" as const }}>{exportStatus.path}</div>}
                  </div>
                )}
                <div style={{ display: "flex", flexDirection: "column" as const, gap: 4 }}>
                  {projects.map(proj => {
                    const st = proj.status || "Generated";
                    const isArchived = st === "Archived";
                    const statusColor = st === "Generated" || st === "Exported" ? C.ok : st === "Iterating" || st === "Generating" ? "#60a5fa" : st === "Planned" || st === "Draft" ? C.warn : st === "Archived" ? C.dim : st.includes("Failed") ? C.err : C.muted;
                    return (
                      <div key={proj.project_id || proj.id}
                        onClick={() => !isArchived && loadProject(proj.project_id || proj.id)}
                        style={{ background: C.surfaceAlt, border: `1px solid ${C.border}`, borderRadius: 6, padding: "8px 10px", cursor: isArchived ? "default" : "pointer", transition: "border-color 0.15s, background 0.15s", opacity: isArchived ? 0.5 : 1 }}
                        onMouseEnter={e => { if (!isArchived) { e.currentTarget.style.borderColor = C.accent; e.currentTarget.style.background = "rgba(0,212,170,0.04)"; } }}
                        onMouseLeave={e => { e.currentTarget.style.borderColor = C.border; e.currentTarget.style.background = C.surfaceAlt; }}
                      >
                        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                          <div style={{ flex: 1, minWidth: 0 }}>
                            <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                              <span style={{ fontSize: 12, fontWeight: 600, color: C.text, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" as const }}>{proj.project_name || proj.name || "Untitled"}</span>
                              <span style={{ fontSize: 8, fontWeight: 700, color: statusColor, background: `${statusColor}15`, border: `1px solid ${statusColor}30`, borderRadius: 3, padding: "1px 5px", textTransform: "uppercase" as const, letterSpacing: 0.5, flexShrink: 0 }}>{st}</span>
                            </div>
                            <div style={{ fontSize: 10, color: C.dim, marginTop: 2 }}>
                              {proj.line_count ?? proj.lines ?? 0} lines {"\u00B7"} ${(proj.total_cost ?? 0).toFixed(2)}
                              {(proj.iteration_count ?? 0) > 0 && <> {"\u00B7"} {proj.iteration_count} iters</>}
                              {proj.template && <> {"\u00B7"} {proj.template}</>}
                            </div>
                            {proj.prompt && <div style={{ fontSize: 9, color: C.dim, marginTop: 2, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" as const, maxWidth: 260 }}>{proj.prompt.slice(0, 60)}</div>}
                          </div>
                          <div style={{ display: "flex", gap: 2, flexShrink: 0 }}>
                            {!isArchived && (st === "Generated" || st === "Exported") && (
                              <button
                                onClick={e => { e.stopPropagation(); exportProject(proj.project_id || proj.id); }}
                                style={{ background: "transparent", border: "none", color: C.dim, fontSize: 11, cursor: "pointer", padding: "2px 4px" }}
                                onMouseEnter={e => { e.currentTarget.style.color = C.accent; }}
                                onMouseLeave={e => { e.currentTarget.style.color = C.dim; }}
                                title="Export as ZIP"
                              >{"\u2913"}</button>
                            )}
                            {!isArchived && (st === "Generated" || st === "Exported") && (
                              <button
                                onClick={e => { e.stopPropagation(); archiveProject(proj.project_id || proj.id); }}
                                style={{ background: "transparent", border: "none", color: C.dim, fontSize: 11, cursor: "pointer", padding: "2px 4px" }}
                                onMouseEnter={e => { e.currentTarget.style.color = C.warn; }}
                                onMouseLeave={e => { e.currentTarget.style.color = C.dim; }}
                                title="Archive project"
                              >{"\u2610"}</button>
                            )}
                            {isArchived && (
                              <button
                                onClick={e => { e.stopPropagation(); unarchiveProject(proj.project_id || proj.id); }}
                                style={{ background: "transparent", border: "none", color: C.dim, fontSize: 11, cursor: "pointer", padding: "2px 4px" }}
                                onMouseEnter={e => { e.currentTarget.style.color = C.ok; }}
                                onMouseLeave={e => { e.currentTarget.style.color = C.dim; }}
                                title="Unarchive project"
                              >{"\u21A9"}</button>
                            )}
                            <button
                              onClick={e => { e.stopPropagation(); deleteProject(proj.project_id || proj.id); }}
                              style={{ background: "transparent", border: "none", color: C.dim, fontSize: 11, cursor: "pointer", padding: "2px 4px" }}
                              onMouseEnter={e => { e.currentTarget.style.color = C.err; }}
                              onMouseLeave={e => { e.currentTarget.style.color = C.dim; }}
                              title="Delete project"
                            >{"\u2715"}</button>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
                <button
                  onClick={() => setShowProjectList(false)}
                  style={{ marginTop: 8, width: "100%", padding: "7px 0", background: "transparent", color: C.accent, border: `1px solid ${C.border}`, borderRadius: 6, fontSize: 11, fontWeight: 600, cursor: "pointer" }}
                >
                  + New Project
                </button>
              </div>
            )}

            {/* == PRE-BUILD: Quick start suggestions == */}
            {isPreBuild && !showProjectList && (
              <>
                <div style={{ fontSize: 10, fontWeight: 600, color: C.dim, textTransform: "uppercase" as const, letterSpacing: 1.2, marginBottom: 8 }}>Quick start</div>
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 6, flexShrink: 0 }}>
                  {SUGGESTIONS.map((s, i) => (
                    <button key={i} onClick={() => { setPrompt(SUGGESTION_PROMPTS[i]); taRef.current?.focus(); setShowProjectList(false); }}
                      style={{ display: "flex", alignItems: "flex-start", gap: 8, background: C.surfaceAlt, color: C.text, border: `1px solid ${C.border}`, borderRadius: 6, padding: "8px 9px", fontSize: 11, cursor: "pointer", fontFamily: C.sans, textAlign: "left" as const, lineHeight: 1.3, transition: "border-color 0.15s, background 0.15s" }}
                      onMouseEnter={e => { e.currentTarget.style.borderColor = C.accent; e.currentTarget.style.background = "rgba(0,212,170,0.04)"; }}
                      onMouseLeave={e => { e.currentTarget.style.borderColor = C.border; e.currentTarget.style.background = C.surfaceAlt; }}
                    >
                      <div style={{ width: 4, minWidth: 4, height: "100%", minHeight: 28, borderRadius: 2, background: s.accent, marginTop: 1 }} />
                      <div>
                        <div style={{ fontWeight: 600, fontSize: 11, marginBottom: 1 }}>{s.label}</div>
                        <div style={{ fontSize: 10, color: C.muted }}>{s.desc}</div>
                      </div>
                    </button>
                  ))}
                </div>
              </>
            )}

            {/* == BUILD NARRATIVE LOG == */}
            {hasNarrative && !showProjectList && (
              <div style={{ display: "flex", flexDirection: "column" as const, gap: 0 }}>
                {narrative.map((entry, idx) => (
                  <NarrativeRow key={entry.id} entry={entry} isLast={idx === narrative.length - 1} result={entry.type === "result" ? r : undefined} />
                ))}

                {/* Plan card — shown after planning, before build */}
                {isPlanned && planData && (
                  <BuildPlanCard
                    brief={planData.brief}
                    criteria={planData.criteria}
                    planCost={planCost}
                    planTime={planTime}
                    planModel={planModel}
                    onApprove={doApprove}
                    onCancel={doCancelPlan}
                    disabled={busy}
                  />
                )}

                {/* Progress bar during build */}
                {isBuilding && (
                  <div style={{ marginTop: 10, animation: "nbfadein 0.2s ease" }}>
                    <div style={{ width: "100%", height: 4, background: C.border, borderRadius: 2, overflow: "hidden", marginBottom: 6 }}>
                      <div style={{ width: `${sPct}%`, height: "100%", background: `linear-gradient(90deg, ${C.accent}, ${C.accentBright})`, borderRadius: 2, transition: "width 0.5s ease", boxShadow: `0 0 8px ${C.accentGlow}` }} />
                    </div>
                    <div style={{ display: "flex", justifyContent: "space-between", fontSize: 10, color: C.dim, fontFamily: C.mono }}>
                      <span>${((sPct / 100) * 0.26).toFixed(2)} est. {"\u00B7"} {sTime.toFixed(0)}s elapsed</span>
                      <span>{sPct}%</span>
                    </div>
                  </div>
                )}
              </div>
            )}

            {/* == ERROR == */}
            {err && !showProjectList && !narrative.some(e => e.type === "error") && (
              <div style={{ background: C.errDim, border: `1px solid rgba(248,81,73,0.25)`, borderRadius: 8, padding: "10px 12px", fontSize: 12, color: C.err, wordBreak: "break-word" as const, marginTop: 8, flexShrink: 0 }}>{err}</div>
            )}

            {/* == EXPORT NOTIFICATION == */}
            {exportStatus && !showProjectList && (
              <div style={{ fontSize: 10, padding: "6px 8px", borderRadius: 5, marginTop: 8, flexShrink: 0, background: exportStatus.path ? "rgba(63,185,80,0.08)" : C.errDim, color: exportStatus.path ? C.ok : C.err, border: `1px solid ${exportStatus.path ? "rgba(63,185,80,0.2)" : "rgba(248,81,73,0.2)"}` }}>
                {exportStatus.msg}
                {exportStatus.path && <div style={{ fontSize: 9, color: C.muted, marginTop: 2, fontFamily: C.mono, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" as const }}>{exportStatus.path}</div>}
              </div>
            )}

            {/* == POST-BUILD: Quick edits == */}
            {isPostBuild && !showProjectList && (
              <div style={{ marginTop: 14, flexShrink: 0 }}>
                <div style={{ fontSize: 10, fontWeight: 600, color: C.dim, textTransform: "uppercase" as const, letterSpacing: 1.2, marginBottom: 6 }}>What's next?</div>
                <div style={{ display: "flex", flexWrap: "wrap" as const, gap: 4 }}>
                  {QUICK_ITER.map(q => (
                    <button key={q} onClick={() => { setIterTxt(q); taRef.current?.focus(); }}
                      style={{ background: "transparent", color: C.dim, border: `1px solid ${C.border}`, borderRadius: 10, padding: "3px 10px", fontSize: 10, cursor: "pointer", transition: "color 0.15s, border-color 0.15s" }}
                      onMouseEnter={e => { e.currentTarget.style.color = C.accent; e.currentTarget.style.borderColor = C.accent; }}
                      onMouseLeave={e => { e.currentTarget.style.color = C.dim; e.currentTarget.style.borderColor = C.border; }}>
                      {q}
                    </button>
                  ))}
                </div>
              </div>
            )}

            {/* Budget line — compact, directly after content */}
            {!noBudget && !showBudgetEdit && !showProjectList && (
              <div style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 10, color: C.dim, marginTop: 10, flexShrink: 0 }}>
                <span>Anthropic: <span style={{ fontFamily: C.mono, color: anthRem / anthTotal > 0.2 ? C.muted : C.err }}>${anthRem.toFixed(2)}</span>/${anthTotal.toFixed(2)}</span>
                {oaiTotal > 0 && (
                  <>
                    <span>{"\u00B7"}</span>
                    <span>OpenAI: <span style={{ fontFamily: C.mono, color: oaiRem / oaiTotal > 0.2 ? C.muted : C.err }}>${oaiRem.toFixed(2)}</span>/${oaiTotal.toFixed(2)}</span>
                  </>
                )}
                <div style={{ flex: 1 }} />
                <span style={{ cursor: "pointer", color: C.muted }} onClick={openBudgetEdit}>edit</span>
              </div>
            )}

            {/* Budget setup / edit (inline) */}
            {(noBudget || showBudgetEdit) && !showProjectList && (
              <div style={{ marginTop: 10, flexShrink: 0 }}>
                {noBudget && !showBudgetEdit ? (
                  <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                    <span style={{ fontSize: 10, color: C.muted, flex: 1 }}>Set API budgets to track costs</span>
                    <button onClick={openBudgetEdit}
                      style={{ background: C.accentDim, color: C.accent, border: `1px solid rgba(0,212,170,0.2)`, borderRadius: 5, padding: "4px 10px", fontSize: 10, fontWeight: 600, cursor: "pointer" }}>
                      Configure
                    </button>
                  </div>
                ) : (
                  <div>
                    <div style={{ fontSize: 10, color: C.dim, fontWeight: 600, textTransform: "uppercase" as const, letterSpacing: 1, marginBottom: 6 }}>Edit budgets</div>
                    <div style={{ fontSize: 10, color: C.muted, marginBottom: 3 }}>Anthropic</div>
                    <div style={{ display: "flex", gap: 6, alignItems: "center", marginBottom: 6 }}>
                      <span style={{ fontSize: 10, color: C.dim, minWidth: 46 }}>Initial $</span>
                      <input value={editAnthInitial} onChange={e => setEditAnthInitial(e.target.value)} type="number" min="0" step="1"
                        style={{ flex: 1, background: C.bg, color: C.text, border: `1px solid ${C.border}`, borderRadius: 5, padding: "3px 6px", fontSize: 11, fontFamily: C.mono, outline: "none", boxSizing: "border-box" as const }} />
                      <span style={{ fontSize: 10, color: C.dim, minWidth: 56 }}>Remain $</span>
                      <input value={editAnthRemaining} onChange={e => setEditAnthRemaining(e.target.value)} type="number" min="0" step="0.01"
                        style={{ flex: 1, background: C.bg, color: C.text, border: `1px solid ${C.border}`, borderRadius: 5, padding: "3px 6px", fontSize: 11, fontFamily: C.mono, outline: "none", boxSizing: "border-box" as const }} />
                    </div>
                    <div style={{ fontSize: 10, color: C.muted, marginBottom: 3 }}>OpenAI</div>
                    <div style={{ display: "flex", gap: 6, alignItems: "center", marginBottom: 8 }}>
                      <span style={{ fontSize: 10, color: C.dim, minWidth: 46 }}>Initial $</span>
                      <input value={editOaiInitial} onChange={e => setEditOaiInitial(e.target.value)} type="number" min="0" step="1"
                        style={{ flex: 1, background: C.bg, color: C.text, border: `1px solid ${C.border}`, borderRadius: 5, padding: "3px 6px", fontSize: 11, fontFamily: C.mono, outline: "none", boxSizing: "border-box" as const }} />
                      <span style={{ fontSize: 10, color: C.dim, minWidth: 56 }}>Remain $</span>
                      <input value={editOaiRemaining} onChange={e => setEditOaiRemaining(e.target.value)} type="number" min="0" step="0.01"
                        style={{ flex: 1, background: C.bg, color: C.text, border: `1px solid ${C.border}`, borderRadius: 5, padding: "3px 6px", fontSize: 11, fontFamily: C.mono, outline: "none", boxSizing: "border-box" as const }} />
                    </div>
                    <div style={{ display: "flex", gap: 6 }}>
                      <button onClick={saveBudgetEdit} style={{ flex: 1, background: C.accent, color: "#0a0e14", border: "none", borderRadius: 5, padding: "5px 0", fontSize: 11, fontWeight: 700, cursor: "pointer" }}>Save</button>
                      <button onClick={() => setShowBudgetEdit(false)} style={{ background: "transparent", color: C.dim, border: `1px solid ${C.border}`, borderRadius: 5, padding: "5px 10px", fontSize: 10, cursor: "pointer" }}>{"\u2715"}</button>
                    </div>
                  </div>
                )}
              </div>
            )}

            {/* Spacer — pushes input to bottom */}
            <div style={{ flex: 1, minHeight: 12 }} />
          </div>

          {/* == INPUT AREA (sticky bottom) == */}
          {!showProjectList && (
          <div style={{ flexShrink: 0 }}>
            <div style={{ padding: "10px 16px 14px", borderTop: `1px solid ${C.border}`, background: C.surface }}>
              {isPlanned ? (
                <div style={{ fontSize: 12, color: C.muted, textAlign: "center" as const, padding: "10px 0" }}>
                  Review the plan above, then approve to start building
                </div>
              ) : (isPreBuild || (phase === "error" && !outDir)) ? (
                <>
                  <textarea
                    ref={taRef}
                    value={prompt}
                    onChange={e => setPrompt(e.target.value)}
                    placeholder={"Describe the website you want to build..."}
                    rows={4}
                    style={{ width: "100%", minHeight: 90, background: C.bg, color: C.text, border: `1px solid ${C.border}`, borderRadius: 8, padding: "10px 12px", fontSize: 13, fontFamily: C.sans, resize: "vertical" as const, outline: "none", boxSizing: "border-box" as const, lineHeight: 1.55, transition: "border-color 0.2s" }}
                    onFocus={e => { e.currentTarget.style.borderColor = C.borderFocus; }}
                    onBlur={e => { e.currentTarget.style.borderColor = C.border; }}
                    onKeyDown={e => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) doBuild(); }}
                  />
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: 4, marginBottom: 6 }}>
                    <span style={{ fontSize: 10, color: C.dim }}>Est. ~$0.26 {"\u00B7"} ~70s</span>
                    <span style={{ fontSize: 10, color: C.dim, opacity: 0.6 }}>Ctrl+Enter</span>
                  </div>
                  <button
                    onClick={doBuild}
                    disabled={busy || !prompt.trim()}
                    style={{
                      width: "100%", padding: "14px 0",
                      background: busy || !prompt.trim() ? "#1a2332" : `linear-gradient(135deg, ${C.accent}, ${C.accentBright})`,
                      color: busy || !prompt.trim() ? C.dim : "#ffffff",
                      border: "none", borderRadius: 8, fontSize: 15, fontWeight: 600,
                      cursor: busy || !prompt.trim() ? "not-allowed" : "pointer",
                      fontFamily: C.sans, letterSpacing: 0.3,
                      textShadow: busy || !prompt.trim() ? "none" : "0 1px 2px rgba(0,0,0,0.3)",
                      boxShadow: busy || !prompt.trim() ? "none" : `0 0 20px ${C.accentGlow}, 0 4px 12px rgba(0,212,170,0.3), inset 0 1px 0 rgba(255,255,255,0.2)`,
                      transition: "box-shadow 0.2s, background 0.2s, transform 0.1s, filter 0.15s",
                    }}
                    onMouseEnter={e => { if (prompt.trim() && !busy) e.currentTarget.style.filter = "brightness(1.15)"; }}
                    onMouseDown={e => { if (prompt.trim() && !busy) e.currentTarget.style.transform = "scale(0.98)"; }}
                    onMouseUp={e => { e.currentTarget.style.transform = "scale(1)"; }}
                    onMouseLeave={e => { e.currentTarget.style.transform = "scale(1)"; e.currentTarget.style.filter = ""; }}
                  >
                    {busy ? "Planning..." : "\u26A1 Build It"}
                  </button>
                </>
              ) : (isPostBuild || (phase === "error" && outDir)) ? (
                <>
                  <textarea
                    ref={taRef}
                    value={iterTxt}
                    onChange={e => setIterTxt(e.target.value)}
                    placeholder="Describe what to change..."
                    rows={3}
                    disabled={itering}
                    style={{ width: "100%", minHeight: 70, background: C.bg, color: C.text, border: `1px solid ${C.border}`, borderRadius: 8, padding: "10px 12px", fontSize: 13, fontFamily: C.sans, resize: "vertical" as const, outline: "none", boxSizing: "border-box" as const, opacity: itering ? 0.5 : 1, transition: "border-color 0.2s" }}
                    onFocus={e => { e.currentTarget.style.borderColor = C.borderFocus; }}
                    onBlur={e => { e.currentTarget.style.borderColor = C.border; }}
                    onKeyDown={e => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) doIter(); }}
                  />
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: 4, marginBottom: 6 }}>
                    <span style={{ fontSize: 10, color: C.dim }}>
                      {lineCount > 0 && <>{lineCount} lines {"\u00B7"} </>}
                      {r && <>${(r.actual_cost ?? 0).toFixed(2)} {"\u00B7"} {(r.elapsed_seconds ?? 0).toFixed(0)}s</>}
                    </span>
                    <span style={{ fontSize: 10, color: C.dim, opacity: 0.6 }}>Ctrl+Enter</span>
                  </div>
                  <button
                    onClick={doIter}
                    disabled={itering || !iterTxt.trim()}
                    style={{
                      width: "100%", padding: "11px 0",
                      background: itering || !iterTxt.trim() ? "#1a2332" : C.accentDim,
                      color: itering || !iterTxt.trim() ? C.dim : C.accent,
                      border: `1px solid ${itering || !iterTxt.trim() ? C.border : "rgba(0,212,170,0.25)"}`,
                      borderRadius: 8, fontSize: 13, fontWeight: 600,
                      cursor: itering || !iterTxt.trim() ? "not-allowed" : "pointer",
                      transition: "background 0.2s, transform 0.1s",
                    }}
                    onMouseDown={e => { if (iterTxt.trim() && !itering) e.currentTarget.style.transform = "scale(0.98)"; }}
                    onMouseUp={e => { e.currentTarget.style.transform = "scale(1)"; }}
                    onMouseLeave={e => { e.currentTarget.style.transform = "scale(1)"; }}
                  >
                    {itering ? "Updating..." : "\u26A1 Update"}
                  </button>
                </>
              ) : isBuilding ? (
                <div style={{ fontSize: 12, color: C.muted, textAlign: "center" as const, padding: "10px 0" }}>
                  <span style={{ display: "inline-block", width: 10, height: 10, borderRadius: "50%", border: `2px solid ${C.border}`, borderTopColor: C.accent, animation: "nbspin 0.8s linear infinite", marginRight: 8, verticalAlign: "middle" }} />
                  {planPhase === "planning" ? "Planning..." : `Building... ${sPct}%`}
                </div>
              ) : null}
            </div>
          </div>
          )}
        </div>

        {/* ---- RIGHT PANEL ---- */}
        <div style={{ flex: 1, display: "flex", flexDirection: "column" as const, overflow: "hidden", background: C.bg }}>

          {/* Toolbar */}
          <div style={{ display: "flex", alignItems: "center", gap: 3, padding: "5px 12px", borderBottom: `1px solid ${C.border}`, background: C.surface, flexShrink: 0 }}>
            {(["mobile", "tablet", "desktop"] as const).map(v => (
              <button key={v} onClick={() => setVp(v)}
                style={{ background: vp === v ? C.accentDim : "transparent", color: vp === v ? C.accent : C.dim, border: vp === v ? `1px solid rgba(0,212,170,0.25)` : "1px solid transparent", borderRadius: 4, padding: "3px 9px", fontSize: 10, cursor: "pointer", fontWeight: vp === v ? 600 : 400, textTransform: "capitalize" as const }}>
                {v}
              </button>
            ))}
            <div style={{ width: 1, height: 14, background: C.border, margin: "0 4px" }} />
            {(["preview", "code"] as const).map(m => (
              <button key={m} onClick={() => setViewMode(m)}
                style={{ background: viewMode === m ? C.accentDim : "transparent", color: viewMode === m ? C.accent : C.dim, border: viewMode === m ? `1px solid rgba(0,212,170,0.25)` : "1px solid transparent", borderRadius: 4, padding: "3px 9px", fontSize: 10, cursor: "pointer", fontWeight: viewMode === m ? 600 : 400, textTransform: "capitalize" as const }}>
                {m}
              </button>
            ))}
            <div style={{ flex: 1 }} />
            <button onClick={() => { if (outDir) rPreview(outDir); }} disabled={!outDir} style={{ background: "transparent", color: outDir ? C.muted : C.dim, border: "none", padding: "3px 6px", fontSize: 13, cursor: outDir ? "pointer" : "default" }} title="Refresh">{"\u21BB"}</button>
            <button onClick={doDownload} disabled={!html} style={{ background: "transparent", color: html ? C.muted : C.dim, border: "none", padding: "3px 6px", fontSize: 13, cursor: html ? "pointer" : "default" }} title="Download HTML">{"\u2193"}</button>
            {outDir && <button onClick={() => { const pid = outDir.split("/").pop(); if (pid) exportProject(pid); }} style={{ background: "transparent", color: C.muted, border: "none", padding: "3px 6px", fontSize: 10, cursor: "pointer", fontFamily: C.sans }} title="Export as ZIP">ZIP</button>}
          </div>

          {/* Preview */}
          <div style={{ flex: 1, display: "flex", justifyContent: "center", alignItems: "stretch", overflow: "hidden", padding: 10 }}>
            {html ? (
              viewMode === "preview" ? (
                <div style={{ width: "100%", maxWidth: vpMax, height: "100%", margin: "0 auto", border: `1px solid ${C.border}`, borderRadius: 6, overflow: "hidden", background: "#fff", transition: "max-width 0.3s ease" }}>
                  <iframe ref={iframeRef} srcDoc={html} sandbox="allow-scripts" style={{ width: "100%", height: "100%", border: "none" }} title="Preview" />
                </div>
              ) : (
                <div style={{ width: "100%", height: "100%", overflow: "auto", background: C.surface, border: `1px solid ${C.border}`, borderRadius: 6 }}>
                  <pre style={{ margin: 0, padding: 14, fontSize: 11, color: C.text, fontFamily: C.mono, whiteSpace: "pre-wrap" as const, wordBreak: "break-word" as const, lineHeight: 1.6 }}>{html}</pre>
                </div>
              )
            ) : (
              <div style={{ display: "flex", flexDirection: "column" as const, alignItems: "center", justifyContent: "center", width: "100%", gap: 16 }}>
                {isBuilding ? (
                  <>
                    <div style={{ width: 44, height: 44, borderRadius: "50%", border: `3px solid ${C.border}`, borderTopColor: C.accent, animation: "nbspin 0.8s linear infinite" }} />
                    <div style={{ fontSize: 13, color: C.muted }}>Generating your site... {sPct}%</div>
                  </>
                ) : (
                  <>
                    <div style={{ width: 200, opacity: 0.12 }}>
                      <div style={{ height: 8, background: C.muted, borderRadius: 2, marginBottom: 6 }} />
                      <div style={{ display: "flex", gap: 4, marginBottom: 10 }}>
                        <div style={{ flex: 1, height: 4, background: C.muted, borderRadius: 2 }} />
                        <div style={{ flex: 1, height: 4, background: C.muted, borderRadius: 2 }} />
                        <div style={{ flex: 1, height: 4, background: C.muted, borderRadius: 2 }} />
                      </div>
                      <div style={{ height: 50, background: C.muted, borderRadius: 3, marginBottom: 8 }} />
                      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 4 }}>
                        <div style={{ height: 30, background: C.muted, borderRadius: 2 }} />
                        <div style={{ height: 30, background: C.muted, borderRadius: 2 }} />
                        <div style={{ height: 30, background: C.muted, borderRadius: 2 }} />
                      </div>
                    </div>
                    <div style={{ fontSize: 12, color: C.dim, marginTop: 4 }}>Your website will appear here</div>
                    <div style={{ fontSize: 10, color: C.dim, opacity: 0.7 }}>Builds take 60{"\u2013"}90 seconds with Sonnet 4.6</div>
                  </>
                )}
              </div>
            )}
          </div>

          {/* Status bar */}
          <div style={{ height: 24, minHeight: 24, display: "flex", alignItems: "center", padding: "0 12px", borderTop: `1px solid ${C.border}`, background: C.surface, fontSize: 10, color: C.dim, gap: 12 }}>
            {html && <span>{lineCount} lines</span>}
            {planCost > 0 && <span style={{ fontFamily: C.mono }}>Plan ({planModel || "Haiku 4.5"}): ${planCost.toFixed(4)}</span>}
            {r && planCost > 0 && <span style={{ fontFamily: C.mono }}>Build (Sonnet 4.6): ${(r.actual_cost ?? 0).toFixed(4)}</span>}
            {r && planCost > 0 && <span style={{ fontFamily: C.mono }}>Total: ${((r.actual_cost ?? 0) + planCost).toFixed(4)}</span>}
            {r && !planCost && <span style={{ fontFamily: C.mono }}>${(r.actual_cost ?? 0).toFixed(4)}</span>}
            {r && <span style={{ fontFamily: C.mono }}>{(r.input_tokens ?? 0).toLocaleString()} in / {(r.output_tokens ?? 0).toLocaleString()} out</span>}
            <div style={{ flex: 1 }} />
            {outDir && <span style={{ opacity: 0.4, maxWidth: 280, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" as const, fontFamily: C.mono }}>{outDir}</span>}
          </div>
        </div>
      </div>
    </div>
  );
}

/* === Narrative Row Component === */

function NarrativeRow({ entry, isLast, result }: { entry: NarrativeEntry; isLast: boolean; result?: any }) {
  const isActive = entry.status === "active";
  const isComplete = entry.status === "complete";

  if (entry.type === "header") {
    return (
      <div style={{ marginBottom: 10, animation: "nbfadein 0.2s ease" }}>
        <div style={{ fontSize: 13, fontWeight: 700, color: isComplete ? C.text : C.accent, lineHeight: 1.5 }}>
          {entry.label}
        </div>
      </div>
    );
  }

  if (entry.type === "user_message") {
    return (
      <div style={{ display: "flex", justifyContent: "flex-end", padding: "10px 0 6px", animation: "nbfadein 0.2s ease" }}>
        <div style={{
          background: "rgba(0,212,170,0.08)",
          border: `1px solid rgba(0,212,170,0.15)`,
          borderRadius: "12px 12px 4px 12px",
          padding: "8px 12px",
          maxWidth: "85%",
        }}>
          <div style={{ fontSize: 12, color: C.text, lineHeight: 1.4 }}>{entry.label}</div>
        </div>
      </div>
    );
  }

  if (entry.type === "error") {
    return (
      <div style={{ display: "flex", alignItems: "flex-start", gap: 8, padding: "6px 0", animation: "nbfadein 0.2s ease" }}>
        <span style={{ color: C.err, fontSize: 12, fontWeight: 700, lineHeight: "18px", flexShrink: 0 }}>{"\u2717"}</span>
        <div style={{ flex: 1 }}>
          <div style={{ fontSize: 12, color: C.err, wordBreak: "break-word" as const }}>{entry.label}</div>
        </div>
      </div>
    );
  }

  if (entry.type === "checkpoint") {
    return (
      <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "4px 0", animation: "nbfadein 0.2s ease" }}>
        <span style={{ color: C.dim, fontSize: 10, lineHeight: "18px", flexShrink: 0 }}>{"\u25CB"}</span>
        <span style={{ fontSize: 10, color: C.dim }}>{entry.label}</span>
      </div>
    );
  }

  if (entry.type === "result") {
    const r = result;
    return (
      <div style={{ padding: "8px 0 4px", animation: "nbfadein 0.3s ease" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
          <span style={{ color: C.ok, fontSize: 12, fontWeight: 700 }}>{"\u2713"}</span>
          <span style={{ fontSize: 12, fontWeight: 700, color: C.ok }}>Build complete</span>
          <div style={{ flex: 1 }} />
          <span style={{ fontSize: 10, color: C.dim, fontFamily: C.mono }}>{(entry.elapsed ?? 0).toFixed(1)}s</span>
        </div>
        {r && (
          <div style={{ marginLeft: 20 }}>
            <div style={{ fontSize: 11, color: C.muted, lineHeight: 1.8 }}>
              <span style={{ fontFamily: C.mono, color: C.text }}>{(r.total_lines ?? 0).toLocaleString()}</span> lines {"\u00B7"}{" "}
              <span style={{ fontFamily: C.mono, color: C.text }}>{(r.total_chars ?? 0).toLocaleString()}</span> chars
            </div>
            <div style={{ fontSize: 11, color: C.muted, lineHeight: 1.8 }}>
              <span style={{ fontFamily: C.mono, color: C.text }}>{(r.input_tokens ?? 0).toLocaleString()}</span> in /{" "}
              <span style={{ fontFamily: C.mono, color: C.text }}>{(r.output_tokens ?? 0).toLocaleString()}</span> out tokens
            </div>
            <div style={{ fontSize: 11, lineHeight: 1.8 }}>
              Cost: <span style={{ fontFamily: C.mono, color: C.accent, fontWeight: 600 }}>${(r.actual_cost ?? 0).toFixed(4)}</span>
              <span style={{ color: C.dim }}> (Sonnet 4.6)</span>
            </div>
            {r.governance_status && (
              <div style={{ display: "flex", gap: 10, marginTop: 4, fontSize: 10, fontWeight: 600 }}>
                {[
                  { k: "OWASP", v: r.governance_status.owasp_passed },
                  { k: "XSS", v: r.governance_status.xss_clean },
                  { k: "ARIA", v: r.governance_status.aria_present },
                  { k: "Signed", v: r.governance_status.signed },
                ].map(g => (
                  <span key={g.k} style={{ color: g.v ? C.ok : C.dim }}>{g.v ? "\u2713" : "\u2717"} {g.k}</span>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    );
  }

  // Phase entry
  return (
    <div style={{ display: "flex", alignItems: "flex-start", gap: 8, padding: "5px 0", animation: "nbfadein 0.2s ease" }}>
      {/* Status indicator */}
      <div style={{ width: 14, flexShrink: 0, paddingTop: 2 }}>
        {isComplete ? (
          <span style={{ color: C.ok, fontSize: 11, fontWeight: 700 }}>{"\u2713"}</span>
        ) : isActive ? (
          <span style={{ color: C.accent, fontSize: 11, fontWeight: 700, animation: "nbpulse 1.5s ease infinite" }}>&gt;</span>
        ) : (
          <span style={{ color: C.dim, fontSize: 9 }}>{"\u25CB"}</span>
        )}
      </div>

      {/* Content */}
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", gap: 8 }}>
          <span style={{ fontSize: 12, color: isActive ? C.accent : isComplete ? C.text : C.dim, fontWeight: isActive ? 600 : 400 }}>
            {entry.label}{isActive ? "..." : ""}
          </span>
          {entry.elapsed != null && entry.elapsed > 0 && (
            <span style={{ fontSize: 10, color: C.dim, fontFamily: C.mono, flexShrink: 0 }}>
              {entry.elapsed.toFixed(1)}s
            </span>
          )}
        </div>
        {isComplete && entry.detail && (
          <div style={{ fontSize: 10, color: C.dim, marginTop: 1 }}>{entry.detail}</div>
        )}
      </div>
    </div>
  );
}

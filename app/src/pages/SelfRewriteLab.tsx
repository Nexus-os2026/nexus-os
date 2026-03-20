import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  selfRewriteGetHistory as fetchRewriteHistory,
  selfRewritePreviewPatch as fetchPreviewPatch,
  selfRewriteTestPatch as fetchTestPatch,
} from "../api/backend";
import {
  ActionButton,
  CommandModal,
  EmptyState,
  MetricBar,
  Panel,
  alpha,
  commandHeaderMetaStyle,
  commandInsetStyle,
  commandLabelStyle,
  commandMonoValueStyle,
  commandMutedStyle,
  commandPageStyle,
  commandScrollStyle,
  formatTimestamp,
  normalizeArray,
  toTitleCase,
} from "./commandCenterUi";

interface Bottleneck {
  function_name: string;
  module_path: string;
  severity: string;
  reason: string;
  suggestion: string;
}

interface PatchRecord {
  id: string;
  target_file: string;
  target_function: string;
  original_code: string;
  optimized_code: string;
  optimization_goal: string;
  status: string;
  created_at: number;
  requires_approval: boolean;
}

interface RollbackEvent {
  patch_id: string;
  reason: string;
  reverted_at: number;
  health_metrics_before: unknown;
  health_metrics_after: unknown;
}

interface PatchCard extends PatchRecord {
  virtual?: boolean;
  risk: "LOW" | "MEDIUM" | "HIGH";
}

interface SessionHistoryEntry {
  id: string;
  patch_id?: string;
  title: string;
  status: string;
  timestamp: number;
  detail: string;
}

function severityColor(severity: string): string {
  switch (String(severity)) {
    case "Critical":
      return "#ef4444";
    case "High":
      return "#fb923c";
    case "Medium":
      return "#eab308";
    case "Low":
      return "#22c55e";
    default:
      return "#94a3b8";
  }
}

function statusColor(status: string): string {
  switch (String(status).toLowerCase()) {
    case "generated":
    case "validated":
      return "#38bdf8";
    case "testing":
      return "#eab308";
    case "tested":
    case "approved":
    case "applied":
      return "#22c55e";
    case "reverted":
    case "rejected":
    case "failed":
      return "#ef4444";
    case "analysis only":
      return "#94a3b8";
    default:
      return "#94a3b8";
  }
}

function severityScore(severity: string): number {
  switch (String(severity)) {
    case "Critical":
      return 96;
    case "High":
      return 78;
    case "Medium":
      return 58;
    case "Low":
      return 34;
    default:
      return 20;
  }
}

function riskFromSeverity(severity: string): "LOW" | "MEDIUM" | "HIGH" {
  if (severity === "Critical" || severity === "High") return "HIGH";
  if (severity === "Medium") return "MEDIUM";
  return "LOW";
}

function patchTitle(patch: PatchCard): string {
  return patch.optimization_goal || `${patch.target_function} optimization`;
}

function previewFromPatch(patch: PatchCard): string {
  if (patch.original_code && patch.optimized_code) {
    const before = patch.original_code.split("\n").map((line) => `- ${line}`);
    const after = patch.optimized_code.split("\n").map((line) => `+ ${line}`);
    return [...before, ...after].join("\n");
  }
  return `- Current issue\n+ Suggested improvement: ${patch.optimization_goal}`;
}

export default function SelfRewriteLab(): JSX.Element {
  const [bottlenecks, setBottlenecks] = useState<Bottleneck[]>([]);
  const [patches, setPatches] = useState<PatchCard[]>([]);
  const [remoteHistory, setRemoteHistory] = useState<RollbackEvent[]>([]);
  const [sessionHistory, setSessionHistory] = useState<SessionHistoryEntry[]>([]);
  const [previewTitle, setPreviewTitle] = useState<string | null>(null);
  const [previewDiff, setPreviewDiff] = useState("");
  const [applyTarget, setApplyTarget] = useState<PatchCard | null>(null);
  const [working, setWorking] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const appendSessionHistory = useCallback((entry: Omit<SessionHistoryEntry, "id" | "timestamp">) => {
    setSessionHistory((current) => [
      {
        id: `${Date.now()}-${current.length}`,
        timestamp: Math.floor(Date.now() / 1000),
        ...entry,
      },
      ...current,
    ]);
  }, []);

  const loadHistory = useCallback(async () => {
    try {
      let history: RollbackEvent[];
      try {
        const raw = await fetchRewriteHistory();
        history = JSON.parse(raw) as RollbackEvent[];
      } catch {
        history = await invoke<RollbackEvent[]>("self_rewrite_get_history");
      }
      setRemoteHistory(normalizeArray<RollbackEvent>(history));
    } catch {
      setRemoteHistory([]);
    }
  }, []);

  const loadPatches = useCallback(async (currentBottlenecks: Bottleneck[] = bottlenecks) => {
    try {
      const remote = normalizeArray<PatchRecord>(await invoke("self_rewrite_suggest_patches"));
      if (remote.length > 0) {
        setPatches(
          remote.map((patch) => ({
            ...patch,
            risk: patch.requires_approval ? "LOW" : "MEDIUM",
          }))
        );
        return;
      }

      if (currentBottlenecks.length > 0) {
        setPatches(
          currentBottlenecks.map((bottleneck, index) => ({
            id: `virtual-${index}`,
            target_file: bottleneck.module_path || "kernel",
            target_function: bottleneck.function_name,
            original_code: "",
            optimized_code: "",
            optimization_goal: bottleneck.suggestion,
            status: "analysis only",
            created_at: Math.floor(Date.now() / 1000),
            requires_approval: true,
            risk: riskFromSeverity(bottleneck.severity),
            virtual: true,
          }))
        );
      } else {
        setPatches([]);
      }
    } catch (patchError) {
      setError(patchError instanceof Error ? patchError.message : String(patchError));
      setPatches([]);
    }
  }, [bottlenecks]);

  useEffect(() => {
    void loadHistory();
  }, [loadHistory]);

  const handleAnalyze = useCallback(async () => {
    setWorking("analyze");
    setError(null);
    try {
      const result = normalizeArray<Bottleneck>(await invoke("self_rewrite_analyze"));
      setBottlenecks(result);
      await loadPatches(result);
      appendSessionHistory({
        title: "Performance analysis",
        status: "completed",
        detail: result.length > 0 ? `${result.length} bottlenecks detected` : "No bottlenecks detected",
      });
    } catch (analyzeError) {
      setError(analyzeError instanceof Error ? analyzeError.message : String(analyzeError));
    } finally {
      setWorking(null);
    }
  }, [appendSessionHistory, loadPatches]);

  const handlePreview = useCallback(async (patch: PatchCard) => {
    setWorking(`preview-${patch.id}`);
    setError(null);
    try {
      if (patch.virtual) {
        setPreviewTitle(patchTitle(patch));
        setPreviewDiff(previewFromPatch(patch));
        return;
      }

      let diffText: string;
      try {
        const raw = await fetchPreviewPatch(patch.id);
        diffText = typeof raw === "string" ? raw : JSON.stringify(raw, null, 2);
      } catch {
        const preview = await invoke<unknown>("self_rewrite_preview_patch", { patchId: patch.id });
        diffText = typeof preview === "string" ? preview : JSON.stringify(preview, null, 2);
      }
      setPreviewTitle(patchTitle(patch));
      setPreviewDiff(diffText);
    } catch (previewError) {
      setError(previewError instanceof Error ? previewError.message : String(previewError));
    } finally {
      setWorking(null);
    }
  }, []);

  const handleTest = useCallback(async (patch: PatchCard) => {
    if (patch.virtual) return;
    setWorking(`test-${patch.id}`);
    setError(null);
    try {
      let result: unknown;
      try {
        result = await fetchTestPatch(patch.id);
      } catch {
        result = await invoke<unknown>("self_rewrite_test_patch", { patchId: patch.id });
      }
      appendSessionHistory({
        patch_id: patch.id,
        title: patchTitle(patch),
        status: "tested",
        detail: typeof result === "string" ? result : "Patch tests completed",
      });
      await loadPatches();
    } catch (testError) {
      setError(testError instanceof Error ? testError.message : String(testError));
      appendSessionHistory({
        patch_id: patch.id,
        title: patchTitle(patch),
        status: "rejected",
        detail: "Patch test run failed",
      });
    } finally {
      setWorking(null);
    }
  }, [appendSessionHistory, loadPatches]);

  const handleApply = useCallback(async () => {
    if (!applyTarget || applyTarget.virtual) return;
    setWorking(`apply-${applyTarget.id}`);
    setError(null);
    try {
      await invoke("self_rewrite_apply_patch", { patchId: applyTarget.id });
      appendSessionHistory({
        patch_id: applyTarget.id,
        title: patchTitle(applyTarget),
        status: "applied",
        detail: "Patch applied after approval",
      });
      setApplyTarget(null);
      await Promise.all([loadPatches(), loadHistory()]);
    } catch (applyError) {
      setError(applyError instanceof Error ? applyError.message : String(applyError));
    } finally {
      setWorking(null);
    }
  }, [appendSessionHistory, applyTarget, loadHistory, loadPatches]);

  const handleRollback = useCallback(async (patchId: string, title: string) => {
    setWorking(`rollback-${patchId}`);
    setError(null);
    try {
      await invoke("self_rewrite_rollback", { patchId });
      appendSessionHistory({
        patch_id: patchId,
        title,
        status: "rolled-back",
        detail: "Rollback requested",
      });
      await loadHistory();
    } catch (rollbackError) {
      setError(rollbackError instanceof Error ? rollbackError.message : String(rollbackError));
    } finally {
      setWorking(null);
    }
  }, [appendSessionHistory, loadHistory]);

  const historyEntries = useMemo(() => {
    const remoteEntries: SessionHistoryEntry[] = remoteHistory.map((entry) => ({
      id: entry.patch_id,
      patch_id: entry.patch_id,
      title: `Patch ${entry.patch_id}`,
      status: "rolled-back",
      timestamp: entry.reverted_at,
      detail: entry.reason,
    }));
    return [...sessionHistory, ...remoteEntries].sort((a, b) => (b.timestamp ?? 0) - (a.timestamp ?? 0));
  }, [remoteHistory, sessionHistory]);

  return (
    <div style={commandPageStyle}>
      <div
        style={{
          marginBottom: 18,
          padding: "14px 16px",
          borderRadius: 16,
          border: "1px solid rgba(250, 204, 21, 0.35)",
          background: "linear-gradient(90deg, rgba(250, 204, 21, 0.18), rgba(15, 23, 42, 0.86))",
        }}
      >
        <div style={{ ...commandLabelStyle, color: "#facc15", marginBottom: 6 }}>Human In The Loop</div>
        <div style={{ ...commandMonoValueStyle, color: "#fef08a" }}>All patches require HITL approval before apply.</div>
      </div>

      <div style={{ marginBottom: 20 }}>
        <h1 style={{ margin: 0, fontFamily: "monospace", fontSize: "1.8rem", color: "#00ffcc", letterSpacing: "0.16em", textTransform: "uppercase" }}>
          Self-Rewrite Lab
        </h1>
        <div style={{ ...commandHeaderMetaStyle, marginTop: 10 }}>
          <span>{bottlenecks.length} bottlenecks tracked</span>
          <span>{patches.length} suggested patches</span>
          <span>{historyEntries.length} history events</span>
        </div>
      </div>

      {error ? <div style={{ marginBottom: 16, color: "#fca5a5", fontSize: "0.82rem" }}>{error}</div> : null}

      <Panel
        title="Performance Analysis"
        accent="#38bdf8"
        action={
          <ActionButton accent="#38bdf8" disabled={working === "analyze"} onClick={() => void handleAnalyze()}>
            {working === "analyze" ? "Analyzing..." : "Analyze"}
          </ActionButton>
        }
        style={{ marginBottom: 18 }}
      >
        {bottlenecks.length === 0 ? <EmptyState text={working === "analyze" ? "Analyzing..." : "No bottlenecks captured yet"} /> : null}
        <div style={{ display: "grid", gap: 10 }}>
          {bottlenecks.map((bottleneck) => {
            const color = severityColor(bottleneck.severity);
            return (
              <article key={`${bottleneck.module_path}-${bottleneck.function_name}`} style={commandInsetStyle}>
                <div style={{ display: "flex", justifyContent: "space-between", gap: 12, marginBottom: 8, alignItems: "center" }}>
                  <span style={{ ...commandMonoValueStyle, color: "#f8fafc" }}>
                    {bottleneck.function_name} <span style={{ color: "#94a3b8" }}>({bottleneck.module_path || "kernel"})</span>
                  </span>
                  <span
                    style={{
                      padding: "4px 8px",
                      borderRadius: 999,
                      background: alpha(color, 0.14),
                      border: `1px solid ${alpha(color, 0.32)}`,
                      color,
                      fontSize: "0.72rem",
                      fontFamily: "monospace",
                    }}
                  >
                    {String(bottleneck.severity).toUpperCase()}
                  </span>
                </div>
                <div style={{ ...commandMutedStyle, marginBottom: 10 }}>{bottleneck.reason}</div>
                <MetricBar value={severityScore(bottleneck.severity)} color={color} />
                <div style={{ ...commandMutedStyle, marginTop: 10 }}>
                  Suggested: {bottleneck.suggestion}
                </div>
              </article>
            );
          })}
        </div>
      </Panel>

      <Panel title="Suggested Patches" accent="#00ffcc" style={{ marginBottom: 18 }}>
        {patches.length === 0 ? <EmptyState text="No patches available yet. Run analysis to populate this queue." /> : null}
        <div style={{ display: "grid", gap: 10 }}>
          {patches.map((patch) => {
            const badge = statusColor(patch.status);
            return (
              <article key={patch.id} style={commandInsetStyle}>
                <div style={{ display: "flex", justifyContent: "space-between", gap: 12, marginBottom: 8 }}>
                  <div>
                    <div style={{ ...commandMonoValueStyle, color: "#f8fafc", marginBottom: 6 }}>{patchTitle(patch)}</div>
                    <div style={commandMutedStyle}>{patch.target_file} :: {patch.target_function}</div>
                  </div>
                  <div style={{ textAlign: "right" }}>
                    <div
                      style={{
                        padding: "4px 8px",
                        borderRadius: 999,
                        background: alpha(badge, 0.14),
                        border: `1px solid ${alpha(badge, 0.32)}`,
                        color: badge,
                        fontSize: "0.72rem",
                        fontFamily: "monospace",
                        marginBottom: 6,
                      }}
                    >
                      {String(patch.status).toUpperCase()}
                    </div>
                    <div style={{ ...commandLabelStyle, color: patch.risk === "HIGH" ? "#ef4444" : patch.risk === "MEDIUM" ? "#eab308" : "#22c55e" }}>
                      Risk: {patch.risk}
                    </div>
                  </div>
                </div>
                <div style={{ ...commandMutedStyle, marginBottom: 10 }}>
                  {patch.optimization_goal || "Patch queue entry prepared from performance analysis."}
                </div>
                <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                  <ActionButton accent="#38bdf8" disabled={working === `preview-${patch.id}`} onClick={() => void handlePreview(patch)}>
                    Preview Diff
                  </ActionButton>
                  <ActionButton accent="#00ffcc" disabled={patch.virtual || working === `test-${patch.id}`} onClick={() => void handleTest(patch)}>
                    Run Tests
                  </ActionButton>
                  <ActionButton
                    destructive
                    disabled={patch.virtual}
                    onClick={() => setApplyTarget(patch)}
                  >
                    Apply
                  </ActionButton>
                </div>
                {patch.virtual ? <div style={{ ...commandMutedStyle, marginTop: 10 }}>Analysis-only card: the backend patch queue did not return a concrete patch id for this item.</div> : null}
              </article>
            );
          })}
        </div>
      </Panel>

      <Panel title="Patch History" accent="#f59e0b">
        <div style={{ ...commandScrollStyle, maxHeight: 260, paddingRight: 6 }}>
          {historyEntries.length === 0 ? <EmptyState text="No patch history yet" /> : null}
          {historyEntries.map((entry) => {
            const color = statusColor(entry.status);
            return (
              <article key={entry.id} style={{ ...commandInsetStyle, marginBottom: 8 }}>
                <div style={{ display: "flex", justifyContent: "space-between", gap: 12, marginBottom: 8 }}>
                  <div style={{ ...commandMonoValueStyle, color: "#f8fafc" }}>{entry.title}</div>
                  <div style={{ textAlign: "right" }}>
                    <div style={{ ...commandLabelStyle, color }}>{toTitleCase(entry.status)}</div>
                    <div style={{ ...commandMonoValueStyle, color: "#94a3b8" }}>{formatTimestamp(entry.timestamp)}</div>
                  </div>
                </div>
                <div style={{ ...commandMutedStyle, marginBottom: 10 }}>{entry.detail}</div>
                {entry.status === "applied" && entry.patch_id ? (
                  (() => {
                    const patchId = entry.patch_id;
                    return (
                      <ActionButton destructive={false} accent="#f59e0b" disabled={working === `rollback-${patchId}`} onClick={() => void handleRollback(patchId, entry.title)}>
                        Rollback
                      </ActionButton>
                    );
                  })()
                ) : null}
              </article>
            );
          })}
        </div>
      </Panel>

      <CommandModal
        open={Boolean(previewTitle)}
        title={previewTitle ?? "Patch Preview"}
        accent="#38bdf8"
        footer={
          <div style={{ display: "flex", justifyContent: "flex-end" }}>
            <ActionButton accent="#38bdf8" onClick={() => {
              setPreviewTitle(null);
              setPreviewDiff("");
            }}>
              Close
            </ActionButton>
          </div>
        }
      >
        <div
          style={{
            margin: 0,
            padding: 16,
            borderRadius: 14,
            background: "rgba(2, 6, 23, 0.92)",
            border: "1px solid rgba(56, 189, 248, 0.16)",
            color: "#e2e8f0",
            fontFamily: "monospace",
            fontSize: "0.78rem",
            whiteSpace: "pre-wrap",
            overflow: "auto",
          }}
        >
          {previewDiff.split("\n").map((line, index) => (
            <div
              key={`${line}-${index}`}
              style={{
                color: line.startsWith("+") ? "#4ade80" : line.startsWith("-") ? "#f87171" : "#cbd5e1",
              }}
            >
              {line}
            </div>
          ))}
        </div>
      </CommandModal>

      <CommandModal
        open={Boolean(applyTarget)}
        title="Apply Patch"
        accent="#f59e0b"
        footer={
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
            <ActionButton accent="#38bdf8" onClick={() => setApplyTarget(null)}>
              Cancel
            </ActionButton>
            <ActionButton destructive disabled={working === `apply-${applyTarget?.id ?? ""}`} onClick={() => void handleApply()}>
              {working === `apply-${applyTarget?.id ?? ""}` ? "Applying..." : "Confirm"}
            </ActionButton>
          </div>
        }
      >
        <div style={{ ...commandMutedStyle, marginBottom: 10 }}>
          This will modify kernel code. Are you sure?
        </div>
        {applyTarget ? (
          <div style={commandInsetStyle}>
            <div style={{ ...commandMonoValueStyle, color: "#f8fafc", marginBottom: 8 }}>{patchTitle(applyTarget)}</div>
            <div style={commandMutedStyle}>{applyTarget.target_file} :: {applyTarget.target_function}</div>
          </div>
        ) : null}
      </CommandModal>
    </div>
  );
}

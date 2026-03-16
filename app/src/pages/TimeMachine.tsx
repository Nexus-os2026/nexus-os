import { useCallback, useEffect, useState } from "react";
import {
  timeMachineListCheckpoints,
  timeMachineGetCheckpoint,
  timeMachineCreateCheckpoint,
  timeMachineUndo,
  timeMachineUndoCheckpoint,
  timeMachineRedo,
  timeMachineGetDiff,
  timeMachineWhatIf,
} from "../api/backend";

/* ── types ── */

interface CheckpointSummary {
  id: string;
  label: string;
  timestamp: number;
  agent_id: string | null;
  agent_name?: string | null;
  action?: string;
  state_hash?: string;
  change_count: number;
  undone: boolean;
}

interface DiffEntry {
  change_type: string;
  path: string;
  size_before: number;
  size_after: number;
  before_value?: unknown;
  after_value?: unknown;
}

interface UndoResult {
  checkpoint_id: string;
  label: string;
  actions_applied: number;
  files_restored: string[];
  agents_affected: string[];
}

interface WhatIfResult {
  rewind: UndoResult;
  replayed_checkpoints: number;
}

/* ── helpers ── */

function relativeTime(timestampMs: number): string {
  const now = Date.now();
  const diff = now - timestampMs;
  if (diff < 0) return "just now";
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return new Date(timestampMs).toLocaleDateString();
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function changeIcon(changeType: string): { symbol: string; color: string } {
  switch (changeType) {
    case "create":
      return { symbol: "+", color: "#00ff9d" };
    case "delete":
      return { symbol: "-", color: "#f87171" };
    case "modify":
    default:
      return { symbol: "~", color: "#fbbf24" };
  }
}

function isAgentOrConfigPath(path: string): boolean {
  return path.startsWith("agent://") || path.startsWith("config://");
}

function summarizeUndoActions(entries: DiffEntry[]): string[] {
  return entries.slice(0, 8).map((entry) => {
    if (entry.path.startsWith("agent://")) {
      return `Restore ${entry.path.replace("agent://", "agent ")}`;
    }
    if (entry.path.startsWith("config://")) {
      return `Restore config ${entry.path.replace("config://", "")}`;
    }
    return `${entry.change_type} ${entry.path}`;
  });
}

/* ── component ── */

export default function TimeMachine() {
  const [checkpoints, setCheckpoints] = useState<CheckpointSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [selectedDetail, setSelectedDetail] = useState<CheckpointSummary | null>(null);
  const [diffEntries, setDiffEntries] = useState<DiffEntry[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isUndoing, setIsUndoing] = useState(false);
  const [isRedoing, setIsRedoing] = useState(false);
  const [toast, setToast] = useState<{ text: string; type: "success" | "error" } | null>(null);
  const [createLabel, setCreateLabel] = useState("");
  const [showCreateInput, setShowCreateInput] = useState(false);

  /* ── styles ── */

  const accent = "#00ff9d";
  const bgPage = "#0d0d1a";
  const bgPanel = "#141428";
  const bgCard = "#1a1a2e";
  const bgInput = "#0f0f1e";
  const borderColor = "#2a2a3e";
  const textPrimary = "#e0e0e0";
  const textSecondary = "#888";

  /* ── toast helper ── */

  const showToast = useCallback((text: string, type: "success" | "error") => {
    setToast({ text, type });
    setTimeout(() => setToast(null), 4000);
  }, []);

  /* ── data loading ── */

  const loadCheckpoints = useCallback(async () => {
    try {
      const raw = await timeMachineListCheckpoints();
      const data: CheckpointSummary[] = JSON.parse(raw);
      setCheckpoints(data);
    } catch {
      setCheckpoints([]);
    }
  }, []);

  const loadDetail = useCallback(async (id: string) => {
    try {
      const [cpRaw, diffRaw] = await Promise.all([
        timeMachineGetCheckpoint(id),
        timeMachineGetDiff(id),
      ]);
      setSelectedDetail(JSON.parse(cpRaw));
      setDiffEntries(JSON.parse(diffRaw));
    } catch {
      setSelectedDetail(null);
      setDiffEntries([]);
    }
  }, []);

  useEffect(() => {
    loadCheckpoints();
  }, [loadCheckpoints]);

  useEffect(() => {
    if (selectedId) {
      loadDetail(selectedId);
    } else {
      setSelectedDetail(null);
      setDiffEntries([]);
    }
  }, [selectedId, loadDetail]);

  /* ── keyboard shortcuts ── */

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === "Z") {
        e.preventDefault();
        handleRedo();
      } else if ((e.ctrlKey || e.metaKey) && e.key === "z") {
        e.preventDefault();
        handleUndo();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /* ── actions ── */

  const handleUndo = useCallback(async () => {
    setIsUndoing(true);
    try {
      const raw = await timeMachineUndo();
      const result: UndoResult = JSON.parse(raw);
      showToast(`Undone: ${result.label} \u2014 ${result.actions_applied} action${result.actions_applied !== 1 ? "s" : ""} reversed`, "success");
      await loadCheckpoints();
      if (selectedId) await loadDetail(selectedId);
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setIsUndoing(false);
    }
  }, [showToast, loadCheckpoints, selectedId, loadDetail]);

  const handleRedo = useCallback(async () => {
    setIsRedoing(true);
    try {
      const raw = await timeMachineRedo();
      const result: UndoResult = JSON.parse(raw);
      showToast(`Redone: ${result.label}`, "success");
      await loadCheckpoints();
      if (selectedId) await loadDetail(selectedId);
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setIsRedoing(false);
    }
  }, [showToast, loadCheckpoints, selectedId, loadDetail]);

  const handleUndoCheckpoint = useCallback(async (id: string) => {
    setIsUndoing(true);
    try {
      const diffRaw = await timeMachineGetDiff(id);
      const diff: DiffEntry[] = JSON.parse(diffRaw);
      const preview = summarizeUndoActions(diff);
      const confirmed = window.confirm(
        preview.length === 0
          ? "Rewind to this checkpoint?"
          : `These actions will be undone:\n${preview.map((item) => `- ${item}`).join("\n")}\n\nRewind to this point?`
      );
      if (!confirmed) {
        return;
      }
      const raw = await timeMachineUndoCheckpoint(id);
      const result: UndoResult = JSON.parse(raw);
      showToast(`Rewound to: ${result.label}`, "success");
      await loadCheckpoints();
      setSelectedId(id);
      await loadDetail(id);
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setIsUndoing(false);
    }
  }, [showToast, loadCheckpoints, selectedId, loadDetail]);

  const handleWhatIf = useCallback(async (checkpoint: CheckpointSummary) => {
    try {
      const suggestedKey =
        checkpoint.agent_id ? `agent://${checkpoint.agent_id}/fuel_remaining` : "governance.enable_warden_review";
      const variableKey = window.prompt("Variable to modify after rewind", suggestedKey);
      if (!variableKey) {
        return;
      }
      const defaultValue =
        variableKey === "governance.enable_warden_review"
          ? "true"
          : variableKey.endsWith("/status")
            ? "Paused"
            : "250";
      const variableValue = window.prompt("New value", defaultValue);
      if (variableValue === null) {
        return;
      }

      const diffRaw = await timeMachineGetDiff(checkpoint.id);
      const diff: DiffEntry[] = JSON.parse(diffRaw);
      const preview = summarizeUndoActions(diff);
      const confirmed = window.confirm(
        preview.length === 0
          ? `Run What if? for ${checkpoint.label}?`
          : `These actions will be undone:\n${preview.map((item) => `- ${item}`).join("\n")}\n\nThen ${variableKey} will be set to ${variableValue}. Continue?`
      );
      if (!confirmed) {
        return;
      }

      const raw = await timeMachineWhatIf(checkpoint.id, variableKey, variableValue);
      const result: WhatIfResult = JSON.parse(raw);
      showToast(
        `What if replayed ${result.replayed_checkpoints} checkpoint${result.replayed_checkpoints === 1 ? "" : "s"}`,
        "success"
      );
      await loadCheckpoints();
      setSelectedId(checkpoint.id);
      await loadDetail(checkpoint.id);
    } catch (e) {
      showToast(String(e), "error");
    }
  }, [loadCheckpoints, loadDetail, showToast]);

  const handleCreate = useCallback(async () => {
    const label = createLabel.trim();
    if (!label) return;
    setIsLoading(true);
    try {
      await timeMachineCreateCheckpoint(label);
      showToast(`Checkpoint created: ${label}`, "success");
      setCreateLabel("");
      setShowCreateInput(false);
      await loadCheckpoints();
    } catch (e) {
      showToast(String(e), "error");
    } finally {
      setIsLoading(false);
    }
  }, [createLabel, showToast, loadCheckpoints]);

  /* ── sub-renders ── */

  const renderTimeline = () => {
    const reversed = [...checkpoints].reverse();
    return (
      <div style={{ flex: 1, overflow: "auto" }}>
        {reversed.length === 0 ? (
          <div style={{ padding: 20, textAlign: "center", color: textSecondary, fontSize: 13 }}>
            No checkpoints yet. Create one or trigger an agent action.
          </div>
        ) : (
          reversed.map((cp) => {
            const isSelected = selectedId === cp.id;
            return (
              <div
                key={cp.id}
                onClick={() => setSelectedId(isSelected ? null : cp.id)}
                style={{
                  display: "flex",
                  gap: 12,
                  padding: "12px 14px",
                  borderBottom: `1px solid ${borderColor}`,
                  cursor: "pointer",
                  background: isSelected ? `${accent}0d` : "transparent",
                  borderLeft: isSelected ? `3px solid ${accent}` : "3px solid transparent",
                  opacity: cp.undone ? 0.5 : 1,
                  transition: "all 0.15s",
                }}
              >
                {/* Timeline dot */}
                <div style={{ display: "flex", flexDirection: "column", alignItems: "center", paddingTop: 4 }}>
                  <div
                    style={{
                      width: 10,
                      height: 10,
                      borderRadius: "50%",
                      background: cp.undone ? textSecondary : accent,
                      flexShrink: 0,
                    }}
                  />
                  <div
                    style={{
                      width: 2,
                      flex: 1,
                      background: borderColor,
                      marginTop: 4,
                    }}
                  />
                </div>

                {/* Content */}
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div
                    style={{
                      fontWeight: 600,
                      fontSize: 13,
                      textDecoration: cp.undone ? "line-through" : "none",
                      color: cp.undone ? textSecondary : textPrimary,
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                    }}
                    title={cp.label}
                  >
                    {cp.label}
                  </div>
                  <div style={{ display: "flex", gap: 8, marginTop: 4, alignItems: "center", flexWrap: "wrap" }}>
                    <span style={{ fontSize: 11, color: textSecondary }}>
                      {relativeTime(cp.timestamp)}
                    </span>
                    {cp.agent_id && (
                      <span
                        style={{
                          fontSize: 10,
                          padding: "1px 6px",
                          borderRadius: 3,
                          background: "#60a5fa22",
                          color: "#60a5fa",
                          fontWeight: 600,
                        }}
                      >
                        {cp.agent_name || cp.agent_id.slice(0, 8)}
                      </span>
                    )}
                    {cp.action && (
                      <span style={{ fontSize: 11, color: textPrimary }}>
                        {cp.action}
                      </span>
                    )}
                    {cp.state_hash && (
                      <span style={{ fontSize: 10, color: textSecondary, fontFamily: "monospace" }}>
                        {cp.state_hash.slice(0, 12)}
                      </span>
                    )}
                    <span
                      style={{
                        fontSize: 10,
                        padding: "1px 6px",
                        borderRadius: 3,
                        background: `${accent}15`,
                        color: accent,
                        fontWeight: 600,
                      }}
                    >
                      {cp.change_count} change{cp.change_count !== 1 ? "s" : ""}
                    </span>
                    {cp.undone && (
                      <span
                        style={{
                          fontSize: 10,
                          padding: "1px 6px",
                          borderRadius: 3,
                          background: "#f8717122",
                          color: "#f87171",
                          fontWeight: 600,
                        }}
                      >
                        Undone
                      </span>
                    )}
                  </div>
                </div>

                {/* Selective undo button */}
                {!cp.undone && (
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleUndoCheckpoint(cp.id);
                    }}
                    disabled={isUndoing}
                    style={{
                      background: "none",
                      border: `1px solid ${borderColor}`,
                      color: textSecondary,
                      cursor: isUndoing ? "not-allowed" : "pointer",
                      fontSize: 11,
                      padding: "2px 8px",
                      borderRadius: 4,
                      alignSelf: "center",
                      whiteSpace: "nowrap",
                    }}
                    title="Rewind to this checkpoint"
                  >
                    Rewind to this point
                  </button>
                )}
                {!cp.undone && (
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      void handleWhatIf(cp);
                    }}
                    style={{
                      background: "none",
                      border: `1px solid ${borderColor}`,
                      color: accent,
                      cursor: "pointer",
                      fontSize: 11,
                      padding: "2px 8px",
                      borderRadius: 4,
                      alignSelf: "center",
                      whiteSpace: "nowrap",
                    }}
                    title="Rewind, modify one variable, and replay"
                  >
                    What if?
                  </button>
                )}
              </div>
            );
          })
        )}
      </div>
    );
  };

  const renderDetail = () => {
    if (!selectedDetail) {
      return (
        <div
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            color: textSecondary,
            fontSize: 13,
            textAlign: "center",
            padding: 20,
          }}
        >
          Select a checkpoint to view details
        </div>
      );
    }

    const cp = selectedDetail;

    return (
      <div style={{ flex: 1, overflow: "auto", padding: 16, display: "flex", flexDirection: "column", gap: 16 }}>
        {/* Header */}
        <div>
          <div style={{ fontSize: 16, fontWeight: 700, color: textPrimary, marginBottom: 6 }}>
            {cp.label}
          </div>
          <div style={{ display: "flex", gap: 10, alignItems: "center", flexWrap: "wrap" }}>
            <span style={{ fontSize: 12, color: textSecondary }}>
              {new Date(cp.timestamp).toLocaleString()}
            </span>
            {cp.agent_id && (
              <span
                style={{
                  fontSize: 11,
                  padding: "1px 8px",
                  borderRadius: 3,
                  background: "#60a5fa22",
                  color: "#60a5fa",
                  fontWeight: 600,
                }}
              >
                Agent: {cp.agent_name || cp.agent_id.slice(0, 8)}
              </span>
            )}
            {cp.action && (
              <span style={{ fontSize: 11, color: textPrimary }}>
                Action: {cp.action}
              </span>
            )}
            {cp.state_hash && (
              <span style={{ fontSize: 11, color: textSecondary, fontFamily: "monospace" }}>
                State hash: {cp.state_hash}
              </span>
            )}
            <span
              style={{
                fontSize: 11,
                padding: "2px 8px",
                borderRadius: 10,
                background: cp.undone ? "#f8717122" : `${accent}22`,
                color: cp.undone ? "#f87171" : accent,
                fontWeight: 600,
              }}
            >
              {cp.undone ? "Undone" : "Active"}
            </span>
          </div>
        </div>

        {/* Changes section header */}
        <div
          style={{
            fontSize: 11,
            fontWeight: 700,
            color: textSecondary,
            textTransform: "uppercase",
            letterSpacing: 1,
          }}
        >
          Changes ({diffEntries.length})
        </div>

        {diffEntries.length > 0 && (
          <div
            style={{
              fontSize: 12,
              color: textSecondary,
              padding: "10px 12px",
              background: bgCard,
              border: `1px solid ${borderColor}`,
              borderRadius: 6,
              lineHeight: 1.5,
            }}
          >
            <strong style={{ color: textPrimary }}>These actions will be undone:</strong>{" "}
            {summarizeUndoActions(diffEntries).join(", ")}
          </div>
        )}

        {/* Change list */}
        {diffEntries.length === 0 ? (
          <div style={{ fontSize: 13, color: textSecondary, fontStyle: "italic" }}>
            No changes recorded (bookmark checkpoint)
          </div>
        ) : (
          diffEntries.map((entry, i) => {
            const icon = changeIcon(entry.change_type);
            const isState = isAgentOrConfigPath(entry.path || "");

            return (
              <div
                key={i}
                style={{
                  background: bgCard,
                  border: `1px solid ${borderColor}`,
                  borderRadius: 6,
                  padding: "10px 14px",
                }}
              >
                <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
                  {/* Icon */}
                  <div
                    style={{
                      width: 24,
                      height: 24,
                      borderRadius: 4,
                      background: `${icon.color}18`,
                      color: icon.color,
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      fontWeight: 700,
                      fontSize: 14,
                      flexShrink: 0,
                    }}
                  >
                    {isState ? (entry.path?.startsWith("config://") ? "\u2699" : "\u2699") : icon.symbol}
                  </div>

                  {/* Details */}
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div
                      style={{
                        fontSize: 13,
                        fontWeight: 500,
                        color: textPrimary,
                        whiteSpace: "nowrap",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                      }}
                      title={entry.path || ""}
                    >
                      {entry.path || "Unknown"}
                    </div>

                    {/* Size info for file changes */}
                    {!isState && (
                      <div style={{ fontSize: 11, color: textSecondary, marginTop: 2 }}>
                        {entry.change_type === "create" && `Created: ${formatBytes(entry.size_after)}`}
                        {entry.change_type === "delete" && `Deleted: ${formatBytes(entry.size_before)}`}
                        {entry.change_type === "modify" && `${formatBytes(entry.size_before)} \u2192 ${formatBytes(entry.size_after)}`}
                      </div>
                    )}

                    {/* Before/after for state changes */}
                    {isState && entry.before_value !== undefined && (
                      <div style={{ fontSize: 11, color: textSecondary, marginTop: 2, fontFamily: "monospace" }}>
                        {JSON.stringify(entry.before_value)} {"\u2192"} {JSON.stringify(entry.after_value)}
                      </div>
                    )}
                  </div>

                  {/* Change type badge */}
                  <span
                    style={{
                      fontSize: 10,
                      padding: "1px 6px",
                      borderRadius: 3,
                      background: `${icon.color}18`,
                      color: icon.color,
                      fontWeight: 600,
                      textTransform: "uppercase",
                      flexShrink: 0,
                    }}
                  >
                    {entry.change_type}
                  </span>
                </div>
              </div>
            );
          })
        )}
      </div>
    );
  };

  /* ── render ── */

  return (
    <div style={{ padding: 24, color: textPrimary, height: "100%", display: "flex", flexDirection: "column", background: bgPage }}>
      {/* Toast */}
      {toast && (
        <div
          style={{
            padding: "10px 16px",
            background: toast.type === "success" ? `${accent}18` : "#ff444422",
            border: `1px solid ${toast.type === "success" ? `${accent}44` : "#ff444466"}`,
            borderRadius: 6,
            color: toast.type === "success" ? accent : "#ff6666",
            marginBottom: 12,
            fontSize: 13,
          }}
        >
          {toast.text}
        </div>
      )}

      {/* Header */}
      <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 20 }}>
        <h1 style={{ color: accent, margin: 0, fontSize: 22 }}>Time Machine</h1>
        <span
          style={{
            background: `${accent}22`,
            color: accent,
            padding: "2px 10px",
            borderRadius: 10,
            fontSize: 12,
            fontWeight: 600,
          }}
        >
          {checkpoints.length} checkpoint{checkpoints.length !== 1 ? "s" : ""}
        </span>
      </div>

      {/* Main 2-panel layout */}
      <div style={{ display: "flex", gap: 16, flex: 1, minHeight: 0 }}>
        {/* Left panel — Timeline */}
        <div
          style={{
            width: "40%",
            display: "flex",
            flexDirection: "column",
            background: bgPanel,
            borderRadius: 8,
            border: `1px solid ${borderColor}`,
            minHeight: 0,
          }}
        >
          {/* Panel header */}
          <div
            style={{
              padding: "12px 14px",
              borderBottom: `1px solid ${borderColor}`,
              fontWeight: 600,
              fontSize: 13,
              color: textSecondary,
            }}
          >
            Checkpoint Timeline
          </div>

          {/* Action buttons */}
          <div
            style={{
              padding: "10px 14px",
              borderBottom: `1px solid ${borderColor}`,
              display: "flex",
              gap: 6,
              flexWrap: "wrap",
            }}
          >
            <button
              onClick={handleUndo}
              disabled={isUndoing || checkpoints.filter((c) => !c.undone).length === 0}
              style={{
                padding: "6px 14px",
                background: `${accent}22`,
                color: accent,
                border: `1px solid ${accent}44`,
                borderRadius: 5,
                cursor: isUndoing ? "not-allowed" : "pointer",
                fontWeight: 600,
                fontSize: 12,
                opacity: isUndoing || checkpoints.filter((c) => !c.undone).length === 0 ? 0.5 : 1,
              }}
            >
              {isUndoing ? "Undoing..." : "Undo"}
            </button>
            <button
              onClick={handleRedo}
              disabled={isRedoing}
              style={{
                padding: "6px 14px",
                background: bgCard,
                color: textSecondary,
                border: `1px solid ${borderColor}`,
                borderRadius: 5,
                cursor: isRedoing ? "not-allowed" : "pointer",
                fontWeight: 600,
                fontSize: 12,
                opacity: isRedoing ? 0.5 : 1,
              }}
            >
              {isRedoing ? "Redoing..." : "Redo"}
            </button>
            {showCreateInput ? (
              <div style={{ display: "flex", gap: 4, flex: 1, minWidth: 140 }}>
                <input
                  type="text"
                  value={createLabel}
                  onChange={(e) => setCreateLabel(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleCreate();
                    if (e.key === "Escape") {
                      setShowCreateInput(false);
                      setCreateLabel("");
                    }
                  }}
                  placeholder="Checkpoint label..."
                  autoFocus
                  style={{
                    flex: 1,
                    padding: "5px 8px",
                    background: bgInput,
                    color: textPrimary,
                    border: `1px solid ${borderColor}`,
                    borderRadius: 4,
                    fontSize: 12,
                    outline: "none",
                  }}
                />
                <button
                  onClick={handleCreate}
                  disabled={isLoading || !createLabel.trim()}
                  style={{
                    padding: "5px 10px",
                    background: createLabel.trim() ? `${accent}22` : bgCard,
                    color: createLabel.trim() ? accent : textSecondary,
                    border: `1px solid ${createLabel.trim() ? `${accent}44` : borderColor}`,
                    borderRadius: 4,
                    cursor: isLoading || !createLabel.trim() ? "not-allowed" : "pointer",
                    fontWeight: 600,
                    fontSize: 11,
                  }}
                >
                  {isLoading ? "..." : "Save"}
                </button>
              </div>
            ) : (
              <button
                onClick={() => setShowCreateInput(true)}
                style={{
                  padding: "6px 14px",
                  background: bgCard,
                  color: textSecondary,
                  border: `1px solid ${borderColor}`,
                  borderRadius: 5,
                  cursor: "pointer",
                  fontWeight: 600,
                  fontSize: 12,
                }}
              >
                + Checkpoint
              </button>
            )}
          </div>

          {/* Timeline list */}
          {renderTimeline()}
        </div>

        {/* Right panel — Details */}
        <div
          style={{
            width: "60%",
            display: "flex",
            flexDirection: "column",
            background: bgPanel,
            borderRadius: 8,
            border: `1px solid ${borderColor}`,
            minHeight: 0,
          }}
        >
          <div
            style={{
              padding: "12px 14px",
              borderBottom: `1px solid ${borderColor}`,
              fontWeight: 600,
              fontSize: 13,
              color: textSecondary,
            }}
          >
            Checkpoint Details
          </div>
          {renderDetail()}
        </div>
      </div>
    </div>
  );
}

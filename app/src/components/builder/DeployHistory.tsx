/**
 * DeployHistory — timeline of all deploys with status, diff, and rollback.
 *
 * Shows newest-first with Live/Superseded/RolledBack badges.
 * Each entry: timestamp, provider, URL, build hash, quality score, file stats.
 * Actions: Open, Share, QR Code, Diff, Rollback.
 */

import { useState, useCallback, useEffect } from "react";
import {
  builderDeployHistory,
  builderDeployRollbackTo,
  type DeployHistoryEntry,
} from "../../api/backend";
import DeployDiff from "./DeployDiff";
import ShareDialog from "./ShareDialog";

const C = {
  bg: "#0a0e14",
  surface: "#111820",
  surfaceAlt: "#0d1219",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  green: "#3fb950",
  orange: "#f0c040",
  red: "#f85149",
  redDim: "rgba(248,81,73,0.08)",
  sans: "system-ui,-apple-system,sans-serif",
  mono: "'JetBrains Mono','Fira Code',monospace",
};

interface DeployHistoryProps {
  projectId: string;
  onClose: () => void;
}

const STATUS_COLORS: Record<string, string> = {
  Live: C.green,
  Superseded: C.dim,
  RolledBack: C.orange,
  Failed: C.red,
};

const STATUS_DOTS: Record<string, string> = {
  Live: C.green,
  Superseded: C.dim,
  RolledBack: C.orange,
  Failed: C.red,
};

export default function DeployHistory({ projectId, onClose }: DeployHistoryProps) {
  const [entries, setEntries] = useState<DeployHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [diffPair, setDiffPair] = useState<[string, string] | null>(null);
  const [shareEntry, setShareEntry] = useState<DeployHistoryEntry | null>(null);
  const [rollbackConfirm, setRollbackConfirm] = useState<string | null>(null);
  const [status, setStatus] = useState("");

  const loadHistory = useCallback(async () => {
    try {
      const data = await builderDeployHistory(projectId);
      setEntries(data);
    } catch {
      setEntries([]);
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  useEffect(() => { loadHistory(); }, [loadHistory]);

  const handleRollback = useCallback(async (entryId: string) => {
    setRollbackConfirm(null);
    setStatus("Rolling back...");
    try {
      await builderDeployRollbackTo(projectId, entryId);
      setStatus("Rollback complete!");
      loadHistory();
    } catch (e) {
      setStatus(`Rollback failed: ${e}`);
    }
  }, [projectId, loadHistory]);

  const handleDiff = useCallback((entryId: string, idx: number) => {
    if (idx + 1 < entries.length) {
      setDiffPair([entries[idx + 1].id, entryId]);
    }
  }, [entries]);

  const formatTime = (iso: string) => {
    try {
      return new Date(iso).toLocaleString(undefined, {
        month: "short", day: "numeric", year: "numeric",
        hour: "numeric", minute: "2-digit",
      });
    } catch { return iso; }
  };

  const formatBytes = (b: number) => b < 1024 ? `${b} B` : `${(b / 1024).toFixed(0)} KB`;

  if (diffPair) {
    return (
      <DeployDiff
        projectId={projectId}
        fromId={diffPair[0]}
        toId={diffPair[1]}
        onClose={() => setDiffPair(null)}
      />
    );
  }

  if (shareEntry) {
    return (
      <ShareDialog
        projectId={projectId}
        entry={shareEntry}
        onClose={() => setShareEntry(null)}
      />
    );
  }

  return (
    <div style={{
      position: "fixed", top: 0, right: 0, bottom: 0, left: 0, zIndex: 1000,
      display: "flex", justifyContent: "flex-end",
      background: "rgba(0,0,0,0.5)",
    }} onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div style={{
        width: 400, height: "100%", background: C.surface,
        borderLeft: `1px solid ${C.border}`, padding: 20,
        overflowY: "auto", fontFamily: C.sans,
        display: "flex", flexDirection: "column", gap: 12,
      }}>
        {/* Header */}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span style={{ color: C.text, fontSize: 14, fontWeight: 600 }}>Deploy History</span>
          <button onClick={onClose} style={{
            background: "transparent", border: "none", color: C.dim, fontSize: 16,
            cursor: "pointer", padding: "2px 6px",
          }}>x</button>
        </div>

        {loading && <div style={{ color: C.muted, fontSize: 11 }}>Loading...</div>}

        {!loading && entries.length === 0 && (
          <div style={{ color: C.dim, fontSize: 11, textAlign: "center", paddingTop: 20 }}>
            No deploys yet. Deploy your project to see history here.
          </div>
        )}

        {/* Timeline */}
        {entries.map((entry, idx) => (
          <div key={entry.id} style={{
            background: C.surfaceAlt,
            border: `1px solid ${entry.status === "Live" ? "rgba(0,212,170,0.25)" : C.border}`,
            borderRadius: 6,
            padding: "10px 12px",
          }}>
            {/* Status row */}
            <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 4 }}>
              <div style={{
                width: 8, height: 8, borderRadius: "50%",
                background: STATUS_DOTS[entry.status] || C.dim,
              }} />
              {entry.status === "Live" && (
                <span style={{ fontSize: 9, fontWeight: 700, color: C.green, textTransform: "uppercase" }}>
                  LIVE
                </span>
              )}
              {entry.status === "RolledBack" && (
                <span style={{ fontSize: 9, color: C.orange }}>Rolled Back</span>
              )}
              <span style={{ fontSize: 10, color: C.muted, marginLeft: "auto" }}>
                {formatTime(entry.timestamp)}
              </span>
            </div>

            {/* Details */}
            <div style={{ fontSize: 10, color: C.muted, marginBottom: 2 }}>
              {entry.provider.charAt(0).toUpperCase() + entry.provider.slice(1)}
              {" \u2192 "}
              <span style={{ color: C.text }}>{entry.url}</span>
            </div>
            <div style={{ fontSize: 9, color: C.dim, fontFamily: C.mono }}>
              Build: {entry.build_hash.slice(0, 7)}
              {entry.quality_score != null && ` | Quality: ${entry.quality_score}/100`}
              {` | ${entry.file_count} files, ${formatBytes(entry.total_bytes)}`}
            </div>

            {/* Actions */}
            <div style={{ display: "flex", gap: 4, marginTop: 6, flexWrap: "wrap" }}>
              {entry.status === "Live" && (
                <>
                  <SmallBtn label="Open" onClick={() => window.open(entry.url, "_blank")} />
                  <SmallBtn label="Share" onClick={() => setShareEntry(entry)} />
                </>
              )}
              {entry.status !== "Live" && entry.status !== "Failed" && (
                <SmallBtn
                  label="Rollback to this"
                  onClick={() => setRollbackConfirm(entry.id)}
                  accent
                />
              )}
              {idx + 1 < entries.length && (
                <SmallBtn label="Diff" onClick={() => handleDiff(entry.id, idx)} />
              )}
            </div>
          </div>
        ))}

        {/* Summary */}
        {entries.length > 0 && (
          <div style={{ fontSize: 9, color: C.dim, textAlign: "center" }}>
            {entries.length} deploy{entries.length !== 1 ? "s" : ""} total
            {entries.find(e => e.status === "Live") &&
              ` | Current: ${entries.find(e => e.status === "Live")?.build_hash.slice(0, 7)}`
            }
          </div>
        )}

        {/* Status */}
        {status && (
          <div style={{ fontSize: 9, color: status.includes("fail") ? C.red : C.accent }}>
            {status}
          </div>
        )}

        {/* Rollback confirmation modal */}
        {rollbackConfirm && (
          <div style={{
            position: "fixed", top: 0, left: 0, right: 0, bottom: 0,
            background: "rgba(0,0,0,0.6)", zIndex: 2000,
            display: "flex", alignItems: "center", justifyContent: "center",
          }}>
            <div style={{
              background: C.surface, border: `1px solid ${C.border}`,
              borderRadius: 8, padding: 20, maxWidth: 300,
            }}>
              <div style={{ color: C.text, fontSize: 12, fontWeight: 600, marginBottom: 8 }}>
                Confirm Rollback
              </div>
              <div style={{ color: C.muted, fontSize: 10, marginBottom: 16 }}>
                Roll back to build {entries.find(e => e.id === rollbackConfirm)?.build_hash.slice(0, 7)}?
                This will replace the current live version.
              </div>
              <div style={{ display: "flex", gap: 8 }}>
                <button onClick={() => setRollbackConfirm(null)} style={{
                  background: "transparent", border: `1px solid ${C.border}`,
                  borderRadius: 4, padding: "6px 14px", color: C.muted, fontSize: 10,
                  cursor: "pointer", flex: 1,
                }}>Cancel</button>
                <button onClick={() => handleRollback(rollbackConfirm)} style={{
                  background: C.accent, border: "none", borderRadius: 4,
                  padding: "6px 14px", color: C.bg, fontSize: 10,
                  fontWeight: 600, cursor: "pointer", flex: 1,
                }}>Rollback</button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function SmallBtn({ label, onClick, accent }: { label: string; onClick: () => void; accent?: boolean }) {
  return (
    <button onClick={onClick} style={{
      background: accent ? C.accentDim : "transparent",
      border: `1px solid ${accent ? "rgba(0,212,170,0.25)" : C.border}`,
      borderRadius: 3,
      padding: "2px 7px",
      fontSize: 9,
      color: accent ? C.accent : C.muted,
      cursor: "pointer",
      fontFamily: C.sans,
    }}>
      {label}
    </button>
  );
}

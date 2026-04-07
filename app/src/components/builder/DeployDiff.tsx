/**
 * DeployDiff — compact diff view between two deploys.
 *
 * Shows added/modified/removed files (color-coded), unchanged count.
 * Hash-based comparison, not line-by-line content diff.
 */

import { useState, useEffect } from "react";
import { builderDeployDiff, type DeployDiffResult } from "../../api/backend";

const C = {
  surface: "#111820",
  surfaceAlt: "#0d1219",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  green: "#3fb950",
  yellow: "#f0c040",
  red: "#f85149",
  sans: "system-ui,-apple-system,sans-serif",
  mono: "'JetBrains Mono','Fira Code',monospace",
};

interface DeployDiffProps {
  projectId: string;
  fromId: string;
  toId: string;
  onClose: () => void;
}

export default function DeployDiff({ projectId, fromId, toId, onClose }: DeployDiffProps) {
  const [diff, setDiff] = useState<DeployDiffResult | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    builderDeployDiff(projectId, fromId, toId)
      .then(setDiff)
      .catch((e) => setError(String(e)));
  }, [projectId, fromId, toId]);

  const totalChanged = diff
    ? diff.added.length + diff.removed.length + diff.modified.length
    : 0;

  return (
    <div style={{
      position: "fixed", top: 0, right: 0, bottom: 0, left: 0, zIndex: 1000,
      display: "flex", justifyContent: "flex-end",
      background: "rgba(0,0,0,0.5)",
    }} onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div style={{
        width: 380, height: "100%", background: C.surface,
        borderLeft: `1px solid ${C.border}`, padding: 20,
        overflowY: "auto", fontFamily: C.sans,
        display: "flex", flexDirection: "column", gap: 12,
      }}>
        {/* Header */}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span style={{ color: C.text, fontSize: 14, fontWeight: 600 }}>Deploy Diff</span>
          <button onClick={onClose} style={{
            background: "transparent", border: "none", color: C.dim, fontSize: 16,
            cursor: "pointer", padding: "2px 6px",
          }}>x</button>
        </div>

        {error && <div style={{ color: C.red, fontSize: 10 }}>{error}</div>}

        {diff && (
          <>
            <div style={{ color: C.muted, fontSize: 10, fontFamily: C.mono }}>
              Changes: {diff.from_hash.slice(0, 7)} {"\u2192"} {diff.to_hash.slice(0, 7)}
            </div>

            {/* File list */}
            <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
              {diff.added.map((f) => (
                <FileEntry key={`+${f}`} prefix="+" color={C.green} path={f} />
              ))}
              {diff.modified.map((f) => (
                <FileEntry key={`~${f}`} prefix="~" color={C.yellow} path={f} />
              ))}
              {diff.removed.map((f) => (
                <FileEntry key={`-${f}`} prefix="-" color={C.red} path={f} />
              ))}
            </div>

            {totalChanged === 0 && (
              <div style={{ color: C.dim, fontSize: 10, textAlign: "center", paddingTop: 8 }}>
                No changes between these deploys.
              </div>
            )}

            {/* Summary */}
            <div style={{ fontSize: 9, color: C.dim, paddingTop: 4 }}>
              {diff.modified.length > 0 && `${diff.modified.length} modified`}
              {diff.added.length > 0 && `${diff.modified.length > 0 ? ", " : ""}${diff.added.length} added`}
              {diff.removed.length > 0 && `${totalChanged > diff.removed.length ? ", " : ""}${diff.removed.length} removed`}
              {diff.unchanged > 0 && `, ${diff.unchanged} unchanged`}
            </div>
          </>
        )}

        {!diff && !error && (
          <div style={{ color: C.muted, fontSize: 11 }}>Loading diff...</div>
        )}
      </div>
    </div>
  );
}

function FileEntry({ prefix, color, path }: { prefix: string; color: string; path: string }) {
  return (
    <div style={{
      display: "flex",
      gap: 6,
      padding: "3px 6px",
      borderRadius: 3,
      background: `${color}08`,
      fontFamily: "'JetBrains Mono','Fira Code',monospace",
      fontSize: 10,
    }}>
      <span style={{ color, fontWeight: 700, width: 10 }}>{prefix}</span>
      <span style={{ color: "#e2e8f0" }}>{path}</span>
    </div>
  );
}

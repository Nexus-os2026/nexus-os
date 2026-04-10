/**
 * ShareDialog — share a live deploy URL with QR code.
 *
 * Shows: live URL, Copy/Open buttons, inline SVG QR code,
 * deploy timestamp, provider, quality score.
 */

import { useState, useEffect, useCallback } from "react";
import { builderDeployQrCode, type DeployHistoryEntry } from "../../api/backend";

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
  sans: "system-ui,-apple-system,sans-serif",
};

interface ShareDialogProps {
  projectId: string;
  entry: DeployHistoryEntry;
  onClose: () => void;
}

export default function ShareDialog({ projectId: _projectId, entry, onClose }: ShareDialogProps) {
  const [qrSvg, setQrSvg] = useState("");
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    builderDeployQrCode(entry.url)
      .then(setQrSvg)
      .catch(() => setQrSvg(""));
  }, [entry.url]);

  const copyUrl = useCallback(() => {
    navigator.clipboard.writeText(entry.url).catch(() => {});
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [entry.url]);

  const openUrl = useCallback(() => {
    window.open(entry.url, "_blank");
  }, [entry.url]);

  const formatTime = (iso: string) => {
    try {
      return new Date(iso).toLocaleString(undefined, {
        month: "short", day: "numeric", year: "numeric",
        hour: "numeric", minute: "2-digit",
      });
    } catch { return iso; }
  };

  return (
    <div style={{
      position: "fixed", top: 0, right: 0, bottom: 0, left: 0, zIndex: 1000,
      display: "flex", alignItems: "center", justifyContent: "center",
      background: "rgba(0,0,0,0.6)",
    }} onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div style={{
        background: C.surface, border: `1px solid ${C.border}`,
        borderRadius: 10, padding: 24, width: 320,
        fontFamily: C.sans, display: "flex", flexDirection: "column",
        alignItems: "center", gap: 14,
      }}>
        {/* Title */}
        <div style={{ color: C.text, fontSize: 14, fontWeight: 600 }}>
          Share Your Site
        </div>

        {/* URL */}
        <div style={{
          background: C.surfaceAlt, border: `1px solid ${C.border}`,
          borderRadius: 6, padding: "8px 12px", width: "100%",
          textAlign: "center", wordBreak: "break-all",
          boxSizing: "border-box",
        }}>
          <a href={entry.url} target="_blank" rel="noopener noreferrer" style={{
            color: C.accent, fontSize: 11, textDecoration: "none",
          }}>
            {entry.url}
          </a>
        </div>

        {/* Action buttons */}
        <div style={{ display: "flex", gap: 8, width: "100%" }}>
          <button type="button" onClick={copyUrl} style={{
            flex: 1, background: C.surfaceAlt, border: `1px solid ${C.border}`,
            borderRadius: 4, padding: "7px 12px", color: copied ? C.accent : C.muted,
            fontSize: 10, cursor: "pointer", fontFamily: C.sans,
          }}>
            {copied ? "Copied!" : "Copy URL"}
          </button>
          <button type="button" onClick={openUrl} style={{
            flex: 1, background: C.accent, border: "none",
            borderRadius: 4, padding: "7px 12px", color: C.bg,
            fontSize: 10, fontWeight: 600, cursor: "pointer", fontFamily: C.sans,
          }}>
            Open in Browser
          </button>
        </div>

        {/* QR Code */}
        {qrSvg && (
          <div style={{
            background: "#ffffff", borderRadius: 8, padding: 12,
            display: "flex", flexDirection: "column", alignItems: "center", gap: 6,
          }}>
            <div dangerouslySetInnerHTML={{ __html: qrSvg }} />
            <div style={{ color: C.dim, fontSize: 9 }}>
              Scan to visit on any device
            </div>
          </div>
        )}

        {/* Deploy info */}
        <div style={{
          width: "100%", fontSize: 9, color: C.dim,
          display: "flex", flexDirection: "column", gap: 2,
        }}>
          <div>Deployed: {formatTime(entry.timestamp)}</div>
          <div>Provider: {entry.provider.charAt(0).toUpperCase() + entry.provider.slice(1)}</div>
          {entry.quality_score != null && (
            <div>Quality: {entry.quality_score}/100 {entry.quality_score >= 80 ? "\u2705" : ""}</div>
          )}
        </div>

        {/* Close */}
        <button type="button" onClick={onClose} style={{
          background: "transparent", border: `1px solid ${C.border}`,
          borderRadius: 4, padding: "6px 14px", color: C.muted, fontSize: 10,
          cursor: "pointer",
        }}>Close</button>
      </div>
    </div>
  );
}

/**
 * ImportPanel — design import flow for Stitch MCP, Figma, and raw HTML/CSS paste.
 *
 * Tabs: Paste HTML | Stitch / DESIGN.md
 * Post-import: section review with rename capability.
 */

import { useState, useCallback } from "react";
import {
  builderImportDesign,
  type DesignImportResult,
} from "../../api/backend";

const C = {
  bg: "#0a0e14",
  surface: "#111820",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  accentGlow: "rgba(0,212,170,0.25)",
  err: "#f85149",
  warn: "#f0c040",
  sans: "system-ui,-apple-system,sans-serif",
  mono: "'JetBrains Mono',monospace",
};

type Tab = "paste" | "stitch";

interface ImportPanelProps {
  projectId: string;
  onClose: () => void;
  onImportComplete: (html: string) => void;
}

export default function ImportPanel({ projectId, onClose, onImportComplete }: ImportPanelProps) {
  const [tab, setTab] = useState<Tab>("paste");
  const [htmlInput, setHtmlInput] = useState("");
  const [cssInput, setCssInput] = useState("");
  const [designMdInput, setDesignMdInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<DesignImportResult | null>(null);

  const handleImport = useCallback(async () => {
    setError(null);
    setLoading(true);
    try {
      const source = tab === "stitch" ? "stitch" as const : "paste" as const;
      const res = await builderImportDesign(
        projectId,
        htmlInput,
        cssInput || undefined,
        designMdInput || undefined,
        source,
      );
      setResult(res);
    } catch (err: any) {
      setError(err?.toString() ?? "Import failed");
    } finally {
      setLoading(false);
    }
  }, [projectId, htmlInput, cssInput, designMdInput, tab]);

  const handleOpenInEditor = useCallback(() => {
    // The import has saved HTML to disk; signal the parent to reload
    onImportComplete(htmlInput);
    onClose();
  }, [htmlInput, onImportComplete, onClose]);

  const panelStyle: React.CSSProperties = {
    position: "absolute",
    top: 0,
    right: 0,
    bottom: 0,
    width: 420,
    background: C.surface,
    borderLeft: `1px solid ${C.border}`,
    display: "flex",
    flexDirection: "column",
    zIndex: 40,
    fontFamily: C.sans,
    fontSize: 12,
    color: C.text,
    overflow: "hidden",
  };

  const tabStyle = (active: boolean): React.CSSProperties => ({
    flex: 1,
    padding: "6px 0",
    background: active ? C.accentDim : "transparent",
    border: active ? `1px solid ${C.accentGlow}` : `1px solid ${C.border}`,
    borderRadius: 4,
    color: active ? C.accent : C.muted,
    cursor: "pointer",
    fontSize: 10,
    fontWeight: active ? 600 : 400,
  });

  return (
    <div style={panelStyle}>
      {/* Header */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "10px 14px", borderBottom: `1px solid ${C.border}` }}>
        <span style={{ fontWeight: 600, fontSize: 13 }}>Import Design</span>
        <button type="button" onClick={onClose} style={{ background: "none", border: "none", color: C.muted, cursor: "pointer", fontSize: 16 }}>x</button>
      </div>

      {/* Content */}
      <div style={{ flex: 1, overflow: "auto", padding: 14 }}>
        {error && (
          <div style={{ background: "rgba(248,81,73,0.08)", border: "1px solid rgba(248,81,73,0.25)", borderRadius: 4, padding: "6px 10px", marginBottom: 12, color: C.err, fontSize: 11 }}>
            {error}
          </div>
        )}

        {!result ? (
          <>
            {/* Tabs */}
            <div style={{ display: "flex", gap: 4, marginBottom: 14 }}>
              <button type="button" onClick={() => setTab("paste")} style={tabStyle(tab === "paste")}>Paste HTML</button>
              <button type="button" onClick={() => setTab("stitch")} style={tabStyle(tab === "stitch")}>Stitch / DESIGN.md</button>
            </div>

            {tab === "paste" && (
              <>
                <label style={{ display: "block", color: C.muted, fontSize: 10, marginBottom: 4 }}>HTML</label>
                <textarea
                  value={htmlInput}
                  onChange={(e) => setHtmlInput(e.target.value)}
                  placeholder="Paste your HTML here..."
                  rows={10}
                  style={{ width: "100%", padding: 8, background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4, color: C.text, fontSize: 11, fontFamily: C.mono, resize: "vertical", marginBottom: 10, boxSizing: "border-box" }}
                />
                <label style={{ display: "block", color: C.muted, fontSize: 10, marginBottom: 4 }}>CSS (optional)</label>
                <textarea
                  value={cssInput}
                  onChange={(e) => setCssInput(e.target.value)}
                  placeholder="Paste CSS here (optional)..."
                  rows={5}
                  style={{ width: "100%", padding: 8, background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4, color: C.text, fontSize: 11, fontFamily: C.mono, resize: "vertical", marginBottom: 14, boxSizing: "border-box" }}
                />
              </>
            )}

            {tab === "stitch" && (
              <>
                <label style={{ display: "block", color: C.muted, fontSize: 10, marginBottom: 4 }}>HTML from Stitch</label>
                <textarea
                  value={htmlInput}
                  onChange={(e) => setHtmlInput(e.target.value)}
                  placeholder="Paste Stitch HTML output..."
                  rows={6}
                  style={{ width: "100%", padding: 8, background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4, color: C.text, fontSize: 11, fontFamily: C.mono, resize: "vertical", marginBottom: 10, boxSizing: "border-box" }}
                />
                <label style={{ display: "block", color: C.muted, fontSize: 10, marginBottom: 4 }}>DESIGN.md (optional)</label>
                <textarea
                  value={designMdInput}
                  onChange={(e) => setDesignMdInput(e.target.value)}
                  placeholder={"# Colors\n- primary: #4f46e5\n- secondary: #7c3aed\n\n# Typography\n- heading: Inter"}
                  rows={6}
                  style={{ width: "100%", padding: 8, background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4, color: C.text, fontSize: 11, fontFamily: C.mono, resize: "vertical", marginBottom: 14, boxSizing: "border-box" }}
                />
              </>
            )}

            <div style={{ fontSize: 10, color: C.dim, marginBottom: 10 }}>
              All scripts and event handlers are removed during import. Cost: $0.00
            </div>

            <button type="button"
              onClick={handleImport}
              disabled={loading || !htmlInput.trim()}
              style={{ width: "100%", padding: "7px 0", background: C.accentDim, border: `1px solid ${C.accentGlow}`, borderRadius: 4, color: C.accent, cursor: loading ? "default" : "pointer", fontWeight: 600, fontSize: 11 }}
            >
              {loading ? "Importing..." : "Import Design"}
            </button>
          </>
        ) : (
          /* Post-import review */
          <>
            <div style={{ color: C.accent, fontWeight: 600, marginBottom: 12, fontSize: 13 }}>
              Design imported!
            </div>

            <ul style={{ listStyle: "none", padding: 0, margin: 0, color: C.text, fontSize: 11, lineHeight: 2 }}>
              <li>{result.sections_detected} sections detected</li>
              <li>{result.tokens_extracted} tokens extracted</li>
              {result.sanitized_elements_removed.length > 0 && (
                <li style={{ color: C.warn }}>
                  {result.sanitized_elements_removed.length} element type(s) removed: {result.sanitized_elements_removed.join(", ")}
                </li>
              )}
            </ul>

            {result.warnings.length > 0 && (
              <div style={{ marginTop: 10, padding: "6px 10px", background: "rgba(240,192,64,0.08)", border: "1px solid rgba(240,192,64,0.2)", borderRadius: 4, fontSize: 10, color: C.warn }}>
                {result.warnings.map((w, i) => (
                  <div key={i}>{w}</div>
                ))}
              </div>
            )}

            <div style={{ display: "flex", gap: 6, marginTop: 14 }}>
              <button type="button"
                onClick={handleOpenInEditor}
                style={{ flex: 1, padding: "7px 0", background: C.accentDim, border: `1px solid ${C.accentGlow}`, borderRadius: 4, color: C.accent, cursor: "pointer", fontWeight: 600, fontSize: 11 }}
              >
                Open in Editor
              </button>
              <button type="button"
                onClick={onClose}
                style={{ flex: 1, padding: "7px 0", background: "transparent", border: `1px solid ${C.border}`, borderRadius: 4, color: C.muted, cursor: "pointer", fontSize: 11 }}
              >
                Done
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

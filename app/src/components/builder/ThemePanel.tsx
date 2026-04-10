/**
 * ThemePanel — global design system editor (Phase 12).
 *
 * Edits Layer 1 foundation tokens in bulk: colors, typography, spacing, radii.
 * Changes apply in real-time to the preview via postMessage, Tauri persists async.
 *
 * Sections:
 *  1. Quick Presets — swatch grid, click to apply
 *  2. Colors — light + dark mode color pickers
 *  3. Typography — font dropdowns + scale preview
 *  4. Spacing & Radii — scale selector + preview
 *  5. Import / Export / Extract — URL extraction, DESIGN.md, DTCG JSON
 */

import { useState, useCallback, useEffect, useRef } from "react";
import {
  builderThemeApply,
  builderThemeGetCurrent,
  builderThemeListPresets,
  builderThemeGetPreset,
  builderThemeExport,
  builderThemeImport,
  builderThemeExtractFromUrl,
  type ThemePresetInfo,
} from "../../api/backend";

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
  accentDim: "rgba(0,212,170,0.10)",
  err: "#f85149",
  sans: "system-ui,-apple-system,sans-serif",
  mono: "'JetBrains Mono','Fira Code',monospace",
};

interface ThemePanelProps {
  projectId: string;
  onClose: () => void;
  /** Send postMessage to preview iframe for instant feedback. */
  previewRef?: React.RefObject<HTMLIFrameElement | null>;
  onCssChanged?: (css: string) => void;
}

interface ThemeColors {
  primary: string;
  secondary: string;
  accent: string;
  bg: string;
  bg_secondary: string;
  text: string;
  text_secondary: string;
  border: string;
  dark_primary: string;
  dark_secondary: string;
  dark_accent: string;
  dark_bg: string;
  dark_bg_secondary: string;
  dark_text: string;
  dark_text_secondary: string;
  dark_border: string;
}

interface ThemeTypography {
  heading_font: string;
  body_font: string;
  mono_font: string;
  text_xs: string;
  text_sm: string;
  text_base: string;
  text_lg: string;
  text_xl: string;
  text_2xl: string;
  text_3xl: string;
  text_4xl: string;
}

interface ThemeSpacing {
  xs: string;
  sm: string;
  md: string;
  lg: string;
  xl: string;
  xxl: string;
  section: string;
}

interface ThemeRadii {
  sm: string;
  md: string;
  lg: string;
  xl: string;
  full: string;
}

interface Theme {
  name: string;
  colors: ThemeColors;
  typography: ThemeTypography;
  spacing: ThemeSpacing;
  radii: ThemeRadii;
  shadows: { sm: string; md: string; lg: string; xl: string };
  motion: { duration_fast: string; duration_normal: string; duration_slow: string; ease_default: string };
}

type Tab = "presets" | "colors" | "typography" | "spacing" | "io";

const FONT_OPTIONS = [
  { label: "Inter", value: "'Inter', system-ui, sans-serif" },
  { label: "Playfair Display", value: "'Playfair Display', Georgia, serif" },
  { label: "Plus Jakarta Sans", value: "'Plus Jakarta Sans', system-ui, sans-serif" },
  { label: "DM Sans", value: "'DM Sans', system-ui, sans-serif" },
  { label: "Source Sans", value: "'Source Sans 3', system-ui, sans-serif" },
  { label: "System UI", value: "system-ui, -apple-system, sans-serif" },
];

const MONO_OPTIONS = [
  { label: "JetBrains Mono", value: "'JetBrains Mono', ui-monospace, monospace" },
  { label: "Fira Code", value: "'Fira Code', ui-monospace, monospace" },
  { label: "DM Mono", value: "'DM Mono', ui-monospace, monospace" },
  { label: "System Mono", value: "ui-monospace, monospace" },
];

const SPACING_SCALES: Record<string, ThemeSpacing> = {
  compact: { xs: "0.125rem", sm: "0.25rem", md: "0.5rem", lg: "1rem", xl: "1.5rem", xxl: "2rem", section: "3rem" },
  default: { xs: "0.25rem", sm: "0.5rem", md: "1rem", lg: "1.5rem", xl: "2rem", xxl: "3rem", section: "4rem" },
  generous: { xs: "0.5rem", sm: "0.75rem", md: "1.5rem", lg: "2rem", xl: "3rem", xxl: "4rem", section: "6rem" },
};

const RADII_PRESETS: Record<string, ThemeRadii> = {
  sharp: { sm: "2px", md: "2px", lg: "4px", xl: "6px", full: "9999px" },
  default: { sm: "0.25rem", md: "0.5rem", lg: "0.75rem", xl: "1rem", full: "9999px" },
  rounded: { sm: "0.5rem", md: "1rem", lg: "1.5rem", xl: "2rem", full: "9999px" },
  pill: { sm: "9999px", md: "9999px", lg: "9999px", xl: "9999px", full: "9999px" },
};

export default function ThemePanel({ projectId, onClose, previewRef, onCssChanged }: ThemePanelProps) {
  const [tab, setTab] = useState<Tab>("presets");
  const [presets, setPresets] = useState<ThemePresetInfo[]>([]);
  const [theme, setTheme] = useState<Theme | null>(null);
  const [extractUrl, setExtractUrl] = useState("");
  const [extracting, setExtracting] = useState(false);
  const [importText, setImportText] = useState("");
  const [importFmt, setImportFmt] = useState("design_md");
  const [exportFmt, setExportFmt] = useState("css");
  const [exportOutput, setExportOutput] = useState("");
  const [status, setStatus] = useState("");
  const applyTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Load current theme + presets on mount
  useEffect(() => {
    builderThemeListPresets()
      .then((json) => { try { setPresets(JSON.parse(json)); } catch { /* ignore */ } })
      .catch(() => {});
    builderThemeGetCurrent(projectId)
      .then((json) => { try { setTheme(JSON.parse(json)); } catch { /* ignore */ } })
      .catch(() => {});
  }, [projectId]);

  // Send live token updates to preview iframe
  const sendToPreview = useCallback((tokenName: string, value: string) => {
    if (previewRef?.current?.contentWindow) {
      previewRef.current.contentWindow.postMessage(
        { type: "update-token", tokenName, value },
        "*"
      );
    }
  }, [previewRef]);

  // Apply theme (debounced persist + instant preview)
  const applyTheme = useCallback((updated: Theme) => {
    setTheme(updated);

    // Instant preview via postMessage — send structured token names AND
    // common LLM short-form aliases so both naming conventions update.
    const colorMap: [string, string][] = [
      // Structured names (TokenSet)
      ["color-primary", updated.colors.primary],
      ["color-secondary", updated.colors.secondary],
      ["color-accent", updated.colors.accent],
      ["color-bg", updated.colors.bg],
      ["color-bg-secondary", updated.colors.bg_secondary],
      ["color-text", updated.colors.text],
      ["color-text-secondary", updated.colors.text_secondary],
      ["color-border", updated.colors.border],
      // LLM short-form aliases (common across builds)
      ["accent", updated.colors.accent || updated.colors.primary],
      ["primary", updated.colors.primary],
      ["secondary", updated.colors.secondary],
      ["bg", updated.colors.bg],
      ["surface", updated.colors.bg_secondary],
      ["text", updated.colors.text],
      ["text-secondary", updated.colors.text_secondary],
      ["text-muted", updated.colors.text_secondary],
      ["muted", updated.colors.text_secondary],
      ["border", updated.colors.border],
    ];
    for (const [token, val] of colorMap) {
      if (val) sendToPreview(token, val);
    }
    sendToPreview("font-heading", updated.typography.heading_font);
    sendToPreview("font-body", updated.typography.body_font);
    sendToPreview("font-mono", updated.typography.mono_font);
    // LLM font aliases
    sendToPreview("font-display", updated.typography.heading_font);
    sendToPreview("ff-display", updated.typography.heading_font);
    sendToPreview("ff-body", updated.typography.body_font);

    // Debounced persist to Tauri
    if (applyTimeout.current) clearTimeout(applyTimeout.current);
    applyTimeout.current = setTimeout(() => {
      builderThemeApply(projectId, JSON.stringify(updated))
        .then((css) => { if (onCssChanged) onCssChanged(css); })
        .catch((e) => setStatus(`Error: ${e}`));
    }, 300);
  }, [projectId, sendToPreview, onCssChanged]);

  const handlePresetClick = useCallback(async (name: string) => {
    try {
      const json = await builderThemeGetPreset(name);
      const preset: Theme = JSON.parse(json);
      applyTheme(preset);
      setStatus(`Applied: ${name}`);
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  }, [applyTheme]);

  const updateColor = useCallback((field: keyof ThemeColors, value: string) => {
    if (!theme) return;
    const updated = { ...theme, colors: { ...theme.colors, [field]: value } };
    applyTheme(updated);
  }, [theme, applyTheme]);

  const updateFont = useCallback((field: "heading_font" | "body_font" | "mono_font", value: string) => {
    if (!theme) return;
    const updated = { ...theme, typography: { ...theme.typography, [field]: value } };
    applyTheme(updated);
  }, [theme, applyTheme]);

  const handleExtract = useCallback(async () => {
    if (!extractUrl) return;
    setExtracting(true);
    setStatus("Extracting...");
    try {
      const json = await builderThemeExtractFromUrl(extractUrl);
      const extracted: Theme = JSON.parse(json);
      applyTheme(extracted);
      setStatus("Theme extracted!");
    } catch (e) {
      setStatus(`Extract failed: ${e}`);
    } finally {
      setExtracting(false);
    }
  }, [extractUrl, applyTheme]);

  const handleImport = useCallback(async () => {
    if (!importText.trim()) return;
    try {
      const json = await builderThemeImport(importText, importFmt);
      const imported: Theme = JSON.parse(json);
      applyTheme(imported);
      setStatus("Theme imported!");
    } catch (e) {
      setStatus(`Import failed: ${e}`);
    }
  }, [importText, importFmt, applyTheme]);

  const handleExport = useCallback(async () => {
    try {
      const output = await builderThemeExport(projectId, exportFmt);
      setExportOutput(output);
      setStatus(`Exported as ${exportFmt}`);
    } catch (e) {
      setStatus(`Export failed: ${e}`);
    }
  }, [projectId, exportFmt]);

  return (
    <div style={{
      position: "absolute",
      top: 0,
      right: 0,
      width: 340,
      height: "100%",
      background: C.surface,
      borderLeft: `1px solid ${C.border}`,
      display: "flex",
      flexDirection: "column",
      zIndex: 30,
      fontFamily: C.sans,
      fontSize: 11,
      color: C.text,
    }}>
      {/* Header */}
      <div style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        padding: "8px 12px",
        borderBottom: `1px solid ${C.border}`,
      }}>
        <span style={{ fontWeight: 600, fontSize: 12 }}>Theme Panel</span>
        <button type="button" onClick={onClose} style={{
          background: "transparent",
          border: "none",
          color: C.muted,
          cursor: "pointer",
          fontSize: 14,
          padding: 2,
        }}>x</button>
      </div>

      {/* Tabs */}
      <div style={{
        display: "flex",
        borderBottom: `1px solid ${C.border}`,
        padding: "0 4px",
      }}>
        {(["presets", "colors", "typography", "spacing", "io"] as Tab[]).map((t) => (
          <button type="button"
            key={t}
            onClick={() => setTab(t)}
            style={{
              background: tab === t ? C.accentDim : "transparent",
              color: tab === t ? C.accent : C.muted,
              border: "none",
              borderBottom: tab === t ? `2px solid ${C.accent}` : "2px solid transparent",
              padding: "6px 8px",
              fontSize: 10,
              cursor: "pointer",
              fontFamily: C.sans,
              textTransform: "capitalize",
            }}
          >
            {t === "io" ? "Import/Export" : t}
          </button>
        ))}
      </div>

      {/* Content */}
      <div style={{ flex: 1, overflow: "auto", padding: "8px 12px" }}>
        {tab === "presets" && (
          <PresetGrid presets={presets} onSelect={handlePresetClick} />
        )}
        {tab === "colors" && theme && (
          <ColorEditor colors={theme.colors} onChange={updateColor} />
        )}
        {tab === "typography" && theme && (
          <TypographyEditor typography={theme.typography} onFontChange={updateFont} />
        )}
        {tab === "spacing" && theme && (
          <SpacingEditor
            spacing={theme.spacing}
            radii={theme.radii}
            onSpacingChange={(s) => { if (theme) applyTheme({ ...theme, spacing: s }); }}
            onRadiiChange={(r) => { if (theme) applyTheme({ ...theme, radii: r }); }}
          />
        )}
        {tab === "io" && (
          <ImportExportPanel
            extractUrl={extractUrl}
            onExtractUrlChange={setExtractUrl}
            onExtract={handleExtract}
            extracting={extracting}
            importText={importText}
            onImportTextChange={setImportText}
            importFmt={importFmt}
            onImportFmtChange={setImportFmt}
            onImport={handleImport}
            exportFmt={exportFmt}
            onExportFmtChange={setExportFmt}
            onExport={handleExport}
            exportOutput={exportOutput}
          />
        )}
      </div>

      {/* Status bar */}
      {status && (
        <div style={{
          padding: "4px 12px",
          fontSize: 9,
          color: status.startsWith("Error") ? C.err : C.accent,
          borderTop: `1px solid ${C.border}`,
        }}>
          {status}
        </div>
      )}
    </div>
  );
}

// ─── Sub-components ─────────────────────────────────────────────────────────

function PresetGrid({ presets, onSelect }: { presets: ThemePresetInfo[]; onSelect: (name: string) => void }) {
  return (
    <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 6 }}>
      {presets.map((p) => (
        <button type="button"
          key={p.name}
          onClick={() => onSelect(p.name)}
          style={{
            background: C.surfaceAlt,
            border: `1px solid ${C.border}`,
            borderRadius: 6,
            padding: "8px 6px",
            cursor: "pointer",
            textAlign: "left",
          }}
        >
          <div style={{ display: "flex", gap: 3, marginBottom: 4 }}>
            {[p.primary, p.secondary, p.accent, p.bg].map((color, i) => (
              <div key={i} style={{
                width: 14,
                height: 14,
                borderRadius: "50%",
                background: color,
                border: `1px solid ${C.border}`,
              }} />
            ))}
          </div>
          <div style={{ fontSize: 9, color: C.muted, fontFamily: C.sans }}>
            {p.name}
          </div>
        </button>
      ))}
    </div>
  );
}

function ColorEditor({ colors, onChange }: { colors: ThemeColors; onChange: (field: keyof ThemeColors, val: string) => void }) {
  const lightColors: [keyof ThemeColors, string][] = [
    ["primary", "Primary"],
    ["secondary", "Secondary"],
    ["accent", "Accent"],
    ["bg", "Background"],
    ["bg_secondary", "Surface"],
    ["text", "Text"],
    ["text_secondary", "Text Muted"],
    ["border", "Border"],
  ];
  const darkColors: [keyof ThemeColors, string][] = [
    ["dark_primary", "Primary"],
    ["dark_secondary", "Secondary"],
    ["dark_accent", "Accent"],
    ["dark_bg", "Background"],
    ["dark_bg_secondary", "Surface"],
    ["dark_text", "Text"],
    ["dark_text_secondary", "Text Muted"],
    ["dark_border", "Border"],
  ];

  return (
    <div>
      <div style={{ fontSize: 10, fontWeight: 600, color: C.muted, marginBottom: 6 }}>Light Mode</div>
      {lightColors.map(([field, label]) => (
        <ColorRow key={field} label={label} value={colors[field]} onChange={(v) => onChange(field, v)} />
      ))}
      <div style={{ fontSize: 10, fontWeight: 600, color: C.muted, margin: "12px 0 6px" }}>Dark Mode</div>
      {darkColors.map(([field, label]) => (
        <ColorRow key={field} label={label} value={colors[field]} onChange={(v) => onChange(field, v)} />
      ))}
    </div>
  );
}

function ColorRow({ label, value, onChange }: { label: string; value: string; onChange: (v: string) => void }) {
  return (
    <div style={{
      display: "flex",
      alignItems: "center",
      gap: 6,
      marginBottom: 4,
    }}>
      <span style={{ width: 70, fontSize: 10, color: C.muted }}>{label}</span>
      <input
        type="color"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        style={{ width: 24, height: 20, border: "none", padding: 0, cursor: "pointer", background: "transparent" }}
      />
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        style={{
          flex: 1,
          background: C.surfaceAlt,
          border: `1px solid ${C.border}`,
          borderRadius: 3,
          padding: "2px 6px",
          fontSize: 10,
          color: C.text,
          fontFamily: C.mono,
        }}
      />
    </div>
  );
}

function TypographyEditor({ typography, onFontChange }: {
  typography: ThemeTypography;
  onFontChange: (field: "heading_font" | "body_font" | "mono_font", val: string) => void;
}) {
  return (
    <div>
      <div style={{ marginBottom: 10 }}>
        <label style={{ fontSize: 10, color: C.muted, display: "block", marginBottom: 3 }}>Heading Font</label>
        <select
          value={typography.heading_font}
          onChange={(e) => onFontChange("heading_font", e.target.value)}
          style={selectStyle}
        >
          {FONT_OPTIONS.map((f) => <option key={f.value} value={f.value}>{f.label}</option>)}
        </select>
      </div>
      <div style={{ marginBottom: 10 }}>
        <label style={{ fontSize: 10, color: C.muted, display: "block", marginBottom: 3 }}>Body Font</label>
        <select
          value={typography.body_font}
          onChange={(e) => onFontChange("body_font", e.target.value)}
          style={selectStyle}
        >
          {FONT_OPTIONS.map((f) => <option key={f.value} value={f.value}>{f.label}</option>)}
        </select>
      </div>
      <div style={{ marginBottom: 10 }}>
        <label style={{ fontSize: 10, color: C.muted, display: "block", marginBottom: 3 }}>Mono Font</label>
        <select
          value={typography.mono_font}
          onChange={(e) => onFontChange("mono_font", e.target.value)}
          style={selectStyle}
        >
          {MONO_OPTIONS.map((f) => <option key={f.value} value={f.value}>{f.label}</option>)}
        </select>
      </div>
      <div style={{ fontSize: 10, fontWeight: 600, color: C.muted, margin: "12px 0 6px" }}>Type Scale Preview</div>
      {["4xl", "3xl", "2xl", "xl", "lg", "base", "sm", "xs"].map((size) => (
        <div key={size} style={{
          fontSize: (typography as unknown as Record<string, string>)[`text_${size}`] || "1rem",
          fontFamily: typography.heading_font,
          color: C.text,
          marginBottom: 2,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}>
          {size}: The quick brown fox
        </div>
      ))}
    </div>
  );
}

function SpacingEditor({ spacing, radii, onSpacingChange, onRadiiChange }: {
  spacing: ThemeSpacing;
  radii: ThemeRadii;
  onSpacingChange: (s: ThemeSpacing) => void;
  onRadiiChange: (r: ThemeRadii) => void;
}) {
  const currentScale = Object.entries(SPACING_SCALES).find(
    ([, v]) => v.md === spacing.md
  )?.[0] || "default";
  const currentRadii = Object.entries(RADII_PRESETS).find(
    ([, v]) => v.md === radii.md
  )?.[0] || "default";

  return (
    <div>
      <div style={{ fontSize: 10, fontWeight: 600, color: C.muted, marginBottom: 6 }}>Spacing Scale</div>
      <div style={{ display: "flex", gap: 4, marginBottom: 12 }}>
        {Object.keys(SPACING_SCALES).map((key) => (
          <button type="button"
            key={key}
            onClick={() => onSpacingChange(SPACING_SCALES[key])}
            style={{
              background: currentScale === key ? C.accentDim : C.surfaceAlt,
              color: currentScale === key ? C.accent : C.muted,
              border: `1px solid ${currentScale === key ? "rgba(0,212,170,0.25)" : C.border}`,
              borderRadius: 4,
              padding: "4px 10px",
              fontSize: 10,
              cursor: "pointer",
              fontFamily: C.sans,
              textTransform: "capitalize",
            }}
          >
            {key}
          </button>
        ))}
      </div>

      <div style={{ fontSize: 10, fontWeight: 600, color: C.muted, marginBottom: 6 }}>Border Radius</div>
      <div style={{ display: "flex", gap: 4, marginBottom: 8 }}>
        {Object.entries(RADII_PRESETS).map(([key, val]) => (
          <button type="button"
            key={key}
            onClick={() => onRadiiChange(val)}
            style={{
              background: currentRadii === key ? C.accentDim : C.surfaceAlt,
              color: currentRadii === key ? C.accent : C.muted,
              border: `1px solid ${currentRadii === key ? "rgba(0,212,170,0.25)" : C.border}`,
              borderRadius: 4,
              padding: "4px 10px",
              fontSize: 10,
              cursor: "pointer",
              fontFamily: C.sans,
              textTransform: "capitalize",
            }}
          >
            {key}
          </button>
        ))}
      </div>

      {/* Visual radius preview */}
      <div style={{ display: "flex", gap: 6, marginTop: 8 }}>
        {(["sm", "md", "lg", "xl"] as const).map((size) => (
          <div key={size} style={{
            width: 32,
            height: 32,
            background: C.accentDim,
            border: `1px solid rgba(0,212,170,0.25)`,
            borderRadius: radii[size],
          }}>
            <div style={{ fontSize: 7, textAlign: "center", paddingTop: 10, color: C.muted }}>{size}</div>
          </div>
        ))}
      </div>
    </div>
  );
}

function ImportExportPanel({
  extractUrl, onExtractUrlChange, onExtract, extracting,
  importText, onImportTextChange, importFmt, onImportFmtChange, onImport,
  exportFmt, onExportFmtChange, onExport, exportOutput,
}: {
  extractUrl: string; onExtractUrlChange: (v: string) => void; onExtract: () => void; extracting: boolean;
  importText: string; onImportTextChange: (v: string) => void; importFmt: string; onImportFmtChange: (v: string) => void; onImport: () => void;
  exportFmt: string; onExportFmtChange: (v: string) => void; onExport: () => void; exportOutput: string;
}) {
  return (
    <div>
      {/* Extract from URL */}
      <div style={{ fontSize: 10, fontWeight: 600, color: C.muted, marginBottom: 4 }}>Extract from URL</div>
      <div style={{ display: "flex", gap: 4, marginBottom: 12 }}>
        <input
          type="text"
          placeholder="https://example.com"
          value={extractUrl}
          onChange={(e) => onExtractUrlChange(e.target.value)}
          style={{ ...inputStyle, flex: 1 }}
        />
        <button type="button" onClick={onExtract} disabled={extracting || !extractUrl} style={btnStyle}>
          {extracting ? "..." : "Extract"}
        </button>
      </div>

      {/* Import */}
      <div style={{ fontSize: 10, fontWeight: 600, color: C.muted, marginBottom: 4 }}>Import</div>
      <div style={{ display: "flex", gap: 4, marginBottom: 4 }}>
        <select value={importFmt} onChange={(e) => onImportFmtChange(e.target.value)} style={{ ...selectStyle, flex: 1 }}>
          <option value="design_md">DESIGN.md</option>
          <option value="dtcg">DTCG JSON</option>
        </select>
        <button type="button" onClick={onImport} disabled={!importText.trim()} style={btnStyle}>Import</button>
      </div>
      <textarea
        value={importText}
        onChange={(e) => onImportTextChange(e.target.value)}
        placeholder="Paste DESIGN.md or DTCG JSON content..."
        style={{
          width: "100%",
          height: 80,
          background: C.surfaceAlt,
          border: `1px solid ${C.border}`,
          borderRadius: 4,
          color: C.text,
          fontSize: 9,
          fontFamily: C.mono,
          padding: 6,
          resize: "vertical",
          boxSizing: "border-box",
        }}
      />

      {/* Export */}
      <div style={{ fontSize: 10, fontWeight: 600, color: C.muted, margin: "12px 0 4px" }}>Export</div>
      <div style={{ display: "flex", gap: 4, marginBottom: 4 }}>
        <select value={exportFmt} onChange={(e) => onExportFmtChange(e.target.value)} style={{ ...selectStyle, flex: 1 }}>
          <option value="css">CSS Variables</option>
          <option value="tailwind">Tailwind Config</option>
          <option value="design_md">DESIGN.md</option>
          <option value="dtcg">DTCG JSON</option>
        </select>
        <button type="button" onClick={onExport} style={btnStyle}>Export</button>
      </div>
      {exportOutput && (
        <textarea
          readOnly
          value={exportOutput}
          style={{
            width: "100%",
            height: 120,
            background: C.surfaceAlt,
            border: `1px solid ${C.border}`,
            borderRadius: 4,
            color: C.text,
            fontSize: 9,
            fontFamily: C.mono,
            padding: 6,
            boxSizing: "border-box",
          }}
        />
      )}
    </div>
  );
}

// ─── Shared Styles ──────────────────────────────────────────────────────────

const selectStyle: React.CSSProperties = {
  background: C.surfaceAlt,
  border: `1px solid ${C.border}`,
  borderRadius: 3,
  color: C.text,
  fontSize: 10,
  padding: "3px 6px",
  fontFamily: C.sans,
};

const inputStyle: React.CSSProperties = {
  background: C.surfaceAlt,
  border: `1px solid ${C.border}`,
  borderRadius: 3,
  color: C.text,
  fontSize: 10,
  padding: "3px 6px",
  fontFamily: C.mono,
};

const btnStyle: React.CSSProperties = {
  background: "rgba(0,212,170,0.10)",
  border: "1px solid rgba(0,212,170,0.25)",
  borderRadius: 4,
  color: "#00d4aa",
  fontSize: 10,
  padding: "3px 10px",
  cursor: "pointer",
  fontFamily: "system-ui,-apple-system,sans-serif",
};

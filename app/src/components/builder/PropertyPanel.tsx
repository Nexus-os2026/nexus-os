/**
 * PropertyPanel — sidebar panel for editing selected element properties.
 *
 * Shows contextual controls based on the selected element type:
 * - Text: inline editing with character count and constraint warnings
 * - Colors: native color pickers mapped to token properties
 * - Typography: font family/size/weight selectors from token presets
 * - Spacing: numeric inputs mapped to spacing tokens
 * - Border: radius selector from token presets
 *
 * All edits are TOKEN OPERATIONS — zero inline style= attributes.
 * Changes apply instantly via postMessage (< 20ms), persist async via Tauri.
 */

import { useState, useCallback, useMemo, useRef } from "react";

/* === Design tokens (match NexusBuilder.tsx) === */
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
  err: "#f85149",
  warn: "#f0c040",
  mono: "'JetBrains Mono','Fira Code','Cascadia Code',monospace",
  sans: "system-ui,-apple-system,sans-serif",
};

// ─── Token Mappings ─────────────────────────────────────────────────────────

/** Map CSS computed property names to token names for color editing. */
const COLOR_TOKEN_MAP: Record<string, { label: string; token: string }> = {
  backgroundColor: { label: "Background", token: "color-bg" },
  color: { label: "Text Color", token: "color-text" },
  borderColor: { label: "Border", token: "color-border" },
};

/** Section-specific semantic color tokens based on element context. */
const SECTION_COLOR_TOKENS: Record<string, { label: string; token: string }[]> = {
  hero: [
    { label: "Background", token: "hero-bg" },
    { label: "Text", token: "hero-text" },
    { label: "Accent", token: "hero-accent" },
  ],
  nav: [
    { label: "Background", token: "nav-bg" },
    { label: "Text", token: "nav-text" },
    { label: "Border", token: "nav-border" },
  ],
  footer: [
    { label: "Background", token: "footer-bg" },
    { label: "Text", token: "footer-text" },
  ],
};

/** Button-specific tokens. */
const BUTTON_COLOR_TOKENS = [
  { label: "Background", token: "btn-bg" },
  { label: "Text", token: "btn-text" },
  { label: "Border", token: "btn-border" },
  { label: "Hover", token: "btn-hover-bg" },
];

/** Typography scale tokens. */
const TYPE_SCALE = [
  { label: "XS", token: "text-xs" },
  { label: "SM", token: "text-sm" },
  { label: "Base", token: "text-base" },
  { label: "LG", token: "text-lg" },
  { label: "XL", token: "text-xl" },
  { label: "2XL", token: "text-2xl" },
  { label: "3XL", token: "text-3xl" },
  { label: "4XL", token: "text-4xl" },
];

/** Radius tokens. */
const RADIUS_TOKENS = [
  { label: "SM", token: "radius-sm", value: "0.25rem" },
  { label: "MD", token: "radius-md", value: "0.5rem" },
  { label: "LG", token: "radius-lg", value: "0.75rem" },
  { label: "XL", token: "radius-xl", value: "1rem" },
  { label: "Full", token: "radius-full", value: "9999px" },
];

// ─── Types ──────────────────────────────────────────────────────────────────

export interface SelectedElement {
  sectionId: string | null;
  slotName: string | null;
  elementTag: string;
  computedStyles: Record<string, string>;
  resolvedTokens: Record<string, string>;
  currentText: string;
}

export interface UndoEntry {
  layer: 1 | 3;
  sectionId: string | null;
  tokenName: string;
  oldValue: string;
  newValue: string;
}

interface PropertyPanelProps {
  selected: SelectedElement | null;
  onTokenChange: (layer: 1 | 3, sectionId: string | null, tokenName: string, value: string) => void;
  onTextChange: (sectionId: string, slotName: string, newText: string) => void;
  onDeselect: () => void;
  maxChars?: number;
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/** Convert rgb(r, g, b) to hex #rrggbb for color input. */
function rgbToHex(rgb: string): string {
  const match = rgb.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
  if (!match) return rgb.startsWith("#") ? rgb : "#000000";
  const r = parseInt(match[1], 10);
  const g = parseInt(match[2], 10);
  const b = parseInt(match[3], 10);
  return "#" + [r, g, b].map(c => c.toString(16).padStart(2, "0")).join("");
}

/** Detect element type for smart property display. */
function detectElementType(tag: string, styles: Record<string, string>): "heading" | "button" | "text" | "section" | "image" | "input" | "generic" {
  if (tag === "h1" || tag === "h2" || tag === "h3" || tag === "h4") return "heading";
  if (tag === "button" || tag === "a") return "button";
  if (tag === "p" || tag === "span" || tag === "div" || tag === "blockquote") return "text";
  if (tag === "section" || tag === "header" || tag === "footer" || tag === "nav" || tag === "main" || tag === "aside") return "section";
  if (tag === "img" || tag === "picture" || tag === "svg") return "image";
  if (tag === "input" || tag === "textarea" || tag === "select") return "input";
  return "generic";
}

// ─── Component ──────────────────────────────────────────────────────────────

export default function PropertyPanel({ selected, onTokenChange, onTextChange, onDeselect, maxChars }: PropertyPanelProps) {
  const [textValue, setTextValue] = useState("");
  const [undoStack, setUndoStack] = useState<UndoEntry[]>([]);

  // Reset text when selection changes
  const prevText = useMemo(() => selected?.currentText || "", [selected]);
  if (textValue === "" && prevText && textValue !== prevText) {
    setTextValue(prevText);
  }

  const elementType = useMemo(
    () => selected ? detectElementType(selected.elementTag, selected.computedStyles) : "generic",
    [selected],
  );

  const handleTokenChange = useCallback(
    (layer: 1 | 3, sectionId: string | null, tokenName: string, value: string, oldValue: string) => {
      setUndoStack(prev => [...prev, { layer, sectionId, tokenName, oldValue, newValue: value }]);
      onTokenChange(layer, sectionId, tokenName, value);
    },
    [onTokenChange],
  );

  const handleUndo = useCallback(() => {
    setUndoStack(prev => {
      if (prev.length === 0) return prev;
      const last = prev[prev.length - 1];
      onTokenChange(last.layer, last.sectionId, last.tokenName, last.oldValue);
      return prev.slice(0, -1);
    });
  }, [onTokenChange]);

  const handleTextSubmit = useCallback(() => {
    if (!selected?.sectionId || !selected?.slotName) return;
    onTextChange(selected.sectionId, selected.slotName, textValue);
  }, [selected, textValue, onTextChange]);

  // Derived values — safe to compute even when selected is null (hooks must be above early return)
  const styles = selected?.computedStyles ?? {};
  const resolvedTokens = selected?.resolvedTokens ?? {};
  const sectionId = selected?.sectionId ?? null;
  const isButton = elementType === "button";
  const isSection = elementType === "section";

  // Build color tokens from actually-discovered CSS variables in the HTML.
  // The LLM generates different variable names per build (e.g. --accent vs --color-primary),
  // so we dynamically read what exists from resolvedTokens rather than hardcoding names.
  const colorTokens = useMemo(() => {
    if (!selected) return [];
    const discovered = Object.keys(resolvedTokens);
    if (discovered.length === 0) {
      // Fallback: show hardcoded token names if bridge found nothing
      return isButton
        ? BUTTON_COLOR_TOKENS
        : sectionId && SECTION_COLOR_TOKENS[sectionId]
        ? SECTION_COLOR_TOKENS[sectionId]
        : [
            { label: "Primary", token: "color-primary" },
            { label: "Accent", token: "color-accent" },
            { label: "Background", token: "color-bg" },
            { label: "Text", token: "color-text" },
          ];
    }
    // Human-readable labels for common CSS variable names
    const LABEL_MAP: Record<string, string> = {
      "bg": "Background", "background": "Background", "color-bg": "Background",
      "surface": "Surface", "color-surface": "Surface",
      "text": "Text", "color-text": "Text",
      "text-secondary": "Text Secondary", "color-text-secondary": "Text Secondary",
      "muted": "Muted Text", "text-muted": "Muted Text",
      "accent": "Accent", "color-accent": "Accent",
      "accent-hover": "Accent Hover", "accent-h": "Accent Hover", "color-accent-hover": "Accent Hover",
      "primary": "Primary", "color-primary": "Primary",
      "secondary": "Secondary", "color-secondary": "Secondary",
      "border": "Border", "color-border": "Border",
      "btn-bg": "Button BG", "btn-text": "Button Text",
      "btn-border": "Button Border", "btn-hover-bg": "Button Hover",
      "hero-bg": "Hero BG", "hero-text": "Hero Text", "hero-accent": "Hero Accent",
      "nav-bg": "Nav BG", "nav-text": "Nav Text", "nav-border": "Nav Border",
      "footer-bg": "Footer BG", "footer-text": "Footer Text",
      "card-bg": "Card BG", "card-border": "Card Border",
      "ghost": "Ghost", "outline": "Outline",
    };
    return discovered.map(token => ({
      label: LABEL_MAP[token] || token.replace(/-/g, " ").replace(/\b\w/g, c => c.toUpperCase()),
      token,
    }));
  }, [selected, resolvedTokens, isButton, sectionId]);

  // Layer routing: when the user selected an element inside a section, color
  // edits should be scoped to that section (Layer 3) so neighbouring sections
  // are unaffected.  The "Theme-wide" row below always uses Layer 1.
  const colorLayer: 1 | 3 = sectionId ? 3 : 1;

  // ── Early return AFTER all hooks ──
  if (!selected) {
    return (
      <div style={{ width: 260, minWidth: 260, background: C.surface, borderLeft: `1px solid ${C.border}`, padding: 16, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 8 }}>
        <div style={{ fontSize: 11, color: C.dim, textAlign: "center" }}>Click an element in the preview to edit its properties</div>
      </div>
    );
  }

  const hasText = elementType === "heading" || elementType === "text" || elementType === "button";
  const charCount = textValue.length;
  const charWarning = maxChars ? charCount > maxChars * 0.9 : false;
  const charOver = maxChars ? charCount > maxChars : false;

  return (
    <div style={{ width: 260, minWidth: 260, background: C.surface, borderLeft: `1px solid ${C.border}`, display: "flex", flexDirection: "column", overflow: "hidden" }}>
      {/* Header */}
      <div style={{ padding: "10px 12px", borderBottom: `1px solid ${C.border}`, display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div>
          <span style={{ fontSize: 10, color: C.accent, fontWeight: 600, textTransform: "uppercase", letterSpacing: 1 }}>Properties</span>
          <div style={{ fontSize: 10, color: C.dim, marginTop: 2 }}>
            &lt;{selected.elementTag}&gt;
            {sectionId && <span style={{ color: C.muted }}> in {sectionId}</span>}
          </div>
        </div>
        <div style={{ display: "flex", gap: 4 }}>
          {undoStack.length > 0 && (
            <button onClick={handleUndo} style={{ background: "transparent", color: C.muted, border: `1px solid ${C.border}`, borderRadius: 4, padding: "2px 8px", fontSize: 10, cursor: "pointer" }} title="Undo last change">
              Undo
            </button>
          )}
          <button onClick={onDeselect} style={{ background: "transparent", color: C.dim, border: "none", padding: "2px 6px", fontSize: 14, cursor: "pointer" }} title="Deselect">
            {"\u00D7"}
          </button>
        </div>
      </div>

      <div style={{ flex: 1, overflow: "auto", padding: "8px 12px", display: "flex", flexDirection: "column", gap: 14 }}>
        {/* Text Editing */}
        {hasText && selected.slotName && (
          <Section title="Text">
            <textarea
              value={textValue}
              onChange={e => setTextValue(e.target.value)}
              onBlur={handleTextSubmit}
              style={{
                width: "100%", minHeight: 60, resize: "vertical",
                background: C.surfaceAlt, color: C.text, border: `1px solid ${charOver ? C.err : C.border}`,
                borderRadius: 4, padding: 8, fontSize: 11, fontFamily: C.sans, lineHeight: 1.4,
                outline: "none",
              }}
              onFocus={e => { e.currentTarget.style.borderColor = charOver ? C.err : C.accent; }}
            />
            <div style={{ display: "flex", justifyContent: "space-between", fontSize: 9, marginTop: 2 }}>
              <span style={{ color: charOver ? C.err : charWarning ? C.warn : C.dim }}>
                {charCount}{maxChars ? ` / ${maxChars}` : ""} chars
              </span>
              {charOver && <span style={{ color: C.err }}>Exceeds limit</span>}
            </div>
          </Section>
        )}

        {/* Colors */}
        <Section title="Colors">
          {colorTokens.map(ct => (
            <ColorRow
              key={ct.token}
              label={ct.label}
              tokenName={ct.token}
              currentValue={resolvedTokens[ct.token] || rgbToHex(styles.backgroundColor || styles.color || "#666666")}
              layer={colorLayer}
              sectionId={colorLayer === 1 ? null : sectionId}
              onChange={handleTokenChange}
            />
          ))}
          {/* Global theme colors — show primary/accent shortcuts when editing a scoped section */}
          {colorLayer === 3 && (
            <div style={{ marginTop: 6, paddingTop: 6, borderTop: `1px solid ${C.border}` }}>
              <div style={{ fontSize: 9, color: C.dim, marginBottom: 4 }}>Theme-wide</div>
              {/* Show the first 2-3 discovered color tokens as global overrides */}
              {Object.entries(resolvedTokens).slice(0, 3).map(([token, hex]) => (
                <ColorRow key={`global-${token}`} label={token.replace(/-/g, " ").replace(/\b\w/g, c => c.toUpperCase())} tokenName={token} currentValue={hex} layer={1} sectionId={null} onChange={handleTokenChange} />
              ))}
            </div>
          )}
        </Section>

        {/* Typography (for text elements) */}
        {(elementType === "heading" || elementType === "text") && (
          <Section title="Typography">
            <div style={{ display: "flex", flexWrap: "wrap", gap: 3 }}>
              {TYPE_SCALE.map(ts => (
                <button
                  key={ts.token}
                  onClick={() => handleTokenChange(sectionId ? 3 : 1, sectionId, ts.token, `var(--${ts.token})`, styles.fontSize || "")}
                  style={{
                    background: C.surfaceAlt, color: C.muted, border: `1px solid ${C.border}`,
                    borderRadius: 3, padding: "2px 6px", fontSize: 9, cursor: "pointer",
                  }}
                >
                  {ts.label}
                </button>
              ))}
            </div>
            <div style={{ marginTop: 6 }}>
              <Label text="Font Weight" />
              <div style={{ display: "flex", gap: 3 }}>
                {["400", "500", "600", "700", "800"].map(w => (
                  <button
                    key={w}
                    onClick={() => {
                      // Font weight is not a token operation but a direct style — skip for now
                      // This would require a CSS class toggle approach in a future phase
                    }}
                    style={{
                      background: styles.fontWeight === w ? C.accentDim : C.surfaceAlt,
                      color: styles.fontWeight === w ? C.accent : C.muted,
                      border: `1px solid ${styles.fontWeight === w ? C.accent : C.border}`,
                      borderRadius: 3, padding: "2px 6px", fontSize: 9, cursor: "pointer", fontWeight: parseInt(w),
                    }}
                  >
                    {w}
                  </button>
                ))}
              </div>
            </div>
          </Section>
        )}

        {/* Border Radius (for buttons, cards) */}
        {(isButton || elementType === "generic") && (
          <Section title="Border Radius">
            <div style={{ display: "flex", gap: 3 }}>
              {RADIUS_TOKENS.map(rt => (
                <button
                  key={rt.token}
                  onClick={() => handleTokenChange(sectionId ? 3 : 1, sectionId, rt.token, rt.value, styles.borderRadius || "")}
                  style={{
                    background: C.surfaceAlt, color: C.muted, border: `1px solid ${C.border}`,
                    borderRadius: 3, padding: "2px 6px", fontSize: 9, cursor: "pointer",
                  }}
                >
                  {rt.label}
                </button>
              ))}
            </div>
          </Section>
        )}

        {/* Spacing (for sections) */}
        {isSection && (
          <Section title="Spacing">
            <Label text="Section Padding" />
            <input
              type="range"
              min={0}
              max={8}
              step={0.5}
              defaultValue={4}
              onChange={e => {
                const val = `${e.target.value}rem`;
                handleTokenChange(3, sectionId, "space-section", val, "");
              }}
              style={{ width: "100%", accentColor: C.accent }}
            />
          </Section>
        )}
      </div>
    </div>
  );
}

// ─── Sub-components ─────────────────────────────────────────────────────────

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <div style={{ fontSize: 9, fontWeight: 600, color: C.dim, textTransform: "uppercase", letterSpacing: 1, marginBottom: 6 }}>{title}</div>
      {children}
    </div>
  );
}

function Label({ text }: { text: string }) {
  return <div style={{ fontSize: 9, color: C.dim, marginBottom: 3 }}>{text}</div>;
}

function ColorRow({
  label,
  tokenName,
  currentValue,
  layer,
  sectionId,
  onChange,
}: {
  label: string;
  tokenName: string;
  currentValue: string;
  layer: 1 | 3;
  sectionId: string | null;
  onChange: (layer: 1 | 3, sectionId: string | null, tokenName: string, value: string, oldValue: string) => void;
}) {
  const [val, setVal] = useState(currentValue);

  // Sync with prop when selection changes
  const prevProp = useRef(currentValue);
  if (prevProp.current !== currentValue) {
    prevProp.current = currentValue;
    setVal(currentValue);
  }

  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
      <input
        type="color"
        value={val}
        onChange={e => {
          setVal(e.target.value);
          onChange(layer, sectionId, tokenName, e.target.value, val);
        }}
        style={{ width: 24, height: 24, border: `1px solid ${C.border}`, borderRadius: 4, cursor: "pointer", padding: 0, background: "transparent" }}
      />
      <div style={{ flex: 1 }}>
        <div style={{ fontSize: 10, color: C.text }}>{label}</div>
        <div style={{ fontSize: 9, color: C.dim, fontFamily: C.mono }}>{tokenName}</div>
      </div>
      <div style={{ fontSize: 9, color: C.dim, fontFamily: C.mono }}>{val}</div>
    </div>
  );
}

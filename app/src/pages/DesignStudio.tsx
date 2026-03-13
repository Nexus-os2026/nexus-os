import { useState, useCallback, useMemo, useRef } from "react";
import "./design-studio.css";

/* ─── types ─── */
type ViewMode = "design" | "preview" | "code" | "tokens";
type ComponentCategory = "layout" | "input" | "display" | "navigation" | "feedback";

interface DesignToken {
  id: string;
  category: "color" | "spacing" | "font" | "radius" | "shadow";
  name: string;
  value: string;
  cssVar: string;
}

interface CanvasComponent {
  id: string;
  type: string;
  label: string;
  x: number;
  y: number;
  w: number;
  h: number;
  props: Record<string, string>;
  children?: CanvasComponent[];
  locked: boolean;
}

interface LibraryComponent {
  type: string;
  label: string;
  icon: string;
  category: ComponentCategory;
  defaultW: number;
  defaultH: number;
  defaultProps: Record<string, string>;
  governed: boolean;
}

interface VersionEntry {
  id: string;
  label: string;
  timestamp: number;
  componentCount: number;
  author: string;
  thumbnail: string; // ascii art representation
}

/* ─── constants ─── */
const COMPONENT_LIBRARY: LibraryComponent[] = [
  // layout
  { type: "container", label: "Container", icon: "▢", category: "layout", defaultW: 400, defaultH: 300, defaultProps: { bg: "#0f172a", border: "1px solid rgba(56,189,248,0.15)", borderRadius: "12px", padding: "16px" }, governed: false },
  { type: "row", label: "Row", icon: "⊟", category: "layout", defaultW: 380, defaultH: 60, defaultProps: { display: "flex", gap: "12px", alignItems: "center" }, governed: false },
  { type: "column", label: "Column", icon: "⊞", category: "layout", defaultW: 180, defaultH: 250, defaultProps: { display: "flex", flexDirection: "column", gap: "8px" }, governed: false },
  { type: "card", label: "Card", icon: "▣", category: "layout", defaultW: 280, defaultH: 180, defaultProps: { bg: "rgba(15,23,42,0.8)", border: "1px solid rgba(56,189,248,0.1)", borderRadius: "10px", padding: "16px", title: "Card Title", content: "Card content goes here." }, governed: false },
  { type: "modal", label: "Modal", icon: "◰", category: "layout", defaultW: 420, defaultH: 280, defaultProps: { bg: "#1e293b", border: "1px solid rgba(56,189,248,0.2)", borderRadius: "14px", padding: "20px", title: "Modal Title", overlay: "true" }, governed: true },
  // input
  { type: "button", label: "Button", icon: "⬡", category: "input", defaultW: 140, defaultH: 40, defaultProps: { text: "Click Me", variant: "primary" }, governed: false },
  { type: "input", label: "Text Input", icon: "▬", category: "input", defaultW: 260, defaultH: 40, defaultProps: { placeholder: "Enter text...", label: "Label" }, governed: false },
  { type: "textarea", label: "Textarea", icon: "▭", category: "input", defaultW: 280, defaultH: 100, defaultProps: { placeholder: "Write something...", rows: "4" }, governed: false },
  { type: "select", label: "Select", icon: "▼", category: "input", defaultW: 200, defaultH: 40, defaultProps: { label: "Choose", options: "Option A,Option B,Option C" }, governed: false },
  { type: "checkbox", label: "Checkbox", icon: "☐", category: "input", defaultW: 160, defaultH: 30, defaultProps: { label: "Accept terms", checked: "false" }, governed: false },
  { type: "toggle", label: "Toggle", icon: "◑", category: "input", defaultW: 60, defaultH: 30, defaultProps: { enabled: "true" }, governed: false },
  { type: "form", label: "Form", icon: "📋", category: "input", defaultW: 320, defaultH: 240, defaultProps: { fields: "Name,Email,Message", submitText: "Submit" }, governed: true },
  // display
  { type: "heading", label: "Heading", icon: "H", category: "display", defaultW: 300, defaultH: 36, defaultProps: { text: "Heading Text", level: "2" }, governed: false },
  { type: "text", label: "Paragraph", icon: "¶", category: "display", defaultW: 300, defaultH: 60, defaultProps: { text: "Lorem ipsum dolor sit amet, consectetur adipiscing elit." }, governed: false },
  { type: "badge", label: "Badge", icon: "◈", category: "display", defaultW: 80, defaultH: 28, defaultProps: { text: "Active", color: "#34d399" }, governed: false },
  { type: "avatar", label: "Avatar", icon: "◉", category: "display", defaultW: 48, defaultH: 48, defaultProps: { initials: "NA", bg: "#a78bfa" }, governed: false },
  { type: "divider", label: "Divider", icon: "—", category: "display", defaultW: 300, defaultH: 2, defaultProps: { color: "rgba(56,189,248,0.12)" }, governed: false },
  { type: "image", label: "Image", icon: "🖼", category: "display", defaultW: 200, defaultH: 140, defaultProps: { alt: "Placeholder", bg: "#1e293b" }, governed: false },
  { type: "table", label: "Table", icon: "▦", category: "display", defaultW: 400, defaultH: 180, defaultProps: { columns: "Name,Status,Fuel", rows: "3" }, governed: true },
  { type: "stat", label: "Stat Card", icon: "◔", category: "display", defaultW: 160, defaultH: 90, defaultProps: { label: "Total Users", value: "1,247", change: "+12%" }, governed: false },
  // navigation
  { type: "navbar", label: "Navbar", icon: "≡", category: "navigation", defaultW: 500, defaultH: 56, defaultProps: { brand: "Nexus OS", items: "Home,Agents,Settings" }, governed: false },
  { type: "sidebar-nav", label: "Sidebar", icon: "◧", category: "navigation", defaultW: 220, defaultH: 400, defaultProps: { items: "Dashboard,Agents,Audit,Settings", active: "Dashboard" }, governed: false },
  { type: "tabs", label: "Tabs", icon: "⊟", category: "navigation", defaultW: 300, defaultH: 40, defaultProps: { items: "Tab 1,Tab 2,Tab 3", active: "Tab 1" }, governed: false },
  { type: "breadcrumb", label: "Breadcrumb", icon: "›", category: "navigation", defaultW: 280, defaultH: 28, defaultProps: { path: "Home / Agents / Coder Agent" }, governed: false },
  // feedback
  { type: "alert", label: "Alert", icon: "⚠", category: "feedback", defaultW: 340, defaultH: 50, defaultProps: { text: "Operation completed successfully.", severity: "success" }, governed: false },
  { type: "toast", label: "Toast", icon: "◫", category: "feedback", defaultW: 300, defaultH: 48, defaultProps: { text: "Changes saved.", icon: "✓" }, governed: false },
  { type: "progress", label: "Progress Bar", icon: "▰", category: "feedback", defaultW: 240, defaultH: 20, defaultProps: { value: "65", max: "100" }, governed: false },
  { type: "spinner", label: "Spinner", icon: "◌", category: "feedback", defaultW: 40, defaultH: 40, defaultProps: { size: "md" }, governed: false },
  { type: "tooltip", label: "Tooltip", icon: "💬", category: "feedback", defaultW: 180, defaultH: 36, defaultProps: { text: "Helpful tip here" }, governed: false },
];

const DESIGN_TOKENS: DesignToken[] = [
  { id: "dt-1", category: "color", name: "Primary", value: "var(--nexus-accent)", cssVar: "--nx-primary" },
  { id: "dt-2", category: "color", name: "Secondary", value: "#a78bfa", cssVar: "--nx-secondary" },
  { id: "dt-3", category: "color", name: "Success", value: "#34d399", cssVar: "--nx-success" },
  { id: "dt-4", category: "color", name: "Warning", value: "#fbbf24", cssVar: "--nx-warning" },
  { id: "dt-5", category: "color", name: "Danger", value: "#f87171", cssVar: "--nx-danger" },
  { id: "dt-6", category: "color", name: "Background", value: "#0b1120", cssVar: "--nx-bg" },
  { id: "dt-7", category: "color", name: "Surface", value: "#0f172a", cssVar: "--nx-surface" },
  { id: "dt-8", category: "color", name: "Border", value: "rgba(56,189,248,0.12)", cssVar: "--nx-border" },
  { id: "dt-9", category: "color", name: "Text Primary", value: "#e2e8f0", cssVar: "--nx-text" },
  { id: "dt-10", category: "color", name: "Text Muted", value: "#64748b", cssVar: "--nx-text-muted" },
  { id: "dt-11", category: "spacing", name: "XS", value: "4px", cssVar: "--nx-space-xs" },
  { id: "dt-12", category: "spacing", name: "SM", value: "8px", cssVar: "--nx-space-sm" },
  { id: "dt-13", category: "spacing", name: "MD", value: "16px", cssVar: "--nx-space-md" },
  { id: "dt-14", category: "spacing", name: "LG", value: "24px", cssVar: "--nx-space-lg" },
  { id: "dt-15", category: "spacing", name: "XL", value: "32px", cssVar: "--nx-space-xl" },
  { id: "dt-16", category: "font", name: "Mono", value: "'JetBrains Mono', monospace", cssVar: "--nx-font-mono" },
  { id: "dt-17", category: "font", name: "Size SM", value: "12px", cssVar: "--nx-font-sm" },
  { id: "dt-18", category: "font", name: "Size MD", value: "14px", cssVar: "--nx-font-md" },
  { id: "dt-19", category: "font", name: "Size LG", value: "18px", cssVar: "--nx-font-lg" },
  { id: "dt-20", category: "radius", name: "SM", value: "4px", cssVar: "--nx-radius-sm" },
  { id: "dt-21", category: "radius", name: "MD", value: "8px", cssVar: "--nx-radius-md" },
  { id: "dt-22", category: "radius", name: "LG", value: "12px", cssVar: "--nx-radius-lg" },
  { id: "dt-23", category: "radius", name: "Full", value: "9999px", cssVar: "--nx-radius-full" },
  { id: "dt-24", category: "shadow", name: "SM", value: "0 1px 3px rgba(0,0,0,0.3)", cssVar: "--nx-shadow-sm" },
  { id: "dt-25", category: "shadow", name: "MD", value: "0 4px 12px rgba(0,0,0,0.4)", cssVar: "--nx-shadow-md" },
  { id: "dt-26", category: "shadow", name: "Glow", value: "0 0 15px rgba(34,211,238,0.15)", cssVar: "--nx-shadow-glow" },
];

const INITIAL_CANVAS: CanvasComponent[] = [
  { id: "c-1", type: "navbar", label: "Top Nav", x: 20, y: 20, w: 560, h: 56, props: { brand: "Nexus OS", items: "Dashboard,Agents,Audit,Settings" }, locked: false },
  { id: "c-2", type: "card", label: "Agent Status", x: 20, y: 96, w: 270, h: 160, props: { bg: "rgba(15,23,42,0.8)", border: "1px solid rgba(56,189,248,0.1)", borderRadius: "10px", padding: "16px", title: "Active Agents", content: "4 agents running, 2340 fuel used" }, locked: false },
  { id: "c-3", type: "card", label: "Fuel Overview", x: 310, y: 96, w: 270, h: 160, props: { bg: "rgba(15,23,42,0.8)", border: "1px solid rgba(56,189,248,0.1)", borderRadius: "10px", padding: "16px", title: "Fuel Budget", content: "7,110 / 15,000 fuel remaining" }, locked: false },
  { id: "c-4", type: "stat", label: "Stat 1", x: 20, y: 276, w: 170, h: 90, props: { label: "Tasks Done", value: "47", change: "+8%" }, locked: false },
  { id: "c-5", type: "stat", label: "Stat 2", x: 210, y: 276, w: 170, h: 90, props: { label: "Audit Events", value: "12.8K", change: "+2.1%" }, locked: false },
  { id: "c-6", type: "stat", label: "Stat 3", x: 400, y: 276, w: 170, h: 90, props: { label: "Uptime", value: "99.7%", change: "+0.1%" }, locked: false },
  { id: "c-7", type: "button", label: "CTA Button", x: 20, y: 390, w: 160, h: 42, props: { text: "Deploy Now", variant: "primary" }, locked: false },
  { id: "c-8", type: "button", label: "Secondary", x: 200, y: 390, w: 140, h: 42, props: { text: "View Logs", variant: "secondary" }, locked: false },
  { id: "c-9", type: "alert", label: "Alert", x: 20, y: 450, w: 550, h: 50, props: { text: "All agents passed governance checks. System healthy.", severity: "success" }, locked: false },
];

const INITIAL_VERSIONS: VersionEntry[] = [
  { id: "v-3", label: "Dashboard layout v3", timestamp: Date.now() - 300000, componentCount: 9, author: "You", thumbnail: "┌─Nav──────────┐\n│ Card │ Card  │\n│ S │ S │ S   │\n│[Btn][Btn]    │\n│ Alert bar    │" },
  { id: "v-2", label: "Added stat cards", timestamp: Date.now() - 3600000, componentCount: 7, author: "Designer Agent", thumbnail: "┌─Nav──────────┐\n│ Card │ Card  │\n│ S │ S │ S   │\n└──────────────┘" },
  { id: "v-1", label: "Initial layout", timestamp: Date.now() - 86400000, componentCount: 3, author: "You", thumbnail: "┌─Nav──────────┐\n│ Card │ Card  │\n└──────────────┘" },
];

const AI_PROMPTS = [
  "Create a dashboard with agent status cards and fuel metrics",
  "Design a login form with email, password, and social sign-in buttons",
  "Build a settings page with toggle switches and save button",
  "Make a kanban board layout with three columns",
  "Design a chat interface with message list and input area",
  "Create a pricing page with three tier cards",
];

/* ─── render component to preview HTML ─── */
function renderComponentPreview(comp: CanvasComponent): string {
  const p = comp.props;
  switch (comp.type) {
    case "button": {
      const cls = p.variant === "primary" ? "ds-prev-btn-primary" : "ds-prev-btn-secondary";
      return `<button class="${cls}">${p.text}</button>`;
    }
    case "input": return `<div class="ds-prev-field"><label>${p.label}</label><input placeholder="${p.placeholder}" /></div>`;
    case "textarea": return `<div class="ds-prev-field"><label>${p.label || ""}</label><textarea placeholder="${p.placeholder}" rows="${p.rows}"></textarea></div>`;
    case "select": return `<div class="ds-prev-field"><label>${p.label}</label><select>${(p.options || "").split(",").map(o => `<option>${o.trim()}</option>`).join("")}</select></div>`;
    case "checkbox": return `<label class="ds-prev-check"><input type="checkbox" ${p.checked === "true" ? "checked" : ""} />${p.label}</label>`;
    case "toggle": return `<div class="ds-prev-toggle ${p.enabled === "true" ? "on" : ""}"><div class="ds-prev-toggle-knob"></div></div>`;
    case "heading": return `<h${p.level} class="ds-prev-heading">${p.text}</h${p.level}>`;
    case "text": return `<p class="ds-prev-text">${p.text}</p>`;
    case "badge": return `<span class="ds-prev-badge" style="background:${p.color}22;color:${p.color}">${p.text}</span>`;
    case "avatar": return `<div class="ds-prev-avatar" style="background:${p.bg}">${p.initials}</div>`;
    case "divider": return `<hr class="ds-prev-divider" style="border-color:${p.color}" />`;
    case "image": return `<div class="ds-prev-image" style="background:${p.bg}"><span>🖼 ${p.alt}</span></div>`;
    case "card": return `<div class="ds-prev-card"><div class="ds-prev-card-title">${p.title}</div><div class="ds-prev-card-content">${p.content}</div></div>`;
    case "stat": return `<div class="ds-prev-stat"><div class="ds-prev-stat-label">${p.label}</div><div class="ds-prev-stat-value">${p.value}</div><div class="ds-prev-stat-change" style="color:${(p.change || "").startsWith("+") ? "#34d399" : "#f87171"}">${p.change}</div></div>`;
    case "navbar": return `<nav class="ds-prev-navbar"><span class="ds-prev-brand">${p.brand}</span><div class="ds-prev-nav-items">${(p.items || "").split(",").map(i => `<a>${i.trim()}</a>`).join("")}</div></nav>`;
    case "tabs": return `<div class="ds-prev-tabs">${(p.items || "").split(",").map(i => `<button class="${i.trim() === p.active ? "active" : ""}">${i.trim()}</button>`).join("")}</div>`;
    case "breadcrumb": return `<div class="ds-prev-breadcrumb">${p.path}</div>`;
    case "alert": {
      const cls = `ds-prev-alert ds-prev-alert-${p.severity}`;
      return `<div class="${cls}">${p.severity === "success" ? "✓" : p.severity === "warning" ? "⚠" : "✗"} ${p.text}</div>`;
    }
    case "toast": return `<div class="ds-prev-toast">${p.icon} ${p.text}</div>`;
    case "progress": return `<div class="ds-prev-progress"><div class="ds-prev-progress-fill" style="width:${Math.round((Number(p.value) / Number(p.max)) * 100)}%"></div></div>`;
    case "spinner": return `<div class="ds-prev-spinner"></div>`;
    case "table": {
      const cols = (p.columns || "").split(",");
      const rows = Number(p.rows) || 3;
      return `<table class="ds-prev-table"><thead><tr>${cols.map(c => `<th>${c.trim()}</th>`).join("")}</tr></thead><tbody>${Array.from({ length: rows }, (_, i) => `<tr>${cols.map(c => `<td>${c.trim()} ${i + 1}</td>`).join("")}</tr>`).join("")}</tbody></table>`;
    }
    case "form": {
      const fields = (p.fields || "").split(",");
      return `<form class="ds-prev-form">${fields.map(f => `<div class="ds-prev-field"><label>${f.trim()}</label><input placeholder="${f.trim()}..." /></div>`).join("")}<button class="ds-prev-btn-primary">${p.submitText}</button></form>`;
    }
    case "sidebar-nav": {
      const items = (p.items || "").split(",");
      return `<nav class="ds-prev-sidebar-nav">${items.map(i => `<a class="${i.trim() === p.active ? "active" : ""}">${i.trim()}</a>`).join("")}</nav>`;
    }
    case "modal": return `<div class="ds-prev-modal"><div class="ds-prev-modal-title">${p.title}</div><div class="ds-prev-modal-body">Modal content here</div><div class="ds-prev-modal-actions"><button class="ds-prev-btn-primary">Confirm</button><button class="ds-prev-btn-secondary">Cancel</button></div></div>`;
    case "tooltip": return `<div class="ds-prev-tooltip">${p.text}</div>`;
    default: return `<div class="ds-prev-unknown">${comp.type}</div>`;
  }
}

/* ─── generate code export ─── */
function generateCode(components: CanvasComponent[]): string {
  const imports = new Set<string>();
  imports.add("import React from 'react';");

  const lines = components.map(c => {
    const p = c.props;
    switch (c.type) {
      case "button": return `      <button className="${p.variant === "primary" ? "btn-primary" : "btn-secondary"}">${p.text}</button>`;
      case "input": return `      <div className="field">\n        <label>${p.label}</label>\n        <input placeholder="${p.placeholder}" />\n      </div>`;
      case "heading": return `      <h${p.level}>${p.text}</h${p.level}>`;
      case "text": return `      <p>${p.text}</p>`;
      case "card": return `      <div className="card">\n        <h3>${p.title}</h3>\n        <p>${p.content}</p>\n      </div>`;
      case "stat": return `      <div className="stat-card">\n        <span className="stat-label">${p.label}</span>\n        <span className="stat-value">${p.value}</span>\n        <span className="stat-change">${p.change}</span>\n      </div>`;
      case "navbar": return `      <nav className="navbar">\n        <span className="brand">${p.brand}</span>\n        <div className="nav-items">\n${(p.items || "").split(",").map(i => `          <a href="#">${i.trim()}</a>`).join("\n")}\n        </div>\n      </nav>`;
      case "alert": return `      <div className="alert alert-${p.severity}">${p.text}</div>`;
      case "badge": return `      <span className="badge" style={{ color: "${p.color}" }}>${p.text}</span>`;
      case "divider": return `      <hr />`;
      case "table": {
        const cols = (p.columns || "").split(",");
        return `      <table>\n        <thead><tr>${cols.map(c => `<th>${c.trim()}</th>`).join("")}</tr></thead>\n        <tbody>{/* rows */}</tbody>\n      </table>`;
      }
      default: return `      {/* ${c.type}: ${c.label} */}`;
    }
  });

  return `${Array.from(imports).join("\n")}\n\nexport default function GeneratedLayout() {\n  return (\n    <div className="layout">\n${lines.join("\n\n")}\n    </div>\n  );\n}`;
}

/* ─── component ─── */
export default function DesignStudio() {
  const [viewMode, setViewMode] = useState<ViewMode>("design");
  const [canvas, setCanvas] = useState<CanvasComponent[]>(INITIAL_CANVAS);
  const [selectedId, setSelectedId] = useState<string | null>("c-1");
  const [tokens, setTokens] = useState<DesignToken[]>(DESIGN_TOKENS);
  const [versions, setVersions] = useState<VersionEntry[]>(INITIAL_VERSIONS);
  const [libFilter, setLibFilter] = useState<ComponentCategory | "all">("all");
  const [libSearch, setLibSearch] = useState("");
  const [aiPrompt, setAiPrompt] = useState("");
  const [aiGenerating, setAiGenerating] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [fuelUsed, setFuelUsed] = useState(128);
  const [zoom, setZoom] = useState(100);
  const [draggedLib, setDraggedLib] = useState<string | null>(null);
  const [dragging, setDragging] = useState<{ id: string; offsetX: number; offsetY: number } | null>(null);
  const [resizing, setResizing] = useState<string | null>(null);

  const canvasRef = useRef<HTMLDivElement>(null);
  const selected = useMemo(() => canvas.find(c => c.id === selectedId), [canvas, selectedId]);

  /* ─── filtered library ─── */
  const filteredLib = useMemo(() => {
    let list = COMPONENT_LIBRARY;
    if (libFilter !== "all") list = list.filter(c => c.category === libFilter);
    if (libSearch.trim()) {
      const q = libSearch.toLowerCase();
      list = list.filter(c => c.label.toLowerCase().includes(q) || c.type.includes(q));
    }
    return list;
  }, [libFilter, libSearch]);

  /* ─── canvas handlers ─── */
  const addToCanvas = useCallback((type: string, x?: number, y?: number) => {
    const lib = COMPONENT_LIBRARY.find(c => c.type === type);
    if (!lib) return;
    const comp: CanvasComponent = {
      id: `c-${Date.now()}`, type, label: lib.label,
      x: x ?? 20 + Math.random() * 100, y: y ?? 100 + Math.random() * 200,
      w: lib.defaultW, h: lib.defaultH, props: { ...lib.defaultProps }, locked: false,
    };
    setCanvas(prev => [...prev, comp]);
    setSelectedId(comp.id);
    setFuelUsed(f => f + 1);
  }, []);

  const updateComponent = useCallback((id: string, updates: Partial<CanvasComponent>) => {
    setCanvas(prev => prev.map(c => c.id === id ? { ...c, ...updates } : c));
  }, []);

  const deleteComponent = useCallback((id: string) => {
    setCanvas(prev => prev.filter(c => c.id !== id));
    if (selectedId === id) setSelectedId(null);
  }, [selectedId]);

  const duplicateComponent = useCallback((id: string) => {
    const comp = canvas.find(c => c.id === id);
    if (!comp) return;
    const dup: CanvasComponent = { ...comp, id: `c-${Date.now()}`, x: comp.x + 20, y: comp.y + 20, label: `${comp.label} Copy` };
    setCanvas(prev => [...prev, dup]);
    setSelectedId(dup.id);
  }, [canvas]);

  /* ─── drag on canvas ─── */
  const handleCanvasMouseDown = (e: React.MouseEvent, compId: string) => {
    if (resizing) return;
    const comp = canvas.find(c => c.id === compId);
    if (!comp || comp.locked) return;
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    setDragging({ id: compId, offsetX: e.clientX - rect.left - comp.x * (zoom / 100), offsetY: e.clientY - rect.top - comp.y * (zoom / 100) });
    setSelectedId(compId);
  };

  const handleCanvasMouseMove = (e: React.MouseEvent) => {
    if (!dragging) return;
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = Math.max(0, (e.clientX - rect.left - dragging.offsetX) / (zoom / 100));
    const y = Math.max(0, (e.clientY - rect.top - dragging.offsetY) / (zoom / 100));
    updateComponent(dragging.id, { x: Math.round(x), y: Math.round(y) });
  };

  const handleCanvasMouseUp = () => { setDragging(null); setResizing(null); };

  /* ─── drop from library ─── */
  const handleCanvasDrop = (e: React.DragEvent) => {
    e.preventDefault();
    if (!draggedLib) return;
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = (e.clientX - rect.left) / (zoom / 100);
    const y = (e.clientY - rect.top) / (zoom / 100);
    addToCanvas(draggedLib, Math.round(x), Math.round(y));
    setDraggedLib(null);
  };

  /* ─── AI generation ─── */
  const handleAiGenerate = useCallback(() => {
    if (!aiPrompt.trim()) return;
    setAiGenerating(true);
    setTimeout(() => {
      // simulate AI-generated layout
      const newComponents: CanvasComponent[] = [
        { id: `c-${Date.now()}-1`, type: "navbar", label: "AI Nav", x: 20, y: 20, w: 560, h: 56, props: { brand: "AI Generated", items: "Home,About,Contact" }, locked: false },
        { id: `c-${Date.now()}-2`, type: "heading", label: "AI Heading", x: 20, y: 96, w: 400, h: 36, props: { text: aiPrompt.slice(0, 40), level: "1" }, locked: false },
        { id: `c-${Date.now()}-3`, type: "card", label: "AI Card 1", x: 20, y: 152, w: 270, h: 150, props: { bg: "rgba(15,23,42,0.8)", border: "1px solid rgba(56,189,248,0.1)", borderRadius: "10px", padding: "16px", title: "Generated Card", content: "AI-designed component based on your description." }, locked: false },
        { id: `c-${Date.now()}-4`, type: "card", label: "AI Card 2", x: 310, y: 152, w: 270, h: 150, props: { bg: "rgba(15,23,42,0.8)", border: "1px solid rgba(56,189,248,0.1)", borderRadius: "10px", padding: "16px", title: "Details", content: "Automatically laid out by Designer Agent." }, locked: false },
        { id: `c-${Date.now()}-5`, type: "button", label: "AI Action", x: 20, y: 320, w: 160, h: 42, props: { text: "Get Started", variant: "primary" }, locked: false },
        { id: `c-${Date.now()}-6`, type: "button", label: "AI Secondary", x: 200, y: 320, w: 140, h: 42, props: { text: "Learn More", variant: "secondary" }, locked: false },
      ];
      setCanvas(newComponents);
      setSelectedId(newComponents[0].id);
      setAiGenerating(false);
      setFuelUsed(f => f + 85);
      setVersions(prev => [{ id: `v-${Date.now()}`, label: `AI: ${aiPrompt.slice(0, 30)}...`, timestamp: Date.now(), componentCount: newComponents.length, author: "Designer Agent", thumbnail: "┌─AI Generated──┐\n│ Nav           │\n│ Card │ Card   │\n│[Btn][Btn]     │\n└───────────────┘" }, ...prev]);
      setAiPrompt("");
    }, 1500);
  }, [aiPrompt]);

  const saveVersion = useCallback(() => {
    setVersions(prev => [{
      id: `v-${Date.now()}`, label: `Snapshot (${canvas.length} components)`,
      timestamp: Date.now(), componentCount: canvas.length, author: "You",
      thumbnail: `${canvas.length} components on canvas`,
    }, ...prev]);
  }, [canvas]);

  const exportedCode = useMemo(() => generateCode(canvas), [canvas]);

  const updateProp = (key: string, value: string) => {
    if (!selected) return;
    updateComponent(selected.id, { props: { ...selected.props, [key]: value } });
  };

  /* ─── render ─── */
  return (
    <div className="ds-container">
      {/* ─── Left: Component Library ─── */}
      <aside className="ds-library">
        <div className="ds-lib-header">
          <h2 className="ds-lib-title">Components</h2>
        </div>
        <input className="ds-lib-search" placeholder="Search components..." value={libSearch} onChange={e => setLibSearch(e.target.value)} />
        <div className="ds-lib-cats">
          {(["all", "layout", "input", "display", "navigation", "feedback"] as const).map(cat => (
            <button key={cat} className={`ds-lib-cat ${libFilter === cat ? "active" : ""}`} onClick={() => setLibFilter(cat)}>
              {cat === "all" ? "All" : cat.charAt(0).toUpperCase() + cat.slice(1)}
            </button>
          ))}
        </div>
        <div className="ds-lib-items">
          {filteredLib.map(comp => (
            <div key={comp.type} className="ds-lib-item" draggable onDragStart={() => setDraggedLib(comp.type)} onClick={() => addToCanvas(comp.type)}>
              <span className="ds-lib-icon">{comp.icon}</span>
              <span className="ds-lib-label">{comp.label}</span>
              {comp.governed && <span className="ds-lib-governed" title="Governed component">⛊</span>}
            </div>
          ))}
        </div>

        {/* AI prompt */}
        <div className="ds-ai-section">
          <div className="ds-ai-header">⬢ Designer Agent</div>
          <textarea className="ds-ai-input" value={aiPrompt} onChange={e => setAiPrompt(e.target.value)} placeholder="Describe your layout..." rows={3} />
          <button className="ds-ai-btn" onClick={handleAiGenerate} disabled={aiGenerating || !aiPrompt.trim()}>
            {aiGenerating ? "Generating..." : "⚡ Generate Layout"}
          </button>
          <div className="ds-ai-suggestions">
            {AI_PROMPTS.slice(0, 3).map((p, i) => (
              <button key={i} className="ds-ai-suggestion" onClick={() => setAiPrompt(p)}>{p}</button>
            ))}
          </div>
        </div>
      </aside>

      {/* ─── Center: Canvas / Preview / Code / Tokens ─── */}
      <div className="ds-center">
        {/* toolbar */}
        <div className="ds-toolbar">
          <div className="ds-toolbar-left">
            <div className="ds-view-toggle">
              {(["design", "preview", "code", "tokens"] as ViewMode[]).map(v => (
                <button key={v} className={`ds-view-btn ${viewMode === v ? "active" : ""}`} onClick={() => setViewMode(v)}>
                  {v === "design" ? "◇" : v === "preview" ? "▶" : v === "code" ? "<>" : "◈"} {v.charAt(0).toUpperCase() + v.slice(1)}
                </button>
              ))}
            </div>
            {viewMode === "design" && (
              <div className="ds-zoom-controls">
                <button className="ds-zoom-btn" onClick={() => setZoom(z => Math.max(50, z - 10))}>−</button>
                <span className="ds-zoom-label">{zoom}%</span>
                <button className="ds-zoom-btn" onClick={() => setZoom(z => Math.min(150, z + 10))}>+</button>
              </div>
            )}
          </div>
          <div className="ds-toolbar-right">
            <button className="ds-toolbar-btn" onClick={saveVersion}>💾 Save Version</button>
            <button className={`ds-toolbar-btn ${showHistory ? "active" : ""}`} onClick={() => setShowHistory(!showHistory)}>⏱ History</button>
            <span className="ds-fuel">⚡ {fuelUsed} fuel</span>
          </div>
        </div>

        {/* design canvas */}
        {viewMode === "design" && (
          <div className="ds-canvas-wrap">
            <div
              ref={canvasRef}
              className="ds-canvas"
              style={{ transform: `scale(${zoom / 100})`, transformOrigin: "top left" }}
              onMouseMove={handleCanvasMouseMove}
              onMouseUp={handleCanvasMouseUp}
              onMouseLeave={handleCanvasMouseUp}
              onClick={e => { if (e.target === e.currentTarget) setSelectedId(null); }}
              onDragOver={e => e.preventDefault()}
              onDrop={handleCanvasDrop}
            >
              {/* grid */}
              <div className="ds-canvas-grid" />

              {canvas.map(comp => (
                <div
                  key={comp.id}
                  className={`ds-canvas-comp ${selectedId === comp.id ? "selected" : ""} ${comp.locked ? "locked" : ""}`}
                  style={{ left: comp.x, top: comp.y, width: comp.w, height: comp.h }}
                  onMouseDown={e => handleCanvasMouseDown(e, comp.id)}
                  onClick={e => { e.stopPropagation(); setSelectedId(comp.id); }}
                >
                  <div className="ds-comp-label">{comp.label}</div>
                  <div className="ds-comp-preview" dangerouslySetInnerHTML={{ __html: renderComponentPreview(comp) }} />
                  {selectedId === comp.id && (
                    <>
                      <div className="ds-comp-handles">
                        <div className="ds-handle ds-handle-tl" />
                        <div className="ds-handle ds-handle-tr" />
                        <div className="ds-handle ds-handle-bl" />
                        <div className="ds-handle ds-handle-br"
                          onMouseDown={e => { e.stopPropagation(); setResizing(comp.id); }}
                        />
                      </div>
                      <div className="ds-comp-size">{comp.w}×{comp.h}</div>
                    </>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* preview */}
        {viewMode === "preview" && (
          <div className="ds-preview-wrap">
            <div className="ds-preview-frame">
              {canvas.map(comp => (
                <div key={comp.id} style={{ position: "absolute", left: comp.x, top: comp.y, width: comp.w, height: comp.h }} dangerouslySetInnerHTML={{ __html: renderComponentPreview(comp) }} />
              ))}
            </div>
          </div>
        )}

        {/* code export */}
        {viewMode === "code" && (
          <div className="ds-code-wrap">
            <div className="ds-code-header">
              <span>Generated React Component — {canvas.length} components</span>
              <button className="ds-toolbar-btn" onClick={() => setFuelUsed(f => f + 5)}>📋 Copy Code</button>
            </div>
            <pre className="ds-code-editor">{exportedCode}</pre>
          </div>
        )}

        {/* tokens */}
        {viewMode === "tokens" && (
          <div className="ds-tokens-wrap">
            <div className="ds-tokens-header">
              <h3>Design Tokens</h3>
              <span className="ds-tokens-count">{tokens.length} tokens</span>
            </div>
            {(["color", "spacing", "font", "radius", "shadow"] as const).map(cat => (
              <div key={cat} className="ds-token-group">
                <div className="ds-token-group-header">{cat.charAt(0).toUpperCase() + cat.slice(1)}</div>
                <div className="ds-token-grid">
                  {tokens.filter(t => t.category === cat).map(token => (
                    <div key={token.id} className="ds-token-card">
                      {cat === "color" && <div className="ds-token-swatch" style={{ background: token.value }} />}
                      <div className="ds-token-info">
                        <span className="ds-token-name">{token.name}</span>
                        <input className="ds-token-value" value={token.value} onChange={e => setTokens(prev => prev.map(t => t.id === token.id ? { ...t, value: e.target.value } : t))} />
                        <span className="ds-token-var">{token.cssVar}</span>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}

        {/* version history panel */}
        {showHistory && (
          <div className="ds-history-panel">
            <div className="ds-history-header">
              <span>Version History</span>
              <button className="ds-history-close" onClick={() => setShowHistory(false)}>×</button>
            </div>
            {versions.map(v => (
              <div key={v.id} className="ds-history-item">
                <div className="ds-history-item-top">
                  <span className="ds-history-label">{v.label}</span>
                  <span className="ds-history-author">{v.author}</span>
                </div>
                <pre className="ds-history-thumb">{v.thumbnail}</pre>
                <div className="ds-history-meta">
                  <span>{v.componentCount} components</span>
                  <span>{new Date(v.timestamp).toLocaleString()}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* ─── Right: Properties ─── */}
      {selected && viewMode === "design" && (
        <aside className="ds-props">
          <div className="ds-props-header">
            <span className="ds-props-type">{selected.type}</span>
            <div className="ds-props-actions">
              <button className="ds-props-btn" onClick={() => duplicateComponent(selected.id)} title="Duplicate">⧉</button>
              <button className="ds-props-btn" onClick={() => updateComponent(selected.id, { locked: !selected.locked })} title="Lock">{selected.locked ? "🔒" : "🔓"}</button>
              <button className="ds-props-btn ds-props-delete" onClick={() => deleteComponent(selected.id)} title="Delete">🗑</button>
            </div>
          </div>

          <input className="ds-props-name" value={selected.label} onChange={e => updateComponent(selected.id, { label: e.target.value })} />

          {/* position */}
          <div className="ds-props-section">
            <div className="ds-props-label">Position & Size</div>
            <div className="ds-props-grid">
              <div className="ds-props-field">
                <label>X</label>
                <input type="number" value={selected.x} onChange={e => updateComponent(selected.id, { x: Number(e.target.value) })} />
              </div>
              <div className="ds-props-field">
                <label>Y</label>
                <input type="number" value={selected.y} onChange={e => updateComponent(selected.id, { y: Number(e.target.value) })} />
              </div>
              <div className="ds-props-field">
                <label>W</label>
                <input type="number" value={selected.w} onChange={e => updateComponent(selected.id, { w: Number(e.target.value) })} />
              </div>
              <div className="ds-props-field">
                <label>H</label>
                <input type="number" value={selected.h} onChange={e => updateComponent(selected.id, { h: Number(e.target.value) })} />
              </div>
            </div>
          </div>

          {/* props */}
          <div className="ds-props-section">
            <div className="ds-props-label">Properties</div>
            {Object.entries(selected.props).map(([key, val]) => (
              <div key={key} className="ds-prop-row">
                <label className="ds-prop-key">{key}</label>
                {val.length > 50 ? (
                  <textarea className="ds-prop-val-area" value={val} onChange={e => updateProp(key, e.target.value)} rows={2} />
                ) : (
                  <input className="ds-prop-val" value={val} onChange={e => updateProp(key, e.target.value)} />
                )}
              </div>
            ))}
          </div>
        </aside>
      )}

      {/* ─── Status Bar ─── */}
      <div className="ds-status-bar">
        <span className="ds-status-item">{canvas.length} components</span>
        <span className="ds-status-item">{viewMode} mode</span>
        {viewMode === "design" && <span className="ds-status-item">Zoom: {zoom}%</span>}
        {selected && <span className="ds-status-item">Selected: {selected.label} ({selected.x},{selected.y})</span>}
        <span className="ds-status-item ds-status-right">⚡ {fuelUsed} fuel</span>
        <span className="ds-status-item">{versions.length} versions</span>
        <span className="ds-status-item">Drag from library or click to add</span>
      </div>
    </div>
  );
}

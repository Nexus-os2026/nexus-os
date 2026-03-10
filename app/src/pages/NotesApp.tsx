import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import "./notes-app.css";

/* ─── types ─── */
interface NoteTag {
  id: string;
  name: string;
  color: string;
}

interface NoteLink {
  type: "agent" | "workflow" | "audit";
  label: string;
  id: string;
}

interface Note {
  id: string;
  title: string;
  content: string;
  folderId: string;
  tags: string[];
  links: NoteLink[];
  createdAt: number;
  updatedAt: number;
  createdBy: "user" | string; // agent name
  pinned: boolean;
  wordCount: number;
  template?: string;
}

interface Folder {
  id: string;
  name: string;
  icon: string;
  parentId: string | null;
  collapsed: boolean;
}

type ExportFormat = "markdown" | "pdf" | "docx";
type ViewMode = "edit" | "preview" | "split";

/* ─── constants ─── */
const TAG_COLORS = ["#22d3ee", "#a78bfa", "#f472b6", "#34d399", "#fbbf24", "#fb923c", "#f87171", "#818cf8"];

const INITIAL_TAGS: NoteTag[] = [
  { id: "t1", name: "research", color: "#22d3ee" },
  { id: "t2", name: "project", color: "#a78bfa" },
  { id: "t3", name: "meeting", color: "#f472b6" },
  { id: "t4", name: "architecture", color: "#34d399" },
  { id: "t5", name: "bug", color: "#f87171" },
  { id: "t6", name: "idea", color: "#fbbf24" },
  { id: "t7", name: "agent-generated", color: "#818cf8" },
];

const INITIAL_FOLDERS: Folder[] = [
  { id: "f-all", name: "All Notes", icon: "📋", parentId: null, collapsed: false },
  { id: "f-projects", name: "Projects", icon: "📁", parentId: null, collapsed: false },
  { id: "f-research", name: "Research", icon: "🔬", parentId: null, collapsed: false },
  { id: "f-meetings", name: "Meetings", icon: "📅", parentId: null, collapsed: true },
  { id: "f-agent", name: "Agent Notes", icon: "⬢", parentId: null, collapsed: false },
  { id: "f-templates", name: "Templates", icon: "📄", parentId: null, collapsed: true },
  { id: "f-archive", name: "Archive", icon: "📦", parentId: null, collapsed: true },
];

const TEMPLATES: Record<string, { title: string; content: string; tags: string[] }> = {
  "meeting": {
    title: "Meeting Notes — ",
    content: `# Meeting Notes\n\n**Date:** ${new Date().toLocaleDateString()}\n**Attendees:** \n**Agenda:**\n\n---\n\n## Discussion Points\n\n1. \n\n## Action Items\n\n- [ ] \n\n## Decisions Made\n\n- \n\n## Next Steps\n\n- `,
    tags: ["t3"],
  },
  "research": {
    title: "Research: ",
    content: "# Research Summary\n\n## Objective\n\n\n## Key Findings\n\n1. \n\n## Sources\n\n- \n\n## Analysis\n\n\n## Conclusions\n\n\n## Related Links\n\n- ",
    tags: ["t1"],
  },
  "project": {
    title: "Project: ",
    content: "# Project Document\n\n## Overview\n\n\n## Goals\n\n- [ ] \n\n## Architecture\n\n```\n\n```\n\n## Tasks\n\n- [ ] \n\n## Timeline\n\n| Phase | Description | Status |\n|-------|------------|--------|\n|       |            |        |\n\n## Notes\n\n",
    tags: ["t2"],
  },
  "bug-report": {
    title: "Bug: ",
    content: "# Bug Report\n\n## Description\n\n\n## Steps to Reproduce\n\n1. \n\n## Expected Behavior\n\n\n## Actual Behavior\n\n\n## Environment\n\n- OS: \n- Version: \n\n## Screenshots / Logs\n\n```\n\n```\n\n## Fix\n\n",
    tags: ["t5"],
  },
  "blank": {
    title: "Untitled Note",
    content: "",
    tags: [],
  },
};

const INITIAL_NOTES: Note[] = [
  {
    id: "n1", title: "Nexus OS Architecture Overview", folderId: "f-projects",
    content: "# Nexus OS Architecture\n\n## Core Principles\n\n- **Governed autonomy**: Every agent action passes through kernel capability checks\n- **Fuel-based economics**: Budget checked before execution, not after\n- **Append-only audit**: Hash-chain integrity for all events\n- **Zero unsafe Rust**: `unsafe_code = forbid` across entire workspace\n\n## Architecture Layers\n\n```\n┌─────────────────────────────────┐\n│         React Frontend          │\n├─────────────────────────────────┤\n│        Tauri Bridge             │\n├─────────────────────────────────┤\n│      Nexus SDK (agents)         │\n├─────────────────────────────────┤\n│      Nexus Kernel               │\n├─────────────────────────────────┤\n│    WASM Sandbox  │  Audit Chain │\n└─────────────────────────────────┘\n```\n\n## Key Modules\n\n- **Kernel**: Supervisor, FuelLedger, AuditTrail, CapabilityManager\n- **SDK**: Agent-facing API, prelude re-exports\n- **Agents**: 9 agent crates (coder, researcher, planner, etc.)\n- **Connectors**: GitHub, Slack, email, calendar\n\n## Governance Model\n\n| Level | Name | Description |\n|-------|------|-------------|\n| L0 | Inert | No actions |\n| L1 | Suggest | Human decides |\n| L2 | Act-with-approval | Human approves |\n| L3 | Act-then-report | Post-action review |\n| L4 | Autonomous-bounded | Anomaly-triggered |\n| L5 | Full autonomy | Kernel override only |",
    tags: ["t2", "t4"], links: [{ type: "agent", label: "Planner Agent", id: "planner-001" }],
    createdAt: Date.now() - 86400000 * 14, updatedAt: Date.now() - 86400000 * 2,
    createdBy: "user", pinned: true, wordCount: 142,
  },
  {
    id: "n2", title: "Research: WASM Sandboxing Approaches", folderId: "f-research",
    content: "# WASM Sandboxing Research\n\n## Objective\n\nEvaluate WebAssembly sandbox strategies for agent execution isolation.\n\n## Key Findings\n\n1. **Wasmtime** — Bytecode Alliance runtime, strong security model, capability-based\n2. **Wasmer** — Good performance, broader language support\n3. **wasm-sandbox** crate — Lightweight, minimal overhead\n4. **Browser WASM** — V8 isolation, not suitable for server-side\n\n## Comparison\n\n| Runtime | Security | Performance | Ecosystem |\n|---------|----------|-------------|----------|\n| Wasmtime | ★★★★★ | ★★★★ | ★★★★ |\n| Wasmer | ★★★★ | ★★★★★ | ★★★★★ |\n| wasm-sandbox | ★★★ | ★★★★★ | ★★ |\n\n## Recommendation\n\nWasmtime for production — capability-based security model aligns with Nexus governance.\n\n## Sources\n\n- Bytecode Alliance spec docs\n- WASI preview2 proposal\n- Nexus kernel capability model",
    tags: ["t1", "t4", "t7"], links: [{ type: "agent", label: "Research Agent", id: "research-001" }, { type: "workflow", label: "Sandbox Evaluation", id: "wf-sandbox" }],
    createdAt: Date.now() - 86400000 * 10, updatedAt: Date.now() - 86400000 * 3,
    createdBy: "Research Agent", pinned: false, wordCount: 118,
  },
  {
    id: "n3", title: "Sprint 14 Planning Meeting", folderId: "f-meetings",
    content: "# Sprint 14 Planning\n\n**Date:** 2026-03-03\n**Attendees:** Suresh, Claude AI, Research Agent, Coder Agent\n**Agenda:** Phase 7 app builds\n\n---\n\n## Discussion Points\n\n1. Code Editor (7.1) complete — Monaco integration working\n2. Terminal (7.3) complete — 30+ commands, HITL approval for dangerous ops\n3. File Manager (7.4) complete — grid/list, vault, trash\n4. System Monitor (7.11) shipped — recharts real-time graphs\n5. Next priorities: Notes App (7.7), then remaining apps\n\n## Action Items\n\n- [x] Ship Code Editor with agent-assisted coding\n- [x] Build Terminal with governed command execution\n- [x] Build File Manager with drag-drop and vault\n- [x] Build System Monitor with recharts\n- [ ] Build Notes App with markdown + templates\n- [ ] Build Design Studio with canvas\n\n## Decisions Made\n\n- Skip 7.2 Design Studio for now — Notes App higher priority\n- Use recharts for all graph needs across apps\n- CSS prefix convention: two-letter per page (ce-, tm-, fm-, sm-, na-)\n\n## Next Steps\n\n- Build remaining Phase 7 apps\n- Integrate Tauri filesystem for real I/O",
    tags: ["t3", "t2"], links: [{ type: "workflow", label: "Phase 7 Build", id: "wf-phase7" }],
    createdAt: Date.now() - 86400000 * 7, updatedAt: Date.now() - 86400000 * 7,
    createdBy: "user", pinned: false, wordCount: 164,
  },
  {
    id: "n4", title: "Agent Fuel Optimization Ideas", folderId: "f-agent",
    content: "# Fuel Optimization Findings\n\n*Auto-generated by Self-Improve Agent*\n\n## Current Fuel Usage Patterns\n\n- Coder Agent: ~2400 fuel/session (high — code generation expensive)\n- Research Agent: ~1800 fuel/session (moderate — web lookups cached)\n- Planner Agent: ~600 fuel/session (low — mostly text planning)\n\n## Optimization Opportunities\n\n1. **Response caching**: Cache identical prompts → 40% fuel reduction for Research Agent\n2. **Incremental generation**: Stream partial results, stop early on confidence → 25% savings\n3. **Model routing**: Use smaller models for simple tasks (classification, formatting)\n4. **Batch operations**: Group similar agent requests into single inference calls\n\n## Estimated Savings\n\n- Caching alone: 1200 fuel/day saved\n- Full optimization suite: 3500 fuel/day saved (58% reduction)\n\n## Implementation Priority\n\n1. Response caching (easy, high impact)\n2. Model routing (medium, high impact)\n3. Incremental generation (hard, medium impact)\n4. Batch operations (hard, medium impact)",
    tags: ["t6", "t7"], links: [{ type: "agent", label: "Self-Improve Agent", id: "self-improve-001" }, { type: "audit", label: "Fuel Audit #847", id: "audit-847" }],
    createdAt: Date.now() - 86400000 * 5, updatedAt: Date.now() - 86400000 * 1,
    createdBy: "Self-Improve Agent", pinned: true, wordCount: 148,
  },
  {
    id: "n5", title: "PII Redaction Pipeline Design", folderId: "f-research",
    content: "# PII Redaction at LLM Gateway\n\n## Problem\n\nAgent-generated prompts may contain user PII. Must redact before sending to external LLMs.\n\n## Detection Patterns\n\n- Email: regex `/[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}/`\n- Phone: regex `/\\+?[1-9]\\d{1,14}/`\n- SSN: regex `/\\d{3}-\\d{2}-\\d{4}/`\n- Credit card: Luhn algorithm validation\n- Names: NER via local SLM (no external API for PII detection)\n\n## Redaction Strategy\n\n1. **Pre-flight scan**: Scan all outbound prompts\n2. **Token replacement**: Replace PII with `[REDACTED_EMAIL_1]` tokens\n3. **Reverse mapping**: Maintain session-scoped map for re-hydration\n4. **Audit logging**: Log redaction events (not the PII itself)\n\n## Architecture\n\n```\nAgent → Gateway → PII Scanner → Redactor → LLM API\n                                    ↓\n                              Audit Trail\n```",
    tags: ["t1", "t4"], links: [{ type: "agent", label: "Research Agent", id: "research-001" }],
    createdAt: Date.now() - 86400000 * 8, updatedAt: Date.now() - 86400000 * 4,
    createdBy: "Research Agent", pinned: false, wordCount: 130,
  },
  {
    id: "n6", title: "Quick idea: Voice-controlled agent dispatch", folderId: "f-projects",
    content: "# Voice-Controlled Agent Dispatch\n\nWhat if users could say:\n\n> \"Hey Nexus, research the latest Rust async patterns and create a summary note\"\n\nAnd the system would:\n1. Parse intent via Jarvis voice mode\n2. Route to Research Agent\n3. Agent creates a note in this Notes App automatically\n4. Notify user when done\n\nThis would close the loop between voice → agent → knowledge management.\n\nNeeds: Jarvis mode integration, agent-to-notes API, notification system.",
    tags: ["t6"], links: [],
    createdAt: Date.now() - 86400000 * 3, updatedAt: Date.now() - 86400000 * 3,
    createdBy: "user", pinned: false, wordCount: 72,
  },
];

/* ─── markdown renderer (simple) ─── */
function renderMarkdown(md: string): string {
  let html = md
    // code blocks
    .replace(/```(\w*)\n([\s\S]*?)```/g, '<pre class="na-code-block"><code>$2</code></pre>')
    // inline code
    .replace(/`([^`]+)`/g, '<code class="na-inline-code">$1</code>')
    // headings
    .replace(/^#### (.+)$/gm, '<h4>$1</h4>')
    .replace(/^### (.+)$/gm, '<h3>$1</h3>')
    .replace(/^## (.+)$/gm, '<h2>$1</h2>')
    .replace(/^# (.+)$/gm, '<h1>$1</h1>')
    // bold and italic
    .replace(/\*\*\*(.+?)\*\*\*/g, '<strong><em>$1</em></strong>')
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(.+?)\*/g, '<em>$1</em>')
    // strikethrough
    .replace(/~~(.+?)~~/g, '<del>$1</del>')
    // blockquote
    .replace(/^> (.+)$/gm, '<blockquote>$1</blockquote>')
    // horizontal rule
    .replace(/^---$/gm, '<hr />')
    // checkboxes
    .replace(/^- \[x\] (.+)$/gm, '<div class="na-checkbox checked">☑ $1</div>')
    .replace(/^- \[ \] (.+)$/gm, '<div class="na-checkbox">☐ $1</div>')
    // unordered list
    .replace(/^- (.+)$/gm, '<li>$1</li>')
    // ordered list
    .replace(/^\d+\. (.+)$/gm, '<li>$1</li>')
    // tables (simple)
    .replace(/^\|(.+)\|$/gm, (match) => {
      const cells = match.split("|").filter(c => c.trim());
      if (cells.every(c => /^[\s-:]+$/.test(c))) return '';
      const tag = "td";
      return `<tr>${cells.map(c => `<${tag}>${c.trim()}</${tag}>`).join("")}</tr>`;
    })
    // images
    .replace(/!\[([^\]]*)\]\(([^)]+)\)/g, '<img alt="$1" src="$2" class="na-img" />')
    // links
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" class="na-link">$1</a>')
    // line breaks
    .replace(/\n\n/g, '</p><p>')
    .replace(/\n/g, '<br />');

  // wrap loose <li> in <ul>
  html = html.replace(/((?:<li>.*?<\/li>\s*)+)/g, '<ul>$1</ul>');
  // wrap <tr> in <table>
  html = html.replace(/((?:<tr>.*?<\/tr>\s*)+)/g, '<table class="na-table">$1</table>');

  return `<p>${html}</p>`;
}

/* ─── component ─── */
export default function NotesApp() {
  const [notes, setNotes] = useState<Note[]>(INITIAL_NOTES);
  const [folders, setFolders] = useState<Folder[]>(INITIAL_FOLDERS);
  const [tags] = useState<NoteTag[]>(INITIAL_TAGS);
  const [selectedNoteId, setSelectedNoteId] = useState<string>("n1");
  const [selectedFolderId, setSelectedFolderId] = useState<string>("f-all");
  const [selectedTagFilter, setSelectedTagFilter] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [viewMode, setViewMode] = useState<ViewMode>("split");
  const [showSidebar, setShowSidebar] = useState(true);
  const [showTemplateMenu, setShowTemplateMenu] = useState(false);
  const [showExportMenu, setShowExportMenu] = useState(false);
  const [showLinkPanel, setShowLinkPanel] = useState(false);
  const [showTagPicker, setShowTagPicker] = useState(false);
  const [sortBy, setSortBy] = useState<"updated" | "created" | "title" | "pinned">("updated");
  const [fuelUsed, setFuelUsed] = useState(47);
  const [auditLog, setAuditLog] = useState<string[]>([
    "Note opened: Nexus OS Architecture Overview",
    "Agent note created: Fuel Optimization Findings",
    "Research Agent updated: WASM Sandboxing Approaches",
  ]);

  const editorRef = useRef<HTMLTextAreaElement>(null);

  const selectedNote = useMemo(() => notes.find(n => n.id === selectedNoteId) ?? null, [notes, selectedNoteId]);

  /* ─── filtered + sorted notes ─── */
  const filteredNotes = useMemo(() => {
    let list = notes;
    if (selectedFolderId !== "f-all") {
      list = list.filter(n => n.folderId === selectedFolderId);
    }
    if (selectedTagFilter) {
      list = list.filter(n => n.tags.includes(selectedTagFilter));
    }
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter(n =>
        n.title.toLowerCase().includes(q) ||
        n.content.toLowerCase().includes(q) ||
        n.tags.some(t => {
          const tag = tags.find(tg => tg.id === t);
          return tag?.name.toLowerCase().includes(q);
        })
      );
    }
    // sort
    list = [...list].sort((a, b) => {
      if (sortBy === "pinned") return (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0) || b.updatedAt - a.updatedAt;
      if (sortBy === "title") return a.title.localeCompare(b.title);
      if (sortBy === "created") return b.createdAt - a.createdAt;
      return b.updatedAt - a.updatedAt;
    });
    return list;
  }, [notes, selectedFolderId, selectedTagFilter, searchQuery, sortBy, tags]);

  /* ─── agent activity simulation ─── */
  useEffect(() => {
    const interval = setInterval(() => {
      const agentMessages = [
        "Research Agent scanning sources...",
        "Coder Agent documenting function...",
        "Self-Improve Agent analyzing patterns...",
        "Planner Agent updating project notes...",
        "Research Agent found 3 new results...",
      ];
      setAuditLog(prev => [agentMessages[Math.floor(Math.random() * agentMessages.length)], ...prev].slice(0, 20));
      setFuelUsed(prev => prev + Math.floor(Math.random() * 3));
    }, 8000);
    return () => clearInterval(interval);
  }, []);

  /* ─── handlers ─── */
  const updateNote = useCallback((id: string, updates: Partial<Note>) => {
    setNotes(prev => prev.map(n => n.id === id ? { ...n, ...updates, updatedAt: Date.now(), wordCount: (updates.content ?? n.content).split(/\s+/).filter(Boolean).length } : n));
  }, []);

  const createNote = useCallback((templateKey?: string) => {
    const tmpl = templateKey ? TEMPLATES[templateKey] : TEMPLATES["blank"];
    const newNote: Note = {
      id: `n-${Date.now()}`,
      title: tmpl.title,
      content: tmpl.content,
      folderId: selectedFolderId === "f-all" ? "f-projects" : selectedFolderId,
      tags: tmpl.tags,
      links: [],
      createdAt: Date.now(),
      updatedAt: Date.now(),
      createdBy: "user",
      pinned: false,
      wordCount: tmpl.content.split(/\s+/).filter(Boolean).length,
      template: templateKey,
    };
    setNotes(prev => [newNote, ...prev]);
    setSelectedNoteId(newNote.id);
    setShowTemplateMenu(false);
    logAudit(`Note created: ${newNote.title}`);
    setFuelUsed(f => f + 2);
  }, [selectedFolderId]);

  const deleteNote = useCallback((id: string) => {
    const note = notes.find(n => n.id === id);
    if (!note) return;
    setNotes(prev => prev.filter(n => n.id !== id));
    if (selectedNoteId === id) {
      setSelectedNoteId(notes.find(n => n.id !== id)?.id ?? "");
    }
    logAudit(`Note deleted: ${note.title}`);
  }, [notes, selectedNoteId]);

  const duplicateNote = useCallback((id: string) => {
    const note = notes.find(n => n.id === id);
    if (!note) return;
    const dup: Note = { ...note, id: `n-${Date.now()}`, title: `${note.title} (copy)`, createdAt: Date.now(), updatedAt: Date.now() };
    setNotes(prev => [dup, ...prev]);
    setSelectedNoteId(dup.id);
    logAudit(`Note duplicated: ${note.title}`);
  }, [notes]);

  const togglePin = useCallback((id: string) => {
    setNotes(prev => prev.map(n => n.id === id ? { ...n, pinned: !n.pinned } : n));
  }, []);

  const toggleFolder = useCallback((id: string) => {
    setFolders(prev => prev.map(f => f.id === id ? { ...f, collapsed: !f.collapsed } : f));
  }, []);

  const toggleNoteTag = useCallback((noteId: string, tagId: string) => {
    setNotes(prev => prev.map(n => {
      if (n.id !== noteId) return n;
      const has = n.tags.includes(tagId);
      return { ...n, tags: has ? n.tags.filter(t => t !== tagId) : [...n.tags, tagId], updatedAt: Date.now() };
    }));
  }, []);

  const logAudit = (msg: string) => {
    setAuditLog(prev => [msg, ...prev].slice(0, 20));
  };

  const handleExport = useCallback((format: ExportFormat) => {
    if (!selectedNote) return;
    logAudit(`Exported "${selectedNote.title}" as ${format.toUpperCase()}`);
    setFuelUsed(f => f + 5);
    setShowExportMenu(false);
  }, [selectedNote]);

  const moveNote = useCallback((noteId: string, folderId: string) => {
    const folder = folders.find(f => f.id === folderId);
    setNotes(prev => prev.map(n => n.id === noteId ? { ...n, folderId, updatedAt: Date.now() } : n));
    logAudit(`Moved note to ${folder?.name ?? folderId}`);
  }, [folders]);

  const getNoteCount = useCallback((folderId: string) => {
    if (folderId === "f-all") return notes.length;
    return notes.filter(n => n.folderId === folderId).length;
  }, [notes]);

  const formatDate = (ts: number) => {
    const d = new Date(ts);
    const now = Date.now();
    const diff = now - ts;
    if (diff < 60000) return "just now";
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
    if (diff < 604800000) return `${Math.floor(diff / 86400000)}d ago`;
    return d.toLocaleDateString();
  };

  const getTagById = (id: string) => tags.find(t => t.id === id);

  /* ─── keyboard shortcuts ─── */
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === "n") { e.preventDefault(); createNote(); }
      if (e.ctrlKey && e.key === "b") { e.preventDefault(); setShowSidebar(s => !s); }
      if (e.ctrlKey && e.key === "f") { e.preventDefault(); document.getElementById("na-search")?.focus(); }
      if (e.ctrlKey && e.key === "e") { e.preventDefault(); setViewMode(v => v === "edit" ? "preview" : v === "preview" ? "split" : "edit"); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [createNote]);

  /* ─── render ─── */
  return (
    <div className="na-container">
      {/* ─── Sidebar ─── */}
      {showSidebar && (
        <aside className="na-sidebar">
          <div className="na-sidebar-header">
            <h2 className="na-sidebar-title">Notes</h2>
            <div className="na-sidebar-actions">
              <button className="na-btn-icon" onClick={() => setShowTemplateMenu(!showTemplateMenu)} title="New note">+</button>
              <button className="na-btn-icon" onClick={() => setShowSidebar(false)} title="Hide sidebar">◀</button>
            </div>
          </div>

          {/* template menu */}
          {showTemplateMenu && (
            <div className="na-template-menu">
              <div className="na-template-header">New from template</div>
              {Object.entries(TEMPLATES).map(([key, tmpl]) => (
                <button key={key} className="na-template-item" onClick={() => createNote(key)}>
                  <span className="na-template-icon">{key === "meeting" ? "📅" : key === "research" ? "🔬" : key === "project" ? "📁" : key === "bug-report" ? "🐛" : "📝"}</span>
                  <span>{key.replace("-", " ").replace(/\b\w/g, c => c.toUpperCase())}</span>
                </button>
              ))}
            </div>
          )}

          {/* search */}
          <div className="na-search-box">
            <span className="na-search-icon">⌕</span>
            <input id="na-search" className="na-search-input" placeholder="Search notes..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)} />
            {searchQuery && <button className="na-search-clear" onClick={() => setSearchQuery("")}>×</button>}
          </div>

          {/* folders */}
          <div className="na-folders">
            {folders.map(folder => (
              <div key={folder.id} className={`na-folder-item ${selectedFolderId === folder.id ? "active" : ""}`}>
                <button className="na-folder-btn" onClick={() => { setSelectedFolderId(folder.id); setSelectedTagFilter(null); }}>
                  <span className="na-folder-icon">{folder.icon}</span>
                  <span className="na-folder-name">{folder.name}</span>
                  <span className="na-folder-count">{getNoteCount(folder.id)}</span>
                </button>
                {folder.parentId === null && folder.id !== "f-all" && (
                  <button className="na-folder-toggle" onClick={() => toggleFolder(folder.id)}>
                    {folder.collapsed ? "▸" : "▾"}
                  </button>
                )}
              </div>
            ))}
          </div>

          {/* tags */}
          <div className="na-tags-section">
            <div className="na-tags-header">Tags</div>
            <div className="na-tags-list">
              {tags.map(tag => (
                <button key={tag.id} className={`na-tag-filter ${selectedTagFilter === tag.id ? "active" : ""}`} onClick={() => setSelectedTagFilter(selectedTagFilter === tag.id ? null : tag.id)} style={{ borderColor: tag.color }}>
                  <span className="na-tag-dot" style={{ background: tag.color }} />
                  {tag.name}
                </button>
              ))}
            </div>
          </div>

          {/* agent activity */}
          <div className="na-agent-activity">
            <div className="na-agent-header">Agent Activity</div>
            <div className="na-agent-log">
              {auditLog.slice(0, 5).map((msg, i) => (
                <div key={i} className="na-agent-entry">{msg}</div>
              ))}
            </div>
          </div>
        </aside>
      )}

      {/* ─── Note List ─── */}
      <div className="na-note-list">
        <div className="na-list-header">
          <div className="na-list-title">
            {!showSidebar && <button className="na-btn-icon" onClick={() => setShowSidebar(true)} title="Show sidebar">▶</button>}
            <span>{folders.find(f => f.id === selectedFolderId)?.name ?? "All Notes"}</span>
            {selectedTagFilter && <span className="na-filter-badge" style={{ borderColor: getTagById(selectedTagFilter)?.color }}>#{getTagById(selectedTagFilter)?.name}</span>}
          </div>
          <div className="na-list-controls">
            <select className="na-sort-select" value={sortBy} onChange={e => setSortBy(e.target.value as typeof sortBy)}>
              <option value="updated">Last Modified</option>
              <option value="created">Date Created</option>
              <option value="title">Title</option>
              <option value="pinned">Pinned First</option>
            </select>
          </div>
        </div>
        <div className="na-list-items">
          {filteredNotes.length === 0 && (
            <div className="na-empty">
              <div className="na-empty-icon">📝</div>
              <div>No notes found</div>
              <button className="na-btn-create" onClick={() => createNote()}>Create Note</button>
            </div>
          )}
          {filteredNotes.map(note => (
            <div key={note.id} className={`na-note-card ${selectedNoteId === note.id ? "active" : ""} ${note.pinned ? "pinned" : ""}`} onClick={() => setSelectedNoteId(note.id)}>
              <div className="na-note-card-header">
                <span className="na-note-card-title">{note.pinned && <span className="na-pin">📌</span>}{note.title}</span>
                <div className="na-note-card-actions">
                  <button className="na-btn-tiny" onClick={e => { e.stopPropagation(); togglePin(note.id); }}>{note.pinned ? "⊘" : "📌"}</button>
                  <button className="na-btn-tiny" onClick={e => { e.stopPropagation(); deleteNote(note.id); }}>×</button>
                </div>
              </div>
              <div className="na-note-card-preview">{note.content.replace(/[#*`>\[\]|_~-]/g, "").slice(0, 100)}...</div>
              <div className="na-note-card-meta">
                <span className="na-note-card-date">{formatDate(note.updatedAt)}</span>
                <span className="na-note-card-author">{note.createdBy === "user" ? "You" : note.createdBy}</span>
                <div className="na-note-card-tags">
                  {note.tags.slice(0, 3).map(tagId => {
                    const tag = getTagById(tagId);
                    return tag ? <span key={tagId} className="na-tag-mini" style={{ background: tag.color + "22", color: tag.color }}>{tag.name}</span> : null;
                  })}
                </div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* ─── Editor Area ─── */}
      <div className="na-editor-area">
        {selectedNote ? (
          <>
            {/* toolbar */}
            <div className="na-toolbar">
              <div className="na-toolbar-left">
                <input className="na-title-input" value={selectedNote.title} onChange={e => updateNote(selectedNote.id, { title: e.target.value })} placeholder="Note title..." />
              </div>
              <div className="na-toolbar-right">
                <div className="na-view-toggle">
                  <button className={`na-view-btn ${viewMode === "edit" ? "active" : ""}`} onClick={() => setViewMode("edit")}>Edit</button>
                  <button className={`na-view-btn ${viewMode === "split" ? "active" : ""}`} onClick={() => setViewMode("split")}>Split</button>
                  <button className={`na-view-btn ${viewMode === "preview" ? "active" : ""}`} onClick={() => setViewMode("preview")}>Preview</button>
                </div>
                <button className={`na-btn-icon ${showTagPicker ? "active" : ""}`} onClick={() => setShowTagPicker(!showTagPicker)} title="Tags">🏷</button>
                <button className={`na-btn-icon ${showLinkPanel ? "active" : ""}`} onClick={() => setShowLinkPanel(!showLinkPanel)} title="Links">🔗</button>
                <div className="na-export-wrapper">
                  <button className="na-btn-icon" onClick={() => setShowExportMenu(!showExportMenu)} title="Export">⤓</button>
                  {showExportMenu && (
                    <div className="na-export-menu">
                      <button className="na-export-item" onClick={() => handleExport("markdown")}>📄 Markdown (.md)</button>
                      <button className="na-export-item" onClick={() => handleExport("pdf")}>📕 PDF (.pdf)</button>
                      <button className="na-export-item" onClick={() => handleExport("docx")}>📘 Word (.docx)</button>
                    </div>
                  )}
                </div>
                <div className="na-move-wrapper">
                  <select className="na-move-select" value={selectedNote.folderId} onChange={e => moveNote(selectedNote.id, e.target.value)}>
                    {folders.filter(f => f.id !== "f-all").map(f => (
                      <option key={f.id} value={f.id}>{f.icon} {f.name}</option>
                    ))}
                  </select>
                </div>
                <button className="na-btn-icon" onClick={() => duplicateNote(selectedNote.id)} title="Duplicate">⧉</button>
              </div>
            </div>

            {/* tag picker */}
            {showTagPicker && (
              <div className="na-tag-picker">
                {tags.map(tag => (
                  <button key={tag.id} className={`na-tag-pick ${selectedNote.tags.includes(tag.id) ? "selected" : ""}`} style={{ borderColor: tag.color, background: selectedNote.tags.includes(tag.id) ? tag.color + "22" : "transparent" }} onClick={() => toggleNoteTag(selectedNote.id, tag.id)}>
                    <span className="na-tag-dot" style={{ background: tag.color }} />
                    {tag.name}
                  </button>
                ))}
              </div>
            )}

            {/* link panel */}
            {showLinkPanel && (
              <div className="na-link-panel">
                <div className="na-link-header">Linked Resources</div>
                {selectedNote.links.length === 0 ? (
                  <div className="na-link-empty">No linked resources</div>
                ) : (
                  selectedNote.links.map((link, i) => (
                    <div key={i} className="na-link-item">
                      <span className={`na-link-type na-link-${link.type}`}>
                        {link.type === "agent" ? "⬢" : link.type === "workflow" ? "⎇" : "⧉"}
                      </span>
                      <span className="na-link-label">{link.label}</span>
                      <span className="na-link-id">{link.id}</span>
                    </div>
                  ))
                )}
              </div>
            )}

            {/* editor + preview */}
            <div className={`na-editor-body ${viewMode}`}>
              {(viewMode === "edit" || viewMode === "split") && (
                <div className="na-edit-pane">
                  <textarea
                    ref={editorRef}
                    className="na-textarea"
                    value={selectedNote.content}
                    onChange={e => updateNote(selectedNote.id, { content: e.target.value })}
                    placeholder="Start writing... (Markdown supported)"
                    spellCheck={false}
                  />
                </div>
              )}
              {(viewMode === "preview" || viewMode === "split") && (
                <div className="na-preview-pane">
                  <div className="na-preview-content" dangerouslySetInnerHTML={{ __html: renderMarkdown(selectedNote.content) }} />
                </div>
              )}
            </div>

            {/* status bar */}
            <div className="na-status-bar">
              <span className="na-status-item">{selectedNote.wordCount} words</span>
              <span className="na-status-item">Created {formatDate(selectedNote.createdAt)}</span>
              <span className="na-status-item">Modified {formatDate(selectedNote.updatedAt)}</span>
              <span className="na-status-item">By {selectedNote.createdBy === "user" ? "You" : selectedNote.createdBy}</span>
              {selectedNote.template && <span className="na-status-item">Template: {selectedNote.template}</span>}
              <span className="na-status-item na-status-right">
                <span className="na-fuel-icon">⚡</span> {fuelUsed} fuel used
              </span>
              <span className="na-status-item">{notes.length} notes</span>
              <span className="na-status-item">Ctrl+N new · Ctrl+B sidebar · Ctrl+E view · Ctrl+F search</span>
            </div>
          </>
        ) : (
          <div className="na-no-note">
            <div className="na-no-note-icon">📝</div>
            <div className="na-no-note-text">Select a note or create a new one</div>
            <button className="na-btn-create" onClick={() => createNote()}>New Note</button>
          </div>
        )}
      </div>
    </div>
  );
}

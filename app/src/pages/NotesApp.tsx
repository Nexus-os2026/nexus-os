import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { ClipboardList, FolderOpen, Microscope, Calendar, Hexagon, FileText, Package, StickyNote, Bug, Search, ChevronLeft, ChevronRight, ChevronDown, Pin, PinOff, Tag, Download, Copy, Zap, Play } from "lucide-react";
import { notesGet, notesList, notesSave, notesDelete } from "../api/backend";
import "./notes-app.css";

/* ─── Backend calls go through backend.ts ─── */

/* ─── types ─── */
interface NoteTag {
  id: string;
  name: string;
  color: string;
}

interface Note {
  id: string;
  title: string;
  content: string;
  folderId: string;
  tags: string[];
  createdAt: number;
  updatedAt: number;
  createdBy: "user" | string;
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

type ViewMode = "edit" | "preview" | "split";

/* ─── constants ─── */
const INITIAL_TAGS: NoteTag[] = [
  { id: "t1", name: "research", color: "var(--nexus-accent)" },
  { id: "t2", name: "project", color: "#a78bfa" },
  { id: "t3", name: "meeting", color: "#f472b6" },
  { id: "t4", name: "architecture", color: "#34d399" },
  { id: "t5", name: "bug", color: "#f87171" },
  { id: "t6", name: "idea", color: "#fbbf24" },
  { id: "t7", name: "agent-generated", color: "#818cf8" },
];

const INITIAL_FOLDERS: Folder[] = [
  { id: "f-all", name: "All Notes", icon: "clipboard-list", parentId: null, collapsed: false },
  { id: "f-projects", name: "Projects", icon: "folder-open", parentId: null, collapsed: false },
  { id: "f-research", name: "Research", icon: "microscope", parentId: null, collapsed: false },
  { id: "f-meetings", name: "Meetings", icon: "calendar", parentId: null, collapsed: true },
  { id: "f-agent", name: "Agent Notes", icon: "hexagon", parentId: null, collapsed: false },
  { id: "f-templates", name: "Templates", icon: "file-text", parentId: null, collapsed: true },
  { id: "f-archive", name: "Archive", icon: "package", parentId: null, collapsed: true },
];

const FOLDER_ICON_MAP: Record<string, React.ReactNode> = {
  "clipboard-list": <ClipboardList size={14} aria-hidden="true" />,
  "folder-open": <FolderOpen size={14} aria-hidden="true" />,
  "microscope": <Microscope size={14} aria-hidden="true" />,
  "calendar": <Calendar size={14} aria-hidden="true" />,
  "hexagon": <Hexagon size={14} aria-hidden="true" />,
  "file-text": <FileText size={14} aria-hidden="true" />,
  "package": <Package size={14} aria-hidden="true" />,
};

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

/* ─── markdown renderer (simple) ─── */
function renderMarkdown(md: string): string {
  let html = md
    .replace(/```(\w*)\n([\s\S]*?)```/g, '<pre class="na-code-block"><code>$2</code></pre>')
    .replace(/`([^`]+)`/g, '<code class="na-inline-code">$1</code>')
    .replace(/^#### (.+)$/gm, '<h4>$1</h4>')
    .replace(/^### (.+)$/gm, '<h3>$1</h3>')
    .replace(/^## (.+)$/gm, '<h2>$1</h2>')
    .replace(/^# (.+)$/gm, '<h1>$1</h1>')
    .replace(/\*\*\*(.+?)\*\*\*/g, '<strong><em>$1</em></strong>')
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(.+?)\*/g, '<em>$1</em>')
    .replace(/~~(.+?)~~/g, '<del>$1</del>')
    .replace(/^> (.+)$/gm, '<blockquote>$1</blockquote>')
    .replace(/^---$/gm, '<hr />')
    .replace(/^- \[x\] (.+)$/gm, '<div class="na-checkbox checked">☑ $1</div>')
    .replace(/^- \[ \] (.+)$/gm, '<div class="na-checkbox">☐ $1</div>')
    .replace(/^- (.+)$/gm, '<li>$1</li>')
    .replace(/^\d+\. (.+)$/gm, '<li>$1</li>')
    .replace(/^\|(.+)\|$/gm, (match) => {
      const cells = match.split("|").filter(c => c.trim());
      if (cells.every(c => /^[\s-:]+$/.test(c))) return '';
      const tag = "td";
      return `<tr>${cells.map(c => `<${tag}>${c.trim()}</${tag}>`).join("")}</tr>`;
    })
    .replace(/!\[([^\]]*)\]\(([^)]+)\)/g, '<img alt="$1" src="$2" class="na-img" />')
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" class="na-link">$1</a>')
    .replace(/\n\n/g, '</p><p>')
    .replace(/\n/g, '<br />');

  html = html.replace(/((?:<li>.*?<\/li>\s*)+)/g, '<ul>$1</ul>');
  html = html.replace(/((?:<tr>.*?<\/tr>\s*)+)/g, '<table class="na-table">$1</table>');

  return `<p>${html}</p>`;
}

/* ─── component ─── */
export default function NotesApp() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [folders, setFolders] = useState<Folder[]>(INITIAL_FOLDERS);
  const [tags] = useState<NoteTag[]>(INITIAL_TAGS);
  const [selectedNoteId, setSelectedNoteId] = useState<string>("");
  const [selectedFolderId, setSelectedFolderId] = useState<string>("f-all");
  const [selectedTagFilter, setSelectedTagFilter] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [viewMode, setViewMode] = useState<ViewMode>("split");
  const [showSidebar, setShowSidebar] = useState(true);
  const [showTemplateMenu, setShowTemplateMenu] = useState(false);
  const [showExportMenu, setShowExportMenu] = useState(false);
  const [showTagPicker, setShowTagPicker] = useState(false);
  const [sortBy, setSortBy] = useState<"updated" | "created" | "title" | "pinned">("updated");
  const [fuelUsed, setFuelUsed] = useState(0);
  const [saving, setSaving] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [lastError, setLastError] = useState<string | null>(null);

  const editorRef = useRef<HTMLTextAreaElement>(null);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const selectedNote = useMemo(() => notes.find(n => n.id === selectedNoteId) ?? null, [notes, selectedNoteId]);

  useEffect(() => {
    return () => {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    };
  }, []);

  /* ─── load notes from disk on mount ─── */
  useEffect(() => {
    (async () => {
      try {
        const raw: string = await notesList();
        const loaded: Note[] = JSON.parse(raw).map((n: Record<string, unknown>) => ({
          id: n.id as string,
          title: n.title as string,
          content: n.content as string,
          folderId: (n.folderId as string) || "f-projects",
          tags: (n.tags as string[]) || [],
          createdAt: n.createdAt as number,
          updatedAt: n.updatedAt as number,
          createdBy: "user" as const,
          pinned: false,
          wordCount: (n.wordCount as number) || 0,
        }));
        setNotes(loaded);
        if (loaded.length > 0) setSelectedNoteId(loaded[0].id);
      } catch (err) {
        setLastError(String(err));
      }
      setLoaded(true);
    })();
  }, []);

  /* ─── auto-save with debounce ─── */
  const persistNote = useCallback(async (note: Note) => {
    setSaving(true);
    try {
      await notesSave(note.id, note.title, note.content, note.folderId, JSON.stringify(note.tags));
    } catch (err) {
      setLastError(String(err));
    }
    setSaving(false);
  }, []);

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
    list = [...list].sort((a, b) => {
      if (sortBy === "pinned") return (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0) || b.updatedAt - a.updatedAt;
      if (sortBy === "title") return a.title.localeCompare(b.title);
      if (sortBy === "created") return b.createdAt - a.createdAt;
      return b.updatedAt - a.updatedAt;
    });
    return list;
  }, [notes, selectedFolderId, selectedTagFilter, searchQuery, sortBy, tags]);

  /* ─── fetch single note from backend on selection ─── */
  const fetchNoteContent = useCallback(async (id: string) => {
    try {
      const raw = await notesGet(id);
      const data: Record<string, unknown> = JSON.parse(raw);
      if (data && data.id) {
        setNotes(prev => prev.map(n => {
          if (n.id !== id) return n;
          return {
            ...n,
            title: (data.title as string) || n.title,
            content: (data.content as string) ?? n.content,
            updatedAt: (data.updatedAt as number) || n.updatedAt,
          };
        }));
      }
    } catch {
      // Backend may not have the note yet, use local state
    }
  }, []);

  // Fetch latest content when selecting a note
  useEffect(() => {
    if (selectedNoteId) {
      void fetchNoteContent(selectedNoteId);
    }
  }, [selectedNoteId, fetchNoteContent]);

  /* ─── handlers ─── */
  const updateNote = useCallback((id: string, updates: Partial<Note>) => {
    setNotes(prev => {
      const updated = prev.map(n => {
        if (n.id !== id) return n;
        const merged = { ...n, ...updates, updatedAt: Date.now(), wordCount: (updates.content ?? n.content).split(/\s+/).filter(Boolean).length };
        // Debounced save
        if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
        saveTimerRef.current = setTimeout(() => persistNote(merged), 500);
        return merged;
      });
      return updated;
    });
  }, [persistNote]);

  const createNote = useCallback(async (templateKey?: string) => {
    const tmpl = templateKey ? TEMPLATES[templateKey] : TEMPLATES["blank"];
    const newNote: Note = {
      id: `n-${Date.now()}`,
      title: tmpl.title,
      content: tmpl.content,
      folderId: selectedFolderId === "f-all" ? "f-projects" : selectedFolderId,
      tags: tmpl.tags,
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
    setFuelUsed(f => f + 2);

    // Persist immediately
    try {
      await notesSave(newNote.id, newNote.title, newNote.content, newNote.folderId, JSON.stringify(newNote.tags));
    } catch (err) {
      setLastError(String(err));
    }
  }, [selectedFolderId]);

  const deleteNote = useCallback(async (id: string) => {
    const note = notes.find(n => n.id === id);
    if (!note) return;
    setNotes(prev => prev.filter(n => n.id !== id));
    if (selectedNoteId === id) {
      setSelectedNoteId(notes.find(n => n.id !== id)?.id ?? "");
    }
    try {
      await notesDelete(id);
    } catch (err) {
      setLastError(String(err));
    }
  }, [notes, selectedNoteId]);

  const duplicateNote = useCallback(async (id: string) => {
    const note = notes.find(n => n.id === id);
    if (!note) return;
    const dup: Note = { ...note, id: `n-${Date.now()}`, title: `${note.title} (copy)`, createdAt: Date.now(), updatedAt: Date.now() };
    setNotes(prev => [dup, ...prev]);
    setSelectedNoteId(dup.id);
    try {
      await notesSave(dup.id, dup.title, dup.content, dup.folderId, JSON.stringify(dup.tags));
    } catch (err) {
      setLastError(String(err));
    }
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
      const updated = { ...n, tags: has ? n.tags.filter(t => t !== tagId) : [...n.tags, tagId], updatedAt: Date.now() };
      // Save after tag change
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(() => persistNote(updated), 300);
      return updated;
    }));
  }, [persistNote]);

  const moveNote = useCallback((noteId: string, folderId: string) => {
    setNotes(prev => prev.map(n => {
      if (n.id !== noteId) return n;
      const updated = { ...n, folderId, updatedAt: Date.now() };
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
      saveTimerRef.current = setTimeout(() => persistNote(updated), 300);
      return updated;
    }));
  }, [persistNote]);

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
  if (!loaded) return (
    <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100%", color: "#64748b", fontSize: 14 }}>
      Loading...
    </div>
  );

  return (
    <div className="na-container">
      {lastError && (
        <div style={{ position: "absolute", top: 8, right: 16, background: "#7f1d1d", color: "#fca5a5", padding: "6px 14px", borderRadius: 6, fontSize: 12, zIndex: 50, cursor: "pointer" }} onClick={() => setLastError(null)}>
          {lastError}
        </div>
      )}
      {/* ─── Sidebar ─── */}
      {showSidebar && (
        <aside className="na-sidebar">
          <div className="na-sidebar-header">
            <h2 className="na-sidebar-title">Notes</h2>
            <div className="na-sidebar-actions">
              <button type="button" className="na-btn-icon" onClick={() => setShowTemplateMenu(!showTemplateMenu)} title="New note">+</button>
              <button type="button" className="na-btn-icon cursor-pointer" onClick={() => setShowSidebar(false)} title="Hide sidebar"><ChevronLeft size={14} aria-hidden="true" /></button>
            </div>
          </div>

          {/* template menu */}
          {showTemplateMenu && (
            <div className="na-template-menu">
              <div className="na-template-header">New from template</div>
              {Object.entries(TEMPLATES).map(([key, _tmpl]) => (
                <button type="button" key={key} className="na-template-item cursor-pointer" onClick={() => createNote(key)}>
                  <span className="na-template-icon">{key === "meeting" ? <Calendar size={14} aria-hidden="true" /> : key === "research" ? <Microscope size={14} aria-hidden="true" /> : key === "project" ? <FolderOpen size={14} aria-hidden="true" /> : key === "bug-report" ? <Bug size={14} aria-hidden="true" /> : <StickyNote size={14} aria-hidden="true" />}</span>
                  <span>{key.replace("-", " ").replace(/\b\w/g, c => c.toUpperCase())}</span>
                </button>
              ))}
            </div>
          )}

          {/* search */}
          <div className="na-search-box">
            <span className="na-search-icon"><Search size={14} aria-hidden="true" /></span>
            <input id="na-search" className="na-search-input" placeholder="Search notes..." value={searchQuery} onChange={e => setSearchQuery(e.target.value)} />
            {searchQuery && <button type="button" className="na-search-clear" onClick={() => setSearchQuery("")}>×</button>}
          </div>

          {/* folders */}
          <div className="na-folders">
            {folders.map(folder => (
              <div key={folder.id} className={`na-folder-item ${selectedFolderId === folder.id ? "active" : ""}`}>
                <button type="button" className="na-folder-btn cursor-pointer" onClick={() => { setSelectedFolderId(folder.id); setSelectedTagFilter(null); }}>
                  <span className="na-folder-icon">{FOLDER_ICON_MAP[folder.icon] ?? folder.icon}</span>
                  <span className="na-folder-name">{folder.name}</span>
                  <span className="na-folder-count">{getNoteCount(folder.id)}</span>
                </button>
                {folder.parentId === null && folder.id !== "f-all" && (
                  <button type="button" className="na-folder-toggle cursor-pointer" onClick={() => toggleFolder(folder.id)}>
                    {folder.collapsed ? <ChevronRight size={12} aria-hidden="true" /> : <ChevronDown size={12} aria-hidden="true" />}
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
                <button type="button" key={tag.id} className={`na-tag-filter ${selectedTagFilter === tag.id ? "active" : ""}`} onClick={() => setSelectedTagFilter(selectedTagFilter === tag.id ? null : tag.id)} style={{ borderColor: tag.color }}>
                  <span className="na-tag-dot" style={{ background: tag.color }} />
                  {tag.name}
                </button>
              ))}
            </div>
          </div>

          {/* status */}
          <div className="na-agent-activity">
            <div className="na-agent-header">Storage</div>
            <div className="na-agent-log">
              <div className="na-agent-entry">{notes.length} notes saved to ~/.nexus/notes/</div>
              {saving && <div className="na-agent-entry">Saving...</div>}
              {loaded && notes.length === 0 && <div className="na-agent-entry">No notes yet. Create one!</div>}
            </div>
          </div>
        </aside>
      )}

      {/* ─── Note List ─── */}
      <div className="na-note-list">
        <div className="na-list-header">
          <div className="na-list-title">
            {!showSidebar && <button type="button" className="na-btn-icon cursor-pointer" onClick={() => setShowSidebar(true)} title="Show sidebar"><Play size={12} aria-hidden="true" /></button>}
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
              <div className="na-empty-icon"><StickyNote size={24} aria-hidden="true" /></div>
              <div>No notes found</div>
              <button type="button" className="na-btn-create" onClick={() => createNote()}>Create Note</button>
            </div>
          )}
          {filteredNotes.map(note => (
            <div key={note.id} className={`na-note-card ${selectedNoteId === note.id ? "active" : ""} ${note.pinned ? "pinned" : ""}`} onClick={() => setSelectedNoteId(note.id)}>
              <div className="na-note-card-header">
                <span className="na-note-card-title">{note.pinned && <span className="na-pin"><Pin size={12} aria-hidden="true" /></span>}{note.title}</span>
                <div className="na-note-card-actions">
                  <button type="button" className="na-btn-tiny cursor-pointer" onClick={e => { e.stopPropagation(); togglePin(note.id); }}>{note.pinned ? <PinOff size={12} aria-hidden="true" /> : <Pin size={12} aria-hidden="true" />}</button>
                  <button type="button" className="na-btn-tiny" onClick={e => { e.stopPropagation(); deleteNote(note.id); }}>×</button>
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
                  <button type="button" className={`na-view-btn ${viewMode === "edit" ? "active" : ""}`} onClick={() => setViewMode("edit")}>Edit</button>
                  <button type="button" className={`na-view-btn ${viewMode === "split" ? "active" : ""}`} onClick={() => setViewMode("split")}>Split</button>
                  <button type="button" className={`na-view-btn ${viewMode === "preview" ? "active" : ""}`} onClick={() => setViewMode("preview")}>Preview</button>
                </div>
                <button type="button" className={`na-btn-icon cursor-pointer ${showTagPicker ? "active" : ""}`} onClick={() => setShowTagPicker(!showTagPicker)} title="Tags"><Tag size={14} aria-hidden="true" /></button>
                <div className="na-export-wrapper">
                  <button type="button" className="na-btn-icon cursor-pointer" onClick={() => setShowExportMenu(!showExportMenu)} title="Export"><Download size={14} aria-hidden="true" /></button>
                  {showExportMenu && (
                    <div className="na-export-menu">
                      <button type="button" className="na-export-item cursor-pointer" onClick={() => { setShowExportMenu(false); setFuelUsed(f => f + 5); }}><FileText size={12} aria-hidden="true" style={{ display: "inline", verticalAlign: "middle", marginRight: 4 }} /> Markdown (.md)</button>
                    </div>
                  )}
                </div>
                <div className="na-move-wrapper">
                  <select className="na-move-select" value={selectedNote.folderId} onChange={e => moveNote(selectedNote.id, e.target.value)}>
                    {folders.filter(f => f.id !== "f-all").map(f => (
                      <option key={f.id} value={f.id}>{f.name}</option>
                    ))}
                  </select>
                </div>
                <button type="button" className="na-btn-icon cursor-pointer" onClick={() => duplicateNote(selectedNote.id)} title="Duplicate"><Copy size={14} aria-hidden="true" /></button>
              </div>
            </div>

            {/* tag picker */}
            {showTagPicker && (
              <div className="na-tag-picker">
                {tags.map(tag => (
                  <button type="button" key={tag.id} className={`na-tag-pick ${selectedNote.tags.includes(tag.id) ? "selected" : ""}`} style={{ borderColor: tag.color, background: selectedNote.tags.includes(tag.id) ? tag.color + "22" : "transparent" }} onClick={() => toggleNoteTag(selectedNote.id, tag.id)}>
                    <span className="na-tag-dot" style={{ background: tag.color }} />
                    {tag.name}
                  </button>
                ))}
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
              {saving && <span className="na-status-item">Saving...</span>}
              {selectedNote.template && <span className="na-status-item">Template: {selectedNote.template}</span>}
              <span className="na-status-item na-status-right">
                <span className="na-fuel-icon"><Zap size={12} aria-hidden="true" /></span> {fuelUsed} fuel used
              </span>
              <span className="na-status-item">{notes.length} notes</span>
              <span className="na-status-item">Ctrl+N new · Ctrl+B sidebar · Ctrl+E view · Ctrl+F search</span>
            </div>
          </>
        ) : (
          <div className="na-no-note">
            <div className="na-no-note-icon"><StickyNote size={32} aria-hidden="true" /></div>
            <div className="na-no-note-text">Select a note or create a new one</div>
            <button type="button" className="na-btn-create" onClick={() => createNote()}>New Note</button>
          </div>
        )}
      </div>
    </div>
  );
}

import { useCallback, useEffect, useMemo, useState } from "react";
import "./file-manager.css";

/* ================================================================== */
/*  Tauri invoke                                                       */
/* ================================================================== */

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const invoke: (cmd: string, args?: Record<string, unknown>) => Promise<any> =
  typeof window !== "undefined" && "__TAURI__" in window
    ? // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (window as any).__TAURI__.invoke
    : async (_cmd: string, _args?: Record<string, unknown>) =>
        JSON.stringify([]);

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface FsEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: number;
}

interface AuditEntry {
  ts: number;
  event: string;
  detail: string;
}

interface BreadcrumbItem {
  name: string;
  path: string;
}

type ViewMode = "grid" | "list";
type SortBy = "name" | "size" | "modified" | "type";
type SortDir = "asc" | "desc";

/* ================================================================== */
/*  Helpers                                                            */
/* ================================================================== */

function extOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

function entryIcon(entry: FsEntry): string {
  if (entry.is_dir) return "📁";
  const ext = extOf(entry.name);
  const map: Record<string, string> = {
    rs: "🦀", ts: "TS", tsx: "TS", js: "JS", jsx: "JS", py: "🐍",
    json: "{}", css: "#", html: "<>", md: "📝", toml: "⚙", yaml: "⚙",
    yml: "⚙", sh: "$", sql: "⊞", go: "Go", lock: "🔒",
    png: "🖼", jpg: "🖼", jpeg: "🖼", gif: "🖼", svg: "🖼", webp: "🖼",
    mp3: "♫", wav: "♫", ogg: "♫", mp4: "🎬", webm: "🎬",
    pdf: "📄", doc: "📄", docx: "📄", txt: "📄", csv: "📊",
    zip: "📦", tar: "📦", gz: "📦", env: "🔑", pem: "🔑", key: "🔑",
  };
  return map[ext] ?? "📄";
}

function formatSize(bytes: number): string {
  if (bytes === 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

function formatDate(ts: number): string {
  if (ts === 0) return "—";
  return new Date(ts).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric", hour: "2-digit", minute: "2-digit" });
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });
}

function isPreviewable(ext: string): boolean {
  return ["rs", "ts", "tsx", "js", "jsx", "py", "json", "css", "html", "md", "toml", "yaml", "yml", "sh", "sql", "go", "txt", "csv", "lock", "cfg", "conf", "xml", "svg", "log", "env"].includes(ext);
}

function isImage(ext: string): boolean {
  return ["png", "jpg", "jpeg", "gif", "svg", "webp", "ico"].includes(ext);
}

function pathJoin(base: string, name: string): string {
  if (base.endsWith("/")) return base + name;
  return base + "/" + name;
}

function parentPath(p: string): string {
  const idx = p.lastIndexOf("/");
  if (idx <= 0) return "/";
  return p.slice(0, idx);
}

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function FileManager(): JSX.Element {
  const FILE_LOAD_ERROR = "Unable to load files. Check permissions.";

  /* ---- State ---- */
  const [currentPath, setCurrentPath] = useState<string>("");
  const [entries, setEntries] = useState<FsEntry[]>([]);
  const [selectedEntry, setSelectedEntry] = useState<FsEntry | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("grid");
  const [sortBy, setSortBy] = useState<SortBy>("name");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [searchQuery, setSearchQuery] = useState("");
  const [showSearch, setShowSearch] = useState(false);
  const [showSidebar, setShowSidebar] = useState(true);
  const [auditLog, setAuditLog] = useState<AuditEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [previewContent, setPreviewContent] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<FsEntry | null>(null);
  const [renaming, setRenaming] = useState<FsEntry | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [newItemType, setNewItemType] = useState<"file" | "dir" | null>(null);
  const [newItemName, setNewItemName] = useState("");
  const [sidebarTab, setSidebarTab] = useState<"preview" | "details">("preview");

  /* ---- Helpers ---- */
  const appendAudit = useCallback((event: string, detail: string) => {
    setAuditLog((prev) => [{ ts: Date.now(), event, detail }, ...prev].slice(0, 200));
  }, []);

  /* ---- Load directory ---- */
  const loadDir = useCallback(async (dirPath: string) => {
    setLoading(true);
    setError(null);
    setSelectedEntry(null);
    setPreviewContent(null);
    try {
      const raw: string = await invoke("file_manager_list", { path: dirPath });
      const parsed: FsEntry[] = JSON.parse(raw);
      setEntries(parsed);
      setCurrentPath(dirPath);
      appendAudit("Navigate", dirPath);
      return true;
    } catch (e) {
      console.error("[FileManager] failed to load directory", dirPath, e);
      setError(FILE_LOAD_ERROR);
      setEntries([]);
      return false;
    } finally {
      setLoading(false);
    }
  }, [appendAudit]);

  /* ---- Init: load home directory ---- */
  useEffect(() => {
    (async () => {
      try {
        const home: string = await invoke("file_manager_home");
        const candidates = [pathJoin(home, ".nexus"), home];
        for (const candidate of candidates) {
          const loaded = await loadDir(candidate);
          if (loaded) return;
        }
        setError(FILE_LOAD_ERROR);
      } catch (e) {
        console.error("[FileManager] failed to resolve initial directory", e);
        setError(FILE_LOAD_ERROR);
        setEntries([]);
      }
    })();
  }, [loadDir]);

  /* ---- Load file preview ---- */
  const loadPreview = useCallback(async (entry: FsEntry) => {
    if (entry.is_dir) { setPreviewContent(null); return; }
    const ext = extOf(entry.name);
    if (!isPreviewable(ext)) { setPreviewContent(null); return; }
    if (entry.size > 512 * 1024) { setPreviewContent("(file too large to preview)"); return; }
    try {
      const content: string = await invoke("file_manager_read", { path: entry.path });
      setPreviewContent(content);
    } catch (e) {
      setPreviewContent(`Error reading file: ${e}`);
    }
  }, []);

  /* ---- Sorted & filtered entries ---- */
  const displayEntries = useMemo(() => {
    let items = [...entries];
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      items = items.filter((e) => e.name.toLowerCase().includes(q));
    }
    items.sort((a, b) => {
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
      let cmp = 0;
      switch (sortBy) {
        case "name": cmp = a.name.localeCompare(b.name); break;
        case "size": cmp = a.size - b.size; break;
        case "modified": cmp = a.modified - b.modified; break;
        case "type": cmp = extOf(a.name).localeCompare(extOf(b.name)); break;
      }
      return sortDir === "asc" ? cmp : -cmp;
    });
    return items;
  }, [entries, searchQuery, sortBy, sortDir]);

  /* ---- Breadcrumbs ---- */
  const breadcrumbs = useMemo((): BreadcrumbItem[] => {
    if (!currentPath) return [];
    const parts = currentPath.split("/").filter(Boolean);
    const crumbs: BreadcrumbItem[] = [{ name: "/", path: "/" }];
    let running = "";
    for (const p of parts) {
      running += `/${p}`;
      crumbs.push({ name: p, path: running });
    }
    return crumbs;
  }, [currentPath]);

  /* ---- Navigation ---- */
  function openItem(entry: FsEntry): void {
    if (entry.is_dir) {
      loadDir(entry.path);
    } else {
      setSelectedEntry(entry);
      setSidebarTab("preview");
      setShowSidebar(true);
      loadPreview(entry);
      appendAudit("FileOpen", entry.path);
    }
  }

  function goUp(): void {
    if (!currentPath || currentPath === "/") return;
    loadDir(parentPath(currentPath));
  }

  function refresh(): void {
    if (currentPath) loadDir(currentPath);
  }

  /* ---- File operations ---- */
  async function handleDelete(entry: FsEntry): Promise<void> {
    try {
      await invoke("file_manager_delete", { path: entry.path });
      appendAudit("Delete", entry.path);
      setConfirmDelete(null);
      setSelectedEntry(null);
      refresh();
    } catch (e) {
      setError(String(e));
      setConfirmDelete(null);
    }
  }

  async function handleRename(entry: FsEntry): Promise<void> {
    if (!renameValue.trim()) { setRenaming(null); return; }
    const newPath = pathJoin(parentPath(entry.path), renameValue.trim());
    try {
      await invoke("file_manager_rename", { from: entry.path, to: newPath });
      appendAudit("Rename", `${entry.name} → ${renameValue.trim()}`);
      setRenaming(null);
      setRenameValue("");
      refresh();
    } catch (e) {
      setError(String(e));
      setRenaming(null);
    }
  }

  async function handleCreateItem(): Promise<void> {
    if (!newItemName.trim() || !newItemType) return;
    const name = newItemName.trim();
    const fullPath = pathJoin(currentPath, name);
    try {
      if (newItemType === "dir") {
        await invoke("file_manager_create_dir", { path: fullPath });
      } else {
        await invoke("file_manager_write", { path: fullPath, content: "" });
      }
      appendAudit("Create", fullPath);
      setNewItemType(null);
      setNewItemName("");
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  /* ---- Keyboard shortcuts ---- */
  useEffect(() => {
    function onKey(e: KeyboardEvent): void {
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key === "f") { e.preventDefault(); setShowSearch((p) => !p); }
      if (mod && e.key === "b") { e.preventDefault(); setShowSidebar((p) => !p); }
      if (e.key === "Backspace" && !renaming && !showSearch && !newItemType) { goUp(); }
      if (e.key === "Delete" && selectedEntry && !renaming) { setConfirmDelete(selectedEntry); }
      if (e.key === "F5") { e.preventDefault(); refresh(); }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function toggleSort(col: SortBy): void {
    if (sortBy === col) setSortDir((p) => p === "asc" ? "desc" : "asc");
    else { setSortBy(col); setSortDir("asc"); }
  }

  /* ================================================================ */
  /*  RENDER                                                           */
  /* ================================================================ */
  return (
    <section className="fm-root">
      {/* ---- Header ---- */}
      <header className="fm-header">
        <div className="fm-header-left">
          <h2 className="fm-title">FILE MANAGER</h2>
          <span className="fm-subtitle">governed filesystem</span>
        </div>
        <div className="fm-header-right">
          <div className="fm-toolbar">
            <button type="button" className={`fm-tool-btn ${viewMode === "grid" ? "fm-tool-active" : ""}`} onClick={() => setViewMode("grid")} title="Grid view">⊞</button>
            <button type="button" className={`fm-tool-btn ${viewMode === "list" ? "fm-tool-active" : ""}`} onClick={() => setViewMode("list")} title="List view">☰</button>
            <button type="button" className={`fm-tool-btn ${showSearch ? "fm-tool-active" : ""}`} onClick={() => setShowSearch((p) => !p)} title="Search (Ctrl+F)">⌕</button>
            <button type="button" className="fm-tool-btn" onClick={refresh} title="Refresh (F5)">↻</button>
            <button type="button" className={`fm-tool-btn ${showSidebar ? "fm-tool-active" : ""}`} onClick={() => setShowSidebar((p) => !p)} title="Sidebar (Ctrl+B)">◨</button>
          </div>
        </div>
      </header>

      {/* ---- Error banner ---- */}
      {error && (
        <div className="fm-error-bar">
          <span>{error}</span>
          <button type="button" className="fm-error-close" onClick={() => setError(null)}>×</button>
        </div>
      )}

      {/* ---- Action bar ---- */}
      <div className="fm-action-bar">
        <div className="fm-breadcrumbs">
          <button type="button" className="fm-nav-btn" onClick={goUp} disabled={!currentPath || currentPath === "/"} title="Go up">↑</button>
          {breadcrumbs.map((bc, i) => (
            <span key={bc.path} className="fm-crumb-wrap">
              {i > 0 && <span className="fm-crumb-sep">/</span>}
              <button type="button" className={`fm-crumb ${i === breadcrumbs.length - 1 ? "fm-crumb-current" : ""}`} onClick={() => loadDir(bc.path)}>
                {bc.name}
              </button>
            </span>
          ))}
        </div>
        <div className="fm-action-btns">
          <button type="button" className="fm-action-btn" onClick={() => { setNewItemType("file"); setNewItemName(""); }} title="New file">+ File</button>
          <button type="button" className="fm-action-btn" onClick={() => { setNewItemType("dir"); setNewItemName(""); }} title="New folder">+ Folder</button>
        </div>
      </div>

      {/* ---- Search bar ---- */}
      {showSearch && (
        <div className="fm-search-bar">
          <span className="fm-search-icon">⌕</span>
          <input className="fm-search-input" placeholder="Filter files by name..." value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} autoFocus onKeyDown={(e) => { if (e.key === "Escape") { setShowSearch(false); setSearchQuery(""); } }} />
          {searchQuery && <span className="fm-search-count">{displayEntries.length} results</span>}
          <button type="button" className="fm-search-close" onClick={() => { setShowSearch(false); setSearchQuery(""); }}>×</button>
        </div>
      )}

      {/* ---- New item input ---- */}
      {newItemType && (
        <div className="fm-new-item-bar">
          <span className="fm-new-item-label">New {newItemType}:</span>
          <input className="fm-new-item-input" placeholder={newItemType === "dir" ? "folder-name" : "filename.ext"} value={newItemName} onChange={(e) => setNewItemName(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleCreateItem(); if (e.key === "Escape") setNewItemType(null); }} autoFocus />
          <button type="button" className="fm-new-item-create" onClick={handleCreateItem}>Create</button>
          <button type="button" className="fm-new-item-cancel" onClick={() => setNewItemType(null)}>Cancel</button>
        </div>
      )}

      {/* ---- Body ---- */}
      <div className="fm-body">
        {/* ---- Files area ---- */}
        <div className="fm-files-area">
          {/* List header */}
          {viewMode === "list" && (
            <div className="fm-list-header">
              <button type="button" className="fm-list-col fm-col-name" onClick={() => toggleSort("name")}>Name {sortBy === "name" ? (sortDir === "asc" ? "↑" : "↓") : ""}</button>
              <button type="button" className="fm-list-col fm-col-size" onClick={() => toggleSort("size")}>Size {sortBy === "size" ? (sortDir === "asc" ? "↑" : "↓") : ""}</button>
              <button type="button" className="fm-list-col fm-col-modified" onClick={() => toggleSort("modified")}>Modified {sortBy === "modified" ? (sortDir === "asc" ? "↑" : "↓") : ""}</button>
            </div>
          )}

          {/* Loading */}
          {loading && <div className="fm-empty">Loading...</div>}

          {/* Files */}
          {!loading && (
            <div className={`fm-files ${viewMode === "grid" ? "fm-files-grid" : "fm-files-list"}`}>
              {displayEntries.map((entry) => (
                <div
                  key={entry.path}
                  className={`fm-file-item ${viewMode === "grid" ? "fm-file-grid" : "fm-file-row"} ${selectedEntry?.path === entry.path ? "fm-file-selected" : ""}`}
                  onClick={() => { setSelectedEntry(entry); if (!entry.is_dir) loadPreview(entry); }}
                  onDoubleClick={() => openItem(entry)}
                >
                  {viewMode === "grid" ? (
                    <>
                      <span className="fm-file-icon-large">{entryIcon(entry)}</span>
                      {renaming?.path === entry.path ? (
                        <input className="fm-rename-input" value={renameValue} onChange={(e) => setRenameValue(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleRename(entry); if (e.key === "Escape") setRenaming(null); }} onBlur={() => handleRename(entry)} autoFocus />
                      ) : (
                        <span className="fm-file-name-grid">{entry.name}</span>
                      )}
                    </>
                  ) : (
                    <>
                      <span className="fm-file-icon">{entryIcon(entry)}</span>
                      {renaming?.path === entry.path ? (
                        <input className="fm-rename-input fm-rename-list" value={renameValue} onChange={(e) => setRenameValue(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleRename(entry); if (e.key === "Escape") setRenaming(null); }} onBlur={() => handleRename(entry)} autoFocus />
                      ) : (
                        <span className="fm-file-name">{entry.name}</span>
                      )}
                      <span className="fm-file-size">{entry.is_dir ? "—" : formatSize(entry.size)}</span>
                      <span className="fm-file-modified">{formatDate(entry.modified)}</span>
                    </>
                  )}
                </div>
              ))}
              {displayEntries.length === 0 && !loading && (
                <div className="fm-empty">
                  {error
                    ? FILE_LOAD_ERROR
                    : searchQuery
                      ? "No files match your search."
                      : currentPath
                        ? "This folder is empty."
                        : "Loading your files..."}
                </div>
              )}
            </div>
          )}

          {/* Context actions for selected */}
          {selectedEntry && !renaming && (
            <div className="fm-context-bar">
              <button type="button" className="fm-ctx-btn" onClick={() => openItem(selectedEntry)}>Open</button>
              <button type="button" className="fm-ctx-btn" onClick={() => { setRenaming(selectedEntry); setRenameValue(selectedEntry.name); }}>Rename</button>
              <button type="button" className="fm-ctx-btn fm-ctx-danger" onClick={() => setConfirmDelete(selectedEntry)}>Delete</button>
              <span className="fm-ctx-info">{selectedEntry.path}</span>
            </div>
          )}
        </div>

        {/* ---- Sidebar ---- */}
        {showSidebar && (
          <aside className="fm-sidebar">
            <div className="fm-sidebar-tabs">
              <button type="button" className={`fm-stab ${sidebarTab === "preview" ? "fm-stab-active" : ""}`} onClick={() => setSidebarTab("preview")}>Preview</button>
              <button type="button" className={`fm-stab ${sidebarTab === "details" ? "fm-stab-active" : ""}`} onClick={() => setSidebarTab("details")}>Details</button>
            </div>

            {/* Preview */}
            {sidebarTab === "preview" && (
              <div className="fm-sidebar-body">
                {selectedEntry ? (
                  <>
                    <div className="fm-preview-header">
                      <span className="fm-preview-icon">{entryIcon(selectedEntry)}</span>
                      <span className="fm-preview-name">{selectedEntry.name}</span>
                    </div>
                    {isImage(extOf(selectedEntry.name)) && (
                      <div className="fm-preview-image">
                        <div className="fm-preview-image-placeholder">
                          🖼 {selectedEntry.name}
                          <br /><span className="fm-preview-image-size">{formatSize(selectedEntry.size)}</span>
                        </div>
                      </div>
                    )}
                    {isPreviewable(extOf(selectedEntry.name)) && previewContent !== null && (
                      <pre className="fm-preview-code">{previewContent}</pre>
                    )}
                    {!isPreviewable(extOf(selectedEntry.name)) && !isImage(extOf(selectedEntry.name)) && !selectedEntry.is_dir && (
                      <div className="fm-preview-nopreview">
                        <span className="fm-preview-nopreview-icon">{entryIcon(selectedEntry)}</span>
                        <p>No preview available</p>
                        <p className="fm-preview-ext">.{extOf(selectedEntry.name)} — {formatSize(selectedEntry.size)}</p>
                      </div>
                    )}
                    {selectedEntry.is_dir && (
                      <div className="fm-preview-nopreview">
                        <span className="fm-preview-nopreview-icon">📁</span>
                        <p>Directory</p>
                        <p className="fm-preview-ext">Double-click to open</p>
                      </div>
                    )}
                  </>
                ) : (
                  <div className="fm-preview-empty">
                    <p>Select a file to preview</p>
                  </div>
                )}
              </div>
            )}

            {/* Details */}
            {sidebarTab === "details" && selectedEntry && (
              <div className="fm-sidebar-body">
                <div className="fm-detail-section">
                  <span className="fm-detail-title">FILE DETAILS</span>
                  <div className="fm-detail-row"><span className="fm-detail-label">Name</span><span className="fm-detail-value">{selectedEntry.name}</span></div>
                  <div className="fm-detail-row"><span className="fm-detail-label">Path</span><span className="fm-detail-value">{selectedEntry.path}</span></div>
                  <div className="fm-detail-row"><span className="fm-detail-label">Type</span><span className="fm-detail-value">{selectedEntry.is_dir ? "Directory" : extOf(selectedEntry.name).toUpperCase() || "File"}</span></div>
                  <div className="fm-detail-row"><span className="fm-detail-label">Size</span><span className="fm-detail-value">{selectedEntry.is_dir ? "—" : formatSize(selectedEntry.size)}</span></div>
                  <div className="fm-detail-row"><span className="fm-detail-label">Modified</span><span className="fm-detail-value">{formatDate(selectedEntry.modified)}</span></div>
                </div>
                <div className="fm-detail-section">
                  <span className="fm-detail-title">AUDIT LOG</span>
                  <div className="fm-audit-mini">
                    {auditLog.filter((a) => a.detail.includes(selectedEntry.name) || a.detail.includes(selectedEntry.path)).slice(0, 5).map((a, i) => (
                      <div key={`${a.ts}-${i}`} className="fm-audit-entry">
                        <span className="fm-audit-time">{formatTime(a.ts)}</span>
                        <span className="fm-audit-event">{a.event}</span>
                      </div>
                    ))}
                    {auditLog.filter((a) => a.detail.includes(selectedEntry.name) || a.detail.includes(selectedEntry.path)).length === 0 && (
                      <p className="fm-trash-empty">No audit entries for this file</p>
                    )}
                  </div>
                </div>
              </div>
            )}
            {sidebarTab === "details" && !selectedEntry && (
              <div className="fm-sidebar-body"><div className="fm-preview-empty"><p>Select a file to see details</p></div></div>
            )}
          </aside>
        )}
      </div>

      {/* ---- Delete confirmation ---- */}
      {confirmDelete && (
        <div className="fm-confirm-overlay">
          <div className="fm-confirm-dialog">
            <p className="fm-confirm-title">GOVERNED DELETE — Confirmation Required</p>
            <p className="fm-confirm-text">Permanently delete <strong>{confirmDelete.name}</strong>?</p>
            <p className="fm-confirm-path">{confirmDelete.path}</p>
            {confirmDelete.is_dir && <p className="fm-confirm-text" style={{ color: "#ef4444" }}>This is a directory — all contents will be deleted.</p>}
            <div className="fm-confirm-actions">
              <button type="button" className="fm-confirm-btn fm-confirm-yes" onClick={() => handleDelete(confirmDelete)}>Delete</button>
              <button type="button" className="fm-confirm-btn fm-confirm-no" onClick={() => setConfirmDelete(null)}>Cancel</button>
            </div>
          </div>
        </div>
      )}

      {/* ---- Status bar ---- */}
      <div className="fm-status-bar">
        <span className="fm-status-item">{entries.length} items</span>
        <span className="fm-status-sep">·</span>
        <span className="fm-status-item">{currentPath}</span>
        <span className="fm-status-sep">·</span>
        <span className="fm-status-item">{viewMode} view</span>
        <span className="fm-status-right">
          <span className="fm-status-item">{auditLog.length} audit events</span>
        </span>
      </div>
    </section>
  );
}

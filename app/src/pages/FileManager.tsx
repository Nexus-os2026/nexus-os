import { useCallback, useEffect, useMemo, useState } from "react";
import "./file-manager.css";

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface VFile {
  id: string;
  name: string;
  path: string;
  type: "file" | "dir";
  size: number;
  modified: number;
  ext: string;
  permissions: FilePermission;
  content?: string;
  children?: VFile[];
  encrypted?: boolean;
  trashed?: boolean;
  trashedAt?: number;
}

interface FilePermission {
  owner: "user" | "agent";
  agentName?: string;
  read: boolean;
  write: boolean;
  execute: boolean;
}

interface AuditEntry {
  ts: number;
  event: string;
  detail: string;
}

interface AgentFileOp {
  id: string;
  agent: string;
  action: string;
  file: string;
  progress: number;
  status: "active" | "done" | "error";
}

interface BreadcrumbItem {
  name: string;
  path: string;
}

type ViewMode = "grid" | "list";
type SortBy = "name" | "size" | "modified" | "type";
type SortDir = "asc" | "desc";
type SidebarTab = "preview" | "details" | "vault" | "trash";

/* ================================================================== */
/*  Helpers                                                            */
/* ================================================================== */

function extOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

function fileIcon(f: VFile): string {
  if (f.type === "dir") return f.encrypted ? "🔒" : "📁";
  const map: Record<string, string> = {
    rs: "🦀", ts: "TS", tsx: "TS", js: "JS", jsx: "JS", py: "🐍",
    json: "{}", css: "#", html: "<>", md: "📝", toml: "⚙", yaml: "⚙",
    yml: "⚙", sh: "$", sql: "⊞", go: "Go", lock: "🔒",
    png: "🖼", jpg: "🖼", jpeg: "🖼", gif: "🖼", svg: "🖼", webp: "🖼",
    mp3: "♫", wav: "♫", ogg: "♫", mp4: "🎬", webm: "🎬",
    pdf: "📄", doc: "📄", docx: "📄", txt: "📄", csv: "📊",
    zip: "📦", tar: "📦", gz: "📦", env: "🔑", pem: "🔑", key: "🔑",
  };
  return map[f.ext] ?? "📄";
}

function formatSize(bytes: number): string {
  if (bytes === 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

function formatDate(ts: number): string {
  return new Date(ts).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric", hour: "2-digit", minute: "2-digit" });
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });
}

function permString(p: FilePermission): string {
  return `${p.read ? "r" : "-"}${p.write ? "w" : "-"}${p.execute ? "x" : "-"}`;
}

function isPreviewable(ext: string): boolean {
  return ["rs", "ts", "tsx", "js", "jsx", "py", "json", "css", "html", "md", "toml", "yaml", "yml", "sh", "sql", "go", "txt", "csv", "lock", "cfg", "conf", "env", "xml", "svg"].includes(ext);
}

function isImage(ext: string): boolean {
  return ["png", "jpg", "jpeg", "gif", "svg", "webp", "ico"].includes(ext);
}

/* ================================================================== */
/*  Mock data                                                          */
/* ================================================================== */

const now = Date.now();
const hour = 3600000;
const day = 86400000;

function mockPerm(owner: "user" | "agent" = "user", agentName?: string, w = true): FilePermission {
  return { owner, agentName, read: true, write: w, execute: false };
}

const MOCK_FILES: VFile[] = [
  {
    id: "d-src", name: "src", path: "/src", type: "dir", size: 0, modified: now - 2 * hour, ext: "",
    permissions: mockPerm(), children: [
      { id: "f-main", name: "main.rs", path: "/src/main.rs", type: "file", size: 1247, modified: now - hour, ext: "rs", permissions: mockPerm(),
        content: `use nexus_kernel::Supervisor;\nuse nexus_sdk::prelude::*;\nuse std::sync::Arc;\n\nfn main() -> Result<(), Box<dyn std::error::Error>> {\n    let supervisor = Arc::new(Supervisor::new());\n    tracing::info!("Nexus OS v7.0");\n    supervisor.boot()?;\n    Ok(())\n}\n` },
      { id: "f-lib", name: "lib.rs", path: "/src/lib.rs", type: "file", size: 834, modified: now - 3 * hour, ext: "rs", permissions: mockPerm(),
        content: `//! Nexus OS Kernel Library\n\npub mod audit;\npub mod capabilities;\npub mod fuel;\npub mod supervisor;\n\npub use supervisor::Supervisor;\npub const VERSION: &str = env!("CARGO_PKG_VERSION");\n` },
      { id: "f-api", name: "api.rs", path: "/src/api.rs", type: "file", size: 2156, modified: now - 30 * 60000, ext: "rs", permissions: mockPerm("agent", "Coder"),
        content: `//! REST API endpoints\nuse axum::{Router, Json, extract::State};\nuse crate::Supervisor;\n\npub fn routes() -> Router<Arc<Supervisor>> {\n    Router::new()\n        .route("/agents", get(list_agents))\n        .route("/fuel", get(fuel_status))\n}\n` },
    ],
  },
  {
    id: "d-agents", name: "agents", path: "/agents", type: "dir", size: 0, modified: now - 5 * hour, ext: "",
    permissions: mockPerm(), children: [
      { id: "f-coder", name: "coder.toml", path: "/agents/coder.toml", type: "file", size: 342, modified: now - day, ext: "toml", permissions: mockPerm("agent", "Coder"),
        content: `[agent]\nname = "coder"\nauthor = "nexus"\nautonomy_level = 3\n\n[capabilities]\nfile_read = true\nfile_write = true\nshell_exec = false\n\n[fuel]\nbudget = 10000\n` },
      { id: "f-designer", name: "designer.toml", path: "/agents/designer.toml", type: "file", size: 298, modified: now - day, ext: "toml", permissions: mockPerm("agent", "Designer") },
      { id: "f-researcher", name: "researcher.toml", path: "/agents/researcher.toml", type: "file", size: 315, modified: now - 2 * day, ext: "toml", permissions: mockPerm("agent", "Researcher") },
    ],
  },
  {
    id: "d-app", name: "app", path: "/app", type: "dir", size: 0, modified: now - hour, ext: "",
    permissions: mockPerm(), children: [
      { id: "f-pkg", name: "package.json", path: "/app/package.json", type: "file", size: 1845, modified: now - 2 * day, ext: "json", permissions: mockPerm(),
        content: `{\n  "name": "nexus-desktop",\n  "version": "7.0.0",\n  "private": true,\n  "scripts": {\n    "dev": "vite",\n    "build": "tsc && vite build"\n  },\n  "dependencies": {\n    "react": "^18.3.1",\n    "@monaco-editor/react": "^4.6.0"\n  }\n}\n` },
      { id: "f-tsconfig", name: "tsconfig.json", path: "/app/tsconfig.json", type: "file", size: 423, modified: now - 3 * day, ext: "json", permissions: mockPerm() },
      { id: "f-vite", name: "vite.config.ts", path: "/app/vite.config.ts", type: "file", size: 567, modified: now - 3 * day, ext: "ts", permissions: mockPerm() },
    ],
  },
  {
    id: "d-vault", name: ".vault", path: "/.vault", type: "dir", size: 0, modified: now - day, ext: "",
    permissions: mockPerm("user", undefined, true), encrypted: true, children: [
      { id: "f-env", name: ".env", path: "/.vault/.env", type: "file", size: 256, modified: now - day, ext: "env", permissions: mockPerm("user", undefined, true), encrypted: true,
        content: `# ENCRYPTED — Requires user approval to view\nDATABASE_URL=postgres://***:***@localhost:5432/nexus\nCLAUDE_API_KEY=sk-ant-***\nJWT_SECRET=***\n` },
      { id: "f-key", name: "agent-signing.pem", path: "/.vault/agent-signing.pem", type: "file", size: 1704, modified: now - 5 * day, ext: "pem", permissions: mockPerm("user", undefined, false), encrypted: true },
      { id: "f-creds", name: "credentials.json", path: "/.vault/credentials.json", type: "file", size: 892, modified: now - 3 * day, ext: "json", permissions: mockPerm("user", undefined, false), encrypted: true },
    ],
  },
  { id: "f-cargo", name: "Cargo.toml", path: "/Cargo.toml", type: "file", size: 1423, modified: now - 2 * day, ext: "toml", permissions: mockPerm(),
    content: `[workspace]\nresolver = "2"\nmembers = [\n  "crates/*",\n  "agents/*",\n]\n\n[workspace.package]\nversion = "7.0.0"\nedition = "2021"\n` },
  { id: "f-lock", name: "Cargo.lock", path: "/Cargo.lock", type: "file", size: 89234, modified: now - day, ext: "lock", permissions: mockPerm("user", undefined, false) },
  { id: "f-claude", name: "CLAUDE.md", path: "/CLAUDE.md", type: "file", size: 4521, modified: now - 3 * hour, ext: "md", permissions: mockPerm(),
    content: `# CLAUDE.md - Nexus OS Development Guide\n\n> Read automatically by Claude Code.\n\n## Project Identity\n- Name: Nexus OS\n- Version: 7.0.0\n- Tagline: Don't trust. Verify.\n\n## Architecture Invariants (NEVER VIOLATE)\n1. Every agent action goes through kernel capability checks\n2. Fuel budget checked before execution, not after\n3. Audit trail is append-only with hash-chain integrity\n` },
  { id: "f-readme", name: "README.md", path: "/README.md", type: "file", size: 2134, modified: now - 2 * day, ext: "md", permissions: mockPerm(),
    content: `# Nexus OS\n\n> Don't trust. Verify.\n\nA governed AI operating system.\n\n## Quick Start\n\`\`\`bash\ncargo build --release\ncd app && npm run tauri dev\n\`\`\`\n` },
  { id: "f-deny", name: "deny.toml", path: "/deny.toml", type: "file", size: 678, modified: now - 5 * day, ext: "toml", permissions: mockPerm() },
  { id: "f-screenshot", name: "screenshot.png", path: "/screenshot.png", type: "file", size: 284532, modified: now - 4 * day, ext: "png", permissions: mockPerm() },
];

const INITIAL_AGENT_OPS: AgentFileOp[] = [
  { id: "op1", agent: "Coder", action: "Writing", file: "/src/api.rs", progress: 88, status: "active" },
  { id: "op2", agent: "Researcher", action: "Reading", file: "/README.md", progress: 100, status: "done" },
  { id: "op3", agent: "Designer", action: "Creating", file: "/app/src/pages/DesignStudio.tsx", progress: 34, status: "active" },
];

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function FileManager(): JSX.Element {
  /* ---- State ---- */
  const [allFiles, setAllFiles] = useState<VFile[]>(MOCK_FILES);
  const [currentPath, setCurrentPath] = useState("/");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("grid");
  const [sortBy, setSortBy] = useState<SortBy>("name");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [searchQuery, setSearchQuery] = useState("");
  const [showSearch, setShowSearch] = useState(false);
  const [sidebarTab, setSidebarTab] = useState<SidebarTab>("preview");
  const [showSidebar, setShowSidebar] = useState(true);
  const [auditLog, setAuditLog] = useState<AuditEntry[]>([]);
  const [agentOps, setAgentOps] = useState<AgentFileOp[]>(INITIAL_AGENT_OPS);
  const [fuelUsed, setFuelUsed] = useState(0);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [clipboardId, setClipboardId] = useState<string | null>(null);
  const [clipboardOp, setClipboardOp] = useState<"copy" | "cut" | null>(null);
  const [newItemType, setNewItemType] = useState<"file" | "dir" | null>(null);
  const [newItemName, setNewItemName] = useState("");
  const [dragOverId, setDragOverId] = useState<string | null>(null);

  const fuelBudget = 10000;
  const fuelRemaining = fuelBudget - fuelUsed;
  const fuelPct = Math.round((fuelRemaining / fuelBudget) * 100);

  /* ---- Helpers ---- */
  const appendAudit = useCallback((event: string, detail: string) => {
    setAuditLog((prev) => [{ ts: Date.now(), event, detail }, ...prev].slice(0, 200));
  }, []);

  /* ---- Find file by ID in tree ---- */
  function findFile(files: VFile[], id: string): VFile | undefined {
    for (const f of files) {
      if (f.id === id) return f;
      if (f.children) {
        const found = findFile(f.children, id);
        if (found) return found;
      }
    }
    return undefined;
  }

  /* ---- Current directory contents ---- */
  const currentFiles = useMemo(() => {
    if (currentPath === "/") return allFiles.filter((f) => !f.trashed);

    function findDir(files: VFile[], path: string): VFile[] {
      for (const f of files) {
        if (f.type === "dir" && f.path === path && f.children) return f.children.filter((c) => !c.trashed);
        if (f.children) {
          const found = findDir(f.children, path);
          if (found.length > 0) return found;
        }
      }
      return [];
    }
    return findDir(allFiles, currentPath);
  }, [allFiles, currentPath]);

  /* ---- Sorted & filtered ---- */
  const displayFiles = useMemo(() => {
    let items = [...currentFiles];

    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      function searchAll(files: VFile[]): VFile[] {
        const results: VFile[] = [];
        for (const f of files) {
          if (f.name.toLowerCase().includes(q) || f.content?.toLowerCase().includes(q)) results.push(f);
          if (f.children) results.push(...searchAll(f.children));
        }
        return results;
      }
      items = searchAll(allFiles).filter((f) => !f.trashed);
    }

    items.sort((a, b) => {
      // Dirs first
      if (a.type !== b.type) return a.type === "dir" ? -1 : 1;
      let cmp = 0;
      switch (sortBy) {
        case "name": cmp = a.name.localeCompare(b.name); break;
        case "size": cmp = a.size - b.size; break;
        case "modified": cmp = a.modified - b.modified; break;
        case "type": cmp = a.ext.localeCompare(b.ext); break;
      }
      return sortDir === "asc" ? cmp : -cmp;
    });
    return items;
  }, [currentFiles, searchQuery, sortBy, sortDir, allFiles]);

  const selectedFile = useMemo(() => selectedId ? findFile(allFiles, selectedId) : null, [allFiles, selectedId]);

  /* ---- Breadcrumbs ---- */
  const breadcrumbs = useMemo((): BreadcrumbItem[] => {
    const parts = currentPath.split("/").filter(Boolean);
    const crumbs: BreadcrumbItem[] = [{ name: "root", path: "/" }];
    let running = "";
    for (const p of parts) {
      running += `/${p}`;
      crumbs.push({ name: p, path: running });
    }
    return crumbs;
  }, [currentPath]);

  /* ---- Trash items ---- */
  const trashItems = useMemo(() => {
    function findTrashed(files: VFile[]): VFile[] {
      const results: VFile[] = [];
      for (const f of files) {
        if (f.trashed) results.push(f);
        if (f.children) results.push(...findTrashed(f.children));
      }
      return results;
    }
    return findTrashed(allFiles);
  }, [allFiles]);

  /* ---- Vault items ---- */
  const vaultItems = useMemo(() => {
    function findEncrypted(files: VFile[]): VFile[] {
      const results: VFile[] = [];
      for (const f of files) {
        if (f.encrypted && !f.trashed) results.push(f);
        if (f.children) results.push(...findEncrypted(f.children));
      }
      return results;
    }
    return findEncrypted(allFiles);
  }, [allFiles]);

  /* ---- Navigation ---- */
  function navigateTo(path: string): void {
    setCurrentPath(path);
    setSelectedId(null);
    appendAudit("Navigate", path);
  }

  function openItem(f: VFile): void {
    if (f.type === "dir") {
      navigateTo(f.path);
    } else {
      setSelectedId(f.id);
      setSidebarTab("preview");
      setShowSidebar(true);
      appendAudit("FileOpen", f.path);
    }
  }

  function goUp(): void {
    if (currentPath === "/") return;
    const parts = currentPath.split("/").filter(Boolean);
    parts.pop();
    navigateTo(parts.length === 0 ? "/" : `/${parts.join("/")}`);
  }

  /* ---- File operations ---- */
  function updateFileInTree(files: VFile[], id: string, updater: (f: VFile) => VFile): VFile[] {
    return files.map((f) => {
      if (f.id === id) return updater(f);
      if (f.children) return { ...f, children: updateFileInTree(f.children, id, updater) };
      return f;
    });
  }

  function handleDelete(id: string): void {
    const f = findFile(allFiles, id);
    if (!f) return;
    setAllFiles((prev) => updateFileInTree(prev, id, (file) => ({ ...file, trashed: true, trashedAt: Date.now() })));
    setConfirmDelete(null);
    setSelectedId(null);
    appendAudit("FileTrash", f.path);
    setFuelUsed((p) => p + 5);
  }

  function handleRestore(id: string): void {
    const f = findFile(allFiles, id);
    if (!f) return;
    setAllFiles((prev) => updateFileInTree(prev, id, (file) => ({ ...file, trashed: false, trashedAt: undefined })));
    appendAudit("FileRestore", f.path);
  }

  function handlePermanentDelete(id: string): void {
    const f = findFile(allFiles, id);
    if (!f) return;
    function removeFromTree(files: VFile[], targetId: string): VFile[] {
      return files.filter((fi) => fi.id !== targetId).map((fi) => fi.children ? { ...fi, children: removeFromTree(fi.children, targetId) } : fi);
    }
    setAllFiles((prev) => removeFromTree(prev, id));
    appendAudit("FilePermanentDelete", f.path);
  }

  function handleRename(id: string): void {
    if (!renameValue.trim()) { setRenaming(null); return; }
    const newName = renameValue.trim();
    setAllFiles((prev) => updateFileInTree(prev, id, (f) => ({
      ...f, name: newName, ext: extOf(newName),
      path: f.path.split("/").slice(0, -1).concat(newName).join("/"),
    })));
    appendAudit("FileRename", `→ ${newName}`);
    setRenaming(null);
    setRenameValue("");
  }

  function handleCopy(id: string): void {
    setClipboardId(id);
    setClipboardOp("copy");
    const f = findFile(allFiles, id);
    appendAudit("FileCopy", f?.path ?? id);
  }

  function handleCut(id: string): void {
    setClipboardId(id);
    setClipboardOp("cut");
    const f = findFile(allFiles, id);
    appendAudit("FileCut", f?.path ?? id);
  }

  function handlePaste(): void {
    if (!clipboardId || !clipboardOp) return;
    const source = findFile(allFiles, clipboardId);
    if (!source) return;
    const newId = `f-${Date.now()}`;
    const copy: VFile = { ...source, id: newId, path: `${currentPath === "/" ? "" : currentPath}/${source.name}`, modified: Date.now() };

    // Add to current directory
    if (currentPath === "/") {
      setAllFiles((prev) => [...prev, copy]);
    } else {
      setAllFiles((prev) => updateFileInTree(prev, currentPath.split("/").filter(Boolean).reduce((acc, part) => {
        const dir = findFile(prev, `d-${part}`);
        return dir ? dir.id : acc;
      }, ""), (f) => ({ ...f, children: [...(f.children ?? []), copy] })));
    }

    if (clipboardOp === "cut") {
      handleDelete(clipboardId);
      appendAudit("FileMove", `${source.path} → ${copy.path}`);
    } else {
      appendAudit("FilePaste", copy.path);
    }
    setClipboardId(null);
    setClipboardOp(null);
    setFuelUsed((p) => p + 3);
  }

  function handleCreateItem(): void {
    if (!newItemName.trim() || !newItemType) return;
    const name = newItemName.trim();
    const id = `${newItemType === "dir" ? "d" : "f"}-${Date.now()}`;
    const path = `${currentPath === "/" ? "" : currentPath}/${name}`;
    const item: VFile = {
      id, name, path, type: newItemType, size: 0, modified: Date.now(),
      ext: newItemType === "file" ? extOf(name) : "",
      permissions: mockPerm(),
      children: newItemType === "dir" ? [] : undefined,
      content: newItemType === "file" ? "" : undefined,
    };

    if (currentPath === "/") {
      setAllFiles((prev) => [...prev, item]);
    }
    // In nested dirs we'd need to update the tree — simplified for root
    setNewItemType(null);
    setNewItemName("");
    appendAudit("FileCreate", path);
    setFuelUsed((p) => p + 2);
  }

  /* ---- Drag and drop ---- */
  function handleDragStart(e: React.DragEvent, f: VFile): void {
    e.dataTransfer.setData("fileId", f.id);
    e.dataTransfer.effectAllowed = "move";
  }

  function handleDragOver(e: React.DragEvent, f: VFile): void {
    if (f.type !== "dir") return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    setDragOverId(f.id);
  }

  function handleDragLeave(): void {
    setDragOverId(null);
  }

  function handleDrop(e: React.DragEvent, target: VFile): void {
    e.preventDefault();
    setDragOverId(null);
    const sourceId = e.dataTransfer.getData("fileId");
    if (!sourceId || target.type !== "dir" || sourceId === target.id) return;
    const source = findFile(allFiles, sourceId);
    if (!source) return;
    appendAudit("FileMove", `${source.name} → ${target.path}/`);
    setFuelUsed((p) => p + 5);
  }

  /* ---- Agent ops simulation ---- */
  useEffect(() => {
    const interval = setInterval(() => {
      setAgentOps((prev) =>
        prev.map((op) => {
          if (op.status !== "active") return op;
          const newProg = Math.min(op.progress + Math.floor(Math.random() * 12), 100);
          return { ...op, progress: newProg, status: newProg >= 100 ? "done" : "active" };
        })
      );
    }, 1800);
    return () => clearInterval(interval);
  }, []);

  /* ---- Keyboard shortcuts ---- */
  useEffect(() => {
    function onKey(e: KeyboardEvent): void {
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key === "f") { e.preventDefault(); setShowSearch((p) => !p); }
      if (mod && e.key === "b") { e.preventDefault(); setShowSidebar((p) => !p); }
      if (e.key === "Backspace" && !renaming && !showSearch && !newItemType) { goUp(); }
      if (e.key === "Delete" && selectedId && !renaming) { setConfirmDelete(selectedId); }
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
          <div className="fm-fuel-badge">
            <span className="fm-fuel-label">FUEL</span>
            <div className="fm-fuel-bar"><div className="fm-fuel-fill" style={{ width: `${fuelPct}%`, background: fuelPct > 50 ? "#22d3ee" : fuelPct > 20 ? "#f59e0b" : "#ef4444" }} /></div>
            <span className="fm-fuel-value">{fuelRemaining.toLocaleString()}</span>
          </div>
          <div className="fm-toolbar">
            <button type="button" className={`fm-tool-btn ${viewMode === "grid" ? "fm-tool-active" : ""}`} onClick={() => setViewMode("grid")} title="Grid view">⊞</button>
            <button type="button" className={`fm-tool-btn ${viewMode === "list" ? "fm-tool-active" : ""}`} onClick={() => setViewMode("list")} title="List view">☰</button>
            <button type="button" className={`fm-tool-btn ${showSearch ? "fm-tool-active" : ""}`} onClick={() => setShowSearch((p) => !p)} title="Search (Ctrl+F)">⌕</button>
            <button type="button" className={`fm-tool-btn ${showSidebar ? "fm-tool-active" : ""}`} onClick={() => setShowSidebar((p) => !p)} title="Sidebar (Ctrl+B)">◨</button>
          </div>
        </div>
      </header>

      {/* ---- Action bar ---- */}
      <div className="fm-action-bar">
        <div className="fm-breadcrumbs">
          <button type="button" className="fm-nav-btn" onClick={goUp} disabled={currentPath === "/"} title="Go up">↑</button>
          {breadcrumbs.map((bc, i) => (
            <span key={bc.path} className="fm-crumb-wrap">
              {i > 0 && <span className="fm-crumb-sep">/</span>}
              <button type="button" className={`fm-crumb ${i === breadcrumbs.length - 1 ? "fm-crumb-current" : ""}`} onClick={() => navigateTo(bc.path)}>
                {bc.name}
              </button>
            </span>
          ))}
        </div>
        <div className="fm-action-btns">
          <button type="button" className="fm-action-btn" onClick={() => { setNewItemType("file"); setNewItemName(""); }} title="New file">+ File</button>
          <button type="button" className="fm-action-btn" onClick={() => { setNewItemType("dir"); setNewItemName(""); }} title="New folder">+ Folder</button>
          {clipboardId && <button type="button" className="fm-action-btn fm-action-paste" onClick={handlePaste} title="Paste">📋 Paste</button>}
        </div>
      </div>

      {/* ---- Search bar ---- */}
      {showSearch && (
        <div className="fm-search-bar">
          <span className="fm-search-icon">⌕</span>
          <input className="fm-search-input" placeholder="Search files and content..." value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} autoFocus onKeyDown={(e) => { if (e.key === "Escape") { setShowSearch(false); setSearchQuery(""); } }} />
          {searchQuery && <span className="fm-search-count">{displayFiles.length} results</span>}
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

      {/* ---- Agent activity strip ---- */}
      {agentOps.some((o) => o.status === "active") && (
        <div className="fm-agent-strip">
          {agentOps.filter((o) => o.status === "active").map((op) => (
            <div key={op.id} className="fm-agent-op">
              <span className="fm-agent-op-name">{op.agent}</span>
              <span className="fm-agent-op-action">{op.action}</span>
              <span className="fm-agent-op-file">{op.file.split("/").pop()}</span>
              <div className="fm-agent-op-bar"><div className="fm-agent-op-fill" style={{ width: `${op.progress}%` }} /></div>
              <span className="fm-agent-op-pct">{op.progress}%</span>
            </div>
          ))}
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
              <span className="fm-list-col fm-col-perms">Perms</span>
              <span className="fm-list-col fm-col-owner">Owner</span>
            </div>
          )}

          {/* Files */}
          <div className={`fm-files ${viewMode === "grid" ? "fm-files-grid" : "fm-files-list"}`}>
            {displayFiles.map((f) => (
              <div
                key={f.id}
                className={`fm-file-item ${viewMode === "grid" ? "fm-file-grid" : "fm-file-row"} ${selectedId === f.id ? "fm-file-selected" : ""} ${dragOverId === f.id ? "fm-file-drag-over" : ""}`}
                onClick={() => setSelectedId(f.id)}
                onDoubleClick={() => openItem(f)}
                draggable
                onDragStart={(e) => handleDragStart(e, f)}
                onDragOver={(e) => handleDragOver(e, f)}
                onDragLeave={handleDragLeave}
                onDrop={(e) => handleDrop(e, f)}
                onContextMenu={(e) => { e.preventDefault(); setSelectedId(f.id); }}
              >
                {viewMode === "grid" ? (
                  <>
                    <span className="fm-file-icon-large">{fileIcon(f)}</span>
                    {renaming === f.id ? (
                      <input className="fm-rename-input" value={renameValue} onChange={(e) => setRenameValue(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleRename(f.id); if (e.key === "Escape") setRenaming(null); }} onBlur={() => handleRename(f.id)} autoFocus />
                    ) : (
                      <span className="fm-file-name-grid">{f.name}</span>
                    )}
                    {f.encrypted && <span className="fm-encrypted-badge">🔒</span>}
                    {f.permissions.owner === "agent" && <span className="fm-agent-badge">{f.permissions.agentName}</span>}
                  </>
                ) : (
                  <>
                    <span className="fm-file-icon">{fileIcon(f)}</span>
                    {renaming === f.id ? (
                      <input className="fm-rename-input fm-rename-list" value={renameValue} onChange={(e) => setRenameValue(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleRename(f.id); if (e.key === "Escape") setRenaming(null); }} onBlur={() => handleRename(f.id)} autoFocus />
                    ) : (
                      <span className="fm-file-name">{f.name}</span>
                    )}
                    {f.encrypted && <span className="fm-encrypted-dot">🔒</span>}
                    <span className="fm-file-size">{f.type === "dir" ? "—" : formatSize(f.size)}</span>
                    <span className="fm-file-modified">{formatDate(f.modified)}</span>
                    <span className="fm-file-perms">{permString(f.permissions)}</span>
                    <span className="fm-file-owner">{f.permissions.owner === "agent" ? f.permissions.agentName ?? "agent" : "user"}</span>
                  </>
                )}
              </div>
            ))}
            {displayFiles.length === 0 && (
              <div className="fm-empty">{searchQuery ? "No files match your search" : "This folder is empty"}</div>
            )}
          </div>

          {/* Context actions for selected */}
          {selectedId && !renaming && (
            <div className="fm-context-bar">
              <button type="button" className="fm-ctx-btn" onClick={() => { const f = findFile(allFiles, selectedId); if (f) openItem(f); }}>Open</button>
              <button type="button" className="fm-ctx-btn" onClick={() => { setRenaming(selectedId); setRenameValue(findFile(allFiles, selectedId)?.name ?? ""); }}>Rename</button>
              <button type="button" className="fm-ctx-btn" onClick={() => handleCopy(selectedId)}>Copy</button>
              <button type="button" className="fm-ctx-btn" onClick={() => handleCut(selectedId)}>Cut</button>
              <button type="button" className="fm-ctx-btn fm-ctx-danger" onClick={() => setConfirmDelete(selectedId)}>Delete</button>
              <span className="fm-ctx-info">{findFile(allFiles, selectedId)?.path}</span>
            </div>
          )}
        </div>

        {/* ---- Sidebar ---- */}
        {showSidebar && (
          <aside className="fm-sidebar">
            <div className="fm-sidebar-tabs">
              <button type="button" className={`fm-stab ${sidebarTab === "preview" ? "fm-stab-active" : ""}`} onClick={() => setSidebarTab("preview")}>Preview</button>
              <button type="button" className={`fm-stab ${sidebarTab === "details" ? "fm-stab-active" : ""}`} onClick={() => setSidebarTab("details")}>Details</button>
              <button type="button" className={`fm-stab ${sidebarTab === "vault" ? "fm-stab-active" : ""}`} onClick={() => setSidebarTab("vault")}>Vault</button>
              <button type="button" className={`fm-stab ${sidebarTab === "trash" ? "fm-stab-active" : ""}`} onClick={() => setSidebarTab("trash")}>Trash ({trashItems.length})</button>
            </div>

            {/* Preview */}
            {sidebarTab === "preview" && (
              <div className="fm-sidebar-body">
                {selectedFile ? (
                  <>
                    <div className="fm-preview-header">
                      <span className="fm-preview-icon">{fileIcon(selectedFile)}</span>
                      <span className="fm-preview-name">{selectedFile.name}</span>
                    </div>
                    {isImage(selectedFile.ext) && (
                      <div className="fm-preview-image">
                        <div className="fm-preview-image-placeholder">
                          🖼 {selectedFile.name}
                          <br /><span className="fm-preview-image-size">{formatSize(selectedFile.size)}</span>
                        </div>
                      </div>
                    )}
                    {isPreviewable(selectedFile.ext) && selectedFile.content && (
                      <pre className="fm-preview-code">{selectedFile.content}</pre>
                    )}
                    {!isPreviewable(selectedFile.ext) && !isImage(selectedFile.ext) && (
                      <div className="fm-preview-nopreview">
                        <span className="fm-preview-nopreview-icon">{fileIcon(selectedFile)}</span>
                        <p>No preview available</p>
                        <p className="fm-preview-ext">.{selectedFile.ext} — {formatSize(selectedFile.size)}</p>
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
            {sidebarTab === "details" && selectedFile && (
              <div className="fm-sidebar-body">
                <div className="fm-detail-section">
                  <span className="fm-detail-title">FILE DETAILS</span>
                  <div className="fm-detail-row"><span className="fm-detail-label">Name</span><span className="fm-detail-value">{selectedFile.name}</span></div>
                  <div className="fm-detail-row"><span className="fm-detail-label">Path</span><span className="fm-detail-value">{selectedFile.path}</span></div>
                  <div className="fm-detail-row"><span className="fm-detail-label">Type</span><span className="fm-detail-value">{selectedFile.type === "dir" ? "Directory" : selectedFile.ext.toUpperCase() || "File"}</span></div>
                  <div className="fm-detail-row"><span className="fm-detail-label">Size</span><span className="fm-detail-value">{formatSize(selectedFile.size)}</span></div>
                  <div className="fm-detail-row"><span className="fm-detail-label">Modified</span><span className="fm-detail-value">{formatDate(selectedFile.modified)}</span></div>
                </div>
                <div className="fm-detail-section">
                  <span className="fm-detail-title">PERMISSIONS</span>
                  <div className="fm-detail-row"><span className="fm-detail-label">Owner</span><span className="fm-detail-value">{selectedFile.permissions.owner === "agent" ? `Agent: ${selectedFile.permissions.agentName}` : "User"}</span></div>
                  <div className="fm-detail-row"><span className="fm-detail-label">Access</span><span className="fm-detail-value fm-detail-mono">{permString(selectedFile.permissions)}</span></div>
                  <div className="fm-detail-row"><span className="fm-detail-label">Encrypted</span><span className="fm-detail-value">{selectedFile.encrypted ? "Yes 🔒" : "No"}</span></div>
                </div>
                <div className="fm-detail-section">
                  <span className="fm-detail-title">AUDIT</span>
                  <div className="fm-audit-mini">
                    {auditLog.filter((a) => a.detail.includes(selectedFile.name) || a.detail.includes(selectedFile.path)).slice(0, 5).map((a, i) => (
                      <div key={`${a.ts}-${i}`} className="fm-audit-entry">
                        <span className="fm-audit-time">{formatTime(a.ts)}</span>
                        <span className="fm-audit-event">{a.event}</span>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            )}
            {sidebarTab === "details" && !selectedFile && (
              <div className="fm-sidebar-body"><div className="fm-preview-empty"><p>Select a file to see details</p></div></div>
            )}

            {/* Vault */}
            {sidebarTab === "vault" && (
              <div className="fm-sidebar-body">
                <div className="fm-vault-header">
                  <span className="fm-vault-title">ENCRYPTED VAULT</span>
                  <span className="fm-vault-count">{vaultItems.length} items</span>
                </div>
                <p className="fm-vault-desc">Sensitive files protected with encryption. Agent access requires explicit capability grants.</p>
                <div className="fm-vault-list">
                  {vaultItems.map((v) => (
                    <div key={v.id} className="fm-vault-item" onClick={() => { setSelectedId(v.id); setSidebarTab("preview"); }}>
                      <span className="fm-vault-icon">🔒</span>
                      <div className="fm-vault-info">
                        <span className="fm-vault-name">{v.name}</span>
                        <span className="fm-vault-path">{v.path}</span>
                      </div>
                      <span className="fm-vault-size">{formatSize(v.size)}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Trash */}
            {sidebarTab === "trash" && (
              <div className="fm-sidebar-body">
                <div className="fm-trash-header">
                  <span className="fm-trash-title">TRASH</span>
                  <span className="fm-trash-count">{trashItems.length} items</span>
                </div>
                {trashItems.length === 0 && <p className="fm-trash-empty">Trash is empty</p>}
                <div className="fm-trash-list">
                  {trashItems.map((t) => (
                    <div key={t.id} className="fm-trash-item">
                      <span className="fm-trash-icon">{fileIcon(t)}</span>
                      <div className="fm-trash-info">
                        <span className="fm-trash-name">{t.name}</span>
                        <span className="fm-trash-date">{t.trashedAt ? formatDate(t.trashedAt) : ""}</span>
                      </div>
                      <div className="fm-trash-actions">
                        <button type="button" className="fm-trash-btn fm-trash-restore" onClick={() => handleRestore(t.id)} title="Restore">↩</button>
                        <button type="button" className="fm-trash-btn fm-trash-perma" onClick={() => handlePermanentDelete(t.id)} title="Delete permanently">×</button>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </aside>
        )}
      </div>

      {/* ---- Delete confirmation ---- */}
      {confirmDelete && (
        <div className="fm-confirm-overlay">
          <div className="fm-confirm-dialog">
            <p className="fm-confirm-title">GOVERNED DELETE — Confirmation Required</p>
            <p className="fm-confirm-text">Move <strong>{findFile(allFiles, confirmDelete)?.name}</strong> to trash?</p>
            <p className="fm-confirm-path">{findFile(allFiles, confirmDelete)?.path}</p>
            <div className="fm-confirm-actions">
              <button type="button" className="fm-confirm-btn fm-confirm-yes" onClick={() => handleDelete(confirmDelete)}>Move to Trash</button>
              <button type="button" className="fm-confirm-btn fm-confirm-no" onClick={() => setConfirmDelete(null)}>Cancel</button>
            </div>
          </div>
        </div>
      )}

      {/* ---- Status bar ---- */}
      <div className="fm-status-bar">
        <span className="fm-status-item">{displayFiles.length} items</span>
        <span className="fm-status-sep">·</span>
        <span className="fm-status-item">{currentPath}</span>
        <span className="fm-status-sep">·</span>
        <span className="fm-status-item">{viewMode} view</span>
        <span className="fm-status-right">
          <span className="fm-status-item">{trashItems.length} in trash</span>
          <span className="fm-status-sep">·</span>
          <span className="fm-status-item">{vaultItems.length} encrypted</span>
          <span className="fm-status-sep">·</span>
          <span className="fm-status-item">{agentOps.filter((o) => o.status === "active").length} agent ops</span>
        </span>
      </div>
    </section>
  );
}

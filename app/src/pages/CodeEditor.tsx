import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Editor, { type OnMount } from "@monaco-editor/react";
import {
  hasDesktopRuntime,
  sendChat,
  terminalExecute,
  terminalExecuteApproved,
  type TerminalCommandResult,
} from "../api/backend";
import {
  Save, Search, Menu, Columns2, TerminalSquare, GitBranch, Hexagon,
  HelpCircle, RotateCcw, Stethoscope, TestTube2, FileText, Zap,
  ArrowRight, ShieldCheck, Plus, Keyboard, ChevronDown, ChevronRight,
  ArrowDown, ArrowUp, RefreshCw, Circle,
} from "lucide-react";
import "./code-editor.css";

/* ================================================================== */
/*  Tauri invoke                                                       */
/* ================================================================== */

const HAS_DESKTOP = hasDesktopRuntime();

// eslint-disable-next-line @typescript-eslint/no-explicit-any
async function invoke(cmd: string, args?: Record<string, unknown>): Promise<any> {
  if (
    typeof window !== "undefined" &&
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    typeof (window as any).__TAURI__?.invoke === "function"
  ) {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    return (window as any).__TAURI__.invoke(cmd, args);
  }
  return JSON.stringify([]);
}

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface VirtualFile {
  id: string;
  name: string;
  path: string;
  language: string;
  content: string;
  dirty: boolean;
}

interface FsEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified: number;
}

interface FileTreeNode {
  name: string;
  path: string;
  type: "file" | "dir";
  children?: FileTreeNode[];
}

interface AgentAction {
  id: string;
  label: string;
  description: string;
  icon: React.ReactNode;
}

interface AuditEntry {
  ts: number;
  event: string;
  detail: string;
}

interface TerminalLine {
  id: number;
  type: "input" | "output" | "error" | "system";
  text: string;
  ts: number;
}

interface GitChange {
  file: string;
  status: "modified" | "added" | "deleted" | "untracked";
}

interface GitCommit {
  hash: string;
  message: string;
  author: string;
  ts: number;
}

interface GitRepoStatus {
  detected: boolean;
  root: string | null;
  branch: string | null;
  changes: GitChange[];
  commits: GitCommit[];
}

interface AgentWorker {
  id: string;
  name: string;
  file: string;
  action: string;
  progress: number;
  status: "working" | "waiting" | "done" | "error";
  fuelUsed: number;
}

type AgentPanelMode = "idle" | "thinking" | "result";
type BottomPanel = "terminal" | "git" | "agents" | "none";
type SplitView = "off" | "suggestion";
type ApprovalState = { cmd: string; cwd: string } | null;

/* ================================================================== */
/*  Language detection                                                  */
/* ================================================================== */

function detectLanguage(filename: string): string {
  const ext = filename.split(".").pop()?.toLowerCase() ?? "";
  const map: Record<string, string> = {
    ts: "typescript", tsx: "typescript", js: "javascript", jsx: "javascript",
    rs: "rust", py: "python", json: "json", css: "css", html: "html",
    md: "markdown", toml: "toml", yaml: "yaml", yml: "yaml",
    sh: "shell", bash: "shell", sql: "sql", go: "go", c: "c",
    cpp: "cpp", h: "c", hpp: "cpp", java: "java", rb: "ruby",
    php: "php", swift: "swift", kt: "kotlin", dart: "dart",
    vue: "html", svelte: "html", scss: "scss", less: "less",
    xml: "xml", svg: "xml", graphql: "graphql", gql: "graphql",
    dockerfile: "dockerfile", makefile: "makefile", tf: "hcl",
    proto: "protobuf", r: "r", lua: "lua", zig: "rust",
  };
  return map[ext] ?? "plaintext";
}

function langIcon(lang: string): string {
  const icons: Record<string, string> = {
    rust: "Rs", typescript: "TS", javascript: "JS", python: "Py",
    json: "{}", css: "#", html: "<>", markdown: "Md", toml: "Tm",
    yaml: "Ym", shell: "$", sql: "Sq", go: "Go", plaintext: "Tx",
    c: "C", cpp: "C+", java: "Jv", ruby: "Rb", php: "Ph",
    swift: "Sw", kotlin: "Kt", dart: "Da", scss: "Sc",
  };
  return icons[lang] ?? "..";
}

/* ================================================================== */
/*  Helpers                                                            */
/* ================================================================== */

function pathBasename(p: string): string {
  const idx = p.lastIndexOf("/");
  return idx >= 0 ? p.slice(idx + 1) : p;
}

function pathJoin(base: string, name: string): string {
  if (base.endsWith("/")) return base + name;
  return base + "/" + name;
}

/* ================================================================== */
/*  Agent actions (requires LLM provider)                              */
/* ================================================================== */

const AGENT_ACTIONS: AgentAction[] = [
  { id: "explain", label: "Explain", description: "Explain selected code", icon: <HelpCircle size={14} /> },
  { id: "refactor", label: "Refactor", description: "Suggest improvements", icon: <RotateCcw size={14} /> },
  { id: "fix", label: "Fix Bugs", description: "Find and fix issues", icon: <Stethoscope size={14} /> },
  { id: "test", label: "Gen Tests", description: "Generate unit tests", icon: <TestTube2 size={14} /> },
  { id: "document", label: "Document", description: "Add documentation", icon: <FileText size={14} /> },
  { id: "optimize", label: "Optimize", description: "Performance improvements", icon: <Zap size={14} /> },
  { id: "complete", label: "Complete", description: "Auto-complete code block", icon: <ArrowRight size={14} /> },
  { id: "review", label: "Review", description: "Security & quality review", icon: <ShieldCheck size={14} /> },
];

const DANGEROUS_COMMANDS = ["rm -rf", "sudo rm", "mkfs", "dd if=", ":(){:|:&};:", "chmod 777", "FORMAT", "shutdown", "reboot", "kill -9 1"];

/** File extensions safe to open in the editor (text-based) */
const EDITABLE_EXTS = new Set([
  "rs", "ts", "tsx", "js", "jsx", "py", "json", "css", "html", "md", "toml",
  "yaml", "yml", "sh", "bash", "sql", "go", "c", "cpp", "h", "hpp", "java",
  "rb", "php", "swift", "kt", "dart", "vue", "svelte", "scss", "less", "xml",
  "svg", "graphql", "gql", "dockerfile", "makefile", "tf", "proto", "r",
  "lua", "zig", "txt", "csv", "lock", "cfg", "conf", "log", "env", "gitignore",
]);

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function CodeEditor(): JSX.Element {
  /* ---- State ---- */
  const [files, setFiles] = useState<VirtualFile[]>([]);
  const [openTabs, setOpenTabs] = useState<string[]>([]);
  const [activeTab, setActiveTab] = useState("");
  const [showExplorer, setShowExplorer] = useState(true);
  const [showAgent, setShowAgent] = useState(true);
  const [agentMode, setAgentMode] = useState<AgentPanelMode>("idle");
  const [agentResult, setAgentResult] = useState("");
  const [agentAction, setAgentAction] = useState("");
  const [fuelUsed, setFuelUsed] = useState(700);
  const [auditLog, setAuditLog] = useState<AuditEntry[]>([]);
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());
  const [newFileName, setNewFileName] = useState("");
  const [showNewFile, setShowNewFile] = useState(false);
  const [splitView, setSplitView] = useState<SplitView>("off");
  const [bottomPanel, setBottomPanel] = useState<BottomPanel>("none");
  const [terminalLines, setTerminalLines] = useState<TerminalLine[]>([
    { id: 0, type: "system", text: "Nexus OS Terminal v9.0 — governed shell", ts: Date.now() },
    { id: 1, type: "system", text: "Type commands below. Dangerous operations require approval.", ts: Date.now() },
  ]);
  const [terminalInput, setTerminalInput] = useState("");
  const [gitRepo, setGitRepo] = useState<GitRepoStatus>({
    detected: false,
    root: null,
    branch: null,
    changes: [],
    commits: [],
  });
  const [commitMsg, setCommitMsg] = useState("");
  const [agentWorkers, setAgentWorkers] = useState<AgentWorker[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [showSearch, setShowSearch] = useState(false);
  const [pendingApproval, setPendingApproval] = useState<ApprovalState>(null);

  /* ---- Filesystem state ---- */
  const [rootPath, setRootPath] = useState<string>("");
  const [fileTree, setFileTree] = useState<FileTreeNode[]>([]);
  const [fsLoading, setFsLoading] = useState(false);
  const [fsError, setFsError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const termRef = useRef<HTMLDivElement>(null);
  const termLineId = useRef(2);
  const scrollTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (scrollTimerRef.current) clearTimeout(scrollTimerRef.current);
    };
  }, []);

  const activeFile = useMemo(() => files.find((f) => f.id === activeTab), [files, activeTab]);
  const fuelBudget = 10000;
  const fuelRemaining = fuelBudget - fuelUsed;
  const fuelPct = Math.round((fuelRemaining / fuelBudget) * 100);
  const gitBranch = gitRepo.branch ?? "No git repo detected";
  const terminalCwd = useMemo(() => {
    if (activeFile?.path.includes("/")) {
      return activeFile.path.slice(0, activeFile.path.lastIndexOf("/")) || "/";
    }
    return gitRepo.root ?? rootPath ?? "/home/nexus/NEXUS/nexus-os";
  }, [activeFile?.path, gitRepo.root, rootPath]);

  /* ---- Audit helper ---- */
  const appendAudit = useCallback((event: string, detail: string) => {
    setAuditLog((prev) => [{ ts: Date.now(), event, detail }, ...prev].slice(0, 100));
  }, []);

  /* ================================================================ */
  /*  Filesystem integration                                           */
  /* ================================================================ */

  /** Load a directory listing from the Tauri backend and build tree nodes */
  const loadDirEntries = useCallback(async (dirPath: string): Promise<FileTreeNode[]> => {
    if (!HAS_DESKTOP) return [];
    try {
      const raw: string = await invoke("file_manager_list", { path: dirPath });
      const entries: FsEntry[] = JSON.parse(raw);
      // Sort: dirs first, then alphabetical
      entries.sort((a, b) => {
        if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
      return entries
        .filter((e) => !e.name.startsWith(".")) // hide dotfiles by default
        .map((e) => ({
          name: e.name,
          path: e.path,
          type: e.is_dir ? "dir" as const : "file" as const,
        }));
    } catch {
      return [];
    }
  }, []);

  /** Initialize: determine root path and load top-level tree */
  useEffect(() => {
    if (!HAS_DESKTOP) {
      setFsError("Running in browser mode — files are not persisted.");
      return;
    }
    (async () => {
      setFsLoading(true);
      try {
        const home: string = await invoke("file_manager_home");
        setRootPath(home);
        const nodes = await loadDirEntries(home);
        setFileTree(nodes);
        appendAudit("EditorInit", `Loaded ${home}`);
      } catch {
        setFsError("Could not load filesystem. Tauri backend unavailable.");
      } finally {
        setFsLoading(false);
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const refreshGitStatus = useCallback(async () => {
    if (!HAS_DESKTOP) {
      setGitRepo({
        detected: false,
        root: null,
        branch: null,
        changes: [],
        commits: [],
      });
      return;
    }
    try {
      const repo = await invoke("get_git_repo_status");
      setGitRepo(repo as GitRepoStatus);
    } catch {
      setGitRepo({
        detected: false,
        root: null,
        branch: null,
        changes: [],
        commits: [],
      });
    }
  }, []);

  useEffect(() => {
    void refreshGitStatus();
  }, [refreshGitStatus]);

  /** Expand a directory in the tree — lazily load its children */
  const expandDir = useCallback(async (dirPath: string) => {
    const children = await loadDirEntries(dirPath);
    setFileTree((prev) => {
      // Recursively find the node and attach children
      function attach(nodes: FileTreeNode[]): FileTreeNode[] {
        return nodes.map((n) => {
          if (n.path === dirPath) return { ...n, children };
          if (n.children) return { ...n, children: attach(n.children) };
          return n;
        });
      }
      return attach(prev);
    });
  }, [loadDirEntries]);

  /** Open a real file from the filesystem */
  const openRealFile = useCallback(async (filePath: string) => {
    // Check if already loaded
    const existing = files.find((f) => f.path === filePath);
    if (existing) {
      if (!openTabs.includes(existing.id)) setOpenTabs((prev) => [...prev, existing.id]);
      setActiveTab(existing.id);
      appendAudit("FileOpen", existing.name);
      return;
    }

    const name = pathBasename(filePath);
    const ext = name.split(".").pop()?.toLowerCase() ?? "";
    if (!EDITABLE_EXTS.has(ext) && ext !== "") {
      setFsError(`Cannot open binary file: ${name}`);
      return;
    }

    try {
      let content: string;
      if (HAS_DESKTOP) {
        content = await invoke("file_manager_read", { path: filePath });
      } else {
        content = "";
      }
      const id = `fs_${Date.now()}_${Math.random().toString(36).slice(2, 6)}`;
      const lang = detectLanguage(name);
      const newFile: VirtualFile = { id, name, path: filePath, language: lang, content, dirty: false };
      setFiles((prev) => [...prev, newFile]);
      setOpenTabs((prev) => [...prev, id]);
      setActiveTab(id);
      appendAudit("FileOpen", filePath);
    } catch (e) {
      setFsError(`Failed to read ${filePath}: ${e}`);
    }
  }, [files, openTabs, appendAudit]);

  /** Save current file to disk */
  const saveFile = useCallback(async () => {
    if (!activeFile) return;
    if (!HAS_DESKTOP) {
      // Browser mode — just mark clean
      setFiles((prev) => prev.map((f) => (f.id === activeTab ? { ...f, dirty: false } : f)));
      appendAudit("FileSave", `${activeFile.name} (in-memory only)`);
      return;
    }
    setSaving(true);
    try {
      await invoke("file_manager_write", { path: activeFile.path, content: activeFile.content });
      setFiles((prev) => prev.map((f) => (f.id === activeTab ? { ...f, dirty: false } : f)));
      appendAudit("FileSave", activeFile.path);
    } catch (e) {
      setFsError(`Failed to save ${activeFile.path}: ${e}`);
    } finally {
      setSaving(false);
    }
  }, [activeFile, activeTab, appendAudit]);

  /* ---- File operations ---- */
  function openFile(fileId: string): void {
    if (!openTabs.includes(fileId)) setOpenTabs((prev) => [...prev, fileId]);
    setActiveTab(fileId);
    appendAudit("FileOpen", files.find((f) => f.id === fileId)?.name ?? fileId);
  }

  function closeTab(fileId: string): void {
    setOpenTabs((prev) => {
      const next = prev.filter((id) => id !== fileId);
      if (activeTab === fileId && next.length > 0) setActiveTab(next[next.length - 1]);
      if (next.length === 0) setActiveTab("");
      return next;
    });
  }

  function handleEditorChange(value: string | undefined): void {
    if (!value || !activeTab) return;
    setFiles((prev) => prev.map((f) => (f.id === activeTab ? { ...f, content: value, dirty: true } : f)));
  }

  function handleCreateFile(): void {
    if (!newFileName.trim()) return;
    const name = newFileName.trim();
    const filePath = rootPath ? pathJoin(rootPath, name) : name;
    const id = `f${Date.now()}`;
    const lang = detectLanguage(name);
    const newFile: VirtualFile = { id, name, path: filePath, language: lang, content: "", dirty: true };
    setFiles((prev) => [...prev, newFile]);
    setOpenTabs((prev) => [...prev, id]);
    setActiveTab(id);
    setNewFileName("");
    setShowNewFile(false);
    appendAudit("FileCreate", name);
  }

  function handleDeleteFile(fileId: string): void {
    const file = files.find((f) => f.id === fileId);
    if (!file) return;
    setFiles((prev) => prev.filter((f) => f.id !== fileId));
    closeTab(fileId);
    appendAudit("FileDelete", file.name);
  }

  /* ---- Terminal ---- */
  function addTermLine(type: TerminalLine["type"], text: string): void {
    const id = termLineId.current++;
    setTerminalLines((prev) => [...prev, { id, type, text, ts: Date.now() }]);
    if (scrollTimerRef.current) clearTimeout(scrollTimerRef.current);
    scrollTimerRef.current = setTimeout(() => termRef.current?.scrollTo(0, termRef.current.scrollHeight), 50);
  }

  function applyTerminalResult(result: TerminalCommandResult): void {
    setFuelUsed((prev) => prev + result.fuel_cost);
    if (result.stdout) {
      result.stdout
        .trimEnd()
        .split("\n")
        .filter((line) => line.length > 0)
        .forEach((line) => addTermLine("output", line));
    }
    if (result.stderr) {
      result.stderr
        .trimEnd()
        .split("\n")
        .filter((line) => line.length > 0)
        .forEach((line) => addTermLine("error", line));
    }
    if (result.exit_code !== 0) {
      addTermLine("error", `[exit code ${result.exit_code}] (${result.duration_ms}ms)`);
    }
  }

  async function runTerminalCommand(
    cmd: string,
    approved: boolean = false,
    cwdOverride?: string,
  ): Promise<void> {
    const cwd = cwdOverride ?? terminalCwd;
    try {
      if (approved) {
        setPendingApproval(null);
      }
      const result = approved
        ? await terminalExecuteApproved(cmd, cwd)
        : await terminalExecute(cmd, cwd);
      if (result.needs_approval) {
        setPendingApproval({ cmd, cwd });
        addTermLine("error", `[APPROVAL REQUIRED] ${cmd}`);
        appendAudit("TermHITL", `${cmd} — ${result.tool}`);
        return;
      }
      applyTerminalResult(result);
    } catch (error) {
      addTermLine("error", `Command failed: ${error instanceof Error ? error.message : String(error)}`);
      appendAudit("TermError", `${cmd}`);
    }
  }

  function denyPendingCommand(): void {
    if (!pendingApproval) {
      return;
    }
    appendAudit("TermDenied", pendingApproval.cmd);
    setPendingApproval(null);
    addTermLine("system", "Command denied.");
  }

  function handleTerminalSubmit(): void {
    const cmd = terminalInput.trim();
    if (!cmd) return;
    addTermLine("input", `$ ${cmd}`);
    setTerminalInput("");
    setPendingApproval(null);

    // Check for dangerous commands
    const isDangerous = DANGEROUS_COMMANDS.some((d) => cmd.toLowerCase().includes(d.toLowerCase()));
    if (isDangerous) {
      addTermLine("error", `[BLOCKED] Command requires Tier2+ HITL approval: "${cmd}"`);
      appendAudit("TermBlocked", cmd);
      return;
    }

    appendAudit("TermExec", cmd);

    if (cmd === "clear") {
      setTerminalLines([]);
      return;
    }

    if (cmd === "help") {
      addTermLine(
        "system",
        "Editor shell runs through terminal_execute. Try: ls, pwd, git status, cargo test, npm run build.",
      );
      return;
    }

    if (!HAS_DESKTOP) {
      addTermLine("error", "Desktop runtime unavailable — editor shell cannot execute commands.");
      return;
    }

    void runTerminalCommand(cmd);
  }

  /* ---- Git operations ---- */
  function handleGitCommit(): void {
    const message = commitMsg.trim();
    if (!message) return;
    appendAudit("GitCommit", commitMsg.trim());
    if (!HAS_DESKTOP || !gitRepo.detected) {
      addTermLine("system", "No git repo detected");
      return;
    }
    void runTerminalCommand(`git commit -m "${message.replace(/"/g, '\\"')}"`);
    setCommitMsg("");
  }

  function handleGitPush(): void {
    appendAudit("GitPush", `push ${gitBranch} → origin`);
    if (!HAS_DESKTOP || !gitRepo.detected) {
      addTermLine("system", "No git repo detected");
      return;
    }
    void runTerminalCommand(`git push origin ${gitBranch === "No git repo detected" ? "main" : gitBranch}`);
  }

  function handleGitPull(): void {
    appendAudit("GitPull", `pull origin/${gitBranch}`);
    if (!HAS_DESKTOP || !gitRepo.detected) {
      addTermLine("system", "No git repo detected");
      return;
    }
    void runTerminalCommand(`git pull origin ${gitBranch === "No git repo detected" ? "main" : gitBranch}`);
  }

  /* ---- Agent actions ---- */
  async function handleAgentAction(action: AgentAction): Promise<void> {
    const estimatedCost = 120;
    if (fuelUsed + estimatedCost > fuelBudget) {
      appendAudit("FuelExhausted", `Cannot run ${action.label} — insufficient fuel`);
      return;
    }
    setAgentMode("thinking");
    setAgentAction(action.label);
    setAgentResult("");
    appendAudit("AgentAction", `${action.label} — capability check passed`);

    // Show split view for code-producing actions
    if (["refactor", "test", "complete", "document"].includes(action.id)) {
      setSplitView("suggestion");
    }

    if (!HAS_DESKTOP) {
      setAgentResult("Desktop runtime unavailable. Open the desktop app and connect an LLM provider in Settings to use agent assist.");
      setAgentMode("result");
      appendAudit("AgentComplete", `${action.label} — desktop runtime unavailable`);
      return;
    }

    if (!activeFile) {
      setAgentResult("Open a file first so agent assist can include the current source in the request.");
      setAgentMode("result");
      appendAudit("AgentComplete", `${action.label} — no active file`);
      return;
    }

    const prompts: Record<string, string> = {
      explain: "Explain the selected file, highlight risks, and suggest next steps.",
      refactor: "Refactor this file and return the proposed code changes with concise reasoning.",
      fix: "Find likely bugs in this file and return the corrected code or patch guidance.",
      test: "Generate focused tests for this file and explain what they cover.",
      document: "Document this file with clear usage notes and inline guidance where helpful.",
      optimize: "Suggest performance and reliability improvements for this file.",
      complete: "Continue or complete the current implementation in a production-ready way.",
      review: "Perform a code review focused on bugs, regressions, and missing tests.",
    };

    try {
      const response = await sendChat(
        `${prompts[action.id]}\n\nFile path: ${activeFile.path}\nLanguage: ${activeFile.language}\n\nSource:\n\`\`\`\n${activeFile.content}\n\`\`\``,
      );
      const nextFuel = Math.max(estimatedCost, Math.min(480, response.token_count || estimatedCost));
      setFuelUsed((prev) => prev + nextFuel);
      setAgentResult(response.text);
      setAgentMode("result");
      appendAudit("AgentComplete", `${action.label} — ${nextFuel} fuel consumed`);
    } catch (error) {
      setAgentResult(
        error instanceof Error
          ? error.message
          : String(error),
      );
      setAgentMode("result");
      appendAudit("AgentError", `${action.label} failed`);
    }
  }

  function applyAgentSuggestion(): void {
    if (!activeFile || !agentResult) return;
    appendAudit("AgentApply", `Reviewed ${agentAction} response for ${activeFile.name}`);
    setSplitView("off");
    setAgentMode("idle");
  }

  /* ---- Agent worker simulation ---- */
  useEffect(() => {
    const interval = setInterval(() => {
      setAgentWorkers((prev) =>
        prev.map((w) => {
          if (w.status !== "working") return w;
          const newProgress = Math.min(w.progress + Math.floor(Math.random() * 8), 100);
          return {
            ...w,
            progress: newProgress,
            status: newProgress >= 100 ? "done" : "working",
            fuelUsed: w.fuelUsed + Math.floor(Math.random() * 15),
          };
        })
      );
    }, 2000);
    return () => clearInterval(interval);
  }, []);

  /* ---- Keyboard shortcuts ---- */
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent): void {
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key === "s") { e.preventDefault(); void saveFile(); }
      if (mod && e.key === "b") { e.preventDefault(); setShowExplorer((p) => !p); }
      if (mod && e.key === "j") { e.preventDefault(); setShowAgent((p) => !p); }
      if (mod && e.key === "n") { e.preventDefault(); setShowNewFile(true); }
      if (mod && e.key === "`") { e.preventDefault(); setBottomPanel((p) => p === "terminal" ? "none" : "terminal"); }
      if (mod && e.key === "f") { e.preventDefault(); setShowSearch((p) => !p); }
      if (mod && e.key === "\\") { e.preventDefault(); setSplitView((p) => p === "off" ? "suggestion" : "off"); }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  /* ---- Editor mount ---- */
  const handleEditorMount: OnMount = (editor) => {
    editorRef.current = editor;
    editor.focus();
  };

  /* ---- Toggle directory ---- */
  function toggleDir(path: string): void {
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
        // Lazily load directory contents on expand
        void expandDir(path);
      }
      return next;
    });
  }

  /* ---- Search results ---- */
  const searchResults = useMemo(() => {
    if (!searchQuery.trim()) return [];
    const q = searchQuery.toLowerCase();
    return files
      .filter((f) => f.content.toLowerCase().includes(q) || f.name.toLowerCase().includes(q))
      .map((f) => {
        const lines = f.content.split("\n");
        const matches = lines
          .map((line, i) => ({ line: i + 1, text: line }))
          .filter((l) => l.text.toLowerCase().includes(q))
          .slice(0, 3);
        return { file: f, matches };
      });
  }, [searchQuery, files]);

  /* ---- Render file tree ---- */
  function renderTree(nodes: FileTreeNode[], depth: number = 0): JSX.Element[] {
    return nodes.map((node) => {
      if (node.type === "dir") {
        const expanded = expandedDirs.has(node.path);
        return (
          <div key={node.path}>
            <button type="button" className="ce-tree-item ce-tree-dir" style={{ paddingLeft: `${depth * 14 + 8}px` }} onClick={() => toggleDir(node.path)}>
              <span className="ce-tree-arrow">{expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}</span>
              <span className="ce-tree-name">{node.name}</span>
            </button>
            {expanded && node.children && renderTree(node.children, depth + 1)}
          </div>
        );
      }
      const isActive = files.some((f) => f.path === node.path && f.id === activeTab);
      return (
        <button key={node.path} type="button" className={`ce-tree-item ce-tree-file ${isActive ? "ce-tree-active" : ""}`} style={{ paddingLeft: `${depth * 14 + 8}px` }} onClick={() => void openRealFile(node.path)}>
          <span className="ce-tree-lang">{langIcon(detectLanguage(node.name))}</span>
          <span className="ce-tree-name">{node.name}</span>
        </button>
      );
    });
  }

  const gitStatusIcon = (s: GitChange["status"]): string =>
    s === "modified" ? "M" : s === "added" ? "A" : s === "deleted" ? "D" : "?";
  const gitStatusColor = (s: GitChange["status"]): string =>
    s === "modified" ? "#f59e0b" : s === "added" ? "#34d399" : s === "deleted" ? "#ef4444" : "#94a3b8";

  /* ================================================================ */
  /*  RENDER                                                           */
  /* ================================================================ */
  return (
    <section className="ce-root">
      {/* ---- Header ---- */}
      <header className="ce-header">
        <div className="ce-header-left">
          <h2 className="ce-title">CODE EDITOR</h2>
          <span className="ce-subtitle">governed development environment</span>
        </div>
          <div className="ce-header-center">
          <div className="ce-branch-badge" title={gitRepo.root ?? "No git repo detected"}>
            <span className="ce-branch-icon"><GitBranch size={14} /></span>
            <span className="ce-branch-name">{gitBranch}</span>
          </div>
        </div>
        <div className="ce-header-right">
          <div className="ce-fuel-badge">
            <span className="ce-fuel-label">FUEL</span>
            <div className="ce-fuel-bar-mini">
              <div className="ce-fuel-bar-fill" style={{ width: `${fuelPct}%`, background: fuelPct > 50 ? "var(--nexus-accent)" : fuelPct > 20 ? "#f59e0b" : "#ef4444" }} />
            </div>
            <span className="ce-fuel-value">{fuelRemaining.toLocaleString()}</span>
          </div>
          <div className="ce-toolbar-btns">
            <button type="button" className="ce-tool-btn cursor-pointer" onClick={() => void saveFile()} disabled={!activeFile?.dirty || saving} title="Save (Ctrl+S)">{saving ? "..." : <Save size={14} />}</button>
            <button type="button" className={`ce-tool-btn cursor-pointer ${showSearch ? "ce-tool-active" : ""}`} onClick={() => setShowSearch((p) => !p)} title="Search (Ctrl+F)"><Search size={14} /></button>
            <button type="button" className={`ce-tool-btn cursor-pointer ${showExplorer ? "ce-tool-active" : ""}`} onClick={() => setShowExplorer((p) => !p)} title="Explorer (Ctrl+B)"><Menu size={14} /></button>
            <button type="button" className={`ce-tool-btn cursor-pointer ${splitView !== "off" ? "ce-tool-active" : ""}`} onClick={() => setSplitView((p) => p === "off" ? "suggestion" : "off")} title="Split View (Ctrl+\)"><Columns2 size={14} /></button>
            <button type="button" className={`ce-tool-btn cursor-pointer ${bottomPanel === "terminal" ? "ce-tool-active" : ""}`} onClick={() => setBottomPanel((p) => p === "terminal" ? "none" : "terminal")} title="Terminal (Ctrl+`)"><TerminalSquare size={14} /></button>
            <button type="button" className={`ce-tool-btn cursor-pointer ${bottomPanel === "git" ? "ce-tool-active" : ""}`} onClick={() => setBottomPanel((p) => p === "git" ? "none" : "git")} title="Git"><GitBranch size={14} /></button>
            <button type="button" className={`ce-tool-btn cursor-pointer ${bottomPanel === "agents" ? "ce-tool-active" : ""}`} onClick={() => setBottomPanel((p) => p === "agents" ? "none" : "agents")} title="Agent Workers"><Hexagon size={14} /></button>
            <button type="button" className={`ce-tool-btn ${showAgent ? "ce-tool-active" : ""}`} onClick={() => setShowAgent((p) => !p)} title="Agent Assist (Ctrl+J)">AI</button>
          </div>
        </div>
      </header>

      {/* ---- Browser mode / error notice ---- */}
      {fsError && (
        <div className="ce-notice-bar">
          <span>{fsError}</span>
          <button type="button" className="ce-notice-close" onClick={() => setFsError(null)}>×</button>
        </div>
      )}

      {/* ---- Search bar ---- */}
      {showSearch && (
        <div className="ce-search-bar">
          <span className="ce-search-icon"><Search size={14} /></span>
          <input className="ce-search-input" placeholder="Search across all files..." value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} autoFocus onKeyDown={(e) => { if (e.key === "Escape") { setShowSearch(false); setSearchQuery(""); } }} />
          {searchQuery && <span className="ce-search-count">{searchResults.reduce((a, r) => a + r.matches.length, 0)} matches</span>}
          <button type="button" className="ce-search-close" onClick={() => { setShowSearch(false); setSearchQuery(""); }}>×</button>
        </div>
      )}

      {/* ---- Search results ---- */}
      {showSearch && searchQuery && searchResults.length > 0 && (
        <div className="ce-search-results">
          {searchResults.map((r) => (
            <div key={r.file.id} className="ce-search-file">
              <button type="button" className="ce-search-file-name" onClick={() => openFile(r.file.id)}>{r.file.path}</button>
              {r.matches.map((m) => (
                <button type="button" key={m.line} className="ce-search-match" onClick={() => openFile(r.file.id)}>
                  <span className="ce-search-line-num">{m.line}</span>
                  <span className="ce-search-line-text">{m.text.trim()}</span>
                </button>
              ))}
            </div>
          ))}
        </div>
      )}

      {/* ---- Main body ---- */}
      <div className="ce-body">
        {/* ---- File Explorer ---- */}
        {showExplorer && (
          <aside className="ce-explorer">
            <div className="ce-explorer-header">
              <span className="ce-explorer-title">EXPLORER</span>
              <button type="button" className="ce-icon-btn cursor-pointer" onClick={() => setShowNewFile(true)} title="New File (Ctrl+N)"><Plus size={14} /></button>
            </div>
            {showNewFile && (
              <div className="ce-new-file-row">
                <input className="ce-new-file-input" placeholder="filename.ext" value={newFileName} onChange={(e) => setNewFileName(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleCreateFile(); if (e.key === "Escape") { setShowNewFile(false); setNewFileName(""); } }} autoFocus />
              </div>
            )}
            <div className="ce-tree">
              {fsLoading && <div className="ce-tree-loading">Loading...</div>}
              {!fsLoading && fileTree.length > 0 && renderTree(fileTree)}
              {!fsLoading && !HAS_DESKTOP && (
                <div className="ce-tree-notice">Browser mode — no filesystem</div>
              )}
              {/* User-created files not yet on disk */}
              {files.filter((f) => f.id.startsWith("f")).map((f) => (
                <div key={f.id} className="ce-tree-item-row">
                  <button type="button" className={`ce-tree-item ce-tree-file ${f.id === activeTab ? "ce-tree-active" : ""}`} onClick={() => openFile(f.id)}>
                    <span className="ce-tree-lang">{langIcon(f.language)}</span>
                    <span className="ce-tree-name">{f.name}</span>
                  </button>
                  <button type="button" className="ce-tree-delete" onClick={() => handleDeleteFile(f.id)}>×</button>
                </div>
              ))}
            </div>
          </aside>
        )}

        {/* ---- Center: editor + bottom panels ---- */}
        <div className="ce-center">
          <div className="ce-editor-area">
            {/* Tabs */}
            <div className="ce-tabs">
              {openTabs.map((tabId) => {
                const f = files.find((file) => file.id === tabId);
                if (!f) return null;
                return (
                  <div key={tabId} className={`ce-tab ${tabId === activeTab ? "ce-tab-active" : ""}`}>
                    <button type="button" className="ce-tab-label" onClick={() => setActiveTab(tabId)}>
                      <span className="ce-tab-icon">{langIcon(f.language)}</span>
                      {f.name}
                      {f.dirty && <span className="ce-tab-dirty"><Circle size={6} fill="currentColor" /></span>}
                    </button>
                    <button type="button" className="ce-tab-close" onClick={() => closeTab(tabId)}>×</button>
                  </div>
                );
              })}
            </div>

            {/* Editor + split */}
            <div className={`ce-editor-split ${splitView !== "off" ? "ce-split-active" : ""}`}>
              {/* Main editor */}
              <div className="ce-monaco-wrap">
                {activeFile ? (
                  <Editor
                    height="100%"
                    language={activeFile.language}
                    value={activeFile.content}
                    theme="nexus-dark"
                    onChange={handleEditorChange}
                    onMount={handleEditorMount}
                    beforeMount={(monaco) => {
                      monaco.editor.defineTheme("nexus-dark", {
                        base: "vs-dark",
                        inherit: true,
                        rules: [
                          { token: "comment", foreground: "6b7280", fontStyle: "italic" },
                          { token: "keyword", foreground: "c084fc" },
                          { token: "string", foreground: "34d399" },
                          { token: "number", foreground: "f59e0b" },
                          { token: "type", foreground: "22d3ee" },
                          { token: "function", foreground: "60a5fa" },
                          { token: "variable", foreground: "e2e8f0" },
                          { token: "operator", foreground: "94a3b8" },
                        ],
                        colors: {
                          "editor.background": "#0b1120",
                          "editor.foreground": "#e2e8f0",
                          "editor.lineHighlightBackground": "#1e293b",
                          "editor.selectionBackground": "#334155",
                          "editorCursor.foreground": "var(--nexus-accent)",
                          "editorLineNumber.foreground": "#475569",
                          "editorLineNumber.activeForeground": "#94a3b8",
                          "editor.selectionHighlightBackground": "#334155aa",
                          "editorIndentGuide.background": "#1e293b",
                          "editorIndentGuide.activeBackground": "#334155",
                          "editorBracketMatch.background": "var(--nexus-accent)22",
                          "editorBracketMatch.border": "var(--nexus-accent)44",
                        },
                      });
                    }}
                    options={{
                      fontSize: 14,
                      fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
                      fontLigatures: true,
                      minimap: { enabled: true, scale: 1 },
                      scrollBeyondLastLine: false,
                      smoothScrolling: true,
                      cursorBlinking: "smooth",
                      cursorSmoothCaretAnimation: "on",
                      renderLineHighlight: "all",
                      bracketPairColorization: { enabled: true },
                      padding: { top: 12 },
                      wordWrap: "on",
                    }}
                  />
                ) : (
                  <div className="ce-empty">
                    <div className="ce-empty-icon"><Keyboard size={32} /></div>
                    <p className="ce-empty-text">No file open</p>
                    <p className="ce-empty-hint">Select a file from the explorer or press Ctrl+N to create one</p>
                    <div className="ce-empty-shortcuts">
                      <span>Ctrl+B Explorer</span>
                      <span>Ctrl+` Terminal</span>
                      <span>Ctrl+J Agent</span>
                      <span>Ctrl+F Search</span>
                    </div>
                  </div>
                )}
              </div>

              {/* Split: agent suggestion */}
              {splitView === "suggestion" && agentMode === "result" && (
                <div className="ce-split-panel">
                    <div className="ce-split-header">
                      <span className="ce-split-title">AGENT SUGGESTION — {agentAction.toUpperCase()}</span>
                      <div className="ce-split-actions">
                      <button type="button" className="ce-split-btn ce-split-apply" onClick={applyAgentSuggestion}>Mark Reviewed</button>
                      <button type="button" className="ce-split-btn ce-split-dismiss" onClick={() => setSplitView("off")}>Dismiss</button>
                    </div>
                  </div>
                  <pre className="ce-split-content">{agentResult}</pre>
                </div>
              )}
              {splitView === "suggestion" && agentMode !== "result" && (
                <div className="ce-split-panel">
                  <div className="ce-split-header">
                    <span className="ce-split-title">AGENT SUGGESTION</span>
                    <button type="button" className="ce-split-btn ce-split-dismiss" onClick={() => setSplitView("off")}>Close</button>
                  </div>
                  <div className="ce-split-empty">
                    <p>Run an agent action to see suggestions here</p>
                    <p className="ce-split-hint">Try: Refactor, Gen Tests, Complete, or Document</p>
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* ---- Bottom Panel ---- */}
          {bottomPanel !== "none" && (
            <div className="ce-bottom">
              <div className="ce-bottom-tabs">
                <button type="button" className={`ce-bottom-tab ${bottomPanel === "terminal" ? "ce-bottom-tab-active" : ""}`} onClick={() => setBottomPanel("terminal")}>TERMINAL</button>
                <button type="button" className={`ce-bottom-tab ${bottomPanel === "git" ? "ce-bottom-tab-active" : ""}`} onClick={() => setBottomPanel("git")}>GIT</button>
                <button type="button" className={`ce-bottom-tab ${bottomPanel === "agents" ? "ce-bottom-tab-active" : ""}`} onClick={() => setBottomPanel("agents")}>AGENTS ({agentWorkers.filter((w) => w.status === "working").length} active)</button>
                <button type="button" className="ce-bottom-close" onClick={() => setBottomPanel("none")}>×</button>
              </div>

              {/* Terminal */}
              {bottomPanel === "terminal" && (
                <div className="ce-terminal">
                <div className="ce-term-output" ref={termRef}>
                  {terminalLines.map((line) => (
                    <div key={line.id} className={`ce-term-line ce-term-${line.type}`}>
                      {line.text}
                    </div>
                  ))}
                  {pendingApproval && (
                    <div className="ce-term-line ce-term-system">
                      HITL approval required for: {pendingApproval.cmd}
                    </div>
                  )}
                </div>
                <div className="ce-term-input-row">
                  <span className="ce-term-prompt">$</span>
                  <input
                      className="ce-term-input"
                      value={terminalInput}
                      onChange={(e) => setTerminalInput(e.target.value)}
                      onKeyDown={(e) => { if (e.key === "Enter") handleTerminalSubmit(); }}
                      placeholder="Type a command..."
                      spellCheck={false}
                    />
                    {pendingApproval && (
                      <>
                        <button
                          type="button"
                          className="ce-tool-btn"
                          onClick={() => void runTerminalCommand(pendingApproval.cmd, true, pendingApproval.cwd)}
                        >
                          Approve
                        </button>
                        <button type="button" className="ce-tool-btn" onClick={denyPendingCommand}>
                          Deny
                        </button>
                      </>
                    )}
                  </div>
                </div>
              )}

              {/* Git panel */}
              {bottomPanel === "git" && (
                <div className="ce-git">
                  <div className="ce-git-section">
                    <div className="ce-git-section-header">
                      <span>Changes ({gitRepo.changes.length})</span>
                      <div className="ce-git-btns">
                        <button type="button" className="ce-git-btn cursor-pointer" onClick={handleGitPull} title="Pull"><ArrowDown size={12} /> Pull</button>
                        <button type="button" className="ce-git-btn cursor-pointer" onClick={handleGitPush} title="Push"><ArrowUp size={12} /> Push</button>
                      </div>
                    </div>
                    <div className="ce-git-changes">
                      {gitRepo.changes.map((c) => (
                        <div key={c.file} className="ce-git-change">
                          <span className="ce-git-status" style={{ color: gitStatusColor(c.status) }}>{gitStatusIcon(c.status)}</span>
                          <span className="ce-git-file">{c.file}</span>
                        </div>
                      ))}
                      {gitRepo.detected && gitRepo.changes.length === 0 && (
                        <div className="ce-git-change">
                          <span className="ce-git-file">Working tree clean</span>
                        </div>
                      )}
                      {!gitRepo.detected && (
                        <div className="ce-git-change">
                          <span className="ce-git-file">No git repo detected</span>
                        </div>
                      )}
                    </div>
                    <div className="ce-git-commit-row">
                      <input className="ce-git-commit-input" placeholder="Commit message..." value={commitMsg} onChange={(e) => setCommitMsg(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleGitCommit(); }} />
                      <button type="button" className="ce-git-commit-btn" onClick={handleGitCommit} disabled={!gitRepo.detected || !commitMsg.trim()}>Commit</button>
                    </div>
                  </div>
                  <div className="ce-git-section">
                    <div className="ce-git-section-header"><span>History</span></div>
                    <div className="ce-git-log">
                      {gitRepo.commits.map((c) => (
                        <div key={c.hash} className="ce-git-log-entry">
                          <span className="ce-git-hash">{c.hash}</span>
                          <span className="ce-git-msg">{c.message}</span>
                          <span className="ce-git-author">{c.author}</span>
                        </div>
                      ))}
                      {!gitRepo.detected && <div className="ce-git-log-entry">No git repo detected</div>}
                    </div>
                  </div>
                </div>
              )}

              {/* Agent workers */}
              {bottomPanel === "agents" && (
                <div className="ce-workers">
                  <div className="ce-workers-header">
                    <span>LIVE AGENT ACTIVITY</span>
                    <span className="ce-workers-count">{agentWorkers.filter((w) => w.status === "working").length} working / {agentWorkers.length} total</span>
                  </div>
                  <div className="ce-workers-list">
                    {agentWorkers.map((w) => (
                      <div key={w.id} className={`ce-worker ${w.status === "done" ? "ce-worker-done" : ""}`}>
                        <div className="ce-worker-top">
                          <span className="ce-worker-name">{w.name}</span>
                          <span className={`ce-worker-status ce-worker-status-${w.status}`}>{w.status}</span>
                        </div>
                        <div className="ce-worker-detail">
                          <span className="ce-worker-action">{w.action}</span>
                          <span className="ce-worker-file">{w.file}</span>
                        </div>
                        <div className="ce-worker-progress-row">
                          <div className="ce-worker-bar">
                            <div className="ce-worker-bar-fill" style={{ width: `${w.progress}%`, background: w.status === "done" ? "#34d399" : w.status === "error" ? "#ef4444" : "var(--nexus-accent)" }} />
                          </div>
                          <span className="ce-worker-pct">{w.progress}%</span>
                          <span className="ce-worker-fuel">{w.fuelUsed} fuel</span>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>

        {/* ---- Agent Panel (right sidebar) ---- */}
        {showAgent && (
          <aside className="ce-agent">
            <div className="ce-agent-header">
              <span className="ce-agent-title">
                AGENT ASSIST <span style={{ fontSize: "0.7em", opacity: 0.6 }}>{HAS_DESKTOP ? "(LLM-backed)" : "(desktop required)"}</span>
              </span>
              <span className={`ce-agent-status ce-agent-status-${agentMode}`}>
                {agentMode === "thinking" ? "thinking..." : agentMode === "result" ? "ready" : "idle"}
              </span>
            </div>

            <div className="ce-agent-actions">
              {AGENT_ACTIONS.map((action) => (
                <button key={action.id} type="button" className="ce-agent-btn" onClick={() => handleAgentAction(action)} disabled={agentMode === "thinking"} title={action.description}>
                  <span className="ce-agent-btn-icon">{action.icon}</span>
                  <span className="ce-agent-btn-label">{action.label}</span>
                </button>
              ))}
            </div>

            {agentMode === "thinking" && (
              <div className="ce-agent-thinking">
                <div className="ce-thinking-dots"><span /><span /><span /></div>
                <p className="ce-thinking-text">Running {agentAction}...</p>
              </div>
            )}

            {agentMode === "result" && (
              <div className="ce-agent-result">
                <div className="ce-result-header">
                  <span className="ce-result-label">{agentAction}</span>
                  <button type="button" className="ce-result-close" onClick={() => setAgentMode("idle")}>×</button>
                </div>
                <pre className="ce-result-content">{agentResult}</pre>
                {splitView === "off" && (
                  <button type="button" className="ce-result-split-btn" onClick={() => setSplitView("suggestion")}>View in Split</button>
                )}
              </div>
            )}

            {/* Audit mini-log */}
            <div className="ce-audit">
              <span className="ce-audit-title">AUDIT LOG</span>
              <div className="ce-audit-entries">
                {auditLog.slice(0, 15).map((entry, i) => (
                  <div key={`${entry.ts}-${i}`} className="ce-audit-entry">
                    <span className="ce-audit-time">{new Date(entry.ts).toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false })}</span>
                    <span className="ce-audit-event">{entry.event}</span>
                    <span className="ce-audit-detail">{entry.detail}</span>
                  </div>
                ))}
                {auditLog.length === 0 && <p className="ce-audit-empty">No events yet</p>}
              </div>
            </div>
          </aside>
        )}
      </div>
    </section>
  );
}

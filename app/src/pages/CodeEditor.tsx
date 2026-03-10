import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Editor, { type OnMount } from "@monaco-editor/react";
import "./code-editor.css";

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

interface FileTreeNode {
  name: string;
  path: string;
  type: "file" | "dir";
  children?: FileTreeNode[];
  fileId?: string;
}

interface AgentAction {
  id: string;
  label: string;
  description: string;
  icon: string;
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
/*  Mock data                                                          */
/* ================================================================== */

const INITIAL_FILES: VirtualFile[] = [
  {
    id: "f1", name: "main.rs", path: "src/main.rs", language: "rust", dirty: false,
    content: `use nexus_kernel::Supervisor;
use nexus_sdk::prelude::*;
use std::sync::Arc;

/// Entry point for the Nexus OS kernel.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::init();

    // Boot the governance supervisor
    let supervisor = Arc::new(Supervisor::new());
    tracing::info!("Nexus OS v7.0 — Don't trust. Verify.");

    // Register built-in agents
    supervisor.register_agent("coder", include_str!("../agents/coder.toml"))?;
    supervisor.register_agent("designer", include_str!("../agents/designer.toml"))?;
    supervisor.register_agent("researcher", include_str!("../agents/researcher.toml"))?;

    // Start the runtime
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(supervisor.run())?;

    Ok(())
}
`,
  },
  {
    id: "f2", name: "agent.toml", path: "agents/coder.toml", language: "toml", dirty: false,
    content: `[agent]
name = "coder"
version = "1.0.0"
author = "nexus"
autonomy_level = 3

[capabilities]
file_read = true
file_write = true
net_access = false
shell_exec = false
code_execute = true

[fuel]
budget = 10000
refill_interval = "1h"
warn_threshold = 0.2

[governance]
hitl_tier = 1
audit_all_actions = true
`,
  },
  {
    id: "f3", name: "App.tsx", path: "src/App.tsx", language: "typescript", dirty: false,
    content: `import { useState, useEffect } from "react";
import { Sidebar } from "./components/layout/Sidebar";
import { Dashboard } from "./pages/Dashboard";
import { CodeEditor } from "./pages/CodeEditor";
import { Chat } from "./pages/Chat";
import type { Page, NexusConfig } from "./types";

export default function App() {
  const [page, setPage] = useState<Page>("chat");
  const [config, setConfig] = useState<NexusConfig | null>(null);

  useEffect(() => {
    // Load config from Tauri backend
    loadConfig().then(setConfig);
  }, []);

  return (
    <div className="nexus-root">
      <Sidebar activePage={page} onNavigate={setPage} />
      <main className="nexus-main">
        {page === "chat" && <Chat />}
        {page === "code-editor" && <CodeEditor />}
        {page === "dashboard" && <Dashboard />}
      </main>
    </div>
  );
}

async function loadConfig(): Promise<NexusConfig> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke("get_config");
}
`,
  },
  {
    id: "f4", name: "styles.css", path: "src/styles.css", language: "css", dirty: false,
    content: `:root {
  --bg-primary: #0f172a;
  --bg-secondary: #1e293b;
  --bg-tertiary: #0b1120;
  --text-primary: #e2e8f0;
  --text-secondary: #94a3b8;
  --text-muted: #475569;
  --accent: #22d3ee;
  --accent-hover: #06b6d4;
  --danger: #ef4444;
  --warning: #f59e0b;
  --success: #34d399;
  --border: rgba(56, 189, 248, 0.15);
}

body {
  margin: 0;
  background: var(--bg-primary);
  color: var(--text-primary);
  font-family: "JetBrains Mono", "Fira Code", monospace;
  -webkit-font-smoothing: antialiased;
}

*, *::before, *::after {
  box-sizing: border-box;
}
`,
  },
  {
    id: "f5", name: "README.md", path: "README.md", language: "markdown", dirty: false,
    content: `# Nexus OS

> Don't trust. Verify.

A governed AI operating system with capability-checked agents,
fuel budgets, and append-only audit trails.

## Architecture

- **Kernel**: Governance hub with Supervisor, fuel ledger, audit trail
- **SDK**: Agent-facing API wrapping kernel
- **Agents**: 9 governed agents (coder, designer, researcher, etc.)
- **Desktop App**: React + Tauri with 15 built-in applications

## Quick Start

\`\`\`bash
# Build the kernel
cargo build --release

# Start the desktop app
cd app && npm run tauri dev
\`\`\`

## Phase 7: Complete OS

15 built-in apps: Code Editor, Design Studio, Terminal, File Manager,
Database Manager, API Client, Notes, Email, Project Manager, Media Studio,
System Monitor, Marketplace, Chat Hub, Deploy Pipeline, Learning Center.
`,
  },
  {
    id: "f6", name: "lib.rs", path: "src/lib.rs", language: "rust", dirty: false,
    content: `//! Nexus OS Kernel Library
//!
//! The kernel provides the core governance primitives:
//! - Capability checking for all agent actions
//! - Fuel budget management and tracking
//! - Append-only audit trail with hash-chain integrity
//! - HITL (Human-in-the-Loop) approval for sensitive operations

pub mod audit;
pub mod capabilities;
pub mod fuel;
pub mod governance;
pub mod permissions;
pub mod speculative;
pub mod supervisor;

pub use supervisor::Supervisor;

/// Kernel version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
`,
  },
];

const FILE_TREE: FileTreeNode[] = [
  {
    name: "src", path: "src", type: "dir",
    children: [
      { name: "main.rs", path: "src/main.rs", type: "file", fileId: "f1" },
      { name: "lib.rs", path: "src/lib.rs", type: "file", fileId: "f6" },
      { name: "App.tsx", path: "src/App.tsx", type: "file", fileId: "f3" },
      { name: "styles.css", path: "src/styles.css", type: "file", fileId: "f4" },
    ],
  },
  {
    name: "agents", path: "agents", type: "dir",
    children: [
      { name: "coder.toml", path: "agents/coder.toml", type: "file", fileId: "f2" },
    ],
  },
  { name: "README.md", path: "README.md", type: "file", fileId: "f5" },
];

const AGENT_ACTIONS: AgentAction[] = [
  { id: "explain", label: "Explain", description: "Explain selected code", icon: "?" },
  { id: "refactor", label: "Refactor", description: "Suggest improvements", icon: "↻" },
  { id: "fix", label: "Fix Bugs", description: "Find and fix issues", icon: "⚕" },
  { id: "test", label: "Gen Tests", description: "Generate unit tests", icon: "⊘" },
  { id: "document", label: "Document", description: "Add documentation", icon: "≡" },
  { id: "optimize", label: "Optimize", description: "Performance improvements", icon: "⚡" },
  { id: "complete", label: "Complete", description: "Auto-complete code block", icon: "→" },
  { id: "review", label: "Review", description: "Security & quality review", icon: "⛨" },
];

const MOCK_RESPONSES: Record<string, string> = {
  explain: "This code initializes the Nexus OS kernel supervisor and boots the system. The `Supervisor::new()` creates a new governance hub that manages agent fuel budgets, capability checks, and the append-only audit trail. The `Arc` wrapper enables shared ownership across async tasks in the tokio runtime.",
  refactor: `// Suggestion: Extract boot into a separate function with proper error handling

async fn boot(supervisor: Arc<Supervisor>) -> Result<(), KernelError> {
    // Initialize subsystems in parallel
    let (audit, fuel, caps) = tokio::try_join!(
        supervisor.init_audit_trail(),
        supervisor.init_fuel_ledger(),
        supervisor.init_capabilities(),
    )?;

    tracing::info!(
        audit_entries = audit.len(),
        fuel_agents = fuel.len(),
        capabilities = caps.len(),
        "All subsystems initialized"
    );

    supervisor.run().await
}`,
  fix: "No critical bugs found. Recommendations:\n\n1. Add graceful shutdown handler:\n   `ctrlc::set_handler(|| supervisor.shutdown())`\n\n2. The `include_str!` calls will panic if files are missing — use `try_include!` or runtime loading.\n\n3. Consider adding a health check endpoint for the supervisor.",
  test: `#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_boots_successfully() {
        let supervisor = Supervisor::new();
        assert!(supervisor.boot().is_ok());
    }

    #[test]
    fn supervisor_has_empty_fuel_ledger() {
        let supervisor = Supervisor::new();
        assert_eq!(supervisor.fuel_ledger().len(), 0);
    }

    #[tokio::test]
    async fn agent_registration_requires_valid_toml() {
        let supervisor = Supervisor::new();
        let result = supervisor.register_agent("bad", "not valid toml");
        assert!(result.is_err());
    }

    #[test]
    fn audit_trail_starts_empty() {
        let supervisor = Supervisor::new();
        assert_eq!(supervisor.audit_trail().len(), 0);
    }
}`,
  document: `/// Nexus OS Kernel Entry Point
///
/// Initializes the governance supervisor and boots all subsystems:
/// - Fuel ledger initialization with per-agent budgets
/// - Audit trail hash-chain setup (append-only, tamper-evident)
/// - Agent manifest loading from TOML declarations
/// - Capability registry population from manifests
///
/// # Errors
///
/// Returns an error if:
/// - Tracing subscriber initialization fails
/// - Agent TOML manifests are malformed
/// - Tokio runtime creation fails
/// - Supervisor encounters a fatal governance violation`,
  optimize: "Performance analysis:\n\n1. Agent registration: Currently sequential. Use `tokio::spawn` for parallel manifest parsing — saves ~15ms per agent.\n\n2. Arc overhead: Negligible. The single Arc allocation is shared via clone, not deep-copied.\n\n3. Runtime: Consider `multi_thread` flavor with `worker_threads = num_cpus::get()` for production.\n\n4. Startup: Lazy-init the speculative engine (saves ~3ms). Pre-allocate audit trail buffer with `Vec::with_capacity(1024)`.",
  complete: `    // Start REST API server
    let api_handle = tokio::spawn({
        let supervisor = Arc::clone(&supervisor);
        async move {
            let app = axum::Router::new()
                .route("/api/agents", get(list_agents))
                .route("/api/agents/:id/fuel", get(agent_fuel))
                .route("/api/audit", get(audit_log))
                .with_state(supervisor);

            let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
            axum::serve(listener, app).await
        }
    });

    // Wait for all tasks
    tokio::select! {
        _ = api_handle => tracing::info!("API server stopped"),
        _ = supervisor.run() => tracing::info!("Supervisor stopped"),
    }`,
  review: "Security Review:\n\n✓ No unsafe code blocks\n✓ Agent capabilities are declared, not assumed\n✓ Fuel budget checked before execution\n\n⚠ Potential issues:\n1. `include_str!` embeds TOML at compile time — ensure no secrets in agent manifests\n2. No TLS configured for the runtime listener\n3. Consider adding rate limiting to agent action dispatch\n4. Audit trail should be flushed to persistent storage periodically",
};

const MOCK_GIT_CHANGES: GitChange[] = [
  { file: "src/main.rs", status: "modified" },
  { file: "agents/coder.toml", status: "modified" },
  { file: "src/api.rs", status: "added" },
  { file: "old_config.toml", status: "deleted" },
];

const MOCK_GIT_LOG: GitCommit[] = [
  { hash: "a1b2c3d", message: "feat: add Code Editor with Monaco integration", author: "Suresh", ts: Date.now() - 3600000 },
  { hash: "e4f5g6h", message: "fix: capability check before agent file write", author: "Claude", ts: Date.now() - 7200000 },
  { hash: "i7j8k9l", message: "refactor: extract Supervisor boot sequence", author: "Suresh", ts: Date.now() - 14400000 },
  { hash: "m0n1o2p", message: "feat: fuel budget warning at 20% threshold", author: "Claude", ts: Date.now() - 28800000 },
  { hash: "q3r4s5t", message: "docs: update architecture invariants", author: "Suresh", ts: Date.now() - 43200000 },
];

const MOCK_AGENT_WORKERS: AgentWorker[] = [
  { id: "w1", name: "Coder Agent", file: "src/main.rs", action: "Refactoring boot sequence", progress: 72, status: "working", fuelUsed: 340 },
  { id: "w2", name: "Test Agent", file: "src/lib.rs", action: "Generating integration tests", progress: 45, status: "working", fuelUsed: 210 },
  { id: "w3", name: "Docs Agent", file: "README.md", action: "Updating API documentation", progress: 100, status: "done", fuelUsed: 150 },
];

const DANGEROUS_COMMANDS = ["rm -rf", "sudo rm", "mkfs", "dd if=", ":(){:|:&};:", "chmod 777", "FORMAT", "shutdown", "reboot", "kill -9 1"];

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function CodeEditor(): JSX.Element {
  /* ---- State ---- */
  const [files, setFiles] = useState<VirtualFile[]>(INITIAL_FILES);
  const [openTabs, setOpenTabs] = useState<string[]>(["f1"]);
  const [activeTab, setActiveTab] = useState("f1");
  const [showExplorer, setShowExplorer] = useState(true);
  const [showAgent, setShowAgent] = useState(true);
  const [agentMode, setAgentMode] = useState<AgentPanelMode>("idle");
  const [agentResult, setAgentResult] = useState("");
  const [agentAction, setAgentAction] = useState("");
  const [fuelUsed, setFuelUsed] = useState(700);
  const [auditLog, setAuditLog] = useState<AuditEntry[]>([]);
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set(["src", "agents"]));
  const [newFileName, setNewFileName] = useState("");
  const [showNewFile, setShowNewFile] = useState(false);
  const [splitView, setSplitView] = useState<SplitView>("off");
  const [bottomPanel, setBottomPanel] = useState<BottomPanel>("none");
  const [terminalLines, setTerminalLines] = useState<TerminalLine[]>([
    { id: 0, type: "system", text: "Nexus OS Terminal v7.0 — governed shell", ts: Date.now() },
    { id: 1, type: "system", text: "Type commands below. Dangerous operations require approval.", ts: Date.now() },
  ]);
  const [terminalInput, setTerminalInput] = useState("");
  const [gitBranch, setGitBranch] = useState("main");
  const [gitChanges] = useState<GitChange[]>(MOCK_GIT_CHANGES);
  const [gitLog] = useState<GitCommit[]>(MOCK_GIT_LOG);
  const [commitMsg, setCommitMsg] = useState("");
  const [agentWorkers, setAgentWorkers] = useState<AgentWorker[]>(MOCK_AGENT_WORKERS);
  const [searchQuery, setSearchQuery] = useState("");
  const [showSearch, setShowSearch] = useState(false);

  const editorRef = useRef<Parameters<OnMount>[0] | null>(null);
  const termRef = useRef<HTMLDivElement>(null);
  const termLineId = useRef(2);

  const activeFile = useMemo(() => files.find((f) => f.id === activeTab), [files, activeTab]);
  const fuelBudget = 10000;
  const fuelRemaining = fuelBudget - fuelUsed;
  const fuelPct = Math.round((fuelRemaining / fuelBudget) * 100);

  /* ---- Audit helper ---- */
  const appendAudit = useCallback((event: string, detail: string) => {
    setAuditLog((prev) => [{ ts: Date.now(), event, detail }, ...prev].slice(0, 100));
  }, []);

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
      return next;
    });
  }

  function handleEditorChange(value: string | undefined): void {
    if (!value || !activeTab) return;
    setFiles((prev) => prev.map((f) => (f.id === activeTab ? { ...f, content: value, dirty: true } : f)));
  }

  function handleSave(): void {
    if (!activeFile) return;
    setFiles((prev) => prev.map((f) => (f.id === activeTab ? { ...f, dirty: false } : f)));
    appendAudit("FileSave", activeFile.name);
  }

  function handleCreateFile(): void {
    if (!newFileName.trim()) return;
    const id = `f${Date.now()}`;
    const lang = detectLanguage(newFileName);
    const newFile: VirtualFile = { id, name: newFileName.trim(), path: newFileName.trim(), language: lang, content: "", dirty: false };
    setFiles((prev) => [...prev, newFile]);
    setOpenTabs((prev) => [...prev, id]);
    setActiveTab(id);
    setNewFileName("");
    setShowNewFile(false);
    appendAudit("FileCreate", newFileName.trim());
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
    setTimeout(() => termRef.current?.scrollTo(0, termRef.current.scrollHeight), 50);
  }

  function handleTerminalSubmit(): void {
    const cmd = terminalInput.trim();
    if (!cmd) return;
    addTermLine("input", `$ ${cmd}`);
    setTerminalInput("");

    // Check for dangerous commands
    const isDangerous = DANGEROUS_COMMANDS.some((d) => cmd.toLowerCase().includes(d.toLowerCase()));
    if (isDangerous) {
      addTermLine("error", `[BLOCKED] Command requires Tier2+ HITL approval: "${cmd}"`);
      appendAudit("TermBlocked", cmd);
      return;
    }

    appendAudit("TermExec", cmd);

    // Mock responses
    setTimeout(() => {
      if (cmd === "ls" || cmd === "ls -la") {
        addTermLine("output", "src/  agents/  README.md  Cargo.toml  Cargo.lock");
      } else if (cmd === "cargo build") {
        addTermLine("output", "   Compiling nexus-kernel v7.0.0");
        setTimeout(() => addTermLine("output", "   Compiling nexus-sdk v7.0.0"), 300);
        setTimeout(() => addTermLine("output", "    Finished release [optimized] target(s) in 4.2s"), 600);
      } else if (cmd === "cargo test") {
        addTermLine("output", "running 804 tests");
        setTimeout(() => addTermLine("output", "test result: ok. 804 passed; 0 failed; 0 ignored"), 500);
      } else if (cmd === "git status") {
        addTermLine("output", `On branch ${gitBranch}\nChanges not staged:\n  modified: src/main.rs\n  modified: agents/coder.toml\nUntracked:\n  src/api.rs`);
      } else if (cmd === "git log --oneline") {
        gitLog.forEach((c) => addTermLine("output", `${c.hash} ${c.message}`));
      } else if (cmd.startsWith("echo ")) {
        addTermLine("output", cmd.slice(5));
      } else if (cmd === "clear") {
        setTerminalLines([]);
      } else if (cmd === "pwd") {
        addTermLine("output", "/home/nexus/NEXUS/nexus-os");
      } else if (cmd === "whoami") {
        addTermLine("output", "nexus-agent (governed, L3)");
      } else if (cmd === "help") {
        addTermLine("system", "Available: ls, cargo build, cargo test, git status, git log, echo, pwd, whoami, clear");
      } else {
        addTermLine("output", `nexus-sh: command simulated: ${cmd}`);
      }
    }, 150);
  }

  /* ---- Git operations ---- */
  function handleGitCommit(): void {
    if (!commitMsg.trim()) return;
    appendAudit("GitCommit", commitMsg.trim());
    setCommitMsg("");
    addTermLine("system", `[git] Committed: "${commitMsg.trim()}" on branch ${gitBranch}`);
  }

  function handleGitPush(): void {
    appendAudit("GitPush", `push ${gitBranch} → origin`);
    addTermLine("system", `[git] Pushing ${gitBranch} to origin... (requires Tier1 approval)`);
  }

  function handleGitPull(): void {
    appendAudit("GitPull", `pull origin/${gitBranch}`);
    addTermLine("system", `[git] Pulling from origin/${gitBranch}... Already up to date.`);
  }

  /* ---- Agent actions ---- */
  function handleAgentAction(action: AgentAction): void {
    const cost = 120 + Math.floor(Math.random() * 130);
    if (fuelUsed + cost > fuelBudget) {
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

    setTimeout(() => {
      setFuelUsed((prev) => prev + cost);
      setAgentResult(MOCK_RESPONSES[action.id] ?? "Analysis complete. No issues found.");
      setAgentMode("result");
      appendAudit("AgentComplete", `${action.label} — ${cost} fuel consumed`);
    }, 600 + Math.random() * 800);
  }

  function applyAgentSuggestion(): void {
    if (!activeFile || !agentResult) return;
    // In a real implementation, this would apply a diff/patch
    appendAudit("AgentApply", `Applied ${agentAction} suggestion to ${activeFile.name}`);
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
      if (mod && e.key === "s") { e.preventDefault(); handleSave(); }
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
      if (next.has(path)) next.delete(path); else next.add(path);
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
              <span className="ce-tree-arrow">{expanded ? "▾" : "▸"}</span>
              <span className="ce-tree-name">{node.name}</span>
            </button>
            {expanded && node.children && renderTree(node.children, depth + 1)}
          </div>
        );
      }
      const isActive = node.fileId === activeTab;
      return (
        <button key={node.fileId ?? node.path} type="button" className={`ce-tree-item ce-tree-file ${isActive ? "ce-tree-active" : ""}`} style={{ paddingLeft: `${depth * 14 + 8}px` }} onClick={() => node.fileId && openFile(node.fileId)}>
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
          <div className="ce-branch-badge" onClick={() => setGitBranch((b) => b === "main" ? "feature/phase-7" : "main")}>
            <span className="ce-branch-icon">⎇</span>
            <span className="ce-branch-name">{gitBranch}</span>
          </div>
        </div>
        <div className="ce-header-right">
          <div className="ce-fuel-badge">
            <span className="ce-fuel-label">FUEL</span>
            <div className="ce-fuel-bar-mini">
              <div className="ce-fuel-bar-fill" style={{ width: `${fuelPct}%`, background: fuelPct > 50 ? "#22d3ee" : fuelPct > 20 ? "#f59e0b" : "#ef4444" }} />
            </div>
            <span className="ce-fuel-value">{fuelRemaining.toLocaleString()}</span>
          </div>
          <div className="ce-toolbar-btns">
            <button type="button" className={`ce-tool-btn ${showSearch ? "ce-tool-active" : ""}`} onClick={() => setShowSearch((p) => !p)} title="Search (Ctrl+F)">⌕</button>
            <button type="button" className={`ce-tool-btn ${showExplorer ? "ce-tool-active" : ""}`} onClick={() => setShowExplorer((p) => !p)} title="Explorer (Ctrl+B)">☰</button>
            <button type="button" className={`ce-tool-btn ${splitView !== "off" ? "ce-tool-active" : ""}`} onClick={() => setSplitView((p) => p === "off" ? "suggestion" : "off")} title="Split View (Ctrl+\)">⊞</button>
            <button type="button" className={`ce-tool-btn ${bottomPanel === "terminal" ? "ce-tool-active" : ""}`} onClick={() => setBottomPanel((p) => p === "terminal" ? "none" : "terminal")} title="Terminal (Ctrl+`)">$</button>
            <button type="button" className={`ce-tool-btn ${bottomPanel === "git" ? "ce-tool-active" : ""}`} onClick={() => setBottomPanel((p) => p === "git" ? "none" : "git")} title="Git">⎇</button>
            <button type="button" className={`ce-tool-btn ${bottomPanel === "agents" ? "ce-tool-active" : ""}`} onClick={() => setBottomPanel((p) => p === "agents" ? "none" : "agents")} title="Agent Workers">⬢</button>
            <button type="button" className={`ce-tool-btn ${showAgent ? "ce-tool-active" : ""}`} onClick={() => setShowAgent((p) => !p)} title="Agent Assist (Ctrl+J)">AI</button>
          </div>
        </div>
      </header>

      {/* ---- Search bar ---- */}
      {showSearch && (
        <div className="ce-search-bar">
          <span className="ce-search-icon">⌕</span>
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
              <button type="button" className="ce-icon-btn" onClick={() => setShowNewFile(true)} title="New File (Ctrl+N)">+</button>
            </div>
            {showNewFile && (
              <div className="ce-new-file-row">
                <input className="ce-new-file-input" placeholder="filename.ext" value={newFileName} onChange={(e) => setNewFileName(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleCreateFile(); if (e.key === "Escape") { setShowNewFile(false); setNewFileName(""); } }} autoFocus />
              </div>
            )}
            <div className="ce-tree">
              {renderTree(FILE_TREE)}
              {files.filter((f) => !INITIAL_FILES.some((i) => i.id === f.id)).map((f) => (
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
                      {f.dirty && <span className="ce-tab-dirty">●</span>}
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
                          "editorCursor.foreground": "#22d3ee",
                          "editorLineNumber.foreground": "#475569",
                          "editorLineNumber.activeForeground": "#94a3b8",
                          "editor.selectionHighlightBackground": "#334155aa",
                          "editorIndentGuide.background": "#1e293b",
                          "editorIndentGuide.activeBackground": "#334155",
                          "editorBracketMatch.background": "#22d3ee22",
                          "editorBracketMatch.border": "#22d3ee44",
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
                    <div className="ce-empty-icon">⌨</div>
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
                      <button type="button" className="ce-split-btn ce-split-apply" onClick={applyAgentSuggestion}>Apply</button>
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
                  </div>
                </div>
              )}

              {/* Git panel */}
              {bottomPanel === "git" && (
                <div className="ce-git">
                  <div className="ce-git-section">
                    <div className="ce-git-section-header">
                      <span>Changes ({gitChanges.length})</span>
                      <div className="ce-git-btns">
                        <button type="button" className="ce-git-btn" onClick={handleGitPull} title="Pull">↓ Pull</button>
                        <button type="button" className="ce-git-btn" onClick={handleGitPush} title="Push">↑ Push</button>
                      </div>
                    </div>
                    <div className="ce-git-changes">
                      {gitChanges.map((c) => (
                        <div key={c.file} className="ce-git-change">
                          <span className="ce-git-status" style={{ color: gitStatusColor(c.status) }}>{gitStatusIcon(c.status)}</span>
                          <span className="ce-git-file">{c.file}</span>
                        </div>
                      ))}
                    </div>
                    <div className="ce-git-commit-row">
                      <input className="ce-git-commit-input" placeholder="Commit message..." value={commitMsg} onChange={(e) => setCommitMsg(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") handleGitCommit(); }} />
                      <button type="button" className="ce-git-commit-btn" onClick={handleGitCommit} disabled={!commitMsg.trim()}>Commit</button>
                    </div>
                  </div>
                  <div className="ce-git-section">
                    <div className="ce-git-section-header"><span>History</span></div>
                    <div className="ce-git-log">
                      {gitLog.map((c) => (
                        <div key={c.hash} className="ce-git-log-entry">
                          <span className="ce-git-hash">{c.hash}</span>
                          <span className="ce-git-msg">{c.message}</span>
                          <span className="ce-git-author">{c.author}</span>
                        </div>
                      ))}
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
                            <div className="ce-worker-bar-fill" style={{ width: `${w.progress}%`, background: w.status === "done" ? "#34d399" : w.status === "error" ? "#ef4444" : "#22d3ee" }} />
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
              <span className="ce-agent-title">AGENT ASSIST</span>
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

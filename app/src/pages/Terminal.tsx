import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import "./terminal.css";

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface TermLine {
  id: number;
  type: "input" | "output" | "error" | "system" | "warn" | "agent-suggest";
  text: string;
  ts: number;
  pane: number;
}

interface TermPane {
  id: number;
  label: string;
  cwd: string;
  shell: string;
}

interface AuditEntry {
  ts: number;
  event: string;
  detail: string;
}

interface CommandHistoryEntry {
  cmd: string;
  ts: number;
  pane: number;
  blocked: boolean;
}

interface AgentSuggestion {
  cmd: string;
  reason: string;
}

type ApprovalState = { cmd: string; reason: string } | null;

/* ================================================================== */
/*  Constants                                                          */
/* ================================================================== */

const BLOCKED_PATTERNS = [
  { pattern: /rm\s+(-[a-zA-Z]*f[a-zA-Z]*\s+|--force\s+)?\//i, reason: "Recursive delete from root" },
  { pattern: /rm\s+-[a-zA-Z]*r[a-zA-Z]*f|rm\s+-[a-zA-Z]*f[a-zA-Z]*r/i, reason: "Force recursive delete" },
  { pattern: /sudo\s+rm/i, reason: "Elevated delete operation" },
  { pattern: /mkfs\b/i, reason: "Filesystem format" },
  { pattern: /dd\s+if=/i, reason: "Raw disk write" },
  { pattern: /:\(\)\{.*:\|:.*\};:/i, reason: "Fork bomb" },
  { pattern: /chmod\s+777/i, reason: "Unrestricted permissions" },
  { pattern: /shutdown|reboot|poweroff|halt/i, reason: "System power control" },
  { pattern: /kill\s+-9\s+1\b/i, reason: "Kill init process" },
  { pattern: />(\/dev\/sd|\/dev\/nvme)/i, reason: "Direct device write" },
  { pattern: /curl\s+.*\|\s*(sudo\s+)?bash/i, reason: "Piped remote execution" },
  { pattern: /wget\s+.*\|\s*(sudo\s+)?bash/i, reason: "Piped remote execution" },
  { pattern: /eval\s*\$\(curl/i, reason: "Remote eval execution" },
  { pattern: /sudo\s+su\b/i, reason: "Elevate to root shell" },
  { pattern: /passwd\b/i, reason: "Password change" },
  { pattern: /userdel\b|useradd\b|groupdel\b/i, reason: "User/group modification" },
  { pattern: /iptables\s+-F/i, reason: "Flush firewall rules" },
  { pattern: /systemctl\s+(stop|disable|mask)/i, reason: "Stop system service" },
  { pattern: /DROP\s+DATABASE|DROP\s+TABLE|TRUNCATE/i, reason: "Destructive SQL" },
];

const WARN_PATTERNS = [
  { pattern: /sudo\b/i, reason: "Elevated privileges" },
  { pattern: /chmod\b/i, reason: "Permission change" },
  { pattern: /chown\b/i, reason: "Ownership change" },
  { pattern: /git\s+push\s+--force/i, reason: "Force push" },
  { pattern: /git\s+reset\s+--hard/i, reason: "Hard reset" },
  { pattern: /npm\s+publish/i, reason: "Package publish" },
  { pattern: /docker\s+rm/i, reason: "Container removal" },
  { pattern: /pip\s+install\b(?!.*--user)/i, reason: "Global package install" },
];

/* Mock filesystem */
const MOCK_FS: Record<string, string[]> = {
  "/home/nexus/NEXUS/nexus-os": ["src/", "agents/", "app/", "crates/", "Cargo.toml", "Cargo.lock", "CLAUDE.md", "README.md", "deny.toml"],
  "/home/nexus/NEXUS/nexus-os/src": ["main.rs", "lib.rs"],
  "/home/nexus/NEXUS/nexus-os/agents": ["coder/", "designer/", "researcher/", "reviewer/"],
  "/home/nexus/NEXUS/nexus-os/app": ["src/", "public/", "package.json", "tsconfig.json", "vite.config.ts"],
  "/home/nexus/NEXUS/nexus-os/app/src": ["App.tsx", "main.tsx", "pages/", "components/", "api/"],
  "/home/nexus/NEXUS/nexus-os/crates": ["nexus-kernel/", "nexus-sdk/", "nexus-audit/", "nexus-fuel/"],
};

const MOCK_PROCESSES = `  PID TTY      STAT   TIME COMMAND
    1 ?        Ss     0:03 nexus-kernel
   42 ?        Sl     0:12 nexus-supervisor
  101 ?        S      0:05 agent:coder [L3]
  102 ?        S      0:03 agent:designer [L2]
  103 ?        S      0:01 agent:researcher [L3]
  201 ?        Sl     0:08 nexus-api-server :3001
  301 pts/0    Ss     0:00 nexus-shell`;

const ENV_VARS: Record<string, string> = {
  NEXUS_VERSION: "7.0.0",
  NEXUS_HOME: "/home/nexus/NEXUS/nexus-os",
  SHELL: "/bin/nexus-sh",
  USER: "nexus",
  HOME: "/home/nexus",
  CARGO_HOME: "/home/nexus/.cargo",
  RUSTUP_HOME: "/home/nexus/.rustup",
  NODE_VERSION: "20.11.0",
  RUST_EDITION: "2021",
  GOVERNANCE_LEVEL: "L3",
  FUEL_BUDGET: "10000",
  AUDIT_MODE: "append-only",
};

/* ================================================================== */
/*  Mock command executor                                              */
/* ================================================================== */

function executeCommand(cmd: string, cwd: string): { lines: string[]; type: TermLine["type"] } {
  const parts = cmd.trim().split(/\s+/);
  const base = parts[0];

  switch (base) {
    case "ls": {
      const target = parts.includes("-la") || parts.includes("-l") || parts.includes("-al") ? "long" : "short";
      const dir = parts.find((p) => p !== "ls" && !p.startsWith("-")) ?? cwd;
      const fullPath = dir.startsWith("/") ? dir : `${cwd}/${dir}`.replace(/\/+/g, "/");
      const contents = MOCK_FS[fullPath] ?? MOCK_FS[cwd];
      if (!contents) return { lines: [`ls: cannot access '${dir}': No such file or directory`], type: "error" };
      if (target === "long") {
        const longLines = contents.map((f) => {
          const isDir = f.endsWith("/");
          const perms = isDir ? "drwxr-xr-x" : "-rw-r--r--";
          const size = isDir ? "4096" : `${Math.floor(Math.random() * 50000) + 500}`;
          return `${perms}  nexus nexus ${size.padStart(6)} Mar 10 14:${String(Math.floor(Math.random() * 60)).padStart(2, "0")} ${f}`;
        });
        return { lines: [`total ${contents.length * 4}`, ...longLines], type: "output" };
      }
      return { lines: [contents.join("  ")], type: "output" };
    }
    case "cd": {
      const target = parts[1] ?? "~";
      if (target === "~" || target === "$HOME") return { lines: [], type: "output" };
      return { lines: [], type: "output" };
    }
    case "pwd":
      return { lines: [cwd], type: "output" };
    case "whoami":
      return { lines: ["nexus (governed, L3, fuel: 10000)"], type: "output" };
    case "echo":
      return { lines: [parts.slice(1).join(" ").replace(/['"]/g, "")], type: "output" };
    case "cat": {
      const file = parts[1];
      if (!file) return { lines: ["cat: missing operand"], type: "error" };
      if (file === "CLAUDE.md") return { lines: ["# CLAUDE.md - Nexus OS Development Guide", "", "> Read automatically by Claude Code.", "", "## Project Identity", "- Name: Nexus OS", "- Version: 7.0.0", "- Tagline: Don't trust. Verify."], type: "output" };
      if (file === "Cargo.toml") return { lines: ['[workspace]', 'resolver = "2"', 'members = [', '  "crates/*",', '  "agents/*",', ']', "", "[workspace.package]", 'version = "7.0.0"', 'edition = "2021"'], type: "output" };
      return { lines: [`cat: ${file}: simulated content`], type: "output" };
    }
    case "clear":
      return { lines: ["__CLEAR__"], type: "system" };
    case "date":
      return { lines: [new Date().toString()], type: "output" };
    case "uptime":
      return { lines: [" 14:32:01 up 47 days, 3:21,  1 user,  load average: 0.42, 0.38, 0.35"], type: "output" };
    case "uname":
      return { lines: ["NexusOS 7.0.0 x86_64 governed-kernel"], type: "output" };
    case "df":
      return { lines: [
        "Filesystem     1K-blocks      Used Available Use% Mounted on",
        "nexus-root     512000000  89234567 422765433  18% /",
        "nexus-agents    10240000   3456789   6783211  34% /agents",
        "nexus-audit      2048000    987654   1060346  49% /audit",
      ], type: "output" };
    case "free":
      return { lines: [
        "              total        used        free      shared  buff/cache   available",
        "Mem:       32768000    12345678     8901234      512345    11521088    19876543",
        "Swap:       8192000           0     8192000",
      ], type: "output" };
    case "ps":
      return { lines: MOCK_PROCESSES.split("\n"), type: "output" };
    case "top":
      return { lines: [
        "NexusOS 7.0 — governed process monitor",
        "",
        "Tasks:   7 total,   4 running,   3 sleeping",
        "Agents:  3 active,  fuel avg: 78%",
        "%Cpu(s):  12.3 us,   2.1 sy,   0.0 ni,  85.1 id",
        "MiB Mem:  32000.0 total,  12345.6 used,  8901.2 free",
        "",
        "  PID  AGENT         CPU%  MEM%  FUEL%  STATUS",
        "  101  coder         8.2   3.4   72%    working",
        "  102  designer      2.1   1.8   91%    idle",
        "  103  researcher    5.7   2.9   65%    working",
      ], type: "output" };
    case "env":
    case "printenv": {
      const key = parts[1];
      if (key) return { lines: [ENV_VARS[key] ?? `${key}: not set`], type: "output" };
      return { lines: Object.entries(ENV_VARS).map(([k, v]) => `${k}=${v}`), type: "output" };
    }
    case "history":
      return { lines: ["(see COMMAND HISTORY panel on the right →)"], type: "system" };
    case "help":
      return { lines: [
        "Nexus OS Terminal — Governed Shell v7.0",
        "",
        "Built-in commands:",
        "  ls [-la] [dir]    List directory contents",
        "  cd [dir]          Change directory",
        "  pwd               Print working directory",
        "  cat [file]        Display file contents",
        "  echo [text]       Print text",
        "  env / printenv    Environment variables",
        "  ps / top          Process list / monitor",
        "  df / free         Disk / memory usage",
        "  date / uptime     System time / uptime",
        "  whoami / uname    User / system info",
        "  clear             Clear terminal",
        "  history           Command history",
        "  help              Show this help",
        "",
        "Build commands:",
        "  cargo build       Build Rust workspace",
        "  cargo test        Run test suite (804 tests)",
        "  cargo clippy      Run linter",
        "  npm run build     Build frontend",
        "  npm run dev       Start dev server",
        "",
        "Git commands:",
        "  git status        Show working tree status",
        "  git log           Show commit history",
        "  git diff          Show changes",
        "  git add / commit  Stage and commit",
        "",
        "Governance:",
        "  Dangerous commands are BLOCKED (Tier2+ approval required)",
        "  Warned commands show a caution notice",
        "  All commands are logged to the audit trail",
      ], type: "output" };
    case "cargo": {
      const sub = parts[1];
      if (sub === "build" || sub === "b") {
        return { lines: [
          "   Compiling nexus-kernel v7.0.0",
          "   Compiling nexus-sdk v7.0.0",
          "   Compiling nexus-audit v7.0.0",
          "   Compiling nexus-fuel v7.0.0",
          "   Compiling nexus-agents v7.0.0",
          "    Finished `release` profile [optimized] target(s) in 12.4s",
        ], type: "output" };
      }
      if (sub === "test" || sub === "t") {
        return { lines: [
          "running 804 tests",
          "test kernel::supervisor::tests::boot ... ok",
          "test kernel::fuel::tests::budget_check ... ok",
          "test kernel::audit::tests::hash_chain ... ok",
          "test sdk::prelude::tests::re_exports ... ok",
          "...",
          "",
          "test result: ok. 804 passed; 0 failed; 0 ignored; 0 filtered out; finished in 8.2s",
        ], type: "output" };
      }
      if (sub === "clippy") {
        return { lines: [
          "    Checking nexus-kernel v7.0.0",
          "    Checking nexus-sdk v7.0.0",
          "    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.1s",
        ], type: "output" };
      }
      if (sub === "fmt") {
        return { lines: ["All files formatted correctly."], type: "output" };
      }
      return { lines: [`cargo: simulated '${sub ?? ""}' complete`], type: "output" };
    }
    case "npm": {
      const sub = parts[1];
      if (sub === "run" && parts[2] === "build") {
        return { lines: [
          "> nexus-desktop@7.0.0 build",
          "> tsc && vite build",
          "",
          "vite v5.4.21 building for production...",
          "✓ 1833 modules transformed.",
          "dist/index.html            0.74 kB │ gzip:  0.41 kB",
          "dist/assets/index.css    192.51 kB │ gzip: 32.18 kB",
          "dist/assets/index.js     451.40 kB │ gzip: 130.81 kB",
          "✓ built in 2.33s",
        ], type: "output" };
      }
      if (sub === "run" && parts[2] === "dev") {
        return { lines: [
          "> nexus-desktop@7.0.0 dev",
          "> vite",
          "",
          "  VITE v5.4.21  ready in 342ms",
          "",
          "  ➜  Local:   http://localhost:5173/",
          "  ➜  Network: http://192.168.1.42:5173/",
        ], type: "output" };
      }
      if (sub === "install" || sub === "i" || sub === "ci") {
        return { lines: ["added 347 packages in 4.2s", "", "82 packages are looking for funding", "  run `npm fund` for details"], type: "output" };
      }
      return { lines: [`npm: simulated '${sub ?? ""}' complete`], type: "output" };
    }
    case "git": {
      const sub = parts[1];
      if (sub === "status") {
        return { lines: [
          "On branch main",
          "Your branch is up to date with 'origin/main'.",
          "",
          "Changes not staged for commit:",
          "  (use \"git add <file>...\" to update what will be committed)",
          "",
          "        modified:   src/main.rs",
          "        modified:   agents/coder/manifest.toml",
          "",
          "Untracked files:",
          "        app/src/pages/Terminal.tsx",
          "",
          "no changes added to commit",
        ], type: "output" };
      }
      if (sub === "log") {
        return { lines: [
          "a1b2c3d feat: add Code Editor with Monaco integration",
          "e4f5g6h fix: capability check before agent file write",
          "i7j8k9l refactor: extract Supervisor boot sequence",
          "m0n1o2p feat: fuel budget warning at 20% threshold",
          "q3r4s5t docs: update architecture invariants",
        ], type: "output" };
      }
      if (sub === "diff") {
        return { lines: [
          "diff --git a/src/main.rs b/src/main.rs",
          "--- a/src/main.rs",
          "+++ b/src/main.rs",
          "@@ -5,6 +5,8 @@ fn main() -> Result<(), Box<dyn std::error::Error>> {",
          "     let supervisor = Arc::new(Supervisor::new());",
          "+    // Initialize terminal governance",
          "+    supervisor.register_terminal_handler()?;",
          "     tracing::info!(\"Nexus OS v7.0\");",
        ], type: "output" };
      }
      if (sub === "branch") {
        return { lines: ["* main", "  feature/phase-7", "  feature/terminal", "  fix/audit-chain"], type: "output" };
      }
      return { lines: [`git: simulated '${sub ?? ""}' complete`], type: "output" };
    }
    case "nexus": {
      const sub = parts[1];
      if (sub === "agents") {
        return { lines: [
          "AGENT          STATUS    AUTONOMY  FUEL      LAST ACTION",
          "coder          running   L3        7200/10k  ToolExec: refactor_boot",
          "designer       idle      L2        9100/10k  —",
          "researcher     running   L3        6500/10k  WebSearch: rust async patterns",
          "reviewer       stopped   L1        10k/10k   —",
          "self-improve   running   L4        5800/10k  ToolExec: optimize_prompt",
        ], type: "output" };
      }
      if (sub === "fuel") {
        return { lines: [
          "FUEL LEDGER — Nexus OS v7.0",
          "─────────────────────────────",
          "Total budget:     50,000",
          "Total consumed:   18,600 (37.2%)",
          "Total remaining:  31,400",
          "",
          "  coder:        2,800 consumed  (28.0%)",
          "  designer:       900 consumed  ( 9.0%)",
          "  researcher:   3,500 consumed  (35.0%)",
          "  self-improve:  4,200 consumed  (42.0%)",
        ], type: "output" };
      }
      if (sub === "audit") {
        return { lines: [
          "AUDIT TRAIL — last 5 entries (hash-chain verified ✓)",
          "─────────────────────────────────────────────────────",
          "[14:31:42] AgentAction   coder → ToolExec: refactor_boot",
          "[14:31:38] CapCheck      coder → file_write: PASS",
          "[14:31:35] FuelDebit     coder → 180 units (7200 remaining)",
          "[14:31:22] TermExec      nexus-shell → cargo test",
          "[14:31:01] AgentAction   researcher → WebSearch: rust async",
        ], type: "output" };
      }
      return { lines: [
        "nexus: Nexus OS CLI v7.0",
        "  nexus agents    List running agents",
        "  nexus fuel      Fuel ledger status",
        "  nexus audit     Recent audit trail",
      ], type: "output" };
    }
    case "tree":
      return { lines: [
        ".",
        "├── src/",
        "│   ├── main.rs",
        "│   └── lib.rs",
        "├── agents/",
        "│   ├── coder/",
        "│   ├── designer/",
        "│   ├── researcher/",
        "│   └── reviewer/",
        "├── app/",
        "│   ├── src/",
        "│   └── package.json",
        "├── crates/",
        "│   ├── nexus-kernel/",
        "│   ├── nexus-sdk/",
        "│   ├── nexus-audit/",
        "│   └── nexus-fuel/",
        "├── Cargo.toml",
        "├── CLAUDE.md",
        "└── README.md",
        "",
        "12 directories, 5 files",
      ], type: "output" };
    case "which":
      return { lines: [parts[1] ? `/usr/bin/${parts[1]}` : "which: missing argument"], type: "output" };
    case "grep": {
      if (parts.length < 3) return { lines: ["grep: missing arguments. Usage: grep <pattern> <file>"], type: "error" };
      return { lines: [`${parts[parts.length - 1]}:3:  match: ${parts[1]}`], type: "output" };
    }
    case "wc":
      return { lines: ["  847  3201  28934 (simulated)"], type: "output" };
    case "head":
    case "tail":
      return { lines: [`(${base}: simulated output for ${parts[1] ?? "stdin"})`], type: "output" };
    case "mkdir":
      return { lines: [], type: "output" };
    case "touch":
      return { lines: [], type: "output" };
    case "cp":
    case "mv":
      return { lines: [], type: "output" };
    case "find":
      return { lines: [
        "./src/main.rs",
        "./src/lib.rs",
        "./Cargo.toml",
        "./CLAUDE.md",
      ], type: "output" };
    case "man":
      return { lines: [`Governed terminal: '${parts[1] ?? ""}' — use 'help' for Nexus OS commands`], type: "system" };
    case "exit":
      return { lines: ["nexus-sh: governed terminal cannot be exited. Close the tab instead."], type: "warn" };
    default:
      return { lines: [`nexus-sh: command not found: ${base}`, "Type 'help' for available commands."], type: "error" };
  }
}

/* ================================================================== */
/*  Agent suggestion engine                                            */
/* ================================================================== */

function getSuggestions(cmd: string, history: CommandHistoryEntry[]): AgentSuggestion[] {
  const suggestions: AgentSuggestion[] = [];
  const recent = history.slice(-5).map((h) => h.cmd);

  if (cmd.startsWith("git s")) {
    suggestions.push({ cmd: "git status", reason: "Check working tree changes" });
    suggestions.push({ cmd: "git stash", reason: "Stash current changes" });
  } else if (cmd.startsWith("git l")) {
    suggestions.push({ cmd: "git log --oneline -10", reason: "Recent commit history" });
    suggestions.push({ cmd: "git log --graph --oneline", reason: "Visual branch history" });
  } else if (cmd.startsWith("git c")) {
    suggestions.push({ cmd: "git commit -m \"\"", reason: "Commit staged changes" });
    suggestions.push({ cmd: "git checkout -b feature/", reason: "Create new branch" });
  } else if (cmd.startsWith("cargo")) {
    suggestions.push({ cmd: "cargo build --release", reason: "Optimized build" });
    suggestions.push({ cmd: "cargo test --workspace", reason: "Run all tests" });
    suggestions.push({ cmd: "cargo clippy -- -D warnings", reason: "Lint check" });
  } else if (cmd.startsWith("npm")) {
    suggestions.push({ cmd: "npm run build", reason: "Build frontend" });
    suggestions.push({ cmd: "npm run dev", reason: "Start dev server" });
  } else if (cmd.startsWith("nexus")) {
    suggestions.push({ cmd: "nexus agents", reason: "List running agents" });
    suggestions.push({ cmd: "nexus fuel", reason: "Check fuel budgets" });
    suggestions.push({ cmd: "nexus audit", reason: "View audit trail" });
  } else if (cmd.startsWith("ls")) {
    suggestions.push({ cmd: "ls -la", reason: "Detailed listing" });
    suggestions.push({ cmd: "tree", reason: "Visual directory tree" });
  } else if (!cmd && recent.length > 0) {
    // Contextual suggestions based on history
    if (recent.some((c) => c.includes("git add"))) {
      suggestions.push({ cmd: "git commit -m \"\"", reason: "Commit after staging" });
    }
    if (recent.some((c) => c.includes("cargo build"))) {
      suggestions.push({ cmd: "cargo test", reason: "Test after build" });
    }
    if (recent.some((c) => c.includes("cargo test"))) {
      suggestions.push({ cmd: "cargo clippy -- -D warnings", reason: "Lint after testing" });
    }
  }

  return suggestions.slice(0, 4);
}

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function Terminal(): JSX.Element {
  /* ---- State ---- */
  const [panes, setPanes] = useState<TermPane[]>([
    { id: 1, label: "Terminal 1", cwd: "/home/nexus/NEXUS/nexus-os", shell: "nexus-sh" },
  ]);
  const [activePane, setActivePane] = useState(1);
  const [lines, setLines] = useState<TermLine[]>([
    { id: 0, type: "system", text: "╔══════════════════════════════════════════════════════════════╗", ts: Date.now(), pane: 1 },
    { id: 1, type: "system", text: "║  Nexus OS Terminal v7.0 — Governed Shell                    ║", ts: Date.now(), pane: 1 },
    { id: 2, type: "system", text: "║  Don't trust. Verify. Every command is audit-logged.        ║", ts: Date.now(), pane: 1 },
    { id: 3, type: "system", text: "║  Dangerous commands require Tier2+ HITL approval.           ║", ts: Date.now(), pane: 1 },
    { id: 4, type: "system", text: "║  Type 'help' for available commands.                        ║", ts: Date.now(), pane: 1 },
    { id: 5, type: "system", text: "╚══════════════════════════════════════════════════════════════╝", ts: Date.now(), pane: 1 },
  ]);
  const [input, setInput] = useState("");
  const [commandHistory, setCommandHistory] = useState<CommandHistoryEntry[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [auditLog, setAuditLog] = useState<AuditEntry[]>([]);
  const [fuelUsed, setFuelUsed] = useState(0);
  const [showSidebar, setShowSidebar] = useState(true);
  const [sidebarTab, setSidebarTab] = useState<"history" | "audit" | "blocked">("history");
  const [pendingApproval, setPendingApproval] = useState<ApprovalState>(null);
  const [suggestions, setSuggestions] = useState<AgentSuggestion[]>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [selectedSuggestion, setSelectedSuggestion] = useState(0);

  const lineId = useRef(6);
  const termRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const fuelBudget = 10000;
  const fuelRemaining = fuelBudget - fuelUsed;
  const fuelPct = Math.round((fuelRemaining / fuelBudget) * 100);

  const currentPane = useMemo(() => panes.find((p) => p.id === activePane), [panes, activePane]);
  const paneLines = useMemo(() => lines.filter((l) => l.pane === activePane), [lines, activePane]);

  /* ---- Helpers ---- */
  const appendAudit = useCallback((event: string, detail: string) => {
    setAuditLog((prev) => [{ ts: Date.now(), event, detail }, ...prev].slice(0, 200));
  }, []);

  const addLine = useCallback((type: TermLine["type"], text: string, pane?: number) => {
    const id = lineId.current++;
    setLines((prev) => [...prev, { id, type, text, ts: Date.now(), pane: pane ?? activePane }]);
  }, [activePane]);

  const scrollToBottom = useCallback(() => {
    setTimeout(() => termRef.current?.scrollTo(0, termRef.current.scrollHeight), 30);
  }, []);

  /* ---- Update suggestions on input change ---- */
  useEffect(() => {
    const s = getSuggestions(input, commandHistory);
    setSuggestions(s);
    setShowSuggestions(s.length > 0 && input.length > 0);
    setSelectedSuggestion(0);
  }, [input, commandHistory]);

  /* ---- Execute command ---- */
  function handleSubmit(): void {
    const cmd = input.trim();
    if (!cmd) return;
    setInput("");
    setHistoryIndex(-1);
    setShowSuggestions(false);

    addLine("input", `${currentPane?.cwd.split("/").pop() ?? "~"} $ ${cmd}`);

    // Log to command history
    const histEntry: CommandHistoryEntry = { cmd, ts: Date.now(), pane: activePane, blocked: false };

    // Check blocked patterns
    const blocked = BLOCKED_PATTERNS.find((p) => p.pattern.test(cmd));
    if (blocked) {
      addLine("error", `[BLOCKED] Command requires Tier2+ HITL approval`);
      addLine("error", `  Reason: ${blocked.reason}`);
      addLine("error", `  Command: ${cmd}`);
      addLine("system", "  Use the approval dialog to request execution.");
      appendAudit("TermBlocked", `${cmd} — ${blocked.reason}`);
      histEntry.blocked = true;
      setCommandHistory((prev) => [...prev, histEntry]);
      setPendingApproval({ cmd, reason: blocked.reason });
      scrollToBottom();
      return;
    }

    // Check warn patterns
    const warned = WARN_PATTERNS.find((p) => p.pattern.test(cmd));
    if (warned) {
      addLine("warn", `[CAUTION] ${warned.reason}: ${cmd}`);
      appendAudit("TermWarn", `${cmd} — ${warned.reason}`);
    }

    setCommandHistory((prev) => [...prev, histEntry]);
    appendAudit("TermExec", cmd);

    // Fuel cost
    const cost = 5 + Math.floor(Math.random() * 10);
    setFuelUsed((prev) => prev + cost);

    // Execute
    setTimeout(() => {
      const result = executeCommand(cmd, currentPane?.cwd ?? "/home/nexus/NEXUS/nexus-os");

      if (result.lines[0] === "__CLEAR__") {
        setLines((prev) => prev.filter((l) => l.pane !== activePane));
        addLine("system", "Terminal cleared.");
      } else {
        // Handle cd
        if (cmd.startsWith("cd ")) {
          const target = cmd.slice(3).trim();
          setPanes((prev) =>
            prev.map((p) => {
              if (p.id !== activePane) return p;
              if (target === "~" || target === "$HOME") return { ...p, cwd: "/home/nexus" };
              if (target === "..") {
                const parts = p.cwd.split("/");
                parts.pop();
                return { ...p, cwd: parts.join("/") || "/" };
              }
              if (target.startsWith("/")) return { ...p, cwd: target };
              return { ...p, cwd: `${p.cwd}/${target}`.replace(/\/+/g, "/") };
            })
          );
        }

        result.lines.forEach((line) => addLine(result.type, line));
      }
      scrollToBottom();
    }, 80 + Math.random() * 120);
  }

  /* ---- History navigation ---- */
  function handleKeyDown(e: React.KeyboardEvent): void {
    if (e.key === "Enter") {
      if (showSuggestions && suggestions[selectedSuggestion]) {
        setInput(suggestions[selectedSuggestion].cmd);
        setShowSuggestions(false);
      } else {
        handleSubmit();
      }
      return;
    }
    if (e.key === "Tab" && showSuggestions && suggestions.length > 0) {
      e.preventDefault();
      setInput(suggestions[selectedSuggestion].cmd);
      setShowSuggestions(false);
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      if (showSuggestions) {
        setSelectedSuggestion((p) => Math.max(0, p - 1));
        return;
      }
      const cmds = commandHistory.map((h) => h.cmd);
      if (cmds.length === 0) return;
      const nextIdx = historyIndex === -1 ? cmds.length - 1 : Math.max(0, historyIndex - 1);
      setHistoryIndex(nextIdx);
      setInput(cmds[nextIdx]);
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (showSuggestions) {
        setSelectedSuggestion((p) => Math.min(suggestions.length - 1, p + 1));
        return;
      }
      const cmds = commandHistory.map((h) => h.cmd);
      if (historyIndex === -1) return;
      const nextIdx = historyIndex + 1;
      if (nextIdx >= cmds.length) {
        setHistoryIndex(-1);
        setInput("");
      } else {
        setHistoryIndex(nextIdx);
        setInput(cmds[nextIdx]);
      }
    }
    if (e.key === "Escape") {
      setShowSuggestions(false);
      setPendingApproval(null);
    }
    if (e.key === "l" && e.ctrlKey) {
      e.preventDefault();
      setLines((prev) => prev.filter((l) => l.pane !== activePane));
      addLine("system", "Terminal cleared.");
    }
  }

  /* ---- Pane management ---- */
  function addPane(): void {
    const id = Math.max(...panes.map((p) => p.id)) + 1;
    setPanes((prev) => [...prev, { id, label: `Terminal ${id}`, cwd: "/home/nexus/NEXUS/nexus-os", shell: "nexus-sh" }]);
    setActivePane(id);
    addLine("system", `╔══ Terminal ${id} ══╗  New governed shell session.`, id);
    appendAudit("PaneCreate", `Terminal ${id}`);
  }

  function closePane(id: number): void {
    if (panes.length <= 1) return;
    setPanes((prev) => prev.filter((p) => p.id !== id));
    setLines((prev) => prev.filter((l) => l.pane !== id));
    if (activePane === id) setActivePane(panes.find((p) => p.id !== id)?.id ?? 1);
    appendAudit("PaneClose", `Terminal ${id}`);
  }

  /* ---- Approval dialog ---- */
  function handleApprove(): void {
    if (!pendingApproval) return;
    appendAudit("TermApproved", `HITL approved: ${pendingApproval.cmd}`);
    addLine("system", `[APPROVED] Command executed with Tier2 approval: ${pendingApproval.cmd}`);
    addLine("output", "(simulated output — real execution requires Tauri backend)");
    setPendingApproval(null);
    scrollToBottom();
  }

  function handleDeny(): void {
    if (!pendingApproval) return;
    appendAudit("TermDenied", `HITL denied: ${pendingApproval.cmd}`);
    addLine("system", `[DENIED] Command rejected: ${pendingApproval.cmd}`);
    setPendingApproval(null);
    scrollToBottom();
  }

  /* ---- Keyboard shortcuts ---- */
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent): void {
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key === "`") { e.preventDefault(); inputRef.current?.focus(); }
      if (mod && e.key === "b") { e.preventDefault(); setShowSidebar((p) => !p); }
      if (mod && e.key === "t") { e.preventDefault(); addPane(); }
      if (mod && e.key === "w") {
        e.preventDefault();
        if (panes.length > 1) closePane(activePane);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  /* ---- Focus terminal on click ---- */
  function handleTermClick(): void {
    inputRef.current?.focus();
  }

  const formatTime = (ts: number): string =>
    new Date(ts).toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false });

  /* ================================================================ */
  /*  RENDER                                                           */
  /* ================================================================ */
  return (
    <section className="tm-root">
      {/* ---- Header ---- */}
      <header className="tm-header">
        <div className="tm-header-left">
          <h2 className="tm-title">TERMINAL</h2>
          <span className="tm-subtitle">governed shell</span>
        </div>
        <div className="tm-header-center">
          <span className="tm-shell-badge">
            <span className="tm-shell-icon">$</span>
            <span className="tm-shell-name">{currentPane?.shell ?? "nexus-sh"}</span>
          </span>
          <span className="tm-cwd-badge" title={currentPane?.cwd}>
            {currentPane?.cwd.split("/").slice(-2).join("/") ?? "~"}
          </span>
        </div>
        <div className="tm-header-right">
          <div className="tm-fuel-badge">
            <span className="tm-fuel-label">FUEL</span>
            <div className="tm-fuel-bar">
              <div className="tm-fuel-fill" style={{ width: `${fuelPct}%`, background: fuelPct > 50 ? "#22d3ee" : fuelPct > 20 ? "#f59e0b" : "#ef4444" }} />
            </div>
            <span className="tm-fuel-value">{fuelRemaining.toLocaleString()}</span>
          </div>
          <div className="tm-toolbar">
            <button type="button" className="tm-tool-btn" onClick={addPane} title="New Tab (Ctrl+T)">+</button>
            <button type="button" className={`tm-tool-btn ${showSidebar ? "tm-tool-active" : ""}`} onClick={() => setShowSidebar((p) => !p)} title="Toggle Sidebar (Ctrl+B)">☰</button>
          </div>
        </div>
      </header>

      {/* ---- Body ---- */}
      <div className="tm-body">
        {/* ---- Terminal area ---- */}
        <div className="tm-main">
          {/* Pane tabs */}
          <div className="tm-pane-tabs">
            {panes.map((p) => (
              <div key={p.id} className={`tm-pane-tab ${p.id === activePane ? "tm-pane-tab-active" : ""}`}>
                <button type="button" className="tm-pane-tab-label" onClick={() => setActivePane(p.id)}>
                  <span className="tm-pane-tab-icon">$</span>
                  {p.label}
                </button>
                {panes.length > 1 && (
                  <button type="button" className="tm-pane-tab-close" onClick={() => closePane(p.id)}>×</button>
                )}
              </div>
            ))}
          </div>

          {/* Terminal output */}
          <div className="tm-output" ref={termRef} onClick={handleTermClick}>
            {paneLines.map((line) => (
              <div key={line.id} className={`tm-line tm-line-${line.type}`}>
                {line.text}
              </div>
            ))}

            {/* Approval dialog */}
            {pendingApproval && (
              <div className="tm-approval">
                <div className="tm-approval-header">HITL APPROVAL REQUIRED — Tier2+ Operation</div>
                <div className="tm-approval-detail">
                  <span className="tm-approval-label">Command:</span>
                  <code className="tm-approval-cmd">{pendingApproval.cmd}</code>
                </div>
                <div className="tm-approval-detail">
                  <span className="tm-approval-label">Reason:</span>
                  <span className="tm-approval-reason">{pendingApproval.reason}</span>
                </div>
                <div className="tm-approval-actions">
                  <button type="button" className="tm-approval-btn tm-approval-approve" onClick={handleApprove}>Approve & Execute</button>
                  <button type="button" className="tm-approval-btn tm-approval-deny" onClick={handleDeny}>Deny</button>
                </div>
              </div>
            )}
          </div>

          {/* Suggestions dropdown */}
          {showSuggestions && (
            <div className="tm-suggestions">
              {suggestions.map((s, i) => (
                <button
                  key={s.cmd}
                  type="button"
                  className={`tm-suggestion ${i === selectedSuggestion ? "tm-suggestion-active" : ""}`}
                  onClick={() => { setInput(s.cmd); setShowSuggestions(false); inputRef.current?.focus(); }}
                  onMouseEnter={() => setSelectedSuggestion(i)}
                >
                  <span className="tm-suggestion-cmd">{s.cmd}</span>
                  <span className="tm-suggestion-reason">{s.reason}</span>
                </button>
              ))}
              <div className="tm-suggestion-hint">Tab to accept · ↑↓ to navigate · Esc to dismiss</div>
            </div>
          )}

          {/* Input line */}
          <div className="tm-input-row">
            <span className="tm-prompt">
              <span className="tm-prompt-user">nexus</span>
              <span className="tm-prompt-sep">:</span>
              <span className="tm-prompt-dir">{currentPane?.cwd.split("/").pop() ?? "~"}</span>
              <span className="tm-prompt-symbol">$</span>
            </span>
            <input
              ref={inputRef}
              className="tm-input"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Type a command..."
              spellCheck={false}
              autoFocus
            />
          </div>
        </div>

        {/* ---- Sidebar ---- */}
        {showSidebar && (
          <aside className="tm-sidebar">
            <div className="tm-sidebar-tabs">
              <button type="button" className={`tm-sidebar-tab ${sidebarTab === "history" ? "tm-sidebar-tab-active" : ""}`} onClick={() => setSidebarTab("history")}>History</button>
              <button type="button" className={`tm-sidebar-tab ${sidebarTab === "audit" ? "tm-sidebar-tab-active" : ""}`} onClick={() => setSidebarTab("audit")}>Audit</button>
              <button type="button" className={`tm-sidebar-tab ${sidebarTab === "blocked" ? "tm-sidebar-tab-active" : ""}`} onClick={() => setSidebarTab("blocked")}>Blocked</button>
            </div>

            {/* Command history */}
            {sidebarTab === "history" && (
              <div className="tm-sidebar-content">
                <div className="tm-sidebar-header">
                  <span>COMMAND HISTORY</span>
                  <span className="tm-sidebar-count">{commandHistory.length}</span>
                </div>
                <div className="tm-sidebar-list">
                  {[...commandHistory].reverse().map((h, i) => (
                    <button
                      key={`${h.ts}-${i}`}
                      type="button"
                      className={`tm-history-item ${h.blocked ? "tm-history-blocked" : ""}`}
                      onClick={() => { setInput(h.cmd); inputRef.current?.focus(); }}
                    >
                      <span className="tm-history-cmd">{h.cmd}</span>
                      <span className="tm-history-time">{formatTime(h.ts)}</span>
                      {h.blocked && <span className="tm-history-badge">BLOCKED</span>}
                    </button>
                  ))}
                  {commandHistory.length === 0 && <p className="tm-sidebar-empty">No commands yet</p>}
                </div>
              </div>
            )}

            {/* Audit log */}
            {sidebarTab === "audit" && (
              <div className="tm-sidebar-content">
                <div className="tm-sidebar-header">
                  <span>AUDIT TRAIL</span>
                  <span className="tm-sidebar-count">{auditLog.length}</span>
                </div>
                <div className="tm-sidebar-list">
                  {auditLog.slice(0, 50).map((entry, i) => (
                    <div key={`${entry.ts}-${i}`} className="tm-audit-entry">
                      <span className="tm-audit-time">{formatTime(entry.ts)}</span>
                      <span className={`tm-audit-event ${entry.event.includes("Blocked") || entry.event.includes("Denied") ? "tm-audit-danger" : entry.event.includes("Warn") ? "tm-audit-warn" : ""}`}>{entry.event}</span>
                      <span className="tm-audit-detail">{entry.detail}</span>
                    </div>
                  ))}
                  {auditLog.length === 0 && <p className="tm-sidebar-empty">No events yet</p>}
                </div>
              </div>
            )}

            {/* Blocked commands */}
            {sidebarTab === "blocked" && (
              <div className="tm-sidebar-content">
                <div className="tm-sidebar-header">
                  <span>BLOCKED COMMANDS</span>
                  <span className="tm-sidebar-count">{BLOCKED_PATTERNS.length}</span>
                </div>
                <div className="tm-sidebar-list">
                  {BLOCKED_PATTERNS.map((bp, i) => (
                    <div key={i} className="tm-blocked-item">
                      <span className="tm-blocked-reason">{bp.reason}</span>
                      <span className="tm-blocked-pattern">{bp.pattern.source.slice(0, 40)}</span>
                    </div>
                  ))}
                  <div className="tm-blocked-footer">
                    <p>All blocked commands require Tier2+ HITL approval to execute.</p>
                    <p>Warned commands (sudo, chmod, git push --force, etc.) show a caution notice but are allowed.</p>
                  </div>
                </div>
              </div>
            )}

            {/* Governance stats */}
            <div className="tm-sidebar-stats">
              <div className="tm-stat">
                <span className="tm-stat-label">Commands</span>
                <span className="tm-stat-value">{commandHistory.length}</span>
              </div>
              <div className="tm-stat">
                <span className="tm-stat-label">Blocked</span>
                <span className="tm-stat-value tm-stat-danger">{commandHistory.filter((h) => h.blocked).length}</span>
              </div>
              <div className="tm-stat">
                <span className="tm-stat-label">Fuel Used</span>
                <span className="tm-stat-value">{fuelUsed}</span>
              </div>
              <div className="tm-stat">
                <span className="tm-stat-label">Panes</span>
                <span className="tm-stat-value">{panes.length}</span>
              </div>
            </div>
          </aside>
        )}
      </div>
    </section>
  );
}

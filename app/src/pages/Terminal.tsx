import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Plus, Menu } from "lucide-react";
import { terminalExecute, terminalExecuteApproved, type TerminalCommandResult } from "../api/backend";
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

type TerminalResult = TerminalCommandResult;

type ApprovalState = { cmd: string; reason: string } | null;

/* ================================================================== */
/*  Constants — Defense-in-depth frontend checks                       */
/* ================================================================== */

/*
 * Governance: blocked dangerous commands (defense-in-depth, frontend layer)
 * Matches: rm -rf, sudo rm, mkfs, dd if=, chmod 777, > /dev/sda,
 *          fork bombs, shutdown, kill init, curl|bash, passwd, iptables -F
 */
const BLOCKED_PATTERNS = [
  { pattern: /rm\s+(-[a-zA-Z]*f[a-zA-Z]*\s+|--force\s+)?\//i, reason: "Recursive delete from root" },
  { pattern: /rm\s+-[a-zA-Z]*r[a-zA-Z]*f|rm\s+-[a-zA-Z]*f[a-zA-Z]*r/i, reason: "Force recursive delete (rm -rf)" },
  { pattern: /sudo\s+rm/i, reason: "Elevated delete operation" },
  { pattern: /mkfs\b/i, reason: "Filesystem format" },
  { pattern: /dd\s+if=/i, reason: "Raw disk write (dd if=/dev/zero)" },
  { pattern: /:\(\)\{.*:\|:.*\};:/i, reason: "Fork bomb" },
  { pattern: /chmod\s+777/i, reason: "Unrestricted permissions (chmod 777)" },
  { pattern: /shutdown|reboot|poweroff|halt/i, reason: "System power control" },
  { pattern: /kill\s+-9\s+1\b/i, reason: "Kill init process" },
  { pattern: />(\/dev\/sd|\/dev\/nvme)/i, reason: "Direct device write (> /dev/sda)" },
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

const HELP_TEXT = [
  "Nexus OS Terminal — Governed Shell v9.0",
  "",
  "All commands execute via TypedTools (no raw shell).",
  "Built-in (client-side):",
  "  cd [dir]          Change directory",
  "  clear             Clear terminal",
  "  history           Command history",
  "  help              Show this help",
  "",
  "Executed via backend:",
  "  ls [-la] [dir]    List directory contents",
  "  cat [file]        Display file contents",
  "  pwd               Print working directory",
  "  echo [text]       Print text",
  "  env / printenv    Environment variables",
  "  ps                Process list",
  "  df [path]         Disk usage",
  "  free              Memory usage",
  "  date / uptime     System time / uptime",
  "  whoami / uname    User / system info",
  "",
  "Build commands:",
  "  cargo build       Build Rust workspace",
  "  cargo test        Run test suite",
  "  cargo clippy      Run linter",
  "  npm run build     Build frontend",
  "  npm run dev       Start dev server",
  "",
  "Git commands:",
  "  git status        Show working tree status",
  "  git log           Show commit history",
  "  git diff          Show changes",
  "  git commit        Commit staged changes",
  "",
  "Governance:",
  "  Dangerous commands are BLOCKED (Tier2+ HITL approval required)",
  "  Warned commands show a caution notice",
  "  All commands are audit-logged on the backend",
];

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
  } else if (cmd.startsWith("ls")) {
    suggestions.push({ cmd: "ls -la", reason: "Detailed listing" });
    suggestions.push({ cmd: "tree", reason: "Visual directory tree" });
  } else if (!cmd && recent.length > 0) {
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
    { id: 1, type: "system", text: "║  Nexus OS Terminal v9.0 — Governed Shell                    ║", ts: Date.now(), pane: 1 },
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
  const [isExecuting, setIsExecuting] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => { setLoading(false); }, []);

  const lineId = useRef(6);
  const termRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const scrollTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
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

  const addLines = useCallback((type: TermLine["type"], text: string, pane?: number) => {
    const outputLines = text.split("\n");
    // Remove trailing empty line from split
    if (outputLines.length > 0 && outputLines[outputLines.length - 1] === "") {
      outputLines.pop();
    }
    for (const line of outputLines) {
      const id = lineId.current++;
      setLines((prev) => [...prev, { id, type, text: line, ts: Date.now(), pane: pane ?? activePane }]);
    }
  }, [activePane]);

  const scrollToBottom = useCallback(() => {
    if (scrollTimerRef.current) clearTimeout(scrollTimerRef.current);
    scrollTimerRef.current = setTimeout(() => termRef.current?.scrollTo(0, termRef.current.scrollHeight), 30);
  }, []);

  /* ---- Handle cd locally ---- */
  const handleCd = useCallback((target: string, cwd: string): string => {
    if (!target || target === "~" || target === "$HOME") return "/home/nexus";
    if (target === "-") return cwd; // no OLDPWD tracking, stay put
    if (target === "..") {
      const parts = cwd.split("/");
      parts.pop();
      return parts.join("/") || "/";
    }
    if (target.startsWith("/")) return target;
    return `${cwd}/${target}`.replace(/\/+/g, "/");
  }, []);

  useEffect(() => {
    return () => {
      if (scrollTimerRef.current) clearTimeout(scrollTimerRef.current);
    };
  }, []);

  /* ---- Update suggestions on input change ---- */
  useEffect(() => {
    const s = getSuggestions(input, commandHistory);
    setSuggestions(s);
    setShowSuggestions(s.length > 0 && input.length > 0);
    setSelectedSuggestion(0);
  }, [input, commandHistory]);

  /* ---- Execute command via Tauri backend ---- */
  async function executeViaBackend(cmd: string, cwd: string): Promise<void> {
    setIsExecuting(true);
    try {
      const result: TerminalResult = await terminalExecute(cmd, cwd);

      // Backend says this command needs HITL approval
      if (result.needs_approval) {
        addLine("warn", `[HITL REQUIRED] Command requires approval: ${cmd}`);
        addLine("system", `  Tool: ${result.tool} — Use the approval dialog to confirm.`);
        appendAudit("TermHITL", `${cmd} — needs approval (${result.tool})`);
        setPendingApproval({ cmd, reason: `Backend: ${result.tool} requires HITL approval` });
        scrollToBottom();
        return;
      }

      // Fuel accounting
      setFuelUsed((prev) => prev + result.fuel_cost);

      // Display stdout
      if (result.stdout) {
        addLines("output", result.stdout);
      }

      // Display stderr in red
      if (result.stderr) {
        addLines("error", result.stderr);
      }

      // Show exit code if non-zero
      if (result.exit_code !== 0) {
        addLine("error", `[exit code ${result.exit_code}] (${result.duration_ms}ms)`);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      addLine("error", `Error: ${message}`);
      appendAudit("TermError", `${cmd} — ${message}`);
    } finally {
      setIsExecuting(false);
      scrollToBottom();
    }
  }

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

    // Defense-in-depth: Check blocked patterns on frontend
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

    const cwd = currentPane?.cwd ?? "/home/nexus/NEXUS/nexus-os";
    const parts = cmd.trim().split(/\s+/);
    const base = parts[0];

    // Client-side builtins (no backend call needed)
    if (base === "clear") {
      setLines((prev) => prev.filter((l) => l.pane !== activePane));
      addLine("system", "Terminal cleared.");
      scrollToBottom();
      return;
    }
    if (base === "help") {
      HELP_TEXT.forEach((line) => addLine("output", line));
      scrollToBottom();
      return;
    }
    if (base === "history") {
      addLine("system", "(see COMMAND HISTORY panel on the right)");
      scrollToBottom();
      return;
    }
    if (base === "exit") {
      addLine("warn", "nexus-sh: governed terminal cannot be exited. Close the tab instead.");
      scrollToBottom();
      return;
    }
    if (base === "cd") {
      const target = parts[1] ?? "~";
      const newCwd = handleCd(target, cwd);
      setPanes((prev) =>
        prev.map((p) => (p.id === activePane ? { ...p, cwd: newCwd } : p))
      );
      scrollToBottom();
      return;
    }

    // All other commands → execute via Tauri backend
    void executeViaBackend(cmd, cwd);
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

  /* ---- Approval dialog — real execution after HITL confirm ---- */
  async function handleApprove(): Promise<void> {
    if (!pendingApproval) return;
    const cmd = pendingApproval.cmd;
    const cwd = currentPane?.cwd ?? "/home/nexus/NEXUS/nexus-os";

    appendAudit("TermApproved", `HITL approved: ${cmd}`);
    addLine("system", `[APPROVED] Executing with Tier2 approval: ${cmd}`);
    setPendingApproval(null);

    setIsExecuting(true);
    try {
      const result: TerminalResult = await terminalExecuteApproved(cmd, cwd);

      setFuelUsed((prev) => prev + result.fuel_cost);

      if (result.stdout) {
        addLines("output", result.stdout);
      }
      if (result.stderr) {
        addLines("error", result.stderr);
      }
      if (result.exit_code !== 0) {
        addLine("error", `[exit code ${result.exit_code}] (${result.duration_ms}ms)`);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      addLine("error", `Error: ${message}`);
      appendAudit("TermError", `approved exec failed: ${cmd} — ${message}`);
    } finally {
      setIsExecuting(false);
      scrollToBottom();
    }
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
  if (loading) return (
    <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100%", color: "#64748b", fontSize: 14 }}>
      Loading...
    </div>
  );

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
              <div className="tm-fuel-fill" style={{ width: `${fuelPct}%`, background: fuelPct > 50 ? "var(--nexus-accent)" : fuelPct > 20 ? "#f59e0b" : "#ef4444" }} />
            </div>
            <span className="tm-fuel-value">{fuelRemaining.toLocaleString()}</span>
          </div>
          <div className="tm-toolbar">
            <button type="button" className="tm-tool-btn cursor-pointer" onClick={addPane} title="New Tab (Ctrl+T)" aria-label="New Tab (Ctrl+T)"><Plus size={14} /></button>
            <button type="button" className={`tm-tool-btn cursor-pointer ${showSidebar ? "tm-tool-active" : ""}`} onClick={() => setShowSidebar((p) => !p)} title="Toggle Sidebar (Ctrl+B)"><Menu size={14} /></button>
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

            {/* Executing indicator */}
            {isExecuting && (
              <div className="tm-line tm-line-system">Running...</div>
            )}

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
                  <button type="button" className="tm-approval-btn tm-approval-approve" onClick={() => void handleApprove()}>Approve & Execute</button>
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
              disabled={isExecuting}
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

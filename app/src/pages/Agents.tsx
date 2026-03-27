import { useEffect, useMemo, useState, useCallback, useRef } from "react";
import { ActivityFeed } from "../components/agents/ActivityFeed";
import { AgentDetail, type AgentDetailTab } from "../components/agents/AgentDetail";
import { CreateAgent } from "../components/agents/CreateAgent";
import { SlmStatusBadge } from "../components/agents/SlmStatusBadge";
import { HeatMap } from "../components/viz/HeatMap";
import { NeuralGraph } from "../components/viz/NeuralGraph";
import { PulseRing } from "../components/viz/PulseRing";
import { getPreinstalledAgents, hasDesktopRuntime, listProviderModels, executeAgentGoal, approveConsentRequest, denyConsentRequest, listPendingConsents, getAvailableProviders, setAgentLlmProvider, flashCreateSession, setAgentReviewMode } from "../api/backend";
import type { AvailableProvider } from "../api/backend";
import type { ConsentNotification } from "../types";
import RequiresLlm from "../components/RequiresLlm";
import type { AgentSummary, AuditEventRow, PreinstalledAgent, SlmStatus } from "../types";
import { Play, Pause, Square, Trash2, Plus, Search, Shield, Settings, Users, Zap, Fuel, MemoryStick, ChevronDown, ChevronUp, Eye, Send, Loader2 } from "lucide-react";
import AgentOutputPanel from "../components/AgentOutputPanel";
import { listen } from "@tauri-apps/api/event";
import "./agents.css";

/* ─── constants ─── */

type AutonomyFilter = "all" | 1 | 2 | 3 | 4 | 5 | 6;
type CategoryFilter = "all" | "research" | "code" | "security" | "creative" | "system" | "devops" | "data" | "communication";
type AgentCardStatus = "Idle" | "Running" | "Paused" | "Error";

const AUTONOMY_COLORS: Record<number, string> = {
  0: "#64748b", 1: "#3b82f6", 2: "#22c55e", 3: "#eab308",
  4: "#f97316", 5: "#ef4444", 6: "#a855f7",
};

const AUTONOMY_LABELS: Record<number, string> = {
  0: "Inert", 1: "Suggest", 2: "Act-with-approval", 3: "Act-then-report",
  4: "Autonomous-bounded", 5: "Full autonomy", 6: "Transcendent",
};

const CATEGORY_KEYWORDS: Record<Exclude<CategoryFilter, "all">, string[]> = {
  research: ["research", "oracle", "analyst", "intelligence", "recon", "osint"],
  code: ["code", "coder", "architect", "forge", "engineer", "debug", "compiler", "refactor", "review"],
  security: ["security", "sentinel", "warden", "guardian", "firewall", "audit", "pentest", "vuln"],
  creative: ["creative", "design", "writer", "content", "media", "art", "muse"],
  system: ["system", "monitor", "ops", "kernel", "runtime", "self-improve", "genesis", "evolution"],
  devops: ["devops", "deploy", "pipeline", "ci", "cd", "infra", "docker", "k8s", "terraform"],
  data: ["data", "database", "analytics", "etl", "migration", "sql"],
  communication: ["communicat", "email", "social", "poster", "message", "notify", "slack"],
};

const AGENT_EXAMPLES: Record<string, { description: string; tryIt: string }> = {
  "research": { description: "Researches any topic and writes a comprehensive summary", tryIt: "Research the latest developments in quantum computing and write a 500-word summary" },
  "coder": { description: "Writes, reviews, and debugs code in any language", tryIt: "Write a Python function that finds the longest palindrome in a string" },
  "analyst": { description: "Analyzes data, finds patterns, and creates reports", tryIt: "Analyze the pros and cons of Rust vs Go for microservices" },
  "writer": { description: "Writes articles, blog posts, emails, and creative content", tryIt: "Write a professional email declining a meeting request politely" },
  "reviewer": { description: "Reviews code, documents, or proposals and provides feedback", tryIt: "Review this approach: using SQLite for a multi-user web application" },
  "security": { description: "Audits code and infrastructure for security vulnerabilities", tryIt: "List the top 5 security checks for a new REST API deployment" },
  "devops": { description: "Automates deployment, CI/CD, and infrastructure tasks", tryIt: "Write a GitHub Actions workflow that runs tests and deploys to staging" },
  "architect": { description: "Designs system architecture and makes technical decisions", tryIt: "Design a microservices architecture for a real-time chat application" },
  "debug": { description: "Diagnoses and fixes bugs in running applications", tryIt: "Debug why a React component re-renders infinitely on state change" },
  "data": { description: "Processes, transforms, and analyzes data sets", tryIt: "Write a SQL query to find the top 10 customers by revenue in the last 90 days" },
  "creative": { description: "Generates creative content, designs, and artistic ideas", tryIt: "Generate 5 unique name ideas for an AI-powered code review tool" },
  "monitor": { description: "Monitors system health and alerts on anomalies", tryIt: "Set up monitoring rules for API latency exceeding 500ms" },
  "social": { description: "Manages social media content and engagement", tryIt: "Draft a Twitter thread announcing a new open-source project" },
  "email": { description: "Manages email workflows and automated responses", tryIt: "Draft an automated welcome email for new user signups" },
  "deploy": { description: "Manages deployments across environments", tryIt: "Create a deployment checklist for a production release" },
};

function getAgentExample(name: string): { description: string; tryIt: string } | undefined {
  const lower = name.toLowerCase();
  for (const [key, val] of Object.entries(AGENT_EXAMPLES)) {
    if (lower.includes(key)) return val;
  }
  return undefined;
}

function inferCategory(agent: PreinstalledAgent): string {
  const lower = `${agent.name} ${agent.description}`.toLowerCase();
  for (const [cat, keywords] of Object.entries(CATEGORY_KEYWORDS)) {
    if (keywords.some(kw => lower.includes(kw))) return cat;
  }
  return "system";
}

function agentStatusLabel(status: string): AgentCardStatus {
  const s = status.toLowerCase();
  if (s === "running" || s === "starting") return "Running";
  if (s === "paused") return "Paused";
  if (s === "error") return "Error";
  return "Idle";
}

function statusTone(status: AgentCardStatus): string {
  return status.toLowerCase();
}

function truncateDescription(description: string, max = 110): string {
  if (description.length <= max) {
    return description;
  }
  return `${description.slice(0, max - 1).trimEnd()}…`;
}

/* ─── props ─── */

interface AgentsProps {
  agents: AgentSummary[];
  auditEvents: AuditEventRow[];
  factoryTrigger?: number;
  onStart: (id: string) => void;
  onPause: (id: string) => void;
  onStop: (id: string) => void;
  onCreate: (manifestJson: string) => void;
  onDelete: (id: string) => void;
  onClearAll?: () => void;
  onPermissions?: (id: string) => void;
  onNavigate?: (page: string) => void;
}

function makeActivityEntry(event: AuditEventRow, agentName: string): string {
  const payload = JSON.stringify(event.payload);
  const summary = payload.length > 52 ? `${payload.slice(0, 49)}...` : payload;
  const ok = event.event_type.toLowerCase().includes("error") ? "✕" : "✓";
  return `${agentName} > ${event.event_type}: ${summary} [${ok}]`;
}

/* ─── component ─── */

export function Agents({
  agents,
  auditEvents,
  factoryTrigger = 0,
  onStart,
  onPause,
  onStop,
  onCreate,
  onDelete,
  onClearAll,
  onPermissions,
  onNavigate,
}: AgentsProps): JSX.Element {
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(agents[0]?.id ?? null);
  const [showCreate, setShowCreate] = useState(false);
  const [detailOpen, setDetailOpen] = useState(false);
  const [detailTab, setDetailTab] = useState<AgentDetailTab>("overview");
  const [searchQuery, setSearchQuery] = useState("");
  const [autonomyFilter, setAutonomyFilter] = useState<AutonomyFilter | null>(null);
  const [categoryFilter] = useState<CategoryFilter>("all");
  const [showMonitoring, setShowMonitoring] = useState(false);
  const [preinstalledAgents, setPreinstalledAgents] = useState<PreinstalledAgent[]>([]);
  const [modelCount, setModelCount] = useState(0);
  const [expandedCardId, setExpandedCardId] = useState<string | null>(null);

  const [slmStatus] = useState<SlmStatus>({
    loaded: false, model_id: null, ram_usage_mb: 0,
    avg_latency_ms: 0, total_queries: 0, governance_routing: "cloud",
  });

  /* ─── goal execution state ─── */
  const [goalInput, setGoalInput] = useState("");
  const [goalRunning, setGoalRunning] = useState(false);
  const [goalPhase, setGoalPhase] = useState<string | null>(null);
  const [goalSteps, setGoalSteps] = useState(0);
  const [goalFuel, setGoalFuel] = useState(0);
  const goalInputRef = useRef<HTMLInputElement>(null);
  const [pendingConsents, setPendingConsents] = useState<ConsentNotification[]>([]);
  const [goalStepDetails, setGoalStepDetails] = useState<Array<{action: string; status: string; result: string; fuel_cost: number}>>([]);
  const [goalQuery, setGoalQuery] = useState("");
  const [availableProviders, setAvailableProviders] = useState<AvailableProvider[]>([]);
  const [selectedProvider, setSelectedProvider] = useState<string>("auto");
  const [selectedModel, setSelectedModel] = useState<string | null>(null);
  const [flashLoading, setFlashLoading] = useState(false);
  const [flashLoadError, setFlashLoadError] = useState<string | null>(null);
  const [showPermissionGate, setShowPermissionGate] = useState(false);
  const [asyncError, setAsyncError] = useState<string | null>(null);

  const startGoalExecution = useCallback(async () => {
    if (!selectedAgentId || !goalInput.trim() || goalRunning) return;
    setGoalRunning(true);
    setGoalPhase("Starting...");
    setGoalSteps(0);
    setGoalFuel(0);
    setGoalStepDetails([]);
    setPendingConsents([]);
    setGoalQuery(goalInput.trim());
    try {
      await executeAgentGoal(selectedAgentId, goalInput.trim(), 5);
    } catch (err) {
      setGoalPhase(`Error: ${err}`);
    }
  }, [selectedAgentId, goalInput, goalRunning]);

  const handleRunGoal = useCallback(async () => {
    if (!selectedAgentId || !goalInput.trim() || goalRunning) return;
    setShowPermissionGate(true);
  }, [selectedAgentId, goalInput, goalRunning]);

  /* ─── listen for cognitive loop events ─── */
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    const unlistenCycle = listen<{
      agent_id: string; phase: string; steps_executed: number;
      fuel_consumed: number; should_continue: boolean;
      blocked_reason?: string;
      steps?: Array<{action: string; status: string; result: string; fuel_cost: number}>;
    }>("agent-cognitive-cycle", (event) => {
      try {
        if (event.payload.agent_id !== selectedAgentId) return;
        setGoalPhase(String(event.payload.phase ?? ""));
        if (!event.payload.should_continue) {
          setGoalSteps(Number(event.payload.steps_executed) || 0);
          setGoalFuel(Number(event.payload.fuel_consumed) || 0);
          setGoalRunning(false);
          setGoalPhase("Complete");
        } else {
          setGoalSteps(prev => prev + (Number(event.payload.steps_executed) || 0));
          setGoalFuel(prev => prev + (Number(event.payload.fuel_consumed) || 0));
        }
        if (Array.isArray(event.payload.steps) && event.payload.steps.length > 0) {
          // Sanitize each step to ensure all fields are strings/numbers
          const safeSteps = event.payload.steps.map(s => ({
            action: String(s?.action ?? "unknown"),
            status: String(s?.status ?? "unknown"),
            result: String(s?.result ?? ""),
            fuel_cost: Number(s?.fuel_cost) || 0,
          }));
          setGoalStepDetails(prev => {
            const updated = [...prev, ...safeSteps];
            return updated.length > 200 ? updated.slice(-200) : updated;
          });
        }
        if (event.payload.blocked_reason) {
          setGoalPhase(`Blocked: ${String(event.payload.blocked_reason)}`);
        }
      } catch (err) {
        console.error("[agent-ui] error processing cycle event:", err);
      }
    });
    const unlistenComplete = listen("agent-goal-completed", (event: any) => {
      try {
        if (event.payload?.agent_id !== selectedAgentId) return;
        setGoalRunning(false);
        if (event.payload?.success === false) {
          const reason = String(event.payload?.reason ?? event.payload?.result_summary ?? "Unknown error");
          setGoalPhase(`Error: ${reason}`);
        } else {
          setGoalPhase("Complete");
        }
      } catch (err) {
        console.error("[agent-ui] error processing completion event:", err);
        setGoalRunning(false);
        setGoalPhase("Complete");
      }
    });
    // Listen for blocked events — fetch pending consents
    const unlistenBlocked = listen("agent-blocked", (event: any) => {
      if (event.payload?.agent_id === selectedAgentId) {
        listPendingConsents().then(consents => {
          const agentConsents = consents.filter(c => c.agent_id === selectedAgentId);
          setPendingConsents(agentConsents);
        }).catch((err) => {
          console.error("[agent-ui] Failed to fetch consent requests:", err);
        });
      }
    });
    // Also listen for direct consent-request-pending events (emitted alongside agent-blocked)
    const unlistenConsentPending = listen("consent-request-pending", (event: any) => {
      if (event.payload?.agent_id === selectedAgentId) {
        listPendingConsents().then(consents => {
          const agentConsents = consents.filter(c => c.agent_id === selectedAgentId);
          setPendingConsents(agentConsents);
        }).catch((err) => {
          console.error("[agent-ui] Failed to fetch consent requests on pending event:", err);
        });
      }
    });
    return () => {
      unlistenCycle.then(f => f());
      unlistenComplete.then(f => f());
      unlistenBlocked.then(f => f());
      unlistenConsentPending.then(f => f());
    };
  }, [selectedAgentId]);

  /* ─── load preinstalled agents + model count ─── */
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    getPreinstalledAgents().then(setPreinstalledAgents).catch(() => {});
    listProviderModels().then(m => setModelCount(m.length)).catch(() => {});
    getAvailableProviders().then(setAvailableProviders).catch(() => {});
  }, []);

  // Refresh provider status when agent is selected or every 15s while panel open
  useEffect(() => {
    if (!hasDesktopRuntime() || !selectedAgentId) return;
    getAvailableProviders().then(setAvailableProviders).catch(() => {});
    const interval = setInterval(() => {
      getAvailableProviders().then(setAvailableProviders).catch(() => {});
    }, 15000);
    return () => clearInterval(interval);
  }, [selectedAgentId]);

  /* ─── derived ─── */
  const activeCount = useMemo(
    () => agents.filter(a => a.status === "Running" || a.status === "Starting").length,
    [agents],
  );

  const selectedAgent = useMemo(
    () => agents.find(a => a.id === selectedAgentId) ?? null,
    [agents, selectedAgentId],
  );

  const selectedPreinstalled = useMemo(
    () => preinstalledAgents.find(pa => pa.agent_id === selectedAgentId) ?? null,
    [preinstalledAgents, selectedAgentId],
  );

  const latestByAgent = useMemo(() => {
    const map = new Map<string, AuditEventRow>();
    for (const event of auditEvents) {
      const previous = map.get(event.agent_id);
      if (!previous || event.timestamp > previous.timestamp) map.set(event.agent_id, event);
    }
    return map;
  }, [auditEvents]);

  const activityEntries = useMemo(() => {
    const nameById = new Map(agents.map(a => [a.id, a.name]));
    return [...auditEvents]
      .sort((l, r) => r.timestamp - l.timestamp)
      .slice(0, 20)
      .map(e => makeActivityEntry(e, nameById.get(e.agent_id) ?? e.agent_id));
  }, [agents, auditEvents]);

  // Merge preinstalled agent data with runtime agent summaries
  const enrichedAgents = useMemo(() => {
    return preinstalledAgents.map(pa => {
      const runtime = agents.find(a => a.id === pa.agent_id || a.name === pa.name);
      return { preinstalled: pa, runtime, category: inferCategory(pa) };
    });
  }, [preinstalledAgents, agents]);

  // If no preinstalled agents loaded, fall back to showing runtime agents as cards
  const hasPreinstalled = preinstalledAgents.length > 0;

  // Filter agents
  const filteredAgents = useMemo(() => {
    let list = hasPreinstalled ? enrichedAgents : agents.map(a => ({
      preinstalled: {
        agent_id: a.id, name: a.name, description: a.last_action ?? "",
        autonomy_level: 2, fuel_budget: a.fuel_budget ?? 10000,
        schedule: null, capabilities: a.capabilities ?? [], status: a.status,
      } as PreinstalledAgent,
      runtime: a,
      category: "system",
    }));

    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter(a =>
        a.preinstalled.name.toLowerCase().includes(q) ||
        a.preinstalled.description.toLowerCase().includes(q) ||
        a.preinstalled.capabilities.some(c => c.toLowerCase().includes(q)),
      );
    }

    if (autonomyFilter !== null && autonomyFilter !== "all") {
      list = list.filter(a => a.preinstalled.autonomy_level === autonomyFilter);
    }

    if (categoryFilter !== "all") {
      list = list.filter(a => a.category === categoryFilter);
    }

    return list;
  }, [hasPreinstalled, enrichedAgents, agents, searchQuery, autonomyFilter, categoryFilter]);

  // Viz data (only computed if monitoring is open)
  const graphNodes = useMemo(() => {
    if (!showMonitoring) return [];
    return agents.map(a => ({
      id: a.id,
      group: a.name.toLowerCase().includes("code") ? "coding"
        : a.name.toLowerCase().includes("social") ? "social"
        : a.name.toLowerCase().includes("design") ? "design"
        : "general",
      activity: latestByAgent.get(a.id) ? 0.65 : 0.28,
    }));
  }, [agents, latestByAgent, showMonitoring]);

  const graphEdges = useMemo(() => {
    if (!showMonitoring) return [];
    return agents.slice(1).map((a, i) => ({
      from: agents[i].id, to: a.id, weight: 0.42 + (i % 3) * 0.2,
    }));
  }, [agents, showMonitoring]);

  const heatmapValues = useMemo(() => {
    if (!showMonitoring) return Array.from({ length: 24 }, () => 0);
    const buckets = Array.from({ length: 24 }, () => 0);
    for (const e of auditEvents) {
      buckets[new Date(e.timestamp * 1000).getHours()] += 1;
    }
    const max = Math.max(1, ...buckets);
    return buckets.map(v => v / max);
  }, [auditEvents, showMonitoring]);

  useEffect(() => {
    if (factoryTrigger > 0) setShowCreate(true);
  }, [factoryTrigger]);

  useEffect(() => {
    if (agents.length === 0) { setSelectedAgentId(null); setDetailOpen(false); return; }
    if (!selectedAgentId || !agents.some(a => a.id === selectedAgentId)) setSelectedAgentId(agents[0].id);
  }, [agents, selectedAgentId]);

  const openDetail = useCallback((agentId: string, tab: AgentDetailTab = "overview") => {
    setSelectedAgentId(agentId);
    setDetailTab(tab);
    setDetailOpen(true);
  }, []);

  const navigateToChat = useCallback((agentId: string) => {
    if (onNavigate) {
      // Store agent ID in sessionStorage for the chat page to pick up
      sessionStorage.setItem("nexus-chat-agent", agentId);
      onNavigate("chat");
    }
  }, [onNavigate]);

  const totalTasks = auditEvents.length;

  // Catch unhandled promise rejections from async agent operations
  useEffect(() => {
    const handler = (event: PromiseRejectionEvent) => {
      console.error("[agent-ui] Unhandled rejection:", event.reason);
      setAsyncError(String(event.reason ?? "Unknown async error"));
      event.preventDefault();
    };
    window.addEventListener("unhandledrejection", handler);
    return () => window.removeEventListener("unhandledrejection", handler);
  }, []);

  return (
    <RequiresLlm feature="Agents">
    <section className="mission-control">
      <div className="mission-grid-overlay" />

      {/* ─── Async Error Banner ─── */}
      {asyncError && (
        <div style={{
          background: "#1a0000",
          border: "1px solid #ef444444",
          borderRadius: 8,
          padding: "12px 16px",
          marginBottom: 12,
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
        }}>
          <span style={{ color: "#fca5a5", fontSize: 13 }}>
            Agent error: {asyncError}
          </span>
          <button
            onClick={() => setAsyncError(null)}
            style={{
              background: "transparent",
              border: "1px solid #ef444444",
              color: "#ef4444",
              borderRadius: 6,
              padding: "4px 12px",
              cursor: "pointer",
              fontSize: 12,
            }}
          >
            Dismiss
          </button>
        </div>
      )}

      {/* ─── Header ─── */}
      <header className="mission-header">
        <div>
          <h2 className="mission-title">AGENT CONTROL // {activeCount} ACTIVE</h2>
          <p className="mission-subtitle">Mission-control view of governed runtime operations</p>
        </div>
        <div className="mission-header-actions">
          <div className="mission-active-counter">
            <span className="mission-active-hex">{activeCount}</span>
            <span className="mission-active-value">ACTIVE</span>
          </div>
          <button type="button" className="create-btn cursor-pointer" onClick={() => setShowCreate(true)}>
            <Plus size={16} /> CREATE AGENT
          </button>
          {onClearAll && (
            <button
              type="button"
              className="create-btn cursor-pointer"
              style={{ background: "#dc2626", borderColor: "#991b1b" }}
              onClick={() => {
                if (window.confirm(`Delete all ${agents.length} agents? This cannot be undone.`)) onClearAll();
              }}
            >
              CLEAR ALL
            </button>
          )}
        </div>
      </header>

      {/* ─── Stats Ribbon (hidden when no level selected) ─── */}
      {autonomyFilter !== null && <div className="mission-stats-ribbon">
        <div className="mission-stat-card glass-panel">
          <span className="mission-stat-icon"><Users size={18} /></span>
          <div>
            <span className="mission-stat-value">{hasPreinstalled ? preinstalledAgents.length : agents.length}</span>
            <span className="mission-stat-label">Total Agents</span>
          </div>
        </div>
        <div className="mission-stat-card glass-panel">
          <span className="mission-stat-icon" style={{ color: "var(--green)" }}><Zap size={18} /></span>
          <div>
            <span className="mission-stat-value">{activeCount}</span>
            <span className="mission-stat-label">Active</span>
          </div>
        </div>
        <div className="mission-stat-card glass-panel">
          <span className="mission-stat-icon" style={{ color: "var(--blue)" }}><Eye size={18} /></span>
          <div>
            <span className="mission-stat-value">{totalTasks}</span>
            <span className="mission-stat-label">Events Today</span>
          </div>
        </div>
        <div className="mission-stat-card glass-panel">
          <span className="mission-stat-icon" style={{ color: "var(--cyan, #06b6d4)" }}><Settings size={18} /></span>
          <div>
            <span className="mission-stat-value">{modelCount}</span>
            <span className="mission-stat-label">Available Models</span>
          </div>
        </div>
        <SlmStatusBadge status={slmStatus} />
      </div>}

      {/* ─── Level Selector ─── */}
      {autonomyFilter === null ? (
        <div style={{ padding: "40px 20px", textAlign: "center" }}>
          {/* ── Agent Search Bar ── */}
          <div style={{ maxWidth: 480, margin: "0 auto 28px auto", position: "relative" }}>
            <Search
              size={16}
              style={{
                position: "absolute",
                left: 14,
                top: "50%",
                transform: "translateY(-50%)",
                color: searchQuery ? "#22d3ee" : "#64748b",
                pointerEvents: "none",
              }}
            />
            <input
              type="text"
              placeholder="Search agents by name, capability, or description…"
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              style={{
                width: "100%",
                padding: "10px 14px 10px 40px",
                background: "#0d1117",
                border: searchQuery ? "1px solid #22d3ee" : "1px solid #30363d",
                borderRadius: 8,
                color: "#e2e8f0",
                fontSize: 14,
                outline: "none",
                transition: "border-color 0.2s",
                boxSizing: "border-box",
              }}
              onFocus={e => { e.target.style.borderColor = "#22d3ee"; }}
              onBlur={e => { if (!searchQuery) e.target.style.borderColor = "#30363d"; }}
            />
            {searchQuery && (
              <button
                type="button"
                onClick={() => setSearchQuery("")}
                style={{
                  position: "absolute",
                  right: 10,
                  top: "50%",
                  transform: "translateY(-50%)",
                  background: "none",
                  border: "none",
                  color: "#64748b",
                  cursor: "pointer",
                  fontSize: 16,
                  padding: "2px 6px",
                }}
              >
                ✕
              </button>
            )}
          </div>

          {/* ── Search Results (shown when searching) ── */}
          {searchQuery.trim() ? (
            <div style={{ maxWidth: 900, margin: "0 auto" }}>
              <p style={{ color: "#94a3b8", fontSize: 13, marginBottom: 16 }}>
                {filteredAgents.length} agent{filteredAgents.length !== 1 ? "s" : ""} matching "{searchQuery}"
              </p>
              {filteredAgents.length === 0 ? (
                <p style={{ color: "#64748b", fontSize: 14 }}>No agents found. Try a different search term.</p>
              ) : (
                <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(260px, 1fr))", gap: 12, textAlign: "left" }}>
                  {filteredAgents.slice(0, 20).map(({ preinstalled: pa }) => (
                    <button
                      key={pa.agent_id}
                      onClick={() => {
                        setAutonomyFilter(pa.autonomy_level as AutonomyFilter);
                        setSelectedAgentId(pa.agent_id);
                        setSearchQuery("");
                      }}
                      style={{
                        background: `${AUTONOMY_COLORS[pa.autonomy_level] ?? "#64748b"}10`,
                        border: `1px solid ${AUTONOMY_COLORS[pa.autonomy_level] ?? "#30363d"}66`,
                        borderRadius: 10,
                        padding: "12px 16px",
                        cursor: "pointer",
                        textAlign: "left",
                        transition: "transform 0.12s, border-color 0.2s",
                      }}
                      onMouseEnter={e => { (e.currentTarget).style.transform = "scale(1.02)"; (e.currentTarget).style.borderColor = AUTONOMY_COLORS[pa.autonomy_level] ?? "#30363d"; }}
                      onMouseLeave={e => { (e.currentTarget).style.transform = "scale(1)"; (e.currentTarget).style.borderColor = `${AUTONOMY_COLORS[pa.autonomy_level] ?? "#30363d"}66`; }}
                    >
                      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                        <span style={{ fontSize: 11, fontWeight: 700, color: AUTONOMY_COLORS[pa.autonomy_level], background: `${AUTONOMY_COLORS[pa.autonomy_level]}22`, padding: "2px 6px", borderRadius: 4 }}>
                          L{pa.autonomy_level}
                        </span>
                        <span style={{ fontSize: 14, fontWeight: 600, color: "#e2e8f0" }}>{pa.name}</span>
                      </div>
                      <p style={{ fontSize: 12, color: "#94a3b8", margin: 0, lineHeight: 1.4 }}>
                        {truncateDescription(pa.description, 80)}
                      </p>
                    </button>
                  ))}
                </div>
              )}
            </div>
          ) : (
          <>
          <h2 style={{ color: "#e2e8f0", fontSize: 20, fontWeight: 600, marginBottom: 8 }}>
            Select an Autonomy Level
          </h2>
          <p style={{ color: "#94a3b8", fontSize: 14, marginBottom: 32 }}>
            Each level defines how much independence an agent has. Higher levels require more governance.
          </p>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 12, justifyContent: "center", maxWidth: 900, margin: "0 auto" }}>
            {([1, 2, 3, 4, 5, 6] as number[]).map(level => {
              const count = (hasPreinstalled ? enrichedAgents : []).filter(a => a.preinstalled.autonomy_level === level).length;
              return (
                <button
                  key={level}
                  onClick={() => setAutonomyFilter(level as AutonomyFilter)}
                  style={{
                    background: `${AUTONOMY_COLORS[level]}15`,
                    border: `2px solid ${AUTONOMY_COLORS[level]}`,
                    borderRadius: 12,
                    padding: "20px 28px",
                    cursor: "pointer",
                    display: "flex",
                    flexDirection: "column",
                    alignItems: "center",
                    gap: 8,
                    minWidth: 130,
                    transition: "transform 0.15s, box-shadow 0.15s",
                  }}
                  onMouseEnter={e => { (e.target as HTMLElement).style.transform = "scale(1.05)"; (e.target as HTMLElement).style.boxShadow = `0 0 20px ${AUTONOMY_COLORS[level]}44`; }}
                  onMouseLeave={e => { (e.target as HTMLElement).style.transform = "scale(1)"; (e.target as HTMLElement).style.boxShadow = "none"; }}
                >
                  <span style={{ fontSize: 28, fontWeight: 800, color: AUTONOMY_COLORS[level] }}>L{level}</span>
                  <span style={{ fontSize: 12, color: "#e2e8f0", fontWeight: 500 }}>{AUTONOMY_LABELS[level]}</span>
                  <span style={{ fontSize: 11, color: "#64748b" }}>{count} agent{count !== 1 ? "s" : ""}</span>
                </button>
              );
            })}
          </div>
          </>
          )}
        </div>
      ) : (
        <div style={{ display: "flex", alignItems: "center", gap: 16, padding: "12px 0" }}>
          <button
            onClick={() => { setAutonomyFilter(null); setSelectedAgentId(null); }}
            style={{
              color: "#94a3b8",
              border: "1px solid #30363d",
              borderRadius: 6,
              padding: "6px 14px",
              fontSize: 13,
              background: "transparent",
              cursor: "pointer",
            }}
          >
            ← Back
          </button>
          <span style={{
            fontSize: 18,
            fontWeight: 700,
            color: AUTONOMY_COLORS[autonomyFilter as number] ?? "#e2e8f0",
          }}>
            L{autonomyFilter} — {AUTONOMY_LABELS[autonomyFilter as number] ?? "Unknown"}
          </span>
          <span style={{ fontSize: 13, color: "#64748b" }}>
            {filteredAgents.length} agent{filteredAgents.length !== 1 ? "s" : ""}
          </span>
          {selectedAgentId && (
            <span style={{
              marginLeft: "auto",
              fontSize: 13,
              color: "#22d3ee",
              fontWeight: 500,
            }}>
              Selected: {selectedPreinstalled?.name ?? selectedAgentId.slice(0, 8)}
            </span>
          )}
        </div>
      )}

      {/* ─── Agent Selector (compact list) ─── */}
      {autonomyFilter !== null && (
      <div style={{ marginBottom: 12 }}>
        {filteredAgents.length === 0 ? (
          <div style={{ padding: "20px", textAlign: "center", color: "#64748b" }}>
            No agents at this level.
          </div>
        ) : (
          <div style={{
            display: "flex",
            flexWrap: "wrap",
            gap: 8,
          }}>
            {filteredAgents.map(({ preinstalled: pa, runtime }) => {
              const agentId = runtime?.id ?? pa.agent_id;
              const status = runtime?.status ?? pa.status;
              const statusLabel = agentStatusLabel(status);
              const isSelected = selectedAgentId === agentId;
              const levelColor = AUTONOMY_COLORS[pa.autonomy_level] ?? "#64748b";

              return (
                <button
                  key={agentId}
                  onClick={() => setSelectedAgentId(isSelected ? null : agentId)}
                  style={{
                    background: isSelected ? `${levelColor}22` : "#161b22",
                    border: `1px solid ${isSelected ? levelColor : "#30363d"}`,
                    borderRadius: 8,
                    padding: "8px 14px",
                    cursor: "pointer",
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    transition: "all 0.15s",
                    boxShadow: isSelected ? `0 0 12px ${levelColor}33` : "none",
                  }}
                >
                  <span style={{
                    width: 8,
                    height: 8,
                    borderRadius: "50%",
                    background: statusLabel === "Running" ? "#22c55e"
                      : statusLabel === "Paused" ? "#f59e0b"
                      : statusLabel === "Error" ? "#ef4444"
                      : "#64748b",
                    flexShrink: 0,
                  }} />
                  <span style={{
                    color: isSelected ? "#e2e8f0" : "#94a3b8",
                    fontSize: 13,
                    fontWeight: isSelected ? 600 : 400,
                    whiteSpace: "nowrap",
                  }}>
                    {pa.name}
                  </span>
                  {pa.llm_model && pa.llm_model !== "auto" ? (
                    <span style={{ fontSize: 10, color: "#22d3ee", opacity: 0.7 }} title={pa.llm_model}>
                      {pa.llm_model.startsWith("flash") ? "\u26A1" : pa.llm_model.startsWith("ollama") ? "\uD83E\uDD99" : "\u2601\uFE0F"}
                    </span>
                  ) : (
                    <span style={{ fontSize: 10, color: "#a78bfa", opacity: 0.5 }} title="Auto — best available">
                      {"\u2B50"}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
        )}

        {/* Selected agent detail strip */}
        {selectedPreinstalled && (
          <div style={{
            marginTop: 12,
            background: "#0d1117",
            border: `1px solid ${AUTONOMY_COLORS[selectedPreinstalled.autonomy_level] ?? "#30363d"}44`,
            borderRadius: 8,
            padding: "10px 16px",
            display: "flex",
            alignItems: "center",
            gap: 12,
            flexWrap: "wrap",
          }}>
            <span style={{ color: "#e2e8f0", fontWeight: 600, fontSize: 14 }}>
              {selectedPreinstalled.name}
            </span>
            <span style={{
              fontSize: 11,
              color: AUTONOMY_COLORS[selectedPreinstalled.autonomy_level],
              fontWeight: 700,
            }}>
              L{selectedPreinstalled.autonomy_level}
            </span>
            <span style={{
              fontSize: 11,
              color: selectedProvider === "auto" ? "#a78bfa" : "#22d3ee",
              background: selectedProvider === "auto" ? "rgba(167,139,250,0.1)" : "rgba(34,211,238,0.1)",
              border: `1px solid ${selectedProvider === "auto" ? "rgba(167,139,250,0.2)" : "rgba(34,211,238,0.2)"}`,
              borderRadius: 4,
              padding: "1px 8px",
            }}>
              {selectedProvider === "auto"
                ? "Auto — best available"
                : `${selectedProvider === "flash" ? "\u26A1" : selectedProvider === "ollama" ? "\uD83E\uDD99" : "\u2601\uFE0F"} ${availableProviders.find(p => p.id === selectedProvider)?.name ?? selectedProvider}`
              }
            </span>
            <span style={{ fontSize: 11, color: "#64748b" }}>
              Fuel {selectedPreinstalled.fuel_budget.toLocaleString()}
            </span>
            <span style={{ fontSize: 12, color: "#94a3b8", flex: 1, minWidth: 200 }}>
              {truncateDescription(selectedPreinstalled.description)}
            </span>
            {selectedPreinstalled.capabilities.slice(0, 4).map(cap => (
              <span key={cap} style={{
                fontSize: 10,
                color: "#8b949e",
                background: "rgba(139,148,158,0.1)",
                borderRadius: 4,
                padding: "1px 6px",
              }}>
                {cap}
              </span>
            ))}
          </div>
        )}
      </div>
      )}

      {/* ─── Goal Execution Panel (immediately after agent selector) ─── */}
      {autonomyFilter !== null && (<>

      {/* ─── Detail Panel ─── */}
      <AgentDetail
        open={detailOpen}
        agent={selectedAgent}
        auditEvents={auditEvents}
        activeTab={detailTab}
        onTabChange={setDetailTab}
        onClose={() => setDetailOpen(false)}
        onStart={onStart}
        onStop={onStop}
        onPause={onPause}
        onResume={onStart}
      />

      {/* ─── Permissions Preview (shown when agent first selected) ─── */}
      {selectedPreinstalled && !goalRunning && !goalPhase && (
        <div style={{
          background: "#0d1117",
          border: "1px solid #1e3a5f",
          borderRadius: 8,
          padding: "12px 16px",
          marginBottom: 12,
        }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 10 }}>
            <Shield size={14} color="#f59e0b" />
            <span style={{ color: "#e0e0e0", fontWeight: 600, fontSize: 13 }}>
              Permissions — {selectedPreinstalled.name}
            </span>
            <span style={{
              fontSize: 11,
              color: AUTONOMY_COLORS[selectedPreinstalled.autonomy_level],
              fontWeight: 700,
            }}>
              L{selectedPreinstalled.autonomy_level}
            </span>
          </div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 8 }}>
            {selectedPreinstalled.capabilities.map(cap => (
              <span key={cap} style={{
                fontSize: 12,
                color: "#e2e8f0",
                background: "rgba(34,211,238,0.08)",
                border: "1px solid rgba(34,211,238,0.2)",
                borderRadius: 6,
                padding: "3px 10px",
              }}>
                {cap}
              </span>
            ))}
          </div>
          <div style={{ fontSize: 11, color: "#64748b" }}>
            {selectedPreinstalled.autonomy_level >= 3
              ? "This agent requires HITL approval for Tier1+ operations."
              : "Standard capability checks apply. Actions are logged to the audit trail."}
          </div>
        </div>
      )}

      {/* ─── Provider Selector ─── */}
      {selectedAgentId && !goalRunning && availableProviders.length > 0 && (
        <div style={{
          background: "#0d1117",
          border: "1px solid #1e3a5f",
          borderRadius: 8,
          padding: "12px 16px",
          marginBottom: 12,
        }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 10 }}>
            <Zap size={14} color="#a78bfa" />
            <span style={{ color: "#e0e0e0", fontWeight: 600, fontSize: 13 }}>LLM Provider</span>
          </div>
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            {/* Auto option */}
            <button
              onClick={() => {
                setSelectedProvider("auto");
                if (selectedAgentId) {
                  setAgentLlmProvider(selectedAgentId, "auto", true, 0, 0).catch(() => {});
                }
              }}
              style={{
                background: selectedProvider === "auto" ? "rgba(167,139,250,0.15)" : "#161b22",
                border: `1px solid ${selectedProvider === "auto" ? "#a78bfa" : "#30363d"}`,
                borderRadius: 8,
                padding: "8px 14px",
                cursor: "pointer",
                display: "flex",
                flexDirection: "column",
                alignItems: "flex-start",
                gap: 2,
                minWidth: 100,
              }}
            >
              <span style={{ fontSize: 13, fontWeight: 600, color: selectedProvider === "auto" ? "#a78bfa" : "#e2e8f0" }}>
                Auto
              </span>
              <span style={{ fontSize: 10, color: "#64748b" }}>Best available</span>
            </button>

            {availableProviders.map(p => {
              const isSelected = selectedProvider === p.id;
              const icon = p.id === "flash" ? "\u26A1" : p.id === "ollama" ? "\uD83E\uDD99" : "\u2601\uFE0F";
              const statusColor = p.status === "ready" || p.status === "running" || p.status === "configured" || p.status === "models_on_disk" ? "#22c55e"
                : p.status === "busy" ? "#f59e0b"
                : p.status === "stopped" ? "#ef4444"
                : "#64748b";
              return (
                <button
                  key={p.id}
                  onClick={() => {
                    if (!p.available) return;
                    setSelectedProvider(p.id);
                    // Auto-select first model when switching to a provider with models
                    const firstModel = p.model ?? p.models[0] ?? null;
                    setSelectedModel(firstModel);
                    if (selectedAgentId) {
                      const modelPart = firstModel ? `/${firstModel}` : "";
                      setAgentLlmProvider(selectedAgentId, `${p.id}${modelPart}`, p.id === "flash" || p.id === "ollama", 0, 0).catch(() => {});
                    }
                  }}
                  style={{
                    background: isSelected ? "rgba(34,211,238,0.1)" : "#161b22",
                    border: `1px solid ${isSelected ? "#22d3ee" : "#30363d"}`,
                    borderRadius: 8,
                    padding: "8px 14px",
                    cursor: p.available ? "pointer" : "not-allowed",
                    opacity: p.available ? 1 : 0.5,
                    display: "flex",
                    flexDirection: "column",
                    alignItems: "flex-start",
                    gap: 2,
                    minWidth: 100,
                  }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                    <span style={{ fontSize: 14 }}>{icon}</span>
                    <span style={{ fontSize: 13, fontWeight: 600, color: isSelected ? "#22d3ee" : "#e2e8f0" }}>
                      {p.name}
                    </span>
                  </div>
                  <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                    <div style={{ width: 6, height: 6, borderRadius: "50%", background: statusColor }} />
                    <span style={{ fontSize: 10, color: "#64748b" }}>
                      {p.status === "ready" ? (p.model ?? "Ready")
                        : p.status === "busy" ? `${p.model ?? "Generating"}...`
                        : p.status === "running" ? (p.model ?? "Running")
                        : p.status === "models_on_disk" ? `${p.models.length} model${p.models.length !== 1 ? "s" : ""} on disk`
                        : p.status === "configured" ? "API key set"
                        : p.status === "stopped" ? "Not running"
                        : p.status === "no_model" ? "No models"
                        : "Not configured"}
                    </span>
                  </div>
                </button>
              );
            })}
          </div>

          {/* ─── Model selector + Load button ─── */}
          {(() => {
            const prov = availableProviders.find(p => p.id === selectedProvider);
            if (!prov || prov.models.length === 0) return null;
            const currentModel = selectedModel ?? prov.model ?? prov.models[0] ?? "";
            const currentModelIdx = prov.models.indexOf(currentModel);
            const currentModelPath = currentModelIdx >= 0 && currentModelIdx < prov.model_paths.length
              ? prov.model_paths[currentModelIdx]
              : "";
            const needsLoad = selectedProvider === "flash" && (prov.status === "models_on_disk");
            const isLoaded = selectedProvider === "flash" && (prov.status === "ready" || prov.status === "busy");
            return (
              <div style={{ marginTop: 10 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
                  <span style={{ fontSize: 12, color: "#94a3b8" }}>Model:</span>
                  <select
                    value={currentModel}
                    onChange={(e) => {
                      const model = e.target.value;
                      setSelectedModel(model);
                      setFlashLoadError(null);
                      if (selectedAgentId) {
                        setAgentLlmProvider(selectedAgentId, `${selectedProvider}/${model}`, selectedProvider === "flash" || selectedProvider === "ollama", 0, 0).catch(() => {});
                      }
                    }}
                    style={{
                      background: "#161b22",
                      color: "#e2e8f0",
                      border: "1px solid #30363d",
                      borderRadius: 6,
                      padding: "4px 10px",
                      fontSize: 12,
                      flex: 1,
                      maxWidth: 400,
                    }}
                  >
                    {prov.models.map(m => (
                      <option key={m} value={m}>{m}</option>
                    ))}
                  </select>
                  {isLoaded && (
                    <span style={{ fontSize: 11, color: "#22c55e", fontWeight: 600, display: "flex", alignItems: "center", gap: 4 }}>
                      <div style={{ width: 6, height: 6, borderRadius: "50%", background: "#22c55e" }} />
                      Loaded
                    </span>
                  )}
                  {prov.status === "busy" && (
                    <span style={{ fontSize: 11, color: "#f59e0b", fontWeight: 600 }}>Busy</span>
                  )}
                  {needsLoad && !flashLoading && (
                    <button
                      onClick={async () => {
                        if (!currentModelPath) {
                          setFlashLoadError("No model path found");
                          return;
                        }
                        setFlashLoading(true);
                        setFlashLoadError(null);
                        try {
                          await flashCreateSession(currentModelPath, 8192, "balanced");
                          // Refresh providers to get updated status
                          const updated = await getAvailableProviders();
                          setAvailableProviders(updated);
                        } catch (err) {
                          setFlashLoadError(err instanceof Error ? err.message : String(err));
                        } finally {
                          setFlashLoading(false);
                        }
                      }}
                      style={{
                        background: "rgba(34,211,238,0.15)",
                        border: "1px solid #22d3ee",
                        borderRadius: 6,
                        padding: "4px 14px",
                        color: "#22d3ee",
                        fontSize: 12,
                        fontWeight: 600,
                        cursor: "pointer",
                        whiteSpace: "nowrap",
                      }}
                    >
                      Load Model
                    </button>
                  )}
                  {flashLoading && (
                    <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                      <Loader2 size={14} color="#22d3ee" style={{ animation: "spin 1s linear infinite" }} />
                      <span style={{ fontSize: 11, color: "#22d3ee" }}>Loading model...</span>
                    </span>
                  )}
                </div>
                {flashLoadError && (
                  <div style={{ fontSize: 11, color: "#ef4444", marginTop: 4 }}>
                    Failed to load: {flashLoadError}
                  </div>
                )}
              </div>
            );
          })()}
        </div>
      )}

      {/* ─── Goal Input ─── */}
      {selectedAgentId && (
        <div style={{
          background: "#0d1117",
          border: "1px solid #1e3a5f",
          borderRadius: 8,
          padding: "12px 16px",
          marginBottom: 12,
        }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
            <Zap size={16} color="#00e5ff" />
            <span style={{ color: "#e0e0e0", fontWeight: 600, fontSize: 14 }}>
              Run Goal
            </span>
            <span style={{
              fontSize: 11,
              color: selectedProvider === "auto" ? "#a78bfa" : "#22d3ee",
              background: selectedProvider === "auto" ? "rgba(167,139,250,0.1)" : "rgba(34,211,238,0.1)",
              border: `1px solid ${selectedProvider === "auto" ? "rgba(167,139,250,0.2)" : "rgba(34,211,238,0.2)"}`,
              borderRadius: 4,
              padding: "1px 8px",
              fontWeight: 500,
            }}>
              {selectedProvider === "auto" ? "Auto" : `${selectedProvider === "flash" ? "\u26A1" : selectedProvider === "ollama" ? "\uD83E\uDD99" : "\u2601\uFE0F"} ${selectedModel ?? selectedProvider}`}
            </span>
            {goalPhase && (
              <span style={{
                marginLeft: "auto",
                fontSize: 12,
                color: goalPhase === "Complete" ? "#22c55e"
                  : goalPhase.startsWith("Error") ? "#ef4444"
                  : "#00e5ff",
                display: "flex", alignItems: "center", gap: 4,
              }}>
                {goalRunning && <Loader2 size={12} style={{ animation: "spin 1s linear infinite" }} />}
                {goalPhase}
                {goalSteps > 0 && ` · ${goalSteps} steps · ${goalFuel.toFixed(0)} fuel`}
              </span>
            )}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <input
              ref={goalInputRef}
              type="text"
              value={goalInput}
              onChange={e => setGoalInput(e.target.value)}
              onKeyDown={e => e.key === "Enter" && handleRunGoal()}
              placeholder="Describe a goal for this agent..."
              disabled={goalRunning}
              style={{
                flex: 1,
                background: "#161b22",
                border: "1px solid #30363d",
                borderRadius: 6,
                padding: "8px 12px",
                color: "#e0e0e0",
                fontSize: 13,
                outline: "none",
              }}
            />
            <button
              onClick={handleRunGoal}
              disabled={goalRunning || !goalInput.trim()}
              style={{
                background: goalRunning ? "#1e3a5f" : "#00e5ff",
                color: goalRunning ? "#64748b" : "#0d1117",
                border: "none",
                borderRadius: 6,
                padding: "8px 16px",
                cursor: goalRunning ? "not-allowed" : "pointer",
                display: "flex",
                alignItems: "center",
                gap: 6,
                fontWeight: 600,
                fontSize: 13,
              }}
            >
              {goalRunning ? <Loader2 size={14} style={{ animation: "spin 1s linear infinite" }} /> : <Send size={14} />}
              {goalRunning ? "Running..." : "Run"}
            </button>
          </div>
        </div>
      )}

      {/* ─── Inline Approval Panel ─── */}
      {pendingConsents.length > 0 && (
        <div style={{
          background: "#1a1200",
          border: "1px solid #f59e0b44",
          borderRadius: 8,
          padding: "12px 16px",
          marginBottom: 12,
        }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 10 }}>
            <Shield size={16} color="#f59e0b" />
            <span style={{ color: "#fbbf24", fontWeight: 600, fontSize: 14 }}>
              Agent needs your approval
            </span>
            <span style={{ fontSize: 12, color: "#92400e" }}>
              {pendingConsents.length} action{pendingConsents.length !== 1 ? "s" : ""} pending
            </span>
          </div>
          {pendingConsents.map(consent => (
            <div key={consent.consent_id} style={{
              background: "#0d1117",
              border: "1px solid #30363d",
              borderRadius: 6,
              padding: "10px 12px",
              marginBottom: 8,
            }}>
              <div style={{ fontSize: 13, color: "#e2e8f0", marginBottom: 6 }}>
                <strong>{consent.operation_type}:</strong> {consent.operation_summary}
              </div>
              {Array.isArray(consent.side_effects_preview) && consent.side_effects_preview.length > 0 && (
                <div style={{ fontSize: 11, color: "#94a3b8", marginBottom: 8 }}>
                  {consent.side_effects_preview.map((effect, i) => (
                    <div key={i}>• {effect}</div>
                  ))}
                </div>
              )}
              <div style={{ display: "flex", gap: 8 }}>
                <button
                  onClick={async () => {
                    try {
                      await approveConsentRequest(consent.consent_id, "user");
                      setPendingConsents(prev => prev.filter(c => c.consent_id !== consent.consent_id));
                    } catch (err) {
                      console.error("[agent-ui] Approve failed:", err);
                      setGoalPhase(`Approval error: ${err}`);
                    }
                  }}
                  style={{
                    background: "#22c55e",
                    color: "#0d1117",
                    border: "none",
                    borderRadius: 6,
                    padding: "6px 16px",
                    cursor: "pointer",
                    fontWeight: 600,
                    fontSize: 12,
                  }}
                >
                  Approve
                </button>
                <button
                  onClick={async () => {
                    try {
                      for (const c of pendingConsents) {
                        await approveConsentRequest(c.consent_id, "user");
                      }
                      setPendingConsents([]);
                    } catch (err) {
                      console.error("[agent-ui] Approve All failed:", err);
                      setGoalPhase(`Approval error: ${err}`);
                    }
                  }}
                  style={{
                    background: "transparent",
                    color: "#22d3ee",
                    border: "1px solid #22d3ee44",
                    borderRadius: 6,
                    padding: "6px 16px",
                    cursor: "pointer",
                    fontWeight: 600,
                    fontSize: 12,
                  }}
                >
                  Approve All
                </button>
                <button
                  onClick={async () => {
                    try {
                      await denyConsentRequest(consent.consent_id, "user", "User denied");
                      setPendingConsents(prev => prev.filter(c => c.consent_id !== consent.consent_id));
                      setGoalRunning(false);
                      setGoalPhase("Denied by user");
                    } catch (err) {
                      console.error("[agent-ui] Deny failed:", err);
                      setGoalPhase(`Deny error: ${err}`);
                    }
                  }}
                  style={{
                    background: "transparent",
                    color: "#ef4444",
                    border: "1px solid #ef444444",
                    borderRadius: 6,
                    padding: "6px 16px",
                    cursor: "pointer",
                    fontWeight: 600,
                    fontSize: 12,
                  }}
                >
                  Deny
                </button>
                <span style={{ fontSize: 11, color: "#64748b", alignSelf: "center" }}>
                  Fuel cost: {consent.fuel_cost_estimate}
                </span>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* ─── Permission Gate (pre-run approval) ─── */}
      {showPermissionGate && selectedPreinstalled && (
        <div style={{
          background: "#0d1117",
          border: "1px solid #1e3a5f",
          borderRadius: 10,
          padding: "16px 20px",
          marginBottom: 12,
        }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 12 }}>
            <Shield size={18} color="#f59e0b" />
            <span style={{ color: "#e2e8f0", fontWeight: 700, fontSize: 15 }}>
              {selectedPreinstalled.name} wants to run:
            </span>
          </div>
          <div style={{
            background: "#161b22",
            border: "1px solid #30363d",
            borderRadius: 8,
            padding: "10px 14px",
            marginBottom: 14,
            fontSize: 13,
            color: "#e2e8f0",
            fontStyle: "italic",
          }}>
            &ldquo;{goalInput.trim()}&rdquo;
          </div>

          <div style={{ marginBottom: 14 }}>
            <div style={{ fontSize: 12, color: "#94a3b8", marginBottom: 6, fontWeight: 600 }}>
              Required Permissions:
            </div>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
              {selectedPreinstalled.capabilities.map(cap => (
                <span key={cap} style={{
                  fontSize: 11,
                  color: "#e2e8f0",
                  background: "rgba(34,211,238,0.08)",
                  border: "1px solid rgba(34,211,238,0.2)",
                  borderRadius: 5,
                  padding: "3px 10px",
                }}>
                  {cap === "process.exec" ? "\uD83D\uDD27" : cap === "fs.read" ? "\uD83D\uDCC4" : cap === "llm.query" ? "\uD83E\uDD16" : cap === "fs.write" ? "\u270F\uFE0F" : "\uD83D\uDD12"} {cap}
                </span>
              ))}
            </div>
          </div>

          <div style={{ fontSize: 12, color: "#94a3b8", marginBottom: 10 }}>
            How should this agent handle operations?
          </div>

          <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
            <button
              onClick={() => {
                setShowPermissionGate(false);
                startGoalExecution();
              }}
              style={{
                background: "rgba(34,211,238,0.12)",
                border: "1px solid #22d3ee",
                borderRadius: 8,
                padding: "10px 20px",
                cursor: "pointer",
                display: "flex",
                flexDirection: "column",
                alignItems: "flex-start",
                gap: 2,
                minWidth: 130,
              }}
            >
              <span style={{ fontSize: 14, fontWeight: 700, color: "#22d3ee" }}>
                Auto
              </span>
              <span style={{ fontSize: 11, color: "#64748b" }}>Agent runs freely. Actions are logged.</span>
            </button>

            <button
              onClick={() => {
                setShowPermissionGate(false);
                // Enable review-each mode via the cognitive runtime
                // This uses the existing HITL system — the agent will pause at each sensitive action
                if (selectedAgentId) {
                  setAgentReviewMode(selectedAgentId, true).catch(() => {});
                }
                startGoalExecution();
              }}
              style={{
                background: "rgba(245,158,11,0.08)",
                border: "1px solid #f59e0b44",
                borderRadius: 8,
                padding: "10px 20px",
                cursor: "pointer",
                display: "flex",
                flexDirection: "column",
                alignItems: "flex-start",
                gap: 2,
                minWidth: 130,
              }}
            >
              <span style={{ fontSize: 14, fontWeight: 700, color: "#fbbf24" }}>
                Ask Me
              </span>
              <span style={{ fontSize: 11, color: "#64748b" }}>I approve each action one by one.</span>
            </button>

            <button
              onClick={() => {
                setShowPermissionGate(false);
              }}
              style={{
                background: "transparent",
                border: "1px solid #30363d",
                borderRadius: 8,
                padding: "10px 20px",
                cursor: "pointer",
                display: "flex",
                flexDirection: "column",
                alignItems: "flex-start",
                gap: 2,
                minWidth: 130,
              }}
            >
              <span style={{ fontSize: 14, fontWeight: 700, color: "#64748b" }}>
                Cancel
              </span>
              <span style={{ fontSize: 11, color: "#475569" }}>Don&apos;t run this task.</span>
            </button>
          </div>

          <div style={{ fontSize: 11, color: "#475569", marginTop: 10 }}>
            Agent autonomy: L{selectedPreinstalled.autonomy_level} — {
              selectedPreinstalled.autonomy_level <= 1 ? "Suggest only" :
              selectedPreinstalled.autonomy_level === 2 ? "Act with approval" :
              selectedPreinstalled.autonomy_level === 3 ? "Act then report" :
              selectedPreinstalled.autonomy_level === 4 ? "Fully autonomous" :
              "Transcendent"
            }
          </div>
        </div>
      )}

      {/* ─── Chat-style Agent Output ─── */}
      {selectedAgentId && (
        <div style={{
          background: "#0d1117",
          border: "1px solid #30363d",
          borderRadius: 8,
          overflow: "hidden",
        }}>
          <div style={{ maxHeight: 500, overflowY: "auto", padding: "12px 16px", display: "flex", flexDirection: "column", gap: 8 }}>
            {!goalQuery && !goalRunning && !goalPhase && (
              <div style={{ padding: 32, textAlign: "center", opacity: 0.4, fontSize: 13 }}>
                Give the agent a goal to see it work.
              </div>
            )}
            {goalQuery && (
              <div style={{ display: "flex", justifyContent: "flex-end", padding: "4px 0" }}>
                <div style={{
                  background: "#1e3a5f",
                  border: "1px solid #2563eb44",
                  borderRadius: "12px 12px 4px 12px",
                  padding: "8px 14px",
                  maxWidth: "80%",
                  fontSize: 13,
                  color: "#e2e8f0",
                  lineHeight: 1.5,
                }}>
                  {goalQuery}
                </div>
              </div>
            )}
            {goalStepDetails.length === 0 && goalRunning && (
              <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "12px 0" }}>
                <div style={{ width: 28, height: 28, borderRadius: "50%", background: "#1e3a5f", display: "flex", alignItems: "center", justifyContent: "center", flexShrink: 0 }}>
                  <Loader2 size={14} color="#00e5ff" style={{ animation: "spin 1s linear infinite" }} />
                </div>
                <span style={{ color: "#94a3b8", fontSize: 13, fontStyle: "italic" }}>Thinking...</span>
              </div>
            )}
            {goalStepDetails.map((step, i) => {
              const act = String(step.action ?? "unknown");
              const status = String(step.status ?? "unknown");
              const result = String(step.result ?? "");
              const fuelCost = Number(step.fuel_cost) || 0;
              const isLlm = act === "llm_query";
              const isShell = act === "shell_command";
              const isRead = act === "file_read";
              const isWrite = act === "file_write";
              const isWeb = act === "web_search" || act === "web_fetch";
              const isCode = act === "code_execute";
              const icon = isLlm ? "🧠" : isShell ? "⚡" : isRead ? "📄" : isWrite ? "✏️" : isWeb ? "🌐" : isCode ? "💻" : "🔧";
              const label = isLlm ? "Querying LLM"
                : isShell ? "Running command"
                : isRead ? "Reading file"
                : isWrite ? "Writing file"
                : isWeb ? "Web search"
                : isCode ? "Running code"
                : act.replace(/_/g, " ");
              const hasResult = result.trim().length > 0;
              const succeeded = status === "succeeded" || status === "ok";

              return (
                <div key={i} style={{ display: "flex", gap: 10, alignItems: "flex-start" }}>
                  <div style={{
                    width: 28, height: 28, borderRadius: "50%",
                    background: succeeded ? "#0d2818" : "#2d0a0a",
                    border: `1px solid ${succeeded ? "#22c55e33" : "#ef444433"}`,
                    display: "flex", alignItems: "center", justifyContent: "center",
                    flexShrink: 0, fontSize: 13,
                  }}>
                    {icon}
                  </div>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 4 }}>
                      <span style={{ color: "#e2e8f0", fontSize: 13, fontWeight: 600 }}>{label}</span>
                      {fuelCost > 0 && (
                        <span style={{ fontSize: 10, color: "#64748b" }}>{fuelCost} fuel</span>
                      )}
                    </div>
                    {hasResult && (
                      <div style={{
                        background: isLlm ? "#0a1628" : "#161b22",
                        border: `1px solid ${isLlm ? "#1e3a5f" : "#30363d"}`,
                        borderRadius: 8,
                        padding: "8px 12px",
                        fontSize: 13,
                        lineHeight: 1.6,
                        color: isLlm ? "#e2e8f0" : "#94a3b8",
                        whiteSpace: "pre-wrap",
                        wordBreak: "break-word",
                        maxHeight: isLlm ? "none" : 200,
                        overflow: isLlm ? "visible" : "auto",
                      }}>
                        {isLlm ? result : (
                          result.length > 500
                            ? result.slice(0, 500) + "..."
                            : result
                        )}
                      </div>
                    )}
                  </div>
                </div>
              );
            })}
            {goalRunning && goalStepDetails.length > 0 && (
              <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "4px 0" }}>
                <div style={{ width: 28, height: 28, borderRadius: "50%", background: "#1e3a5f", display: "flex", alignItems: "center", justifyContent: "center", flexShrink: 0 }}>
                  <Loader2 size={14} color="#00e5ff" style={{ animation: "spin 1s linear infinite" }} />
                </div>
                <span style={{ color: "#64748b", fontSize: 12, fontStyle: "italic" }}>Working...</span>
              </div>
            )}
            {goalPhase === "Complete" && goalStepDetails.length > 0 && (
              <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "8px 0", borderTop: "1px solid #30363d" }}>
                <div style={{ width: 28, height: 28, borderRadius: "50%", background: "#0d2818", border: "1px solid #22c55e33", display: "flex", alignItems: "center", justifyContent: "center", flexShrink: 0, fontSize: 13 }}>
                  ✅
                </div>
                <span style={{ color: "#22c55e", fontSize: 13, fontWeight: 600 }}>
                  Goal complete — {goalSteps} steps, {goalFuel.toFixed(0)} fuel used
                </span>
              </div>
            )}
          </div>
        </div>
      )}

      </>)}

      <CreateAgent
        open={showCreate}
        onClose={() => setShowCreate(false)}
        onDeploy={manifestJson => {
          onCreate(manifestJson);
          setShowCreate(false);
        }}
      />
    </section>
    </RequiresLlm>
  );
}

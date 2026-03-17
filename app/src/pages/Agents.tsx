import { useEffect, useMemo, useState, useCallback } from "react";
import { ActivityFeed } from "../components/agents/ActivityFeed";
import { AgentDetail, type AgentDetailTab } from "../components/agents/AgentDetail";
import { CreateAgent } from "../components/agents/CreateAgent";
import { SlmStatusBadge } from "../components/agents/SlmStatusBadge";
import { HeatMap } from "../components/viz/HeatMap";
import { NeuralGraph } from "../components/viz/NeuralGraph";
import { PulseRing } from "../components/viz/PulseRing";
import { getPreinstalledAgents, hasDesktopRuntime, listProviderModels } from "../api/backend";
import type { AgentSummary, AuditEventRow, PreinstalledAgent, SlmStatus } from "../types";
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
  const [autonomyFilter, setAutonomyFilter] = useState<AutonomyFilter>("all");
  const [categoryFilter, setCategoryFilter] = useState<CategoryFilter>("all");
  const [showMonitoring, setShowMonitoring] = useState(false);
  const [preinstalledAgents, setPreinstalledAgents] = useState<PreinstalledAgent[]>([]);
  const [modelCount, setModelCount] = useState(0);
  const [expandedCardId, setExpandedCardId] = useState<string | null>(null);

  const [slmStatus] = useState<SlmStatus>({
    loaded: false, model_id: null, ram_usage_mb: 0,
    avg_latency_ms: 0, total_queries: 0, governance_routing: "cloud",
  });

  /* ─── load preinstalled agents + model count ─── */
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    getPreinstalledAgents().then(setPreinstalledAgents).catch(() => {});
    listProviderModels().then(m => setModelCount(m.length)).catch(() => {});
  }, []);

  /* ─── derived ─── */
  const activeCount = useMemo(
    () => agents.filter(a => a.status === "Running" || a.status === "Starting").length,
    [agents],
  );

  const selectedAgent = useMemo(
    () => agents.find(a => a.id === selectedAgentId) ?? null,
    [agents, selectedAgentId],
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

    if (autonomyFilter !== "all") {
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
      onNavigate("ai-chat-hub");
    }
  }, [onNavigate]);

  const totalTasks = auditEvents.length;

  return (
    <section className="mission-control">
      <div className="mission-grid-overlay" />

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
          <button type="button" className="create-btn" onClick={() => setShowCreate(true)}>
            + CREATE AGENT
          </button>
          {onClearAll && (
            <button
              type="button"
              className="create-btn"
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

      {/* ─── Stats Ribbon ─── */}
      <div className="mission-stats-ribbon">
        <div className="mission-stat-card">
          <span className="mission-stat-icon">&#x2B22;</span>
          <div>
            <span className="mission-stat-value">{hasPreinstalled ? preinstalledAgents.length : agents.length}</span>
            <span className="mission-stat-label">Total Agents</span>
          </div>
        </div>
        <div className="mission-stat-card">
          <span className="mission-stat-icon" style={{ color: "var(--green)" }}>&#x25C9;</span>
          <div>
            <span className="mission-stat-value">{activeCount}</span>
            <span className="mission-stat-label">Active</span>
          </div>
        </div>
        <div className="mission-stat-card">
          <span className="mission-stat-icon" style={{ color: "var(--blue)" }}>&#x2726;</span>
          <div>
            <span className="mission-stat-value">{totalTasks}</span>
            <span className="mission-stat-label">Events Today</span>
          </div>
        </div>
        <div className="mission-stat-card">
          <span className="mission-stat-icon" style={{ color: "var(--cyan, #06b6d4)" }}>&#x25C8;</span>
          <div>
            <span className="mission-stat-value">{modelCount}</span>
            <span className="mission-stat-label">Available Models</span>
          </div>
        </div>
        <SlmStatusBadge status={slmStatus} />
      </div>

      {/* ─── Search & Filters ─── */}
      <div className="mission-filters">
        <input
          className="mission-search"
          type="text"
          placeholder="Search agents by name, description, or capability..."
          value={searchQuery}
          onChange={e => setSearchQuery(e.target.value)}
        />
        <div className="mission-filter-row">
          <div className="mission-filter-group">
            <span className="mission-filter-label">Level:</span>
            {(["all", 1, 2, 3, 4, 5, 6] as AutonomyFilter[]).map(level => (
              <button
                key={String(level)}
                className={`mission-filter-btn ${autonomyFilter === level ? "active" : ""}`}
                style={level !== "all" ? { borderColor: `${AUTONOMY_COLORS[level as number]}66`, color: autonomyFilter === level ? "#fff" : AUTONOMY_COLORS[level as number] } : undefined}
                onClick={() => setAutonomyFilter(level)}
              >
                {level === "all" ? "All" : `L${level}`}
              </button>
            ))}
          </div>
          <div className="mission-filter-group">
            <span className="mission-filter-label">Category:</span>
            {(["all", "research", "code", "security", "creative", "system", "devops", "data", "communication"] as CategoryFilter[]).map(cat => (
              <button
                key={cat}
                className={`mission-filter-btn ${categoryFilter === cat ? "active" : ""}`}
                onClick={() => setCategoryFilter(cat)}
              >
                {cat === "all" ? "All" : cat.charAt(0).toUpperCase() + cat.slice(1)}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* ─── Agent Cards Grid ─── */}
      <main className="mission-agent-grid">
        {filteredAgents.length === 0 ? (
          <article className="mission-agent-card mission-agent-card--empty">
            <p className="mission-agent-card__empty-copy">
              {searchQuery || autonomyFilter !== "all" || categoryFilter !== "all"
                ? "No agents match your filters."
                : "No agents deployed. Start by creating your first mission agent."}
            </p>
          </article>
        ) : (
          filteredAgents.map(({ preinstalled: pa, runtime, category }) => {
            const agentId = runtime?.id ?? pa.agent_id;
            const status = runtime?.status ?? pa.status;
            const statusLabel = agentStatusLabel(status);
            const statusClass = statusTone(statusLabel);
            const level = pa.autonomy_level;
            const isExpanded = expandedCardId === agentId;
            const capsToShow = pa.capabilities.slice(0, 3);

            return (
              <article
                key={agentId}
                className={[
                  "mission-agent-card",
                  selectedAgentId === agentId ? "is-selected" : "",
                  isExpanded ? "is-expanded" : "",
                ].filter(Boolean).join(" ")}
                onClick={() => setSelectedAgentId(agentId)}
              >
                <div className={`mission-agent-card__accent mission-agent-card__accent--${statusClass}`} />

                <div className="mission-agent-card__header">
                  <div className="mission-agent-card__title-group">
                    <div className="mission-agent-card__status-row">
                      <span className={`mission-agent-card__status-dot mission-agent-card__status-dot--${statusClass}`} />
                      <span className="mission-agent-card__status-text">{statusLabel}</span>
                    </div>
                    <h3 className="mission-agent-card__title">{pa.name}</h3>
                  </div>
                  <span
                    className={`mission-agent-card__level mission-agent-card__level--${level}`}
                  >
                    L{level}
                  </span>
                </div>

                <p className="mission-agent-card__description" title={pa.description}>
                  {truncateDescription(pa.description)}
                </p>

                <div className="mission-agent-card__tags">
                  {capsToShow.map(cap => (
                    <span key={cap} className="mission-agent-card__tag">{cap}</span>
                  ))}
                  {pa.capabilities.length > 3 && (
                    <span className="mission-agent-card__tag mission-agent-card__tag--more">+{pa.capabilities.length - 3}</span>
                  )}
                  <span className="mission-agent-card__category">{category}</span>
                </div>

                <div className="mission-agent-card__meta">
                  <span className="mission-agent-card__meta-item">Fuel {pa.fuel_budget.toLocaleString()}</span>
                  {runtime?.status && runtime.status !== statusLabel && (
                    <span className="mission-agent-card__meta-item mission-agent-card__meta-item--muted">
                      Runtime {runtime.status}
                    </span>
                  )}
                </div>

                <div className="mission-agent-card__actions">
                  {(statusLabel === "Idle" || statusLabel === "Error") && (
                    <button type="button" className="mission-agent-card__action mission-agent-card__action--start" onClick={e => { e.stopPropagation(); onStart(agentId); }}>
                      Start
                    </button>
                  )}
                  {statusLabel === "Running" && (
                    <button type="button" className="mission-agent-card__action mission-agent-card__action--stop" onClick={e => { e.stopPropagation(); onStop(agentId); }}>
                      Stop
                    </button>
                  )}
                  {statusLabel === "Paused" && (
                    <button type="button" className="mission-agent-card__action mission-agent-card__action--resume" onClick={e => { e.stopPropagation(); onStart(agentId); }}>
                      Resume
                    </button>
                  )}
                  <button type="button" className="mission-agent-card__action mission-agent-card__action--chat" onClick={e => { e.stopPropagation(); navigateToChat(pa.agent_id); }}>
                    Chat
                  </button>
                  <button type="button" className="mission-agent-card__action" onClick={e => { e.stopPropagation(); setExpandedCardId(isExpanded ? null : agentId); }}>
                    {isExpanded ? "Less" : "Details"}
                  </button>
                  {onPermissions && (
                    <button type="button" className="mission-agent-card__action mission-agent-card__action--perms" onClick={e => { e.stopPropagation(); onPermissions(agentId); }}>
                      Perms
                    </button>
                  )}
                  <button type="button" className="mission-agent-card__action mission-agent-card__action--delete" onClick={e => { e.stopPropagation(); if (window.confirm(`Delete ${pa.name}?`)) onDelete(agentId); }}>
                    Del
                  </button>
                </div>

                {isExpanded && (
                  <div className="mission-agent-card__details">
                    <div className="mission-agent-card__detail-row">
                      <span className="mission-agent-card__detail-label">Full Description</span>
                      <p className="mission-agent-card__detail-value">{pa.description}</p>
                    </div>
                    <div className="mission-agent-card__detail-row">
                      <span className="mission-agent-card__detail-label">Autonomy Level</span>
                      <span className="mission-agent-card__detail-value" style={{ color: AUTONOMY_COLORS[level] }}>
                        L{level} — {AUTONOMY_LABELS[level] ?? "Unknown"}
                      </span>
                    </div>
                    <div className="mission-agent-card__detail-row">
                      <span className="mission-agent-card__detail-label">All Capabilities</span>
                      <span className="mission-agent-card__detail-value">{pa.capabilities.join(", ")}</span>
                    </div>
                    <div className="mission-agent-card__detail-row">
                      <span className="mission-agent-card__detail-label">Fuel Budget</span>
                      <span className="mission-agent-card__detail-value">{pa.fuel_budget.toLocaleString()}</span>
                    </div>
                    {pa.schedule && (
                      <div className="mission-agent-card__detail-row">
                        <span className="mission-agent-card__detail-label">Schedule</span>
                        <span className="mission-agent-card__detail-value">{pa.schedule}</span>
                      </div>
                    )}
                    <div className="mission-agent-card__detail-row">
                      <span className="mission-agent-card__detail-label">Governance</span>
                      <span className="mission-agent-card__detail-value">
                        {level >= 3 ? "HITL approval required for Tier1+ ops" : "Standard capability checks"}
                      </span>
                    </div>
                  </div>
                )}
              </article>
            );
          })
        )}
      </main>

      {/* ─── Monitoring (collapsible) ─── */}
      <div className="mission-monitoring-toggle">
        <button
          type="button"
          className="mission-monitoring-btn"
          onClick={() => setShowMonitoring(!showMonitoring)}
        >
          {showMonitoring ? "▲" : "▼"} Monitoring {activeCount > 0 && `(${activeCount} active)`}
        </button>
      </div>

      {showMonitoring && (
        <>
          <section className="mission-viz-strip">
            <div className="mission-viz-card">
              <div className="mission-viz-card-head">
                <p className="mission-viz-title">Agent Fuel Matrix</p>
                <PulseRing active={activeCount > 0} />
              </div>
              <div className="mission-fuel-bars">
                {agents.map(agent => {
                  const pct = Math.max(0, Math.min(100, Math.round(agent.fuel_remaining / 100)));
                  const barColor = pct > 50 ? "var(--green)" : pct > 20 ? "var(--amber)" : "var(--red)";
                  return (
                    <div key={agent.id} className="mission-fuel-row">
                      <span className="mission-fuel-name">{agent.name}</span>
                      <div className="mission-fuel-track">
                        <div className="mission-fuel-fill" style={{ width: `${pct}%`, background: `linear-gradient(90deg, ${barColor}, ${barColor}88)` }} />
                      </div>
                      <span className="mission-fuel-pct">{pct}%</span>
                    </div>
                  );
                })}
              </div>
            </div>
            <div className="mission-viz-card mission-viz-card-wide">
              <p className="mission-viz-title">Neural Agent Link Graph</p>
              <NeuralGraph nodes={graphNodes} edges={graphEdges} />
            </div>
            <div className="mission-viz-card">
              <HeatMap values={heatmapValues} columns={8} title="Hourly Activity" />
            </div>
          </section>

          <ActivityFeed entries={activityEntries} />
        </>
      )}

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

      <CreateAgent
        open={showCreate}
        onClose={() => setShowCreate(false)}
        onDeploy={manifestJson => {
          onCreate(manifestJson);
          setShowCreate(false);
        }}
      />
    </section>
  );
}

import { useEffect, useMemo, useState } from "react";
import { ActivityFeed } from "../components/agents/ActivityFeed";
import { AgentCard } from "../components/agents/AgentCard";
import { AgentDetail, type AgentDetailTab } from "../components/agents/AgentDetail";
import { CreateAgent } from "../components/agents/CreateAgent";
import { HeatMap } from "../components/viz/HeatMap";
import { NeuralGraph } from "../components/viz/NeuralGraph";
import { PulseRing } from "../components/viz/PulseRing";
import { RadialGauge } from "../components/viz/RadialGauge";
import type { AgentSummary, AuditEventRow } from "../types";
import "./agents.css";

interface AgentsProps {
  agents: AgentSummary[];
  auditEvents: AuditEventRow[];
  factoryTrigger?: number;
  onStart: (id: string) => void;
  onPause: (id: string) => void;
  onStop: (id: string) => void;
  onCreate: (manifestJson: string) => void;
  onDelete: (id: string) => void;
}

function makeActivityEntry(event: AuditEventRow, agentName: string): string {
  const payload = JSON.stringify(event.payload);
  const summary = payload.length > 52 ? `${payload.slice(0, 49)}...` : payload;
  const ok = event.event_type.toLowerCase().includes("error") ? "✕" : "✓";
  return `${agentName} > ${event.event_type}: ${summary} [${ok}]`;
}

export function Agents({
  agents,
  auditEvents,
  factoryTrigger = 0,
  onStart,
  onPause,
  onStop,
  onCreate,
  onDelete
}: AgentsProps): JSX.Element {
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(agents[0]?.id ?? null);
  const [showCreate, setShowCreate] = useState(false);
  const [detailOpen, setDetailOpen] = useState(false);
  const [detailTab, setDetailTab] = useState<AgentDetailTab>("overview");

  const activeCount = useMemo(
    () => agents.filter((agent) => agent.status === "Running" || agent.status === "Starting").length,
    [agents]
  );

  const selectedAgent = useMemo(
    () => agents.find((agent) => agent.id === selectedAgentId) ?? null,
    [agents, selectedAgentId]
  );

  const latestByAgent = useMemo(() => {
    const map = new Map<string, AuditEventRow>();
    for (const event of auditEvents) {
      const previous = map.get(event.agent_id);
      if (!previous || event.timestamp > previous.timestamp) {
        map.set(event.agent_id, event);
      }
    }
    return map;
  }, [auditEvents]);

  const activityEntries = useMemo(() => {
    const nameById = new Map(agents.map((agent) => [agent.id, agent.name]));
    return [...auditEvents]
      .sort((left, right) => right.timestamp - left.timestamp)
      .slice(0, 20)
      .map((event) => makeActivityEntry(event, nameById.get(event.agent_id) ?? event.agent_id));
  }, [agents, auditEvents]);

  const graphNodes = useMemo(
    () =>
      agents.map((agent) => ({
        id: agent.id,
        group: agent.name.toLowerCase().includes("code")
          ? "coding"
          : agent.name.toLowerCase().includes("social")
            ? "social"
            : agent.name.toLowerCase().includes("design")
              ? "design"
              : "general",
        activity: latestByAgent.get(agent.id) ? 0.65 : 0.28
      })),
    [agents, latestByAgent]
  );

  const graphEdges = useMemo(
    () =>
      agents.slice(1).map((agent, index) => ({
        from: agents[index].id,
        to: agent.id,
        weight: 0.42 + (index % 3) * 0.2
      })),
    [agents]
  );

  const heatmapValues = useMemo(() => {
    const buckets = Array.from({ length: 24 }, () => 0);
    for (const event of auditEvents) {
      const hour = new Date(event.timestamp * 1000).getHours();
      buckets[hour] += 1;
    }
    const max = Math.max(1, ...buckets);
    return buckets.map((value) => value / max);
  }, [auditEvents]);

  useEffect(() => {
    if (factoryTrigger > 0) {
      setShowCreate(true);
    }
  }, [factoryTrigger]);

  useEffect(() => {
    if (agents.length === 0) {
      setSelectedAgentId(null);
      setDetailOpen(false);
      return;
    }
    if (!selectedAgentId || !agents.some((agent) => agent.id === selectedAgentId)) {
      setSelectedAgentId(agents[0].id);
    }
  }, [agents, selectedAgentId]);

  function openDetail(agentId: string, tab: AgentDetailTab = "overview"): void {
    setSelectedAgentId(agentId);
    setDetailTab(tab);
    setDetailOpen(true);
  }

  const totalTasks = auditEvents.length;
  const averageFuel = agents.length > 0
    ? Math.round(agents.reduce((sum, a) => sum + Math.max(0, Math.min(100, a.fuel_remaining / 100)), 0) / agents.length)
    : 0;

  return (
    <section className="mission-control">
      <div className="mission-grid-overlay" />

      <header className="mission-header">
        <div>
          <h2 className="mission-title">AGENT CONTROL // {activeCount} ACTIVE</h2>
          <p className="mission-subtitle">Mission-control view of governed runtime operations</p>
        </div>
        <div className="flex items-center gap-2">
          <div className="mission-active-counter">
            <span className="mission-active-hex">{activeCount}</span>
            <span className="mission-active-value">ACTIVE</span>
          </div>
          <button type="button" className="create-btn" onClick={() => setShowCreate(true)}>
            + CREATE AGENT
          </button>
        </div>
      </header>

      <div className="mission-stats-ribbon">
        <div className="mission-stat-card">
          <span className="mission-stat-icon">&#x2B22;</span>
          <div>
            <span className="mission-stat-value">{agents.length}</span>
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
          <span className="mission-stat-icon" style={{ color: "var(--amber)" }}>&#x26A1;</span>
          <div>
            <span className="mission-stat-value">{averageFuel}%</span>
            <span className="mission-stat-label">Avg Fuel</span>
          </div>
        </div>
      </div>

      <main className="mission-agent-grid">
        {agents.length === 0 ? (
          <article className="agent-card">
            <p className="agent-card-last">No agents deployed. Start by creating your first mission agent.</p>
          </article>
        ) : (
          agents.map((agent) => (
            <AgentCard
              key={agent.id}
              agent={agent}
              selected={agent.id === selectedAgentId}
              latestEvent={latestByAgent.get(agent.id) ?? null}
              onOpen={(id) => openDetail(id, "overview")}
              onStart={onStart}
              onPause={onPause}
              onStop={onStop}
              onLogs={(id) => openDetail(id, "logs")}
              onDelete={onDelete}
            />
          ))
        )}
      </main>

      <section className="mission-viz-strip">
        <div className="mission-viz-card">
          <div className="mission-viz-card-head">
            <p className="mission-viz-title">Agent Fuel Matrix</p>
            <PulseRing active={activeCount > 0} />
          </div>
          <div className="mission-fuel-bars">
            {agents.map((agent) => {
              const pct = Math.max(0, Math.min(100, Math.round(agent.fuel_remaining / 100)));
              const barColor = pct > 50 ? "var(--green)" : pct > 20 ? "var(--amber)" : "var(--red)";
              return (
                <div key={agent.id} className="mission-fuel-row">
                  <span className="mission-fuel-name">{agent.name}</span>
                  <div className="mission-fuel-track">
                    <div
                      className="mission-fuel-fill"
                      style={{ width: `${pct}%`, background: `linear-gradient(90deg, ${barColor}, ${barColor}88)` }}
                    />
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

      <AgentDetail
        open={detailOpen}
        agent={selectedAgent}
        auditEvents={auditEvents}
        activeTab={detailTab}
        onTabChange={setDetailTab}
        onClose={() => setDetailOpen(false)}
      />

      <CreateAgent
        open={showCreate}
        onClose={() => setShowCreate(false)}
        onDeploy={(manifestJson) => {
          onCreate(manifestJson);
          setShowCreate(false);
        }}
      />
    </section>
  );
}

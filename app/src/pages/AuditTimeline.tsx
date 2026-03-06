import { useMemo, useState } from "react";
import "./audit-timeline.css";

interface TimelineEvent {
  id: string;
  timestamp: number;
  agentName: string;
  agentId: string;
  eventType: "AgentAction" | "UserAction" | "SystemEvent" | "PolicyDecision";
  summary: string;
  hasCrossNodeRef: boolean;
}

const EVENT_COLORS: Record<string, string> = {
  AgentAction: "#3b82f6",
  UserAction: "#22c55e",
  SystemEvent: "#eab308",
  PolicyDecision: "#a855f7",
};

const MOCK_EVENTS: TimelineEvent[] = [
  { id: "te-01", timestamp: 1700100470, agentName: "Coder", agentId: "agent-coder", eventType: "AgentAction", summary: "Fixed null check in middleware.rs:88", hasCrossNodeRef: false },
  { id: "te-02", timestamp: 1700100460, agentName: "Workflow Studio", agentId: "agent-workflow-studio", eventType: "SystemEvent", summary: "Task complete: daily analytics pipeline", hasCrossNodeRef: false },
  { id: "te-03", timestamp: 1700100455, agentName: "Self-Improve", agentId: "agent-self-improve", eventType: "AgentAction", summary: "Optimized prompt routing: +12% accuracy", hasCrossNodeRef: true },
  { id: "te-04", timestamp: 1700100430, agentName: "Web Builder", agentId: "agent-web-builder", eventType: "AgentAction", summary: "Deployed staging build v2.4.1", hasCrossNodeRef: false },
  { id: "te-05", timestamp: 1700100410, agentName: "Screen Poster", agentId: "agent-screen-poster", eventType: "PolicyDecision", summary: "Approval required for X post: product launch teaser", hasCrossNodeRef: false },
  { id: "te-06", timestamp: 1700100395, agentName: "Screen Poster", agentId: "agent-screen-poster", eventType: "UserAction", summary: "Human approved social post for X", hasCrossNodeRef: true },
  { id: "te-07", timestamp: 1700100380, agentName: "Designer", agentId: "agent-designer", eventType: "AgentAction", summary: "Generated landing page mockup (webp)", hasCrossNodeRef: false },
  { id: "te-08", timestamp: 1700100350, agentName: "Coder", agentId: "agent-coder", eventType: "AgentAction", summary: "Ran auth test suite: 12 passed, 0 failed", hasCrossNodeRef: false },
  { id: "te-09", timestamp: 1700100320, agentName: "Coder", agentId: "agent-coder", eventType: "SystemEvent", summary: "Fuel burn: 1200 consumed, 8000 remaining", hasCrossNodeRef: false },
  { id: "te-10", timestamp: 1700100290, agentName: "Self-Improve", agentId: "agent-self-improve", eventType: "PolicyDecision", summary: "Trust score evaluation: 0.94 (above promotion threshold)", hasCrossNodeRef: true },
  { id: "te-11", timestamp: 1700100260, agentName: "Workflow Studio", agentId: "agent-workflow-studio", eventType: "AgentAction", summary: "Executed DAG: daily-analytics (6 nodes)", hasCrossNodeRef: false },
  { id: "te-12", timestamp: 1700100230, agentName: "Designer", agentId: "agent-designer", eventType: "AgentAction", summary: "Created 42 design tokens for dark-cyber theme", hasCrossNodeRef: false },
];

const EVENT_TYPES = ["All", "AgentAction", "UserAction", "SystemEvent", "PolicyDecision"];

function formatTimestamp(ts: number): string {
  const d = new Date(ts * 1000);
  const pad = (n: number): string => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export default function AuditTimeline(): JSX.Element {
  const [agentFilter, setAgentFilter] = useState("All");
  const [typeFilter, setTypeFilter] = useState("All");

  const agents = useMemo(() => {
    const names = Array.from(new Set(MOCK_EVENTS.map((e) => e.agentName)));
    return ["All", ...names.sort()];
  }, []);

  const filtered = useMemo(() => {
    return MOCK_EVENTS.filter((e) => {
      if (agentFilter !== "All" && e.agentName !== agentFilter) return false;
      if (typeFilter !== "All" && e.eventType !== typeFilter) return false;
      return true;
    });
  }, [agentFilter, typeFilter]);

  return (
    <section className="at-hub">
      <header className="at-header">
        <h2 className="at-title">AUDIT TIMELINE // GOVERNANCE LOG</h2>
        <p className="at-subtitle">{filtered.length} events shown</p>
      </header>

      <div className="at-filters">
        <select className="at-select" value={agentFilter} onChange={(e) => setAgentFilter(e.target.value)}>
          {agents.map((a) => <option key={a} value={a}>{a}</option>)}
        </select>
        <select className="at-select" value={typeFilter} onChange={(e) => setTypeFilter(e.target.value)}>
          {EVENT_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
        </select>
      </div>

      <div className="at-timeline">
        {filtered.map((event) => (
          <div key={event.id} className="at-event">
            <div className="at-event-line">
              <span className="at-event-dot" style={{ background: EVENT_COLORS[event.eventType] }} />
            </div>
            <div className="at-event-card">
              <div className="at-event-top">
                <span className="at-event-time">{formatTimestamp(event.timestamp)}</span>
                <span className="at-event-agent">{event.agentName}</span>
                <span className="at-event-type" style={{ color: EVENT_COLORS[event.eventType] }}>
                  {event.eventType}
                </span>
                {event.hasCrossNodeRef && <span className="at-federation-badge" title="Cross-node reference">&#x26A1;</span>}
              </div>
              <p className="at-event-summary">{event.summary}</p>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

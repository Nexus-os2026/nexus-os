import { useMemo, useState } from "react";
import { Audit } from "./pages/Audit";
import { Chat } from "./pages/Chat";
import { Dashboard } from "./pages/Dashboard";
import type { AgentSummary, AuditEventRow } from "./types";

type Page = "chat" | "dashboard" | "audit";

function mockAgents(): AgentSummary[] {
  return [
    {
      id: "a-1",
      name: "rust-social-publisher",
      status: "Running",
      fuel_remaining: 8200,
      last_action: "Published daily digest"
    },
    {
      id: "a-2",
      name: "market-researcher",
      status: "Paused",
      fuel_remaining: 4100,
      last_action: "Awaiting approval"
    }
  ];
}

function mockAudit(): AuditEventRow[] {
  return [
    {
      event_id: "evt-1",
      timestamp: 1_700_000_001,
      agent_id: "a-1",
      event_type: "StateChange",
      payload: { state: "Running" },
      previous_hash: "genesis",
      hash: "hash-1"
    },
    {
      event_id: "evt-2",
      timestamp: 1_700_000_002,
      agent_id: "a-1",
      event_type: "ToolCall",
      payload: { action: "social.post" },
      previous_hash: "hash-1",
      hash: "hash-2"
    }
  ];
}

export default function App(): JSX.Element {
  const [page, setPage] = useState<Page>("chat");
  const [agents, setAgents] = useState<AgentSummary[]>(() => mockAgents());
  const auditEvents = useMemo(() => mockAudit(), []);

  function updateAgentStatus(id: string, status: AgentSummary["status"]): void {
    setAgents((prev) =>
      prev.map((agent) => (agent.id === id ? { ...agent, status, last_action: `Status set to ${status}` } : agent))
    );
  }

  return (
    <main className="mx-auto max-w-6xl px-4 py-6 sm:px-6">
      <header className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <div>
          <h1 className="font-display text-3xl text-ink">NEXUS OS Desktop</h1>
          <p className="text-sm text-slate-600">Governed agent operations cockpit</p>
        </div>
        <nav className="flex rounded-xl bg-white/75 p-1 shadow-sm">
          {(["chat", "dashboard", "audit"] as const).map((item) => (
            <button
              key={item}
              onClick={() => setPage(item)}
              className={`rounded-lg px-3 py-2 text-sm font-semibold transition ${
                page === item ? "bg-ink text-white" : "text-slate-700"
              }`}
            >
              {item[0].toUpperCase() + item.slice(1)}
            </button>
          ))}
        </nav>
      </header>

      {page === "chat" && <Chat />}
      {page === "dashboard" && (
        <Dashboard
          agents={agents}
          onStart={(id) => updateAgentStatus(id, "Running")}
          onPause={(id) => updateAgentStatus(id, "Paused")}
          onStop={(id) => updateAgentStatus(id, "Stopped")}
        />
      )}
      {page === "audit" && <Audit events={auditEvents} />}
    </main>
  );
}

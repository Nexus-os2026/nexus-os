import { useEffect, useMemo, useState } from "react";
import type { AgentSummary, AuditEventRow } from "../types";

interface CreateAgentDraft {
  name: string;
  version: string;
  fuel_budget: string;
  capabilities: string;
  llm_model: string;
}

interface AgentsProps {
  agents: AgentSummary[];
  auditEvents: AuditEventRow[];
  factoryTrigger?: number;
  onStart: (id: string) => void;
  onPause: (id: string) => void;
  onStop: (id: string) => void;
  onCreate: (manifestJson: string) => void;
}

const INITIAL_DRAFT: CreateAgentDraft = {
  name: "new-agent",
  version: "0.1.0",
  fuel_budget: "10000",
  capabilities: "web.search,llm.query,fs.read",
  llm_model: "claude-sonnet-4-5"
};

function statusClass(status: AgentSummary["status"]): string {
  if (status === "Running") {
    return "bg-emerald-500/20 text-emerald-300 border-emerald-500/40";
  }
  if (status === "Paused") {
    return "bg-amber-500/20 text-amber-300 border-amber-500/40";
  }
  return "bg-zinc-500/20 text-zinc-300 border-zinc-500/40";
}

export function Agents({
  agents,
  auditEvents,
  factoryTrigger = 0,
  onStart,
  onPause,
  onStop,
  onCreate
}: AgentsProps): JSX.Element {
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(agents[0]?.id ?? null);
  const [showFactory, setShowFactory] = useState(false);
  const [draft, setDraft] = useState<CreateAgentDraft>(INITIAL_DRAFT);

  const selected = useMemo(
    () => agents.find((agent) => agent.id === selectedAgentId) ?? null,
    [agents, selectedAgentId]
  );

  const selectedAudit = useMemo(() => {
    if (!selected) {
      return [];
    }
    return auditEvents.filter((event) => event.agent_id === selected.id).slice(-40).reverse();
  }, [auditEvents, selected]);

  useEffect(() => {
    if (factoryTrigger > 0) {
      setShowFactory(true);
    }
  }, [factoryTrigger]);

  useEffect(() => {
    if (agents.length === 0) {
      setSelectedAgentId(null);
      return;
    }
    if (!selectedAgentId || !agents.some((agent) => agent.id === selectedAgentId)) {
      setSelectedAgentId(agents[0].id);
    }
  }, [agents, selectedAgentId]);

  return (
    <section className="grid h-[calc(100vh-10rem)] grid-cols-1 gap-4 lg:grid-cols-[1.1fr_1fr]">
      <div className="rounded-2xl border border-zinc-800 bg-zinc-900/80 p-4">
        <div className="mb-4 flex items-center justify-between">
          <div>
            <h2 className="font-display text-xl text-zinc-100">Agents</h2>
            <p className="text-xs text-zinc-400">Runtime status, fuel, controls.</p>
          </div>
          <button
            onClick={() => setShowFactory(true)}
            className="rounded-lg bg-sky-600 px-3 py-2 text-xs font-semibold text-white hover:bg-sky-500"
          >
            Create Agent
          </button>
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          {agents.map((agent) => {
            const fuelPct = Math.max(0, Math.min(100, Math.round(agent.fuel_remaining / 100)));
            return (
              <article
                key={agent.id}
                onClick={() => setSelectedAgentId(agent.id)}
                className={`cursor-pointer rounded-xl border p-3 transition ${
                  selectedAgentId === agent.id
                    ? "border-emerald-500/60 bg-zinc-800"
                    : "border-zinc-800 bg-zinc-900 hover:border-zinc-700"
                }`}
              >
                <div className="flex items-start justify-between gap-2">
                  <h3 className="font-display text-base text-zinc-100">{agent.name}</h3>
                  <span className={`rounded-full border px-2 py-0.5 text-[11px] font-semibold ${statusClass(agent.status)}`}>
                    {agent.status}
                  </span>
                </div>
                <p className="mt-2 text-xs text-zinc-400">Last: {agent.last_action}</p>
                <div className="mt-3 h-2 overflow-hidden rounded-full bg-zinc-800">
                  <div
                    className="h-full rounded-full bg-gradient-to-r from-emerald-500 to-sky-500"
                    style={{ width: `${fuelPct}%` }}
                  />
                </div>
                <div className="mt-3 flex gap-1 text-[11px]">
                  <button className="rounded bg-emerald-600 px-2 py-1 text-white" onClick={() => onStart(agent.id)}>
                    Start
                  </button>
                  <button className="rounded bg-amber-600 px-2 py-1 text-white" onClick={() => onPause(agent.id)}>
                    Pause
                  </button>
                  <button className="rounded bg-rose-600 px-2 py-1 text-white" onClick={() => onStop(agent.id)}>
                    Stop
                  </button>
                </div>
              </article>
            );
          })}
        </div>
      </div>

      <div className="rounded-2xl border border-zinc-800 bg-zinc-900/80 p-4">
        <h3 className="font-display text-lg text-zinc-100">
          {selected ? `${selected.name} · detail` : "Agent detail"}
        </h3>
        <p className="text-xs text-zinc-400">Recent audit events and operational timeline.</p>
        <div className="mt-4 max-h-[32rem] space-y-2 overflow-y-auto pr-1">
          {selectedAudit.length === 0 ? (
            <p className="rounded-lg border border-zinc-800 bg-zinc-900 p-3 text-sm text-zinc-400">
              No events yet for this agent.
            </p>
          ) : (
            selectedAudit.map((event) => (
              <article key={event.event_id} className="rounded-lg border border-zinc-800 bg-zinc-950 p-3">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs font-semibold text-zinc-200">{event.event_type}</span>
                  <span className="text-[11px] text-zinc-500">
                    {new Date(event.timestamp * 1000).toLocaleString()}
                  </span>
                </div>
                <p className="mt-2 text-xs text-zinc-300">{JSON.stringify(event.payload)}</p>
              </article>
            ))
          )}
        </div>
      </div>

      {showFactory ? (
        <div className="fixed inset-0 z-50 grid place-items-center bg-black/70 p-4">
          <div className="w-full max-w-xl rounded-2xl border border-zinc-700 bg-zinc-900 p-5 shadow-2xl">
            <h3 className="font-display text-lg text-zinc-100">Agent Factory</h3>
            <p className="mt-1 text-xs text-zinc-400">Create agent manifest JSON for the supervisor.</p>
            <div className="mt-4 grid gap-3 sm:grid-cols-2">
              <input
                value={draft.name}
                onChange={(event) => setDraft((prev) => ({ ...prev, name: event.target.value }))}
                className="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
                placeholder="name"
              />
              <input
                value={draft.version}
                onChange={(event) => setDraft((prev) => ({ ...prev, version: event.target.value }))}
                className="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
                placeholder="version"
              />
              <input
                value={draft.fuel_budget}
                onChange={(event) => setDraft((prev) => ({ ...prev, fuel_budget: event.target.value }))}
                className="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
                placeholder="fuel budget"
              />
              <input
                value={draft.llm_model}
                onChange={(event) => setDraft((prev) => ({ ...prev, llm_model: event.target.value }))}
                className="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
                placeholder="llm model"
              />
            </div>
            <textarea
              value={draft.capabilities}
              onChange={(event) => setDraft((prev) => ({ ...prev, capabilities: event.target.value }))}
              className="mt-3 h-20 w-full rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-100"
              placeholder="capabilities csv"
            />
            <div className="mt-4 flex justify-end gap-2">
              <button
                onClick={() => setShowFactory(false)}
                className="rounded-lg bg-zinc-800 px-3 py-2 text-xs font-semibold text-zinc-200"
              >
                Cancel
              </button>
              <button
                onClick={() => {
                  const capabilities = draft.capabilities
                    .split(",")
                    .map((value) => value.trim())
                    .filter((value) => value.length > 0);
                  const payload = {
                    name: draft.name.trim(),
                    version: draft.version.trim(),
                    capabilities,
                    fuel_budget: Number(draft.fuel_budget),
                    schedule: null,
                    llm_model: draft.llm_model.trim() || null
                  };
                  onCreate(JSON.stringify(payload));
                  setShowFactory(false);
                  setDraft(INITIAL_DRAFT);
                }}
                className="rounded-lg bg-emerald-600 px-3 py-2 text-xs font-semibold text-white"
              >
                Create
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </section>
  );
}

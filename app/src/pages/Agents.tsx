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
    return "border-cyan-300/60 bg-cyan-500/20 text-cyan-100";
  }
  if (status === "Paused") {
    return "border-amber-300/60 bg-amber-500/20 text-amber-100";
  }
  return "border-slate-500/60 bg-slate-500/20 text-slate-100";
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
      <div className="nexus-panel p-4">
        <div className="mb-4 flex items-center justify-between">
          <div>
            <h2 className="nexus-display text-xl text-cyan-100">Agent Matrix</h2>
            <p className="text-xs text-cyan-100/65">Runtime status, fuel envelopes, active controls.</p>
          </div>
          <button
            onClick={() => setShowFactory(true)}
            className="nexus-btn nexus-btn-primary font-semibold"
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
                    ? "border-cyan-300/70 bg-cyan-500/10 shadow-[0_0_18px_rgba(56,189,248,0.18)]"
                    : "border-slate-700/80 bg-slate-900/70 hover:border-cyan-300/45"
                }`}
              >
                <div className="flex items-start justify-between gap-2">
                  <h3 className="font-display text-base text-slate-100">{agent.name}</h3>
                  <span className={`rounded-full border px-2 py-0.5 text-[11px] font-semibold ${statusClass(agent.status)}`}>
                    {agent.status}
                  </span>
                </div>
                <p className="mt-2 text-xs text-cyan-100/60">Last: {agent.last_action}</p>
                <div className="mt-3 h-2 overflow-hidden rounded-full bg-slate-800">
                  <div
                    className="h-full rounded-full bg-gradient-to-r from-cyan-400 to-blue-400"
                    style={{ width: `${fuelPct}%` }}
                  />
                </div>
                <div className="mt-3 flex gap-1 text-[11px]">
                  <button
                    className="rounded border border-cyan-300/70 bg-cyan-500/15 px-2 py-1 text-cyan-50"
                    onClick={(event) => {
                      event.stopPropagation();
                      onStart(agent.id);
                    }}
                  >
                    Start
                  </button>
                  <button
                    className="rounded border border-amber-300/70 bg-amber-500/20 px-2 py-1 text-amber-100"
                    onClick={(event) => {
                      event.stopPropagation();
                      onPause(agent.id);
                    }}
                  >
                    Pause
                  </button>
                  <button
                    className="rounded border border-rose-300/70 bg-rose-500/20 px-2 py-1 text-rose-100"
                    onClick={(event) => {
                      event.stopPropagation();
                      onStop(agent.id);
                    }}
                  >
                    Stop
                  </button>
                </div>
              </article>
            );
          })}
        </div>
      </div>

      <div className="nexus-panel p-4">
        <h3 className="font-display text-lg text-cyan-100">
          {selected ? `${selected.name} · detail` : "Agent detail"}
        </h3>
        <p className="text-xs text-cyan-100/65">Recent audit events and operational timeline.</p>
        <div className="mt-4 max-h-[32rem] space-y-2 overflow-y-auto pr-1">
          {selectedAudit.length === 0 ? (
            <p className="rounded-lg border border-slate-700/70 bg-slate-900 p-3 text-sm text-cyan-100/60">
              No events yet for this agent.
            </p>
          ) : (
            selectedAudit.map((event) => (
              <article key={event.event_id} className="rounded-lg border border-slate-700/70 bg-slate-950/95 p-3">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-xs font-semibold text-cyan-100">{event.event_type}</span>
                  <span className="text-[11px] text-cyan-100/45">
                    {new Date(event.timestamp * 1000).toLocaleString()}
                  </span>
                </div>
                <p className="mt-2 text-xs text-slate-200">{JSON.stringify(event.payload)}</p>
              </article>
            ))
          )}
        </div>
      </div>

      {showFactory ? (
        <div className="fixed inset-0 z-50 grid place-items-center bg-black/70 p-4">
          <div className="w-full max-w-xl rounded-2xl border border-cyan-300/30 bg-slate-950 p-5 shadow-2xl">
            <h3 className="nexus-display text-lg text-cyan-100">Agent Factory</h3>
            <p className="mt-1 text-xs text-cyan-100/65">Create agent manifest JSON for the supervisor.</p>
            <div className="mt-4 grid gap-3 sm:grid-cols-2">
              <input
                value={draft.name}
                onChange={(event) => setDraft((prev) => ({ ...prev, name: event.target.value }))}
                className="nexus-input"
                placeholder="name"
              />
              <input
                value={draft.version}
                onChange={(event) => setDraft((prev) => ({ ...prev, version: event.target.value }))}
                className="nexus-input"
                placeholder="version"
              />
              <input
                value={draft.fuel_budget}
                onChange={(event) => setDraft((prev) => ({ ...prev, fuel_budget: event.target.value }))}
                className="nexus-input"
                placeholder="fuel budget"
              />
              <input
                value={draft.llm_model}
                onChange={(event) => setDraft((prev) => ({ ...prev, llm_model: event.target.value }))}
                className="nexus-input"
                placeholder="llm model"
              />
            </div>
            <textarea
              value={draft.capabilities}
              onChange={(event) => setDraft((prev) => ({ ...prev, capabilities: event.target.value }))}
              className="nexus-input mt-3 h-20 w-full"
              placeholder="capabilities csv"
            />
            <div className="mt-4 flex justify-end gap-2">
              <button
                onClick={() => setShowFactory(false)}
                className="nexus-btn nexus-btn-secondary font-semibold"
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
                className="nexus-btn nexus-btn-primary font-semibold"
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

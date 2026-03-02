import type { AgentSummary } from "../types";

interface DashboardProps {
  agents: AgentSummary[];
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onPause: (id: string) => void;
}

export function Dashboard({ agents, onStart, onStop, onPause }: DashboardProps): JSX.Element {
  return (
    <section className="nexus-panel rounded-2xl p-6 shadow-sm">
      <h2 className="nexus-display text-2xl text-cyan-100">Agent Dashboard</h2>
      <p className="mt-1 text-sm text-cyan-100/65">Live operational status and fuel usage.</p>

      <div className="mt-5 space-y-3">
        {agents.length === 0 ? (
          <p className="rounded-xl border border-slate-700/80 bg-slate-900/80 p-4 text-sm text-cyan-100/60">
            No agents registered yet.
          </p>
        ) : (
          agents.map((agent) => {
            const fuelPct = Math.max(0, Math.min(100, Math.round(agent.fuel_remaining / 100)));
            return (
              <article key={agent.id} className="rounded-xl border border-slate-700/80 bg-slate-900/80 p-4">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div>
                    <h3 className="font-display text-lg text-cyan-50">{agent.name}</h3>
                    <p className="text-xs uppercase tracking-wide text-cyan-100/55">{agent.status}</p>
                  </div>
                  <div className="flex gap-2 text-xs">
                    <button onClick={() => onStart(agent.id)} className="rounded border border-cyan-300/70 bg-cyan-500/15 px-3 py-1 text-cyan-50">Start</button>
                    <button onClick={() => onPause(agent.id)} className="rounded border border-amber-300/70 bg-amber-500/20 px-3 py-1 text-amber-100">Pause</button>
                    <button onClick={() => onStop(agent.id)} className="rounded border border-rose-300/70 bg-rose-500/20 px-3 py-1 text-rose-100">Stop</button>
                  </div>
                </div>

                <p className="mt-2 text-sm text-slate-200">Last action: {agent.last_action}</p>

                <div className="mt-3">
                  <div className="mb-1 flex items-center justify-between text-xs text-cyan-100/65">
                    <span>Fuel Remaining</span>
                    <span>{agent.fuel_remaining}</span>
                  </div>
                  <div className="h-3 overflow-hidden rounded-full bg-slate-800">
                    <div
                      className="h-full rounded-full bg-gradient-to-r from-cyan-400 to-blue-400"
                      style={{ width: `${fuelPct}%` }}
                    />
                  </div>
                </div>
              </article>
            );
          })
        )}
      </div>
    </section>
  );
}

import type { AgentSummary } from "../types";

interface DashboardProps {
  agents: AgentSummary[];
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onPause: (id: string) => void;
}

export function Dashboard({ agents, onStart, onStop, onPause }: DashboardProps): JSX.Element {
  return (
    <section className="soft-card rounded-2xl p-6 shadow-sm">
      <h2 className="font-display text-2xl text-ink">Agent Dashboard</h2>
      <p className="mt-1 text-sm text-slate-600">Live operational status and fuel usage.</p>

      <div className="mt-5 space-y-3">
        {agents.length === 0 ? (
          <p className="rounded-xl border border-slate-200 bg-white/80 p-4 text-sm text-slate-500">
            No agents registered yet.
          </p>
        ) : (
          agents.map((agent) => {
            const fuelPct = Math.max(0, Math.min(100, Math.round(agent.fuel_remaining / 100)));
            return (
              <article key={agent.id} className="rounded-xl border border-slate-200 bg-white/80 p-4">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div>
                    <h3 className="font-display text-lg text-ink">{agent.name}</h3>
                    <p className="text-xs uppercase tracking-wide text-slate-500">{agent.status}</p>
                  </div>
                  <div className="flex gap-2 text-xs">
                    <button onClick={() => onStart(agent.id)} className="rounded bg-mint px-3 py-1 text-white">Start</button>
                    <button onClick={() => onPause(agent.id)} className="rounded bg-slate-500 px-3 py-1 text-white">Pause</button>
                    <button onClick={() => onStop(agent.id)} className="rounded bg-rose-600 px-3 py-1 text-white">Stop</button>
                  </div>
                </div>

                <p className="mt-2 text-sm text-slate-700">Last action: {agent.last_action}</p>

                <div className="mt-3">
                  <div className="mb-1 flex items-center justify-between text-xs text-slate-600">
                    <span>Fuel Remaining</span>
                    <span>{agent.fuel_remaining}</span>
                  </div>
                  <div className="h-3 overflow-hidden rounded-full bg-slate-200">
                    <div
                      className="h-full rounded-full bg-gradient-to-r from-mint to-accent"
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

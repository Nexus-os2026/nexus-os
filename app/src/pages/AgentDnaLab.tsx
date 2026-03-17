import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/* ================================================================== */
/*  Types                                                              */
/* ================================================================== */

interface AgentGenome {
  genome_version: string;
  agent_id: string;
  generation: number;
  parents: string[];
  genes: {
    personality: { system_prompt: string; tone: string; verbosity: number; creativity: number; assertiveness: number };
    capabilities: { domains: string[]; tools: string[]; max_context_tokens: number };
    reasoning: { strategy: string; depth: number; temperature: number; self_reflection: boolean };
    autonomy: { level: number; risk_tolerance: number; escalation_threshold: number };
    evolution: { mutation_rate: number; fitness_history: number[]; generation: number; lineage: string[] };
  };
  phenotype: { avg_task_score: number; tasks_completed: number; specialization: string };
}

interface BreedResult {
  offspring_id: string;
  offspring_genome: AgentGenome;
}

interface EvolutionResult {
  generation: number;
  survivors: string[];
  avg_fitness: number;
}

interface AgentEntry {
  id: string;
  name: string;
  status: string;
}

/* ================================================================== */
/*  Helpers                                                            */
/* ================================================================== */

function geneBar(value: number, label: string, color: string): JSX.Element {
  const pct = Math.round(value * 100);
  const blocks = Math.round(value * 10);
  const full = "█".repeat(blocks);
  const empty = "░".repeat(10 - blocks);
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
      <span style={{ width: 90, fontSize: "0.75rem", color: "#94a3b8" }}>{label}</span>
      <span style={{ fontFamily: "monospace", fontSize: "0.78rem", color, letterSpacing: "0.5px" }}>
        {full}
      </span>
      <span style={{ fontFamily: "monospace", fontSize: "0.78rem", color: "#334155", letterSpacing: "0.5px" }}>
        {empty}
      </span>
      <span style={{ width: 36, fontSize: "0.72rem", color: "#e2e8f0", textAlign: "right", fontFamily: "monospace" }}>
        {(value).toFixed(2)}
      </span>
    </div>
  );
}

function fitnessSparkline(history: number[]): JSX.Element {
  if (history.length === 0) return <span style={{ color: "#64748b", fontSize: "0.72rem" }}>No data</span>;
  const max = Math.max(...history, 1);
  const barW = 6;
  const barGap = 2;
  const H = 24;
  const W = history.length * (barW + barGap);
  return (
    <svg width={W} height={H} viewBox={`0 0 ${W} ${H}`}>
      {history.map((v, i) => {
        const h = (v / max) * H;
        const hue = (v / max) * 120; // 0=red, 120=green
        return (
          <rect
            key={i}
            x={i * (barW + barGap)}
            y={H - h}
            width={barW}
            height={h}
            rx={1}
            fill={`hsl(${hue}, 70%, 50%)`}
          />
        );
      })}
    </svg>
  );
}

/* ================================================================== */
/*  Component                                                          */
/* ================================================================== */

export default function AgentDnaLab(): JSX.Element {
  const [agents, setAgents] = useState<AgentEntry[]>([]);
  const [selectedA, setSelectedA] = useState("");
  const [selectedB, setSelectedB] = useState("");
  const [genomeA, setGenomeA] = useState<AgentGenome | null>(null);
  const [genomeB, setGenomeB] = useState<AgentGenome | null>(null);
  const [viewGenome, setViewGenome] = useState<AgentGenome | null>(null);
  const [offspring, setOffspring] = useState<AgentGenome | null>(null);
  const [evolveResult, setEvolveResult] = useState<EvolutionResult | null>(null);
  const [tab, setTab] = useState<"breed" | "genome" | "evolve" | "lineage">("breed");
  const [lineage, setLineage] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [breeding, setBreeding] = useState(false);
  const [mutating, setMutating] = useState(false);

  const loadAgents = useCallback(async () => {
    try {
      const list = await invoke<AgentEntry[]>("list_agents");
      setAgents(Array.isArray(list) ? list : []);
    } catch { /* ignore */ }
  }, []);

  useEffect(() => { void loadAgents(); }, [loadAgents]);

  const loadGenome = useCallback(async (agentId: string, setter: (g: AgentGenome | null) => void) => {
    if (!agentId) { setter(null); return; }
    try {
      const g = await invoke<AgentGenome>("get_agent_genome", { agentId });
      setter(g);
    } catch { setter(null); }
  }, []);

  useEffect(() => { void loadGenome(selectedA, setGenomeA); }, [selectedA, loadGenome]);
  useEffect(() => { void loadGenome(selectedB, setGenomeB); }, [selectedB, loadGenome]);

  const handleBreed = useCallback(async () => {
    if (!selectedA || !selectedB) return;
    setBreeding(true);
    setError(null);
    try {
      const result = await invoke<BreedResult>("breed_agents", { parentA: selectedA, parentB: selectedB });
      setOffspring(result.offspring_genome);
      await loadAgents();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBreeding(false);
    }
  }, [selectedA, selectedB, loadAgents]);

  const handleMutate = useCallback(async (agentId: string) => {
    if (!agentId) return;
    setMutating(true);
    setError(null);
    try {
      await invoke("mutate_agent", { agentId });
      // Reload genome
      await loadGenome(agentId, agentId === selectedA ? setGenomeA : setGenomeB);
      await loadAgents();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setMutating(false);
    }
  }, [selectedA, loadGenome, loadAgents]);

  const handleEvolve = useCallback(async () => {
    setError(null);
    try {
      const result = await invoke<EvolutionResult>("evolve_population", { generations: 1 });
      setEvolveResult(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const handleLoadLineage = useCallback(async () => {
    if (!selectedA) return;
    try {
      const lin = await invoke<string[]>("get_agent_lineage", { agentId: selectedA });
      setLineage(Array.isArray(lin) ? lin : []);
    } catch { setLineage([]); }
  }, [selectedA]);

  const handleViewGenome = useCallback((agentId: string) => {
    setTab("genome");
    void loadGenome(agentId, setViewGenome);
  }, [loadGenome]);

  return (
    <div style={{ padding: 24, color: "#e2e8f0", maxWidth: 1400, margin: "0 auto" }}>
      <h1 style={{ fontFamily: "monospace", fontSize: "1.8rem", color: "#a78bfa", marginBottom: 8 }}>
        AGENT DNA LAB
      </h1>
      <p style={{ color: "#94a3b8", marginBottom: 20, fontSize: "0.85rem" }}>
        Breed agents, view genomes, evolve populations, explore lineages
      </p>

      {/* Tabs */}
      <div style={{ display: "flex", gap: 8, marginBottom: 20 }}>
        {(["breed", "genome", "evolve", "lineage"] as const).map((t) => (
          <button key={t} type="button" onClick={() => setTab(t)} style={{
            padding: "6px 18px", borderRadius: 6, border: "1px solid #334155", cursor: "pointer",
            background: tab === t ? "#a78bfa" : "transparent",
            color: tab === t ? "#0f172a" : "#94a3b8",
            fontFamily: "monospace", fontSize: "0.82rem", fontWeight: 600,
          }}>
            {t.toUpperCase()}
          </button>
        ))}
      </div>

      {error && <div style={{ color: "#f87171", marginBottom: 12, fontSize: "0.85rem" }}>{error}</div>}

      {/* Breed Tab */}
      {tab === "breed" && (
        <div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 80px 1fr", gap: 16, marginBottom: 20 }}>
            {/* Parent A */}
            <div style={panelStyle}>
              <h3 style={panelHeading}>Parent A</h3>
              <select value={selectedA} onChange={(e) => setSelectedA(e.target.value)} style={selectStyle}>
                <option value="">Select agent...</option>
                {agents.map((a) => <option key={a.id} value={a.id}>{a.name} ({a.id.slice(0, 8)})</option>)}
              </select>
              {genomeA && (
                <div>
                  <GenomeCard genome={genomeA} />
                  <div style={{ display: "flex", gap: 6, marginTop: 8 }}>
                    <button type="button" onClick={() => handleViewGenome(selectedA)} style={smallBtn}>Full View</button>
                    <button type="button" onClick={() => void handleMutate(selectedA)} disabled={mutating}
                      style={{ ...smallBtn, borderColor: "#f97316", color: "#f97316" }}>
                      {mutating ? "..." : "Mutate"}
                    </button>
                  </div>
                </div>
              )}
            </div>

            {/* Breed Button */}
            <div style={{ display: "flex", alignItems: "center", justifyContent: "center" }}>
              <button type="button" onClick={() => void handleBreed()} disabled={breeding || !selectedA || !selectedB} style={{
                width: 56, height: 56, borderRadius: "50%", border: "2px solid #a78bfa", cursor: "pointer",
                background: breeding ? "#334155" : "rgba(167,139,250,0.1)", color: "#a78bfa",
                fontSize: "1.4rem", fontWeight: 700, fontFamily: "monospace",
              }}>
                {breeding ? "..." : "×"}
              </button>
            </div>

            {/* Parent B */}
            <div style={panelStyle}>
              <h3 style={panelHeading}>Parent B</h3>
              <select value={selectedB} onChange={(e) => setSelectedB(e.target.value)} style={selectStyle}>
                <option value="">Select agent...</option>
                {agents.map((a) => <option key={a.id} value={a.id}>{a.name} ({a.id.slice(0, 8)})</option>)}
              </select>
              {genomeB && (
                <div>
                  <GenomeCard genome={genomeB} />
                  <div style={{ display: "flex", gap: 6, marginTop: 8 }}>
                    <button type="button" onClick={() => handleViewGenome(selectedB)} style={smallBtn}>Full View</button>
                    <button type="button" onClick={() => void handleMutate(selectedB)} disabled={mutating}
                      style={{ ...smallBtn, borderColor: "#f97316", color: "#f97316" }}>
                      {mutating ? "..." : "Mutate"}
                    </button>
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* Offspring */}
          {offspring && (
            <div style={{ ...panelStyle, borderColor: "#a78bfa" }}>
              <h3 style={{ ...panelHeading, color: "#a78bfa" }}>
                Offspring — Generation {offspring.generation}
                <span style={{
                  marginLeft: 8, fontSize: "0.65rem", padding: "2px 6px", borderRadius: 4,
                  background: "rgba(167,139,250,0.2)", color: "#a78bfa", verticalAlign: "middle",
                }}>GEN {offspring.generation}</span>
              </h3>
              <GenomeCard genome={offspring} />
            </div>
          )}
        </div>
      )}

      {/* Genome Viewer Tab */}
      {tab === "genome" && (
        <div style={panelStyle}>
          <div style={{ display: "flex", gap: 12, marginBottom: 16 }}>
            <select value={viewGenome?.agent_id ?? ""} onChange={(e) => void loadGenome(e.target.value, setViewGenome)} style={selectStyle}>
              <option value="">Select agent...</option>
              {agents.map((a) => <option key={a.id} value={a.id}>{a.name} ({a.id.slice(0, 8)})</option>)}
            </select>
          </div>
          {viewGenome ? (
            <div>
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 20 }}>
                {/* Left: Info & Bars */}
                <div>
                  <div style={{ marginBottom: 16 }}>
                    <div style={{ fontSize: "0.82rem", color: "#e2e8f0", fontWeight: 600 }}>
                      Agent: {agents.find((a) => a.id === viewGenome.agent_id)?.name ?? viewGenome.agent_id.slice(0, 16)}
                    </div>
                    <div style={{ fontSize: "0.72rem", color: "#64748b" }}>
                      Generation: {viewGenome.generation} | Parents: {viewGenome.parents.length === 0 ? "none (prebuilt)" : viewGenome.parents.map((p) => p.slice(0, 8)).join(", ")}
                    </div>
                  </div>
                  <h4 style={{ fontSize: "0.78rem", color: "#94a3b8", marginBottom: 8 }}>GENE PROFILE</h4>
                  {geneBar(viewGenome.genes.personality.verbosity, "Personality", "#8b5cf6")}
                  {geneBar(viewGenome.genes.personality.creativity, "Creativity", "#ec4899")}
                  {geneBar(viewGenome.genes.personality.assertiveness, "Confidence", "#f97316")}
                  {geneBar(viewGenome.genes.autonomy.risk_tolerance, "Risk Toler.", "#ef4444")}
                  {geneBar(viewGenome.genes.reasoning.temperature, "Temperature", "#eab308")}
                  {geneBar(viewGenome.genes.evolution.mutation_rate, "Mutation", "#22d3ee")}
                </div>

                {/* Right: Capabilities + Fitness */}
                <div>
                  <h4 style={{ fontSize: "0.78rem", color: "#94a3b8", marginBottom: 8 }}>CAPABILITIES</h4>
                  <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 16 }}>
                    {viewGenome.genes.capabilities.domains.map((d) => (
                      <span key={d} style={{
                        fontSize: "0.72rem", padding: "3px 8px", borderRadius: 4,
                        background: "rgba(34,211,238,0.1)", border: "1px solid rgba(34,211,238,0.3)",
                        color: "#22d3ee",
                      }}>{d}</span>
                    ))}
                    {viewGenome.genes.capabilities.tools.map((t) => (
                      <span key={t} style={{
                        fontSize: "0.72rem", padding: "3px 8px", borderRadius: 4,
                        background: "rgba(167,139,250,0.1)", border: "1px solid rgba(167,139,250,0.3)",
                        color: "#a78bfa",
                      }}>{t}</span>
                    ))}
                  </div>

                  <h4 style={{ fontSize: "0.78rem", color: "#94a3b8", marginBottom: 8 }}>FITNESS HISTORY</h4>
                  <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                    {fitnessSparkline(viewGenome.genes.evolution.fitness_history)}
                    <span style={{ fontSize: "0.72rem", color: "#64748b" }}>
                      [{viewGenome.genes.evolution.fitness_history.join(", ")}]
                    </span>
                  </div>

                  <div style={{ marginTop: 16 }}>
                    <h4 style={{ fontSize: "0.78rem", color: "#94a3b8", marginBottom: 8 }}>PHENOTYPE</h4>
                    <StatRow label="Avg task score" value={viewGenome.phenotype.avg_task_score.toFixed(1)} />
                    <StatRow label="Tasks completed" value={viewGenome.phenotype.tasks_completed} />
                    <StatRow label="Specialization" value={viewGenome.phenotype.specialization || "general"} />
                    <StatRow label="Strategy" value={viewGenome.genes.reasoning.strategy} />
                  </div>
                </div>
              </div>
            </div>
          ) : (
            <div style={{ color: "#64748b", fontSize: "0.82rem" }}>Select an agent to view its full genome</div>
          )}
        </div>
      )}

      {/* Evolve Tab */}
      {tab === "evolve" && (
        <div style={panelStyle}>
          <h3 style={panelHeading}>Evolution Playground</h3>
          <p style={{ color: "#94a3b8", fontSize: "0.82rem", marginBottom: 16 }}>
            Run one generation of evolution: tournament selection, crossover, mutation, fitness evaluation.
          </p>
          <button type="button" onClick={() => void handleEvolve()} style={btnStyle}>
            Evolve One Generation
          </button>
          {evolveResult && (
            <div style={{ marginTop: 16 }}>
              <StatRow label="Generation" value={evolveResult.generation} />
              <StatRow label="Survivors" value={evolveResult.survivors.length} />
              <StatRow label="Avg Fitness" value={`${(evolveResult.avg_fitness * 100).toFixed(1)}%`} />
              {evolveResult.survivors.length > 0 && (
                <div style={{ marginTop: 8 }}>
                  <div style={{ fontSize: "0.72rem", color: "#64748b", marginBottom: 4 }}>SURVIVORS</div>
                  {evolveResult.survivors.map((id) => (
                    <div key={id} style={{ fontSize: "0.75rem", color: "#22c55e", padding: "2px 0", fontFamily: "monospace" }}>
                      {id.slice(0, 16)}...
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* Lineage Tab */}
      {tab === "lineage" && (
        <div style={panelStyle}>
          <h3 style={panelHeading}>Lineage Tree</h3>
          <div style={{ display: "flex", gap: 8, marginBottom: 16 }}>
            <select value={selectedA} onChange={(e) => setSelectedA(e.target.value)} style={selectStyle}>
              <option value="">Select agent...</option>
              {agents.map((a) => <option key={a.id} value={a.id}>{a.name}</option>)}
            </select>
            <button type="button" onClick={() => void handleLoadLineage()} style={btnStyle}>
              Load Lineage
            </button>
          </div>
          {lineage.length > 0 ? (
            <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
              {lineage.map((id, i) => {
                const name = agents.find((a) => a.id === id)?.name ?? id.slice(0, 16);
                return (
                  <div key={id} style={{ display: "flex", alignItems: "center", gap: 8, marginLeft: i * 24 }}>
                    {i > 0 && (
                      <span style={{ color: "#334155", fontFamily: "monospace" }}>└─</span>
                    )}
                    <span style={{
                      fontSize: "0.65rem", padding: "1px 4px", borderRadius: 3,
                      background: "rgba(167,139,250,0.15)", color: "#a78bfa",
                    }}>Gen {i}</span>
                    <span style={{ fontFamily: "monospace", fontSize: "0.78rem", color: "#e2e8f0" }}>{name}</span>
                    <span style={{ flex: 1, height: 1, background: "#1e293b" }} />
                    <span style={{ fontSize: "0.68rem", color: "#64748b", fontFamily: "monospace" }}>{id.slice(0, 12)}</span>
                  </div>
                );
              })}
            </div>
          ) : (
            <div style={{ color: "#64748b", fontSize: "0.82rem" }}>Select an agent and load its lineage</div>
          )}
        </div>
      )}
    </div>
  );
}

/* ================================================================== */
/*  Sub-components                                                     */
/* ================================================================== */

function GenomeCard({ genome }: { genome: AgentGenome }): JSX.Element {
  const g = genome.genes;
  return (
    <div style={{ marginTop: 12 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
        <span style={{ fontSize: "0.72rem", color: "#64748b" }}>
          Gen {genome.generation} | {genome.agent_id.slice(0, 12)}
        </span>
        {genome.generation > 0 && (
          <span style={{
            fontSize: "0.6rem", padding: "1px 5px", borderRadius: 3,
            background: "rgba(167,139,250,0.2)", color: "#a78bfa",
          }}>GEN {genome.generation}</span>
        )}
      </div>
      {geneBar(g.personality.verbosity, "Verbosity", "#8b5cf6")}
      {geneBar(g.personality.creativity, "Creativity", "#ec4899")}
      {geneBar(g.personality.assertiveness, "Assertive", "#f97316")}
      {geneBar(g.reasoning.temperature, "Temperature", "#eab308")}
      {geneBar(g.autonomy.risk_tolerance, "Risk", "#ef4444")}
      {geneBar(g.evolution.mutation_rate, "Mutation", "#22d3ee")}
      <div style={{ marginTop: 6, fontSize: "0.72rem", color: "#94a3b8" }}>
        Strategy: {g.reasoning.strategy} | Domains: {g.capabilities.domains.join(", ") || "none"}
      </div>
      {g.evolution.fitness_history.length > 0 && (
        <div style={{ marginTop: 6, display: "flex", alignItems: "center", gap: 8 }}>
          <span style={{ fontSize: "0.68rem", color: "#64748b" }}>Fitness:</span>
          {fitnessSparkline(g.evolution.fitness_history)}
        </div>
      )}
    </div>
  );
}

function StatRow({ label, value }: { label: string; value: string | number }): JSX.Element {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", padding: "3px 0", fontSize: "0.82rem" }}>
      <span style={{ color: "#94a3b8" }}>{label}</span>
      <span style={{ fontFamily: "monospace", color: "#e2e8f0" }}>{value}</span>
    </div>
  );
}

/* ================================================================== */
/*  Styles                                                             */
/* ================================================================== */

const panelStyle: React.CSSProperties = {
  background: "rgba(15,23,42,0.7)",
  border: "1px solid #1e293b",
  borderRadius: 10,
  padding: 20,
  backdropFilter: "blur(8px)",
};

const panelHeading: React.CSSProperties = {
  fontFamily: "monospace",
  fontSize: "0.95rem",
  color: "#a78bfa",
  marginBottom: 12,
  paddingBottom: 8,
  borderBottom: "1px solid #1e293b",
};

const selectStyle: React.CSSProperties = {
  flex: 1,
  padding: "8px 12px",
  background: "#0f172a",
  border: "1px solid #334155",
  borderRadius: 6,
  color: "#e2e8f0",
  fontFamily: "monospace",
  fontSize: "0.82rem",
};

const btnStyle: React.CSSProperties = {
  padding: "8px 20px",
  background: "rgba(167,139,250,0.15)",
  border: "1px solid #a78bfa",
  borderRadius: 6,
  color: "#a78bfa",
  cursor: "pointer",
  fontFamily: "monospace",
  fontSize: "0.82rem",
  fontWeight: 600,
};

const smallBtn: React.CSSProperties = {
  padding: "3px 10px",
  background: "transparent",
  border: "1px solid #22d3ee",
  borderRadius: 4,
  color: "#22d3ee",
  cursor: "pointer",
  fontFamily: "monospace",
  fontSize: "0.68rem",
};

import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  breedAgents,
  getAgentGenome,
  getAgentLineage,
  generateAllGenomes,
  genesisAnalyzeGap,
  genesisPreviewAgent,
  genesisCreateAgent,
  genesisDeleteAgent,
  genesisListGenerated,
  genesisStorePattern,
  evolutionGetStatus,
  evolutionEvolveOnce,
  evolutionGetHistory,
  evolutionGetActiveStrategy,
  evolutionRegisterStrategy,
  evolutionRollback,
  evolvePopulation,
  mutateAgent,
} from "../api/backend";
import {
  Dna, FlaskConical, Zap, TrendingUp, GitMerge, GitBranch,
  BarChart3, RefreshCw, Plus, Eye, Trash2, Layers, Search,
  Sparkles, Settings2, Users,
} from "lucide-react";
import "./dna-lab.css";

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

type TabKey = "breed" | "genome" | "evolve" | "lineage" | "evolution" | "genesis" | "genomeTools";

/* ================================================================== */
/*  Helpers                                                            */
/* ================================================================== */

function tryParseJson<T>(raw: string): T {
  return JSON.parse(raw) as T;
}

function geneBar(value: number, label: string, color: string): JSX.Element {
  return (
    <div style={{ display: "grid", gridTemplateColumns: "88px 1fr 42px", alignItems: "center", gap: 10, marginBottom: 10 }}>
      <span style={{ fontSize: "0.72rem", color: "var(--text-secondary)", textTransform: "uppercase", letterSpacing: "0.08em" }}>
        {label}
      </span>
      <span
        style={{
          position: "relative",
          height: 10,
          borderRadius: 999,
          overflow: "hidden",
          background: "rgba(118, 190, 255, 0.12)",
          border: "1px solid rgba(118, 190, 255, 0.12)",
        }}
      >
        <span
          style={{
            position: "absolute",
            inset: 0,
            width: `${Math.max(6, value * 100)}%`,
            borderRadius: 999,
            background: `linear-gradient(90deg, ${color}, color-mix(in srgb, ${color} 45%, white))`,
            boxShadow: `0 0 14px ${color}55`,
          }}
        />
      </span>
      <span style={{ width: 42, fontSize: "0.72rem", color: "var(--text-primary)", textAlign: "right", fontFamily: "var(--font-mono)" }}>
        {value.toFixed(2)}
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
        const hue = (v / max) * 120;
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
  /* --- original state --- */
  const [agents, setAgents] = useState<AgentEntry[]>([]);
  const [selectedA, setSelectedA] = useState("");
  const [selectedB, setSelectedB] = useState("");
  const [genomeA, setGenomeA] = useState<AgentGenome | null>(null);
  const [genomeB, setGenomeB] = useState<AgentGenome | null>(null);
  const [viewGenome, setViewGenome] = useState<AgentGenome | null>(null);
  const [offspring, setOffspring] = useState<AgentGenome | null>(null);
  const [evolveResult, setEvolveResult] = useState<EvolutionResult | null>(null);
  const [tab, setTab] = useState<TabKey>("breed");
  const [lineage, setLineage] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [breeding, setBreeding] = useState(false);
  const [mutating, setMutating] = useState(false);

  /* --- Evolution tab state --- */
  const [evoStatus, setEvoStatus] = useState<string>("");
  const [evoStatusLoading, setEvoStatusLoading] = useState(false);
  const [evoHistory, setEvoHistory] = useState<string>("");
  const [evoHistoryLoading, setEvoHistoryLoading] = useState(false);
  const [evoActiveStrategy, setEvoActiveStrategy] = useState<string>("");
  const [evoSelectedAgent, setEvoSelectedAgent] = useState("");
  const [evoEvolving, setEvoEvolving] = useState(false);
  const [evoRollingBack, setEvoRollingBack] = useState(false);
  const [stratName, setStratName] = useState("");
  const [stratParams, setStratParams] = useState("");
  const [stratRegistering, setStratRegistering] = useState(false);
  const [evoPopTask, setEvoPopTask] = useState("");
  const [evoPopGens, setEvoPopGens] = useState(1);
  const [evoPopResult, setEvoPopResult] = useState<string>("");
  const [evoPopRunning, setEvoPopRunning] = useState(false);

  /* --- Genesis tab state --- */
  const [gapRequest, setGapRequest] = useState("");
  const [gapResult, setGapResult] = useState<string>("");
  const [gapLoading, setGapLoading] = useState(false);
  const [previewRequest, setPreviewRequest] = useState("");
  const [previewLlm, setPreviewLlm] = useState("");
  const [previewResult, setPreviewResult] = useState<string>("");
  const [previewLoading, setPreviewLoading] = useState(false);
  const [createSpec, setCreateSpec] = useState("");
  const [createPrompt, setCreatePrompt] = useState("");
  const [createResult, setCreateResult] = useState<string>("");
  const [createLoading, setCreateLoading] = useState(false);
  const [generatedList, setGeneratedList] = useState<string>("");
  const [generatedLoading, setGeneratedLoading] = useState(false);
  const [deleteAgentName, setDeleteAgentName] = useState("");
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [patternSpec, setPatternSpec] = useState("");
  const [patternCaps, setPatternCaps] = useState("");
  const [patternScore, setPatternScore] = useState(0);
  const [patternLoading, setPatternLoading] = useState(false);
  const [patternResult, setPatternResult] = useState<string>("");

  /* --- Genome Tools tab state --- */
  const [gtGenomeAgent, setGtGenomeAgent] = useState("");
  const [gtGenomeResult, setGtGenomeResult] = useState<string>("");
  const [gtGenomeLoading, setGtGenomeLoading] = useState(false);
  const [gtLineageAgent, setGtLineageAgent] = useState("");
  const [gtLineageResult, setGtLineageResult] = useState<string>("");
  const [gtLineageLoading, setGtLineageLoading] = useState(false);
  const [gtBreedA, setGtBreedA] = useState("");
  const [gtBreedB, setGtBreedB] = useState("");
  const [gtBreedResult, setGtBreedResult] = useState<string>("");
  const [gtBreedLoading, setGtBreedLoading] = useState(false);
  const [gtGenAllLoading, setGtGenAllLoading] = useState(false);
  const [gtGenAllResult, setGtGenAllResult] = useState<string>("");

  /* ================================================================ */
  /*  Original data fetching                                          */
  /* ================================================================ */

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
      const raw = await getAgentGenome(agentId);
      setter(tryParseJson<AgentGenome>(raw));
    } catch { setter(null); }
  }, []);

  useEffect(() => { void loadGenome(selectedA, setGenomeA); }, [selectedA, loadGenome]);
  useEffect(() => { void loadGenome(selectedB, setGenomeB); }, [selectedB, loadGenome]);

  /* ================================================================ */
  /*  Original handlers                                               */
  /* ================================================================ */

  const handleBreed = useCallback(async () => {
    if (!selectedA || !selectedB) return;
    setBreeding(true);
    setError(null);
    try {
      const raw = await breedAgents(selectedA, selectedB);
      const result = tryParseJson<BreedResult>(raw);
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
      await mutateAgent(agentId, "random");
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
      const raw = await getAgentLineage(selectedA);
      const lin = tryParseJson<string[]>(raw);
      setLineage(Array.isArray(lin) ? lin : []);
    } catch { setLineage([]); }
  }, [selectedA]);

  const handleViewGenome = useCallback((agentId: string) => {
    setTab("genome");
    void loadGenome(agentId, setViewGenome);
  }, [loadGenome]);

  /* ================================================================ */
  /*  Evolution tab - load status on tab switch                       */
  /* ================================================================ */

  useEffect(() => {
    if (tab !== "evolution") return;
    setEvoStatusLoading(true);
    evolutionGetStatus()
      .then((r) => setEvoStatus(r))
      .catch((e) => setError(String(e)))
      .finally(() => setEvoStatusLoading(false));
  }, [tab]);

  const handleEvoEvolveOnce = useCallback(async () => {
    if (!evoSelectedAgent) return;
    setEvoEvolving(true);
    setError(null);
    try {
      const r = await evolutionEvolveOnce(evoSelectedAgent);
      setEvoStatus(r);
    } catch (e) { setError(String(e)); }
    finally { setEvoEvolving(false); }
  }, [evoSelectedAgent]);

  const handleEvoHistory = useCallback(async () => {
    if (!evoSelectedAgent) return;
    setEvoHistoryLoading(true);
    try {
      const r = await evolutionGetHistory(evoSelectedAgent);
      setEvoHistory(r);
    } catch (e) { setError(String(e)); }
    finally { setEvoHistoryLoading(false); }
  }, [evoSelectedAgent]);

  const handleEvoActiveStrategy = useCallback(async () => {
    if (!evoSelectedAgent) return;
    try {
      const r = await evolutionGetActiveStrategy(evoSelectedAgent);
      setEvoActiveStrategy(r);
    } catch (e) { setError(String(e)); }
  }, [evoSelectedAgent]);

  const handleEvoRegisterStrategy = useCallback(async () => {
    if (!evoSelectedAgent || !stratName) return;
    setStratRegistering(true);
    setError(null);
    try {
      await evolutionRegisterStrategy(evoSelectedAgent, stratName, stratParams);
      setStratName("");
      setStratParams("");
      const r = await evolutionGetActiveStrategy(evoSelectedAgent);
      setEvoActiveStrategy(r);
    } catch (e) { setError(String(e)); }
    finally { setStratRegistering(false); }
  }, [evoSelectedAgent, stratName, stratParams]);

  const handleEvoRollback = useCallback(async () => {
    if (!evoSelectedAgent) return;
    setEvoRollingBack(true);
    setError(null);
    try {
      const r = await evolutionRollback(evoSelectedAgent);
      setEvoStatus(r);
    } catch (e) { setError(String(e)); }
    finally { setEvoRollingBack(false); }
  }, [evoSelectedAgent]);

  const handleEvoPopulation = useCallback(async () => {
    if (agents.length === 0) return;
    setEvoPopRunning(true);
    setError(null);
    try {
      const ids = agents.map((a) => a.id);
      const r = await evolvePopulation(ids, evoPopTask, evoPopGens);
      setEvoPopResult(r);
    } catch (e) { setError(String(e)); }
    finally { setEvoPopRunning(false); }
  }, [agents, evoPopTask, evoPopGens]);

  /* ================================================================ */
  /*  Genesis tab - load generated list on tab switch                 */
  /* ================================================================ */

  useEffect(() => {
    if (tab !== "genesis") return;
    setGeneratedLoading(true);
    genesisListGenerated()
      .then((r) => setGeneratedList(r))
      .catch((e) => setError(String(e)))
      .finally(() => setGeneratedLoading(false));
  }, [tab]);

  const handleGapAnalysis = useCallback(async () => {
    if (!gapRequest) return;
    setGapLoading(true);
    setError(null);
    try {
      const r = await genesisAnalyzeGap(gapRequest);
      setGapResult(r);
    } catch (e) { setError(String(e)); }
    finally { setGapLoading(false); }
  }, [gapRequest]);

  const handlePreviewAgent = useCallback(async () => {
    if (!previewRequest) return;
    setPreviewLoading(true);
    setError(null);
    try {
      const r = await genesisPreviewAgent(previewRequest, previewLlm);
      setPreviewResult(r);
    } catch (e) { setError(String(e)); }
    finally { setPreviewLoading(false); }
  }, [previewRequest, previewLlm]);

  const handleCreateAgent = useCallback(async () => {
    if (!createSpec) return;
    setCreateLoading(true);
    setError(null);
    try {
      const r = await genesisCreateAgent(createSpec, createPrompt);
      setCreateResult(r);
      // Refresh generated list
      const list = await genesisListGenerated();
      setGeneratedList(list);
    } catch (e) { setError(String(e)); }
    finally { setCreateLoading(false); }
  }, [createSpec, createPrompt]);

  const handleDeleteAgent = useCallback(async () => {
    if (!deleteAgentName) return;
    setDeleteLoading(true);
    setError(null);
    try {
      await genesisDeleteAgent(deleteAgentName);
      setDeleteAgentName("");
      const list = await genesisListGenerated();
      setGeneratedList(list);
    } catch (e) { setError(String(e)); }
    finally { setDeleteLoading(false); }
  }, [deleteAgentName]);

  const handleStorePattern = useCallback(async () => {
    if (!patternSpec) return;
    setPatternLoading(true);
    setError(null);
    try {
      const caps = patternCaps.split(",").map((c) => c.trim()).filter(Boolean);
      const r = await genesisStorePattern(patternSpec, caps, patternScore);
      setPatternResult(r);
    } catch (e) { setError(String(e)); }
    finally { setPatternLoading(false); }
  }, [patternSpec, patternCaps, patternScore]);

  /* ================================================================ */
  /*  Genome Tools tab handlers                                       */
  /* ================================================================ */

  const handleGtViewGenome = useCallback(async () => {
    if (!gtGenomeAgent) return;
    setGtGenomeLoading(true);
    setError(null);
    try {
      const r = await getAgentGenome(gtGenomeAgent);
      setGtGenomeResult(r);
    } catch (e) { setError(String(e)); }
    finally { setGtGenomeLoading(false); }
  }, [gtGenomeAgent]);

  const handleGtViewLineage = useCallback(async () => {
    if (!gtLineageAgent) return;
    setGtLineageLoading(true);
    setError(null);
    try {
      const r = await getAgentLineage(gtLineageAgent);
      setGtLineageResult(r);
    } catch (e) { setError(String(e)); }
    finally { setGtLineageLoading(false); }
  }, [gtLineageAgent]);

  const handleGtBreed = useCallback(async () => {
    if (!gtBreedA || !gtBreedB) return;
    setGtBreedLoading(true);
    setError(null);
    try {
      const r = await breedAgents(gtBreedA, gtBreedB);
      setGtBreedResult(r);
      await loadAgents();
    } catch (e) { setError(String(e)); }
    finally { setGtBreedLoading(false); }
  }, [gtBreedA, gtBreedB, loadAgents]);

  const handleGtGenerateAll = useCallback(async () => {
    setGtGenAllLoading(true);
    setError(null);
    try {
      const r = await generateAllGenomes();
      setGtGenAllResult(r);
    } catch (e) { setError(String(e)); }
    finally { setGtGenAllLoading(false); }
  }, []);

  /* ================================================================ */
  /*  Render                                                          */
  /* ================================================================ */

  const allTabs: { key: TabKey; label: string; icon: React.ReactNode }[] = [
    { key: "breed", label: "BREED", icon: <GitMerge size={14} /> },
    { key: "genome", label: "GENOME", icon: <Dna size={14} /> },
    { key: "evolve", label: "EVOLVE", icon: <Zap size={14} /> },
    { key: "lineage", label: "LINEAGE", icon: <GitBranch size={14} /> },
    { key: "evolution", label: "EVOLUTION", icon: <TrendingUp size={14} /> },
    { key: "genesis", label: "GENESIS", icon: <Sparkles size={14} /> },
    { key: "genomeTools", label: "GENOME TOOLS", icon: <FlaskConical size={14} /> },
  ];

  return (
    <div className="dna-shell">
      <section className="dna-hero nx-spatial-container">
        <div className="dna-hero__content nx-spatial-layer-front">
          <div className="dna-kicker">Evolution System</div>
          <h1 className="dna-title">
            <Dna size={28} />
            Agent DNA Lab
          </h1>
          <p className="dna-copy">
            Breed agents, inspect genome signatures, explore lineages, and tune evolution strategies inside the Nexus adaptation bay.
          </p>
          <div className="dna-chip-row">
            <span className="dna-chip">
              <Users size={13} aria-hidden="true" />
              {agents.length} loaded genomes
            </span>
            <span className="dna-chip">
              <Sparkles size={13} aria-hidden="true" />
              {tab.toUpperCase()} mode
            </span>
            <span className="dna-chip">
              <GitBranch size={13} aria-hidden="true" />
              controlled evolution
            </span>
          </div>
        </div>
        <div className="dna-helix nx-spatial-layer-back" aria-hidden="true">
          {Array.from({ length: 12 }, (_, index) => (
            <div key={index} className="dna-helix__row" style={{ animationDelay: `${index * 0.14}s` }}>
              <span className="dna-helix__node dna-helix__node--teal" />
              <span className="dna-helix__strand" />
              <span className="dna-helix__node dna-helix__node--purple" />
            </div>
          ))}
        </div>
      </section>

      <div className="dna-tabs">
        {allTabs.map((t) => (
          <button
            key={t.key}
            type="button"
            onClick={() => setTab(t.key)}
            className={`dna-tab ${tab === t.key ? "is-active" : ""}`}
          >
            {t.icon} {t.label}
          </button>
        ))}
      </div>

      {error && <div style={{ color: "#f87171", marginBottom: 12, fontSize: "0.85rem" }}>{error}</div>}

      {/* ============================================================ */}
      {/*  Breed Tab (original)                                        */}
      {/* ============================================================ */}
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
                display: "flex", alignItems: "center", justifyContent: "center",
              }}>
                {breeding ? <RefreshCw size={22} className="animate-spin" /> : <GitMerge size={22} />}
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

      {/* ============================================================ */}
      {/*  Genome Viewer Tab (original)                                */}
      {/* ============================================================ */}
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

      {/* ============================================================ */}
      {/*  Evolve Tab (original)                                       */}
      {/* ============================================================ */}
      {tab === "evolve" && (
        <div style={panelStyle}>
          <h3 style={panelHeading}><Zap size={16} style={{ marginRight: 6 }} />Evolution Playground</h3>
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

      {/* ============================================================ */}
      {/*  Lineage Tab (original)                                      */}
      {/* ============================================================ */}
      {tab === "lineage" && (
        <div style={panelStyle}>
          <h3 style={panelHeading}><GitBranch size={16} style={{ marginRight: 6 }} />Lineage Tree</h3>
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
                      <span style={{ color: "#334155", fontFamily: "monospace" }}>{"\u2514\u2500"}</span>
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

      {/* ============================================================ */}
      {/*  EVOLUTION Tab (NEW)                                         */}
      {/* ============================================================ */}
      {tab === "evolution" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          {/* Status */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><BarChart3 size={16} style={{ marginRight: 6 }} />Evolution Status</h3>
            {evoStatusLoading
              ? <div style={loadingText}>Loading status...</div>
              : <pre style={preStyle}>{evoStatus || "No status data"}</pre>
            }
          </div>

          {/* Agent selector */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><Settings2 size={16} style={{ marginRight: 6 }} />Agent Evolution Controls</h3>
            <select value={evoSelectedAgent} onChange={(e) => setEvoSelectedAgent(e.target.value)} style={{ ...selectStyle, marginBottom: 12 }}>
              <option value="">Select agent...</option>
              {agents.map((a) => <option key={a.id} value={a.id}>{a.name} ({a.id.slice(0, 8)})</option>)}
            </select>

            <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 16 }}>
              <button type="button" onClick={() => void handleEvoEvolveOnce()} disabled={!evoSelectedAgent || evoEvolving} style={btnStyle}>
                {evoEvolving ? "Evolving..." : "Evolve Once"}
              </button>
              <button type="button" onClick={() => void handleEvoHistory()} disabled={!evoSelectedAgent || evoHistoryLoading} style={btnStyle}>
                {evoHistoryLoading ? "Loading..." : "View History"}
              </button>
              <button type="button" onClick={() => void handleEvoActiveStrategy()} disabled={!evoSelectedAgent} style={btnStyle}>
                Active Strategy
              </button>
              <button type="button" onClick={() => void handleEvoRollback()} disabled={!evoSelectedAgent || evoRollingBack}
                style={{ ...btnStyle, borderColor: "#f97316", color: "#f97316" }}>
                {evoRollingBack ? "Rolling back..." : "Rollback"}
              </button>
            </div>

            {evoHistory && (
              <div style={{ marginBottom: 12 }}>
                <div style={sectionLabel}>History</div>
                <pre style={preStyle}>{evoHistory}</pre>
              </div>
            )}
            {evoActiveStrategy && (
              <div>
                <div style={sectionLabel}>Active Strategy</div>
                <pre style={preStyle}>{evoActiveStrategy}</pre>
              </div>
            )}
          </div>

          {/* Register strategy */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><Plus size={16} style={{ marginRight: 6 }} />Register Strategy</h3>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              <select value={evoSelectedAgent} onChange={(e) => setEvoSelectedAgent(e.target.value)} style={selectStyle}>
                <option value="">Select agent...</option>
                {agents.map((a) => <option key={a.id} value={a.id}>{a.name} ({a.id.slice(0, 8)})</option>)}
              </select>
              <input
                type="text"
                placeholder="Strategy name"
                value={stratName}
                onChange={(e) => setStratName(e.target.value)}
                style={inputStyle}
              />
              <textarea
                placeholder='Parameters JSON, e.g. {"mutation_rate": 0.1, "crossover": "uniform"}'
                value={stratParams}
                onChange={(e) => setStratParams(e.target.value)}
                rows={3}
                style={textareaStyle}
              />
              <button type="button" onClick={() => void handleEvoRegisterStrategy()} disabled={!evoSelectedAgent || !stratName || stratRegistering} style={btnStyle}>
                {stratRegistering ? "Registering..." : "Register Strategy"}
              </button>
            </div>
          </div>

          {/* Evolve population */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><Users size={16} style={{ marginRight: 6 }} />Evolve Population</h3>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              <input
                type="text"
                placeholder="Task description"
                value={evoPopTask}
                onChange={(e) => setEvoPopTask(e.target.value)}
                style={inputStyle}
              />
              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <label style={{ fontSize: "0.78rem", color: "#94a3b8" }}>Generations:</label>
                <input
                  type="number"
                  min={1}
                  max={100}
                  value={evoPopGens}
                  onChange={(e) => setEvoPopGens(Number(e.target.value))}
                  style={{ ...inputStyle, width: 80 }}
                />
              </div>
              <button type="button" onClick={() => void handleEvoPopulation()} disabled={evoPopRunning || agents.length === 0} style={btnStyle}>
                {evoPopRunning ? "Running..." : `Evolve All Agents (${agents.length})`}
              </button>
              {evoPopResult && <pre style={preStyle}>{evoPopResult}</pre>}
            </div>
          </div>
        </div>
      )}

      {/* ============================================================ */}
      {/*  GENESIS Tab (NEW)                                           */}
      {/* ============================================================ */}
      {tab === "genesis" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          {/* Gap Analysis */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><Search size={16} style={{ marginRight: 6 }} />Gap Analysis</h3>
            <p style={{ color: "#94a3b8", fontSize: "0.78rem", marginBottom: 8 }}>
              Describe what you need and Genesis will analyze capability gaps.
            </p>
            <textarea
              placeholder="e.g. I need an agent that can manage Kubernetes clusters and auto-scale pods"
              value={gapRequest}
              onChange={(e) => setGapRequest(e.target.value)}
              rows={3}
              style={textareaStyle}
            />
            <button type="button" onClick={() => void handleGapAnalysis()} disabled={!gapRequest || gapLoading} style={{ ...btnStyle, marginTop: 8 }}>
              {gapLoading ? "Analyzing..." : "Analyze Gap"}
            </button>
            {gapResult && <pre style={{ ...preStyle, marginTop: 8 }}>{gapResult}</pre>}
          </div>

          {/* Preview Agent */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><Eye size={16} style={{ marginRight: 6 }} />Preview Agent</h3>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              <textarea
                placeholder="User request"
                value={previewRequest}
                onChange={(e) => setPreviewRequest(e.target.value)}
                rows={2}
                style={textareaStyle}
              />
              <textarea
                placeholder="LLM response (optional)"
                value={previewLlm}
                onChange={(e) => setPreviewLlm(e.target.value)}
                rows={2}
                style={textareaStyle}
              />
              <button type="button" onClick={() => void handlePreviewAgent()} disabled={!previewRequest || previewLoading} style={btnStyle}>
                {previewLoading ? "Previewing..." : "Preview Agent"}
              </button>
              {previewResult && <pre style={{ ...preStyle, marginTop: 8 }}>{previewResult}</pre>}
            </div>
          </div>

          {/* Create Agent */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><Plus size={16} style={{ marginRight: 6 }} />Create Agent</h3>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              <textarea
                placeholder='Spec JSON, e.g. {"name": "k8s-agent", "capabilities": ["kubernetes", "scaling"]}'
                value={createSpec}
                onChange={(e) => setCreateSpec(e.target.value)}
                rows={4}
                style={textareaStyle}
              />
              <textarea
                placeholder="System prompt"
                value={createPrompt}
                onChange={(e) => setCreatePrompt(e.target.value)}
                rows={3}
                style={textareaStyle}
              />
              <button type="button" onClick={() => void handleCreateAgent()} disabled={!createSpec || createLoading} style={btnStyle}>
                {createLoading ? "Creating..." : "Create Agent"}
              </button>
              {createResult && <pre style={{ ...preStyle, marginTop: 8 }}>{createResult}</pre>}
            </div>
          </div>

          {/* List Generated */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><Layers size={16} style={{ marginRight: 6 }} />Generated Agents</h3>
            {generatedLoading
              ? <div style={loadingText}>Loading...</div>
              : <pre style={preStyle}>{generatedList || "No generated agents"}</pre>
            }
          </div>

          {/* Delete Agent */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><Trash2 size={16} style={{ marginRight: 6 }} />Delete Generated Agent</h3>
            <div style={{ display: "flex", gap: 8 }}>
              <input
                type="text"
                placeholder="Agent name"
                value={deleteAgentName}
                onChange={(e) => setDeleteAgentName(e.target.value)}
                style={inputStyle}
              />
              <button type="button" onClick={() => void handleDeleteAgent()} disabled={!deleteAgentName || deleteLoading}
                style={{ ...btnStyle, borderColor: "#ef4444", color: "#ef4444" }}>
                {deleteLoading ? "Deleting..." : "Delete"}
              </button>
            </div>
          </div>

          {/* Store Pattern */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><Sparkles size={16} style={{ marginRight: 6 }} />Store Pattern</h3>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              <textarea
                placeholder="Spec JSON"
                value={patternSpec}
                onChange={(e) => setPatternSpec(e.target.value)}
                rows={3}
                style={textareaStyle}
              />
              <input
                type="text"
                placeholder="Missing capabilities (comma-separated)"
                value={patternCaps}
                onChange={(e) => setPatternCaps(e.target.value)}
                style={inputStyle}
              />
              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <label style={{ fontSize: "0.78rem", color: "#94a3b8" }}>Test score:</label>
                <input
                  type="number"
                  min={0}
                  max={100}
                  step={0.1}
                  value={patternScore}
                  onChange={(e) => setPatternScore(Number(e.target.value))}
                  style={{ ...inputStyle, width: 80 }}
                />
              </div>
              <button type="button" onClick={() => void handleStorePattern()} disabled={!patternSpec || patternLoading} style={btnStyle}>
                {patternLoading ? "Storing..." : "Store Pattern"}
              </button>
              {patternResult && <pre style={{ ...preStyle, marginTop: 8 }}>{patternResult}</pre>}
            </div>
          </div>
        </div>
      )}

      {/* ============================================================ */}
      {/*  GENOME TOOLS Tab (NEW)                                      */}
      {/* ============================================================ */}
      {tab === "genomeTools" && (
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          {/* View Genome */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><Dna size={16} style={{ marginRight: 6 }} />View Agent Genome</h3>
            <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
              <select value={gtGenomeAgent} onChange={(e) => setGtGenomeAgent(e.target.value)} style={selectStyle}>
                <option value="">Select agent...</option>
                {agents.map((a) => <option key={a.id} value={a.id}>{a.name} ({a.id.slice(0, 8)})</option>)}
              </select>
              <button type="button" onClick={() => void handleGtViewGenome()} disabled={!gtGenomeAgent || gtGenomeLoading} style={btnStyle}>
                {gtGenomeLoading ? "Loading..." : "View Genome"}
              </button>
            </div>
            {gtGenomeResult && <pre style={preStyle}>{gtGenomeResult}</pre>}
          </div>

          {/* View Lineage */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><GitBranch size={16} style={{ marginRight: 6 }} />View Agent Lineage</h3>
            <div style={{ display: "flex", gap: 8, marginBottom: 12 }}>
              <select value={gtLineageAgent} onChange={(e) => setGtLineageAgent(e.target.value)} style={selectStyle}>
                <option value="">Select agent...</option>
                {agents.map((a) => <option key={a.id} value={a.id}>{a.name} ({a.id.slice(0, 8)})</option>)}
              </select>
              <button type="button" onClick={() => void handleGtViewLineage()} disabled={!gtLineageAgent || gtLineageLoading} style={btnStyle}>
                {gtLineageLoading ? "Loading..." : "View Lineage"}
              </button>
            </div>
            {gtLineageResult && <pre style={preStyle}>{gtLineageResult}</pre>}
          </div>

          {/* Breed */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><GitMerge size={16} style={{ marginRight: 6 }} />Breed Agents</h3>
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, marginBottom: 12 }}>
              <select value={gtBreedA} onChange={(e) => setGtBreedA(e.target.value)} style={selectStyle}>
                <option value="">Parent A...</option>
                {agents.map((a) => <option key={a.id} value={a.id}>{a.name} ({a.id.slice(0, 8)})</option>)}
              </select>
              <select value={gtBreedB} onChange={(e) => setGtBreedB(e.target.value)} style={selectStyle}>
                <option value="">Parent B...</option>
                {agents.map((a) => <option key={a.id} value={a.id}>{a.name} ({a.id.slice(0, 8)})</option>)}
              </select>
            </div>
            <button type="button" onClick={() => void handleGtBreed()} disabled={!gtBreedA || !gtBreedB || gtBreedLoading} style={btnStyle}>
              {gtBreedLoading ? "Breeding..." : "Breed"}
            </button>
            {gtBreedResult && <pre style={{ ...preStyle, marginTop: 8 }}>{gtBreedResult}</pre>}
          </div>

          {/* Generate All Genomes */}
          <div style={panelStyle}>
            <h3 style={panelHeading}><FlaskConical size={16} style={{ marginRight: 6 }} />Generate All Genomes</h3>
            <p style={{ color: "#94a3b8", fontSize: "0.78rem", marginBottom: 12 }}>
              Generate genomes for all agents that do not yet have one.
            </p>
            <button type="button" onClick={() => void handleGtGenerateAll()} disabled={gtGenAllLoading} style={btnStyle}>
              {gtGenAllLoading ? "Generating..." : "Generate All Genomes"}
            </button>
            {gtGenAllResult && <pre style={{ ...preStyle, marginTop: 8 }}>{gtGenAllResult}</pre>}
          </div>
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
  position: "relative",
  background: "linear-gradient(165deg, rgba(8,15,27,0.92), rgba(11,22,38,0.72))",
  border: "1px solid rgba(118,190,255,0.14)",
  borderRadius: 24,
  padding: 20,
  backdropFilter: "blur(16px)",
  boxShadow: "inset 0 1px 0 rgba(255,255,255,0.08), 0 24px 64px -42px rgba(74,247,211,0.22)",
};

const panelHeading: React.CSSProperties = {
  fontFamily: "var(--font-display)",
  fontSize: "0.92rem",
  color: "var(--nexus-purple)",
  marginBottom: 12,
  paddingBottom: 8,
  borderBottom: "1px solid rgba(118,190,255,0.12)",
  display: "flex",
  alignItems: "center",
  letterSpacing: "0.1em",
  textTransform: "uppercase",
};

const selectStyle: React.CSSProperties = {
  flex: 1,
  minHeight: 44,
  padding: "10px 14px",
  background: "rgba(5,11,21,0.82)",
  border: "1px solid rgba(118,190,255,0.16)",
  borderRadius: 16,
  color: "var(--text-primary)",
  fontFamily: "var(--font-mono)",
  fontSize: "0.82rem",
  cursor: "pointer",
};

const btnStyle: React.CSSProperties = {
  minHeight: 44,
  padding: "8px 20px",
  background: "linear-gradient(135deg, rgba(33,21,58,0.92), rgba(12,18,32,0.88))",
  border: "1px solid rgba(140,123,255,0.28)",
  borderRadius: 999,
  color: "var(--nexus-purple)",
  cursor: "pointer",
  fontFamily: "var(--font-mono)",
  fontSize: "0.82rem",
  fontWeight: 600,
};

const smallBtn: React.CSSProperties = {
  minHeight: 36,
  padding: "3px 12px",
  background: "rgba(7,14,25,0.82)",
  border: "1px solid rgba(74,247,211,0.24)",
  borderRadius: 999,
  color: "var(--nexus-accent)",
  cursor: "pointer",
  fontFamily: "var(--font-mono)",
  fontSize: "0.68rem",
};

const inputStyle: React.CSSProperties = {
  minHeight: 44,
  padding: "10px 14px",
  background: "rgba(5,11,21,0.82)",
  border: "1px solid rgba(118,190,255,0.16)",
  borderRadius: 16,
  color: "var(--text-primary)",
  fontFamily: "var(--font-mono)",
  fontSize: "0.82rem",
};

const textareaStyle: React.CSSProperties = {
  padding: "10px 14px",
  background: "rgba(5,11,21,0.82)",
  border: "1px solid rgba(118,190,255,0.16)",
  borderRadius: 18,
  color: "var(--text-primary)",
  fontFamily: "var(--font-mono)",
  fontSize: "0.82rem",
  resize: "vertical",
};

const preStyle: React.CSSProperties = {
  background: "rgba(5,11,21,0.88)",
  border: "1px solid rgba(118,190,255,0.12)",
  borderRadius: 18,
  padding: 12,
  fontSize: "0.75rem",
  fontFamily: "var(--font-mono)",
  color: "var(--text-primary)",
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
  maxHeight: 300,
  overflow: "auto",
};

const loadingText: React.CSSProperties = {
  color: "var(--text-secondary)",
  fontSize: "0.82rem",
  fontFamily: "var(--font-mono)",
};

const sectionLabel: React.CSSProperties = {
  fontSize: "0.72rem",
  color: "var(--text-muted)",
  marginBottom: 4,
  textTransform: "uppercase",
  letterSpacing: "0.12em",
};

import { useEffect, useMemo, useRef, useState } from "react";
import {
  chatWithSimulationPersona,
  createSimulation,
  getSimulationReport,
  getSimulationStatus,
  hasDesktopRuntime,
  injectSimulationVariable,
  listSimulations,
  pauseSimulation,
  runParallelSimulations,
  startSimulation,
} from "../api/backend";
import type {
  PredictionReport,
  SimulationCompleteEvent,
  SimulationLiveEvent,
  SimulationPersonaView,
  SimulationStatus,
  SimulationSummary,
  SimulationTickEvent,
} from "../types";
import "./world-simulation.css";

type ChatLine = {
  role: "user" | "persona";
  content: string;
};

type SimulationTab = "create" | "live" | "results" | "chat";

type SeedPreset = {
  id: string;
  label: string;
  worldName: string;
  text: string;
};

const SEED_PRESETS: SeedPreset[] = [
  {
    id: "tech",
    label: "Tech Industry",
    worldName: "Tech Industry Rivalry",
    text:
      "Three major AI companies are racing to ship multimodal assistant platforms. One has a distribution edge through enterprise contracts, one has a research lead, and one is cutting prices aggressively. Regulators are studying model safety disclosures, investors want rapid growth, developers are choosing ecosystems, and journalists are amplifying every benchmark result. Rumors of a partnership, an acquisition, and a talent exodus are shaping public perception.",
  },
  {
    id: "election",
    label: "Election Scenario",
    worldName: "National Election Stress Test",
    text:
      "A closely contested national election is approaching. The incumbent is campaigning on economic stability, while the challenger promises institutional reform and lower living costs. Swing voters are worried about inflation, younger voters care about housing and climate, donors are shifting allegiances, and news outlets are framing every debate as a turning point. A late policy announcement and a televised scandal could reshape turnout in key regions.",
  },
  {
    id: "market",
    label: "Market Crash",
    worldName: "Market Crash Contagion",
    text:
      "Global markets are under strain after a major regional bank reveals liquidity problems. Bond yields spike, hedge funds unwind crowded positions, retail traders panic on social media, and central banks are signaling conflicting priorities between inflation control and financial stability. Corporate treasurers are delaying investment, credit spreads are widening, and analysts are debating whether this is a contained shock or the start of a broader credit event.",
  },
];

const TABS: Array<{ id: SimulationTab; label: string }> = [
  { id: "create", label: "Create World" },
  { id: "live", label: "Live Simulation" },
  { id: "results", label: "Results" },
  { id: "chat", label: "Chat With Personas" },
];

function personaAverage(persona: SimulationPersonaView): number {
  const values = Object.values(persona.beliefs);
  if (values.length === 0) {
    return 0;
  }
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function personaColor(persona: SimulationPersonaView): string {
  const average = Math.max(-1, Math.min(1, personaAverage(persona)));
  const red = Math.round(((average * -1 + 1) / 2) * 225 + 20);
  const blue = Math.round(((average + 1) / 2) * 225 + 20);
  return `rgb(${red}, 104, ${blue})`;
}

function personaSize(persona: SimulationPersonaView): number {
  return 7 + persona.influence_score * 10;
}

function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 60);
}

function formatStatus(status: string | undefined): string {
  if (!status) {
    return "Unknown";
  }
  return status
    .split("_")
    .map((token) => token.charAt(0).toUpperCase() + token.slice(1))
    .join(" ");
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function describeLiveEvent(event: SimulationLiveEvent): string {
  switch (event.action_type) {
    case "speak":
      return `${event.actor_name} said: ${event.content}`;
    case "whisper":
      return `${event.actor_name} whispered to ${event.target_name ?? event.target_id ?? "someone"}: ${event.content}`;
    case "act":
      return `${event.actor_name} acted: ${event.content}`;
    case "observe":
      return `${event.actor_name} observed the environment`;
    default:
      return `${event.actor_name} held position`;
  }
}

function buildReportMarkdown(
  worldName: string,
  report: PredictionReport,
  parallelReports: PredictionReport[],
): string {
  return [
    `# ${worldName} Simulation Report`,
    "",
    "## Summary",
    "",
    report.summary,
    "",
    "## Key Findings",
    "",
    ...report.key_findings.map(
      (finding, index) =>
        `${index + 1}. **${finding.title}** (${Math.round(finding.confidence * 100)}%) ${finding.detail}`,
    ),
    "",
    "## Opinion Shifts",
    "",
    ...report.opinion_shifts.map(
      (shift) =>
        `- **${shift.topic}** before ${shift.before.toFixed(2)}, after ${shift.after.toFixed(2)}, delta ${shift.delta.toFixed(2)}`,
    ),
    "",
    "## Coalitions",
    "",
    ...report.coalitions.map(
      (coalition) =>
        `- **${coalition.name}**: ${coalition.members.join(", ")} | Topics: ${coalition.focus_topics.join(", ")}`,
    ),
    "",
    "## Turning Points",
    "",
    ...report.turning_points.map(
      (turningPoint) =>
        `- Tick ${turningPoint.tick}: ${turningPoint.description} (${turningPoint.shift_magnitude.toFixed(2)})`,
    ),
    "",
    "## Final Prediction",
    "",
    `${report.prediction}`,
    "",
    `Confidence: ${Math.round(report.confidence * 100)}%`,
    "",
    "## Uncertainties",
    "",
    ...report.uncertainties.map((uncertainty) => `- ${uncertainty}`),
    "",
    "## Parallel Variants",
    "",
    ...(parallelReports.length > 0
      ? parallelReports.map(
          (variant, index) =>
            `- Variant ${index + 1}: ${variant.prediction} (${Math.round(variant.confidence * 100)}%)`,
        )
      : ["- No parallel simulations were exported."]),
    "",
  ].join("\n");
}

function beliefEntries(persona: SimulationPersonaView | null): Array<[string, number]> {
  if (!persona) {
    return [];
  }
  return Object.entries(persona.beliefs).sort((left, right) => Math.abs(right[1]) - Math.abs(left[1]));
}

export default function WorldSimulation(): JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const eventFeedRef = useRef<HTMLDivElement | null>(null);
  const [activeTab, setActiveTab] = useState<SimulationTab>("create");
  const [worldName, setWorldName] = useState("Nexus World");
  const [seedText, setSeedText] = useState(SEED_PRESETS[0].text);
  const [personaCount, setPersonaCount] = useState(50);
  const [maxTicks, setMaxTicks] = useState(50);
  const [tickSpeed, setTickSpeed] = useState(1000);
  const [variantCount, setVariantCount] = useState(5);
  const [summaries, setSummaries] = useState<SimulationSummary[]>([]);
  const [worldId, setWorldId] = useState("");
  const [simulation, setSimulation] = useState<SimulationStatus | null>(null);
  const [report, setReport] = useState<PredictionReport | null>(null);
  const [parallelReports, setParallelReports] = useState<PredictionReport[]>([]);
  const [selectedPersonaId, setSelectedPersonaId] = useState("");
  const [chatInput, setChatInput] = useState("");
  const [chatLines, setChatLines] = useState<ChatLine[]>([]);
  const [injectKey, setInjectKey] = useState("policy_signal");
  const [injectValue, setInjectValue] = useState("passed");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [liveTick, setLiveTick] = useState<SimulationTickEvent | null>(null);
  const [eventFeed, setEventFeed] = useState<SimulationLiveEvent[]>([]);

  const activePersona = useMemo(
    () => simulation?.personas.find((persona) => persona.id === selectedPersonaId) ?? null,
    [selectedPersonaId, simulation],
  );
  const topBeliefs = useMemo(() => beliefEntries(activePersona).slice(0, 6), [activePersona]);

  async function refreshSummaries(): Promise<void> {
    if (!hasDesktopRuntime()) {
      return;
    }
    const rows = await listSimulations();
    setSummaries(rows);
    if (!worldId && rows.length > 0) {
      setWorldId(rows[0].id);
    }
  }

  async function refreshSimulation(targetWorldId: string): Promise<void> {
    if (!targetWorldId || !hasDesktopRuntime()) {
      return;
    }
    const status = await getSimulationStatus(targetWorldId);
    setSimulation(status);
    setSelectedPersonaId((current) => {
      if (current && status.personas.some((persona) => persona.id === current)) {
        return current;
      }
      return status.personas[0]?.id ?? "";
    });
    if (status.report_available) {
      setReport(await getSimulationReport(targetWorldId));
    } else {
      setReport(null);
    }
  }

  function pickSeed(seed: SeedPreset): void {
    setWorldName(seed.worldName);
    setSeedText(seed.text);
  }

  function selectExistingSimulation(summary: SimulationSummary): void {
    setWorldId(summary.id);
    setError(null);
    setChatLines([]);
    setEventFeed([]);
    if (summary.status === "completed") {
      setActiveTab("results");
    } else if (summary.status === "running" || summary.status === "paused" || summary.status === "ready") {
      setActiveTab("live");
    } else {
      setActiveTab("create");
    }
  }

  useEffect(() => {
    void refreshSummaries();
  }, []);

  useEffect(() => {
    if (!worldId) {
      return;
    }
    void refreshSimulation(worldId);
  }, [worldId]);

  useEffect(() => {
    if (!hasDesktopRuntime()) {
      return;
    }
    let tickCleanup: (() => void) | undefined;
    let completeCleanup: (() => void) | undefined;
    import("@tauri-apps/api/event").then((mod) => {
      mod.listen<SimulationTickEvent>("simulation-tick", (event) => {
        if (event.payload.world_id !== worldId) {
          return;
        }
        setLiveTick(event.payload);
        setEventFeed((current) => [...current, ...event.payload.events].slice(-140));
        void refreshSimulation(event.payload.world_id);
      }).then((cleanup) => {
        tickCleanup = cleanup;
      });
      mod.listen<SimulationCompleteEvent>("simulation-complete", (event) => {
        if (event.payload.world_id !== worldId) {
          return;
        }
        void refreshSimulation(event.payload.world_id);
        void refreshSummaries();
        setActiveTab("results");
      }).then((cleanup) => {
        completeCleanup = cleanup;
      });
    });
    return () => {
      tickCleanup?.();
      completeCleanup?.();
    };
  }, [worldId]);

  useEffect(() => {
    if (!eventFeedRef.current) {
      return;
    }
    eventFeedRef.current.scrollTop = eventFeedRef.current.scrollHeight;
  }, [eventFeed]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !simulation) {
      return;
    }
    const context = canvas.getContext("2d");
    if (!context) {
      return;
    }
    const width = canvas.width;
    const height = canvas.height;
    const personas = simulation.personas;
    const tick = liveTick?.tick ?? simulation.tick_count;

    context.clearRect(0, 0, width, height);
    context.fillStyle = "#081019";
    context.fillRect(0, 0, width, height);

    const positions = personas.map((persona, index) => {
      const baseAngle = (index / Math.max(personas.length, 1)) * Math.PI * 2;
      const orbit = Math.min(width, height) * 0.28 + (index % 5) * 12 + persona.influence_score * 22;
      const driftX = Math.sin(tick * 0.18 + index * 0.9) * 22;
      const driftY = Math.cos(tick * 0.16 + index * 1.3) * 18;
      return {
        persona,
        x: width / 2 + Math.cos(baseAngle + tick * 0.045) * orbit + driftX,
        y: height / 2 + Math.sin(baseAngle + tick * 0.05) * orbit + driftY,
      };
    });

    positions.forEach((origin) => {
      Object.entries(origin.persona.relationships).forEach(([targetId, strength]) => {
        const target = positions.find((candidate) => candidate.persona.id === targetId);
        if (!target || Math.abs(strength) < 0.16) {
          return;
        }
        context.beginPath();
        context.lineWidth = Math.max(0.8, Math.abs(strength) * 3.8);
        context.strokeStyle =
          strength >= 0 ? "rgba(89, 231, 161, 0.34)" : "rgba(255, 107, 107, 0.3)";
        context.moveTo(origin.x, origin.y);
        context.lineTo(target.x, target.y);
        context.stroke();
      });
    });

    positions.forEach(({ persona, x, y }) => {
      context.beginPath();
      context.arc(x, y, personaSize(persona), 0, Math.PI * 2);
      context.fillStyle = personaColor(persona);
      context.fill();
      context.lineWidth = selectedPersonaId === persona.id ? 2.5 : 1.1;
      context.strokeStyle = selectedPersonaId === persona.id ? "#ffd889" : "rgba(255, 255, 255, 0.42)";
      context.stroke();
    });
  }, [liveTick, selectedPersonaId, simulation]);

  async function handleCreateAndRun(): Promise<void> {
    setBusy("create");
    setError(null);
    try {
      const id = await createSimulation(
        worldName.trim() || "Nexus World",
        seedText,
        personaCount,
        maxTicks,
        tickSpeed,
      );
      setWorldId(id);
      setReport(null);
      setParallelReports([]);
      setChatLines([]);
      setEventFeed([]);
      setLiveTick(null);
      await refreshSummaries();
      await refreshSimulation(id);
      await startSimulation(id);
      await refreshSimulation(id);
      setActiveTab("live");
    } catch (cause) {
      setError(String(cause));
      await refreshSummaries();
    } finally {
      setBusy(null);
    }
  }

  async function handleStart(): Promise<void> {
    if (!worldId) {
      return;
    }
    setBusy("start");
    setError(null);
    try {
      await startSimulation(worldId);
      await refreshSimulation(worldId);
      setActiveTab("live");
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function handlePause(): Promise<void> {
    if (!worldId) {
      return;
    }
    setBusy("pause");
    setError(null);
    try {
      await pauseSimulation(worldId);
      await refreshSimulation(worldId);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function handleInject(): Promise<void> {
    if (!worldId || !injectKey.trim() || !injectValue.trim()) {
      return;
    }
    setBusy("inject");
    setError(null);
    try {
      await injectSimulationVariable(worldId, injectKey.trim(), injectValue.trim());
      await refreshSimulation(worldId);
      setInjectKey("");
      setInjectValue("");
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function handleParallelRun(): Promise<void> {
    setBusy("parallel");
    setError(null);
    try {
      const reports = await runParallelSimulations(seedText, variantCount);
      setParallelReports(reports);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function handlePersonaChat(): Promise<void> {
    if (!worldId || !selectedPersonaId || !chatInput.trim()) {
      return;
    }
    const message = chatInput.trim();
    setChatInput("");
    setChatLines((current) => [...current, { role: "user", content: message }]);
    try {
      const reply = await chatWithSimulationPersona(worldId, selectedPersonaId, message);
      setChatLines((current) => [...current, { role: "persona", content: reply }]);
    } catch (cause) {
      setError(String(cause));
    }
  }

  function handleExportReport(): void {
    if (!report || !simulation) {
      return;
    }
    const markdown = buildReportMarkdown(simulation.name, report, parallelReports);
    const blob = new Blob([markdown], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `${slugify(simulation.name)}-simulation-report.md`;
    link.click();
    URL.revokeObjectURL(url);
  }

  if (!hasDesktopRuntime()) {
    return (
      <section className="world-sim-page">
        <div className="world-sim-hero">
          <div>
            <p className="world-sim-kicker">NEXUS WORLD ENGINE</p>
            <h2>Governed Parallel World Prediction</h2>
            <p>The desktop backend is required to create persistent, auditable world simulations.</p>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="world-sim-page">
      <header className="world-sim-hero">
        <div>
          <p className="world-sim-kicker">NEXUS WORLD ENGINE</p>
          <h2>Governed Parallel World Prediction</h2>
          <p>
            Build a world from live seed material, watch personas move in real time, inject
            counterfactuals, and review the final forecast with an auditable trail.
          </p>
        </div>
        <div className="world-sim-hero-stats">
          <span>Worlds {summaries.length}</span>
          <span>Live tick {liveTick?.tick ?? simulation?.tick_count ?? "--"}</span>
          <span>Fuel {simulation?.fuel_consumed.toFixed(1) ?? "0.0"}</span>
        </div>
      </header>

      {error ? <div className="world-sim-error">{error}</div> : null}

      <nav className="world-sim-tabs" aria-label="Simulation tabs">
        {TABS.map((tab) => {
          const disabled =
            (tab.id === "live" && !simulation) ||
            (tab.id === "results" && !report) ||
            (tab.id === "chat" && !(report && simulation?.personas.length));
          return (
            <button
              key={tab.id}
              type="button"
              className={tab.id === activeTab ? "is-active" : ""}
              disabled={disabled}
              onClick={() => setActiveTab(tab.id)}
            >
              {tab.label}
            </button>
          );
        })}
      </nav>

      {activeTab === "create" ? (
        <div className="world-sim-tab-content">
          <div className="world-sim-panel world-sim-create-grid">
            <div className="world-sim-create-main">
              <div className="world-sim-section-head">
                <div>
                  <h3>Create World</h3>
                  <p>Paste a scenario, article, memo, or draft and turn it into a governed simulation.</p>
                </div>
              </div>

              <div className="world-sim-seed-buttons">
                {SEED_PRESETS.map((seed) => (
                  <button key={seed.id} type="button" onClick={() => pickSeed(seed)}>
                    {seed.label}
                  </button>
                ))}
              </div>

              <label className="world-sim-field">
                <span>World Name</span>
                <input
                  value={worldName}
                  onChange={(event) => setWorldName(event.target.value)}
                  placeholder="Name this simulation"
                />
              </label>

              <label className="world-sim-field">
                <span>Paste seed material (news article, policy draft, financial report, story)</span>
                <textarea
                  className="world-sim-seed"
                  value={seedText}
                  onChange={(event) => setSeedText(event.target.value)}
                  placeholder="Paste the raw seed material here..."
                />
              </label>

              <div className="world-sim-slider-grid">
                <label className="world-sim-range">
                  <span>Persona Count</span>
                  <strong>{personaCount}</strong>
                  <input
                    type="range"
                    min={10}
                    max={200}
                    step={1}
                    value={personaCount}
                    onChange={(event) => setPersonaCount(Number(event.target.value))}
                  />
                </label>

                <label className="world-sim-range">
                  <span>Max Ticks</span>
                  <strong>{maxTicks}</strong>
                  <input
                    type="range"
                    min={10}
                    max={200}
                    step={1}
                    value={maxTicks}
                    onChange={(event) => setMaxTicks(Number(event.target.value))}
                  />
                </label>

                <label className="world-sim-range">
                  <span>Tick Speed</span>
                  <strong>{tickSpeed}ms</strong>
                  <input
                    type="range"
                    min={500}
                    max={5000}
                    step={100}
                    value={tickSpeed}
                    onChange={(event) => setTickSpeed(Number(event.target.value))}
                  />
                </label>
              </div>

              <div className="world-sim-actions">
                <button type="button" onClick={() => void handleCreateAndRun()} disabled={busy !== null}>
                  {busy === "create" ? "Creating..." : "Create & Run Simulation"}
                </button>
              </div>
            </div>

            <aside className="world-sim-create-side">
              <div className="world-sim-mini-card">
                <span>Default Runtime</span>
                <strong>{tickSpeed}ms per tick</strong>
              </div>
              <div className="world-sim-mini-card">
                <span>Projected Fuel</span>
                <strong>{simulation?.estimated_fuel ?? Math.round(personaCount * maxTicks * 4.6)}</strong>
              </div>
              <div className="world-sim-mini-card">
                <span>Governance</span>
                <strong>{personaCount > 100 ? "HITL required" : "Auto-run eligible"}</strong>
              </div>
            </aside>
          </div>

          <div className="world-sim-panel">
            <div className="world-sim-section-head">
              <div>
                <h3>Previous Simulations</h3>
                <p>Reload a completed world or resume one that is still active.</p>
              </div>
            </div>

            {summaries.length === 0 ? (
              <p className="world-sim-empty">
                No simulations saved yet. Create one above or start with one of the built-in seeds.
              </p>
            ) : (
              <div className="world-sim-history-grid">
                {summaries.map((summary) => (
                  <button
                    key={summary.id}
                    type="button"
                    className={`world-sim-history-card ${summary.id === worldId ? "is-selected" : ""}`}
                    onClick={() => selectExistingSimulation(summary)}
                  >
                    <div className="world-sim-history-head">
                      <strong>{summary.name}</strong>
                      <span className={`world-sim-badge status-${summary.status}`}>{formatStatus(summary.status)}</span>
                    </div>
                    <p>{summary.prediction_summary ?? "Prediction report not available yet."}</p>
                    <div className="world-sim-history-meta">
                      <span>{formatDate(summary.created_at)}</span>
                      <span>{summary.persona_count} personas</span>
                      <span>{summary.tick_count} ticks</span>
                    </div>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      ) : null}

      {activeTab === "live" ? (
        <div className="world-sim-tab-content">
          {!simulation ? (
            <div className="world-sim-panel">
              <p className="world-sim-empty">Create or select a simulation to stream live activity.</p>
            </div>
          ) : (
            <>
              <div className="world-sim-panel world-sim-live-topbar">
                <div>
                  <p className="world-sim-kicker">LIVE WORLD</p>
                  <h3>{simulation.name}</h3>
                </div>
                <div className="world-sim-live-stats">
                  <div>
                    <span>Tick</span>
                    <strong>
                      {simulation.tick_count}/{simulation.max_ticks}
                    </strong>
                  </div>
                  <div>
                    <span>Personas</span>
                    <strong>{simulation.persona_count}</strong>
                  </div>
                  <div>
                    <span>Status</span>
                    <strong className={`world-sim-badge status-${simulation.status}`}>
                      {formatStatus(simulation.status)}
                    </strong>
                  </div>
                  <div>
                    <span>Speed</span>
                    <strong>{simulation.tick_interval_ms}ms</strong>
                  </div>
                </div>
                <div className="world-sim-actions">
                  <button
                    type="button"
                    onClick={() => void handleStart()}
                    disabled={!worldId || busy !== null}
                  >
                    {busy === "start" ? "Starting..." : "Start"}
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => void handlePause()}
                    disabled={!worldId || busy !== null}
                  >
                    {busy === "pause" ? "Pausing..." : "Pause"}
                  </button>
                </div>
              </div>

              <div className="world-sim-live-grid">
                <div className="world-sim-panel">
                  <div className="world-sim-section-head">
                    <div>
                      <h3>Persona Map</h3>
                      <p>Dot color tracks dominant belief, size tracks influence, and line thickness tracks relationship strength.</p>
                    </div>
                  </div>
                  <canvas ref={canvasRef} className="world-sim-canvas" width={920} height={540} />
                  <div className="world-sim-belief-strip">
                    {Object.entries(liveTick?.belief_summary ?? {})
                      .slice(0, 6)
                      .map(([topic, value]) => (
                        <span key={topic}>
                          {topic}: {value.toFixed(2)}
                        </span>
                      ))}
                  </div>
                </div>

                <div className="world-sim-panel">
                  <div className="world-sim-section-head">
                    <div>
                      <h3>Live Event Feed</h3>
                      <p>Events stream from the runtime on each simulation tick.</p>
                    </div>
                    <span>{eventFeed.length} events</span>
                  </div>
                  <div ref={eventFeedRef} className="world-sim-event-feed">
                    {eventFeed.length === 0 ? (
                      <p className="world-sim-empty">The feed will populate as personas act.</p>
                    ) : (
                      eventFeed.map((event, index) => (
                        <article
                          key={`${event.actor_id}-${event.action_type}-${index}`}
                          className={`world-sim-event-card action-${event.action_type}`}
                        >
                          <strong>{event.action_type}</strong>
                          <p>{describeLiveEvent(event)}</p>
                          <span>Impact {event.impact.toFixed(2)}</span>
                        </article>
                      ))
                    )}
                  </div>
                </div>
              </div>

              <div className="world-sim-panel">
                <div className="world-sim-section-head">
                  <div>
                    <h3>Variable Injection</h3>
                    <p>Change the world state and watch the ripple effects on the next tick.</p>
                  </div>
                </div>
                <div className="world-sim-inject-bar">
                  <input
                    value={injectKey}
                    onChange={(event) => setInjectKey(event.target.value)}
                    placeholder="Inject variable"
                  />
                  <input
                    value={injectValue}
                    onChange={(event) => setInjectValue(event.target.value)}
                    placeholder="Value"
                  />
                  <button
                    type="button"
                    onClick={() => void handleInject()}
                    disabled={!worldId || busy !== null}
                  >
                    {busy === "inject" ? "Injecting..." : "Inject"}
                  </button>
                </div>
                <div className="world-sim-tag-row">
                  {Object.entries(simulation.variables).length === 0 ? (
                    <p className="world-sim-empty">No injected variables yet.</p>
                  ) : (
                    Object.entries(simulation.variables).map(([key, value]) => (
                      <span key={key}>
                        {key}: {value}
                      </span>
                    ))
                  )}
                </div>
              </div>
            </>
          )}
        </div>
      ) : null}

      {activeTab === "results" ? (
        <div className="world-sim-tab-content">
          {!report || !simulation ? (
            <div className="world-sim-panel">
              <p className="world-sim-empty">Complete a simulation to generate the prediction report.</p>
            </div>
          ) : (
            <>
              <div className="world-sim-panel">
                <div className="world-sim-section-head">
                  <div>
                    <h3>Summary</h3>
                    <p>High-level interpretation of the completed world timeline.</p>
                  </div>
                  <div className="world-sim-actions">
                    <button type="button" onClick={() => void handleParallelRun()} disabled={busy !== null}>
                      {busy === "parallel" ? "Running..." : "Run Parallel Simulations"}
                    </button>
                    <button type="button" className="secondary" onClick={handleExportReport}>
                      Export Report
                    </button>
                  </div>
                </div>
                <p className="world-sim-summary">{report.summary}</p>
              </div>

              <div className="world-sim-results-grid">
                <div className="world-sim-panel">
                  <h3>Key Findings</h3>
                  <ol className="world-sim-numbered-list">
                    {report.key_findings.map((finding) => (
                      <li key={finding.title}>
                        <strong>{finding.title}</strong>
                        <p>{finding.detail}</p>
                        <span>{Math.round(finding.confidence * 100)}% confidence</span>
                      </li>
                    ))}
                  </ol>
                </div>

                <div className="world-sim-panel">
                  <h3>Final Prediction</h3>
                  <div className="world-sim-prediction-box">
                    <strong>{report.prediction}</strong>
                    <span>{Math.round(report.confidence * 100)}% confidence</span>
                  </div>
                  <label className="world-sim-range">
                    <span>Parallel Variants</span>
                    <strong>{variantCount}</strong>
                    <input
                      type="range"
                      min={3}
                      max={10}
                      step={1}
                      value={variantCount}
                      onChange={(event) => setVariantCount(Number(event.target.value))}
                    />
                  </label>
                </div>
              </div>

              <div className="world-sim-panel">
                <h3>Opinion Shifts</h3>
                <div className="world-sim-shift-list">
                  {report.opinion_shifts.map((shift) => (
                    <article key={shift.topic} className="world-sim-shift-card">
                      <div className="world-sim-shift-head">
                        <strong>{shift.topic}</strong>
                        <span>{shift.delta >= 0 ? "+" : ""}{shift.delta.toFixed(2)}</span>
                      </div>
                      <div className="world-sim-shift-bars">
                        <div>
                          <label>Before</label>
                          <div className="world-sim-bar-track">
                            <div
                              className="world-sim-bar before"
                              style={{ width: `${Math.max(6, ((shift.before + 1) / 2) * 100)}%` }}
                            />
                          </div>
                        </div>
                        <div>
                          <label>After</label>
                          <div className="world-sim-bar-track">
                            <div
                              className="world-sim-bar after"
                              style={{ width: `${Math.max(6, ((shift.after + 1) / 2) * 100)}%` }}
                            />
                          </div>
                        </div>
                      </div>
                    </article>
                  ))}
                </div>
              </div>

              <div className="world-sim-results-grid">
                <div className="world-sim-panel">
                  <h3>Coalitions Detected</h3>
                  <div className="world-sim-coalition-list">
                    {report.coalitions.map((coalition) => (
                      <article key={coalition.name} className="world-sim-coalition-card">
                        <strong>{coalition.name}</strong>
                        <p>{coalition.members.join(", ")}</p>
                        <span>{coalition.focus_topics.join(", ")}</span>
                      </article>
                    ))}
                  </div>
                </div>

                <div className="world-sim-panel">
                  <h3>Turning Points</h3>
                  <div className="world-sim-turning-list">
                    {report.turning_points.map((turningPoint) => (
                      <article key={`${turningPoint.tick}-${turningPoint.description}`} className="world-sim-turning-card">
                        <strong>Tick {turningPoint.tick}</strong>
                        <p>{turningPoint.description}</p>
                        <span>Shift magnitude {turningPoint.shift_magnitude.toFixed(2)}</span>
                      </article>
                    ))}
                  </div>
                </div>
              </div>

              <div className="world-sim-results-grid">
                <div className="world-sim-panel">
                  <h3>Uncertainties</h3>
                  <ul className="world-sim-plain-list">
                    {report.uncertainties.map((uncertainty) => (
                      <li key={uncertainty}>{uncertainty}</li>
                    ))}
                  </ul>
                </div>

                <div className="world-sim-panel">
                  <h3>Parallel Results</h3>
                  {parallelReports.length === 0 ? (
                    <p className="world-sim-empty">
                      Run parallel variants to compare convergence and expose divergence factors.
                    </p>
                  ) : (
                    <div className="world-sim-parallel-list">
                      {parallelReports.map((entry, index) => (
                        <article key={`${entry.prediction}-${index}`}>
                          <strong>Variant {index + 1}</strong>
                          <p>{entry.prediction}</p>
                          <span>{Math.round(entry.confidence * 100)}% confidence</span>
                        </article>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            </>
          )}
        </div>
      ) : null}

      {activeTab === "chat" ? (
        <div className="world-sim-tab-content">
          {!report || !simulation ? (
            <div className="world-sim-panel">
              <p className="world-sim-empty">Complete a simulation to interview the personas.</p>
            </div>
          ) : (
            <>
              <div className="world-sim-panel">
                <div className="world-sim-section-head">
                  <div>
                    <h3>Persona Selection</h3>
                    <p>Choose any simulated actor and ask them to explain their final worldview.</p>
                  </div>
                </div>
                <select
                  className="world-sim-select"
                  value={selectedPersonaId}
                  onChange={(event) => {
                    setSelectedPersonaId(event.target.value);
                    setChatLines([]);
                  }}
                >
                  <option value="">Select a persona</option>
                  {simulation.personas.map((persona) => (
                    <option key={persona.id} value={persona.id}>
                      {persona.name} :: {persona.role}
                    </option>
                  ))}
                </select>
              </div>

              {activePersona ? (
                <div className="world-sim-chat-grid">
                  <div className="world-sim-panel">
                    <div className="world-sim-persona-card">
                      <h3>{activePersona.name}</h3>
                      <p>{activePersona.role}</p>
                      <div className="world-sim-trait-grid">
                        {Object.entries(activePersona.personality).map(([trait, value]) => (
                          <div key={trait}>
                            <span>{trait.replace(/_/g, " ")}</span>
                            <strong>{value.toFixed(2)}</strong>
                          </div>
                        ))}
                      </div>

                      <div className="world-sim-subsection">
                        <strong>Final Beliefs</strong>
                        <div className="world-sim-belief-list">
                          {topBeliefs.map(([topic, value]) => (
                            <span key={topic}>
                              {topic}: {value.toFixed(2)}
                            </span>
                          ))}
                        </div>
                      </div>

                      <div className="world-sim-subsection">
                        <strong>Key Memories</strong>
                        <div className="world-sim-memory-list">
                          {activePersona.memories.length === 0 ? (
                            <p className="world-sim-empty">No memorable events recorded.</p>
                          ) : (
                            activePersona.memories.map((memory) => (
                              <article key={`${memory.timestamp}-${memory.event}`}>
                                <strong>Tick {memory.timestamp}</strong>
                                <p>{memory.event}</p>
                              </article>
                            ))
                          )}
                        </div>
                      </div>
                    </div>
                  </div>

                  <div className="world-sim-panel">
                    <h3>Interview</h3>
                    <div className="world-sim-chat-log">
                      {chatLines.length === 0 ? (
                        <p className="world-sim-empty">
                          Ask how this persona interpreted the simulation or why they changed their beliefs.
                        </p>
                      ) : (
                        chatLines.map((line, index) => (
                          <div key={`${line.role}-${index}`} className={`world-sim-chat-line ${line.role}`}>
                            {line.content}
                          </div>
                        ))
                      )}
                    </div>
                    <div className="world-sim-chat-input">
                      <input
                        value={chatInput}
                        onChange={(event) => setChatInput(event.target.value)}
                        placeholder="Why did you make your last decision?"
                        onKeyDown={(event) => {
                          if (event.key === "Enter") {
                            event.preventDefault();
                            void handlePersonaChat();
                          }
                        }}
                      />
                      <button type="button" onClick={() => void handlePersonaChat()} disabled={!selectedPersonaId}>
                        Send
                      </button>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="world-sim-panel">
                  <p className="world-sim-empty">Select a persona to view their profile and start chatting.</p>
                </div>
              )}
            </>
          )}
        </div>
      ) : null}
    </section>
  );
}

import { useCallback, useEffect, useRef, useState } from "react";
import "./setup-wizard.css";
import type { AvailableModel, HardwareInfo, ModelPullProgress, OllamaStatus } from "../types";

type SetupStep = "welcome" | "ollama" | "models" | "agents" | "complete";
type DownloadState = "idle" | "downloading" | "installed" | "error";

interface SetupWizardProps {
  onDetectHardware: () => Promise<HardwareInfo>;
  onCheckOllama: (url?: string) => Promise<OllamaStatus>;
  onEnsureOllama: () => Promise<boolean>;
  onIsOllamaInstalled: () => Promise<boolean>;
  onPullModel: (model: string) => Promise<string>;
  onListAvailableModels: () => Promise<AvailableModel[]>;
  onSetAgentModel: (agent: string, model: string) => Promise<void>;
  onComplete: (hw: HardwareInfo, ollama: OllamaStatus) => void;
  onSkip: () => void;
}

function formatGB(bytes: number): string {
  return (bytes / 1_000_000_000).toFixed(1);
}

function formatSize(bytes: number): string {
  if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
  if (bytes >= 1_000_000) return `${(bytes / 1_000_000).toFixed(0)} MB`;
  return `${bytes} B`;
}

const AGENTS = [
  { id: "coder", name: "Coder Agent", icon: "\u2699" },
  { id: "designer", name: "Designer Agent", icon: "\uD83C\uDFA8" },
  { id: "screen_poster", name: "Screen Poster", icon: "\uD83D\uDCF1" },
  { id: "web_builder", name: "Web Builder", icon: "\uD83C\uDF10" },
  { id: "workflow", name: "Workflow Studio", icon: "\u26A1" },
  { id: "self_improve", name: "Self-Improve", icon: "\uD83E\uDDE0" },
];

export function SetupWizard({
  onDetectHardware,
  onCheckOllama,
  onEnsureOllama,
  onIsOllamaInstalled,
  onPullModel,
  onListAvailableModels,
  onSetAgentModel,
  onComplete,
  onSkip
}: SetupWizardProps): JSX.Element {
  const [step, setStep] = useState<SetupStep>("welcome");
  const [hardware, setHardware] = useState<HardwareInfo | null>(null);
  const [ollama, setOllama] = useState<OllamaStatus | null>(null);
  const [ollamaInstalled, setOllamaInstalled] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hwScanning, setHwScanning] = useState(true);
  const [ollamaChecking, setOllamaChecking] = useState(false);
  const [ollamaStarting, setOllamaStarting] = useState(false);

  // Model browser
  const [availableModels, setAvailableModels] = useState<AvailableModel[]>([]);
  const [downloadStates, setDownloadStates] = useState<Record<string, DownloadState>>({});
  const [downloadErrors, setDownloadErrors] = useState<Record<string, string>>({});
  const activeDownloadRef = useRef<string | null>(null);

  // Refs for progress bar DOM manipulation (keyed by model id)
  const barRefs = useRef<Record<string, HTMLElement | null>>({});
  const textRefs = useRef<Record<string, HTMLElement | null>>({});

  // Agent assignment
  const [agentModels, setAgentModels] = useState<Record<string, string>>({});

  // All installed model names (kept in sync)
  const [installedModels, setInstalledModels] = useState<Set<string>>(new Set());

  // Event listener for model-pull-progress
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;

    import("@tauri-apps/api/event").then(({ listen }) => {
      if (!mounted) return;
      listen<ModelPullProgress>("model-pull-progress", (event) => {
        const { model, status, percent, completed_bytes, total_bytes, error: pullError } = event.payload;

        const barEl = barRefs.current[model];
        const textEl = textRefs.current[model];

        if (status === "success") {
          setDownloadStates(prev => ({ ...prev, [model]: "installed" }));
          setInstalledModels(prev => new Set([...prev, model]));
          if (barEl) barEl.style.width = "100%";
          if (textEl) textEl.textContent = "Download complete";
          return;
        }

        if (status.includes("error") || pullError) {
          setDownloadStates(prev => ({ ...prev, [model]: "error" }));
          setDownloadErrors(prev => ({ ...prev, [model]: pullError || status }));
          return;
        }

        // Direct DOM update — no React re-render
        if (barEl) barEl.style.width = `${percent}%`;
        if (textEl) {
          if (total_bytes > 0) {
            textEl.textContent = `Downloading ${formatGB(completed_bytes)} / ${formatGB(total_bytes)} GB (${percent}%)`;
          } else {
            textEl.textContent = status || "Preparing download...";
          }
        }
      }).then((fn) => { unlisten = fn; });
    }).catch(() => {});

    return () => {
      mounted = false;
      unlisten?.();
    };
  }, []);

  // Step 1: Auto-detect hardware on mount
  const detectStep = useCallback(async () => {
    try {
      setError(null);
      setHwScanning(true);
      const hw = await onDetectHardware();
      setHardware(hw);
      setHwScanning(false);
    } catch (e) {
      setError(String(e));
      setHwScanning(false);
    }
  }, [onDetectHardware]);

  useEffect(() => {
    void detectStep();
  }, [detectStep]);

  async function goToOllamaStep(): Promise<void> {
    setStep("ollama");
    setOllamaChecking(true);
    setError(null);
    try {
      const installed = await onIsOllamaInstalled();
      setOllamaInstalled(installed);
      if (installed) {
        const status = await onCheckOllama();
        setOllama(status);
        if (status.connected) {
          setInstalledModels(new Set(status.models.map((m) => m.name)));
        }
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setOllamaChecking(false);
    }
  }

  async function handleEnsureOllama(): Promise<void> {
    setOllamaStarting(true);
    setError(null);
    try {
      await onEnsureOllama();
      const status = await onCheckOllama();
      setOllama(status);
      if (status.connected) {
        setInstalledModels(new Set(status.models.map((m) => m.name)));
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setOllamaStarting(false);
    }
  }

  async function handleRetryOllama(): Promise<void> {
    setOllamaChecking(true);
    setError(null);
    try {
      const status = await onCheckOllama();
      setOllama(status);
      if (status.connected) {
        setInstalledModels(new Set(status.models.map((m) => m.name)));
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setOllamaChecking(false);
    }
  }

  async function goToModelsStep(): Promise<void> {
    setStep("models");
    setError(null);
    try {
      const models = await onListAvailableModels();
      setAvailableModels(models);
      // Initialize download states from installed status
      const states: Record<string, DownloadState> = {};
      const names = new Set(installedModels);
      for (const m of models) {
        if (m.installed) {
          states[m.id] = "installed";
          names.add(m.id);
        }
      }
      setDownloadStates(states);
      setInstalledModels(names);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleDownload(modelId: string): Promise<void> {
    if (activeDownloadRef.current) return;
    activeDownloadRef.current = modelId;

    setDownloadStates(prev => ({ ...prev, [modelId]: "downloading" }));
    setError(null);

    try {
      await onPullModel(modelId);
      // Event listener handles marking "installed" on success.
      // Fallback for mock mode:
      setDownloadStates(prev =>
        prev[modelId] === "downloading" ? { ...prev, [modelId]: "installed" } : prev
      );
      setInstalledModels(prev => new Set([...prev, modelId]));
    } catch (err) {
      setDownloadStates(prev => ({ ...prev, [modelId]: "error" }));
      setDownloadErrors(prev => ({
        ...prev,
        [modelId]: err instanceof Error ? err.message : String(err)
      }));
    } finally {
      activeDownloadRef.current = null;
    }
  }

  async function handleDownloadRecommended(): Promise<void> {
    const recommended = availableModels.filter(m => m.recommended && !m.installed);
    for (const model of recommended) {
      if (downloadStates[model.id] !== "installed") {
        await handleDownload(model.id);
      }
    }
  }

  function goToAgentsStep(): void {
    // Pre-populate agent models with the best installed model
    const allInstalled = availableModels
      .filter(m => m.installed || installedModels.has(m.id))
      .map(m => m.id);

    // Find the best model (largest recommended installed, or first installed)
    const bestModel = hardware?.recommended_primary
      && installedModels.has(hardware.recommended_primary)
        ? hardware.recommended_primary
        : allInstalled[0] || "qwen3.5:4b";

    const fastModel = hardware?.recommended_fast
      && installedModels.has(hardware.recommended_fast)
        ? hardware.recommended_fast
        : bestModel;

    const defaults: Record<string, string> = {};
    for (const agent of AGENTS) {
      // Use fast model for lighter agents, best for heavy ones
      if (agent.id === "screen_poster" || agent.id === "workflow") {
        defaults[agent.id] = fastModel;
      } else {
        defaults[agent.id] = bestModel;
      }
    }
    setAgentModels(defaults);
    setStep("agents");
  }

  async function finishSetup(): Promise<void> {
    // Save agent model assignments
    try {
      for (const [agent, model] of Object.entries(agentModels)) {
        await onSetAgentModel(agent, model);
      }
    } catch {
      // Best effort — config will still be saved by onComplete
    }

    setStep("complete");
    if (hardware && ollama) {
      onComplete(hardware, ollama);
    } else if (hardware) {
      onComplete(hardware, {
        connected: false,
        base_url: "http://localhost:11434",
        models: []
      });
    }
  }

  const anyDownloading = Object.values(downloadStates).includes("downloading");

  // All installed model IDs for the agent dropdowns
  const installedModelList: string[] = [];
  for (const m of availableModels) {
    if (m.installed || installedModels.has(m.id)) {
      if (!installedModelList.includes(m.id)) installedModelList.push(m.id);
    }
  }
  // Also add any from Ollama that aren't in the catalog
  for (const name of installedModels) {
    if (!installedModelList.includes(name)) installedModelList.push(name);
  }

  const recommended = availableModels.filter(m => m.recommended);
  const allQwen = availableModels.filter(m => m.id.startsWith("qwen3.5:"));
  const otherInstalled = availableModels.filter(m => !m.id.startsWith("qwen3.5:") && m.installed);

  const stepNames: SetupStep[] = ["welcome", "ollama", "models", "agents", "complete"];

  return (
    <div className="setup-wizard-overlay">
      <div className="setup-wizard-card setup-wizard-card-wide">
        <header className="setup-wizard-header">
          <span className="setup-wizard-logo">&#x25C8;</span>
          <h1 className="setup-wizard-title">NEXUS OS</h1>
          <p className="setup-wizard-subtitle">Smart Setup Wizard</p>
        </header>

        <div className="setup-wizard-steps">
          {stepNames.map((s, i) => {
            const currentIdx = stepNames.indexOf(step);
            const cls = i === currentIdx ? "active" : i < currentIdx ? "done" : "";
            return (
              <span key={s}>
                {i > 0 && <span className="setup-step-line" />}
                <span className={`setup-step-dot ${cls}`} />
              </span>
            );
          })}
        </div>

        <div className="setup-wizard-body">
          {/* Step 1: Welcome + Hardware Scan */}
          {step === "welcome" && (
            <div className="setup-section">
              <h2 className="setup-section-title">Welcome to NexusOS</h2>
              <p className="setup-section-desc">
                {hwScanning
                  ? "Scanning your hardware \u2014 GPU, VRAM, and system memory..."
                  : "Hardware detected. Review your system profile below."}
              </p>

              {hwScanning && <div className="setup-spinner" />}

              {hardware && !hwScanning && (
                <>
                  <div className="setup-hw-grid">
                    <div className="setup-hw-item">
                      <span className="setup-hw-label">GPU</span>
                      <span className="setup-hw-value">
                        {hardware.gpu === "none" ? "No GPU detected" : hardware.gpu}
                      </span>
                    </div>
                    <div className="setup-hw-item">
                      <span className="setup-hw-label">VRAM</span>
                      <span className="setup-hw-value">
                        {hardware.vram_mb > 0 ? `${hardware.vram_mb} MB` : "N/A"}
                      </span>
                    </div>
                    <div className="setup-hw-item">
                      <span className="setup-hw-label">RAM</span>
                      <span className="setup-hw-value">{hardware.ram_mb} MB</span>
                    </div>
                    <div className="setup-hw-item">
                      <span className="setup-hw-label">Tier</span>
                      <span className="setup-hw-value">{hardware.tier}</span>
                    </div>
                  </div>
                  <div className="setup-actions">
                    <button
                      type="button"
                      className="setup-btn setup-btn-primary"
                      onClick={() => { void goToOllamaStep(); }}
                    >
                      Continue
                    </button>
                  </div>
                </>
              )}
            </div>
          )}

          {/* Step 2: Ollama Check / Install / Start */}
          {step === "ollama" && (
            <div className="setup-section">
              <h2 className="setup-section-title">Ollama Runtime</h2>
              <p className="setup-section-desc">
                NexusOS uses Ollama to run AI models locally on your machine.
              </p>

              {ollamaChecking && (
                <>
                  <div className="setup-spinner" />
                  <p className="setup-section-desc">Checking Ollama installation...</p>
                </>
              )}

              {!ollamaChecking && ollama?.connected && (
                <div className="setup-ollama-badge connected">
                  Ollama Connected \u2014 {ollama.models.length} model{ollama.models.length !== 1 ? "s" : ""} installed
                </div>
              )}

              {!ollamaChecking && !ollama?.connected && ollamaInstalled && (
                <div className="setup-ollama-section">
                  <div className="setup-ollama-badge disconnected">
                    Ollama is installed but not running
                  </div>
                  <button
                    type="button"
                    className="setup-btn setup-btn-download"
                    onClick={() => { void handleEnsureOllama(); }}
                    disabled={ollamaStarting}
                  >
                    {ollamaStarting ? "Starting Ollama..." : "Start Ollama Server"}
                  </button>
                </div>
              )}

              {!ollamaChecking && !ollama?.connected && ollamaInstalled === false && (
                <div className="setup-ollama-section">
                  <div className="setup-ollama-badge disconnected">
                    Ollama is not installed
                  </div>
                  <p className="setup-section-desc">
                    Install Ollama to run AI models locally. It&apos;s free and open source.
                  </p>
                  <div className="setup-actions" style={{ justifyContent: "flex-start", gap: "0.5rem" }}>
                    <a
                      href="https://ollama.ai"
                      target="_blank"
                      rel="noreferrer"
                      className="setup-btn setup-btn-download"
                    >
                      Download Ollama
                    </a>
                    <button
                      type="button"
                      className="setup-btn setup-btn-ghost"
                      onClick={() => { void handleRetryOllama(); }}
                    >
                      Retry Connection
                    </button>
                  </div>
                </div>
              )}

              {!ollamaChecking && (
                <div className="setup-actions">
                  <button
                    type="button"
                    className="setup-btn setup-btn-ghost"
                    onClick={() => setStep("welcome")}
                  >
                    Back
                  </button>
                  <button
                    type="button"
                    className="setup-btn setup-btn-primary"
                    onClick={() => { void goToModelsStep(); }}
                  >
                    {ollama?.connected ? "Continue" : "Skip \u2014 Continue Without Ollama"}
                  </button>
                </div>
              )}
            </div>
          )}

          {/* Step 3: Model Browser */}
          {step === "models" && hardware && (
            <div className="setup-section">
              <div className="setup-system-bar">
                <span>
                  {hardware.gpu !== "none" ? hardware.gpu : "CPU"}{" "}
                  {hardware.vram_mb > 0 && `(${Math.round(hardware.vram_mb / 1024)}GB VRAM)`}{" "}
                  &middot; {Math.round(hardware.ram_mb / 1024)}GB RAM &middot;{" "}
                  {ollama?.connected ? "Ollama Connected" : "Ollama Offline"}
                </span>
              </div>

              {/* Recommended section */}
              {recommended.length > 0 && (
                <>
                  <h3 className="setup-sub-title">RECOMMENDED FOR YOU</h3>
                  <div className="setup-model-cards">
                    {recommended.map(m => renderModelCard(m, true))}
                  </div>
                  {recommended.some(m => !m.installed && !installedModels.has(m.id)) && (
                    <button
                      type="button"
                      className="setup-btn setup-btn-download"
                      style={{ width: "100%", marginBottom: "1rem" }}
                      disabled={anyDownloading}
                      onClick={() => { void handleDownloadRecommended(); }}
                    >
                      Download All Recommended
                    </button>
                  )}
                </>
              )}

              {/* All Qwen 3.5 models */}
              <h3 className="setup-sub-title">ALL QWEN 3.5 MODELS</h3>
              <div className="setup-model-list">
                {allQwen.map(m => (
                  <div key={m.id} className="setup-model-row">
                    <span className="setup-model-row-name">{m.id}</span>
                    <span className="setup-model-row-size">{m.size_gb} GB</span>
                    <span className="setup-model-row-tag">{m.tag}</span>
                    <span className="setup-model-row-action">
                      {renderRowAction(m)}
                    </span>
                  </div>
                ))}
              </div>

              {/* Already installed non-Qwen models */}
              {otherInstalled.length > 0 && (
                <>
                  <h3 className="setup-sub-title">YOUR INSTALLED MODELS</h3>
                  <div className="setup-model-list">
                    {otherInstalled.map(m => (
                      <div key={m.id} className="setup-model-row">
                        <span className="setup-model-row-name">{m.id}</span>
                        <span className="setup-model-row-size">{m.size_gb > 0 ? `${m.size_gb} GB` : ""}</span>
                        <span className="setup-model-row-tag">Available</span>
                        <span className="setup-model-row-action">
                          <span className="setup-check">{"\u2713"}</span>
                        </span>
                      </div>
                    ))}
                  </div>
                </>
              )}

              <div className="setup-actions">
                <button
                  type="button"
                  className="setup-btn setup-btn-ghost"
                  onClick={() => setStep("ollama")}
                >
                  Back
                </button>
                <button
                  type="button"
                  className="setup-btn setup-btn-primary"
                  disabled={anyDownloading}
                  onClick={goToAgentsStep}
                >
                  {anyDownloading ? "Downloading..." : "Continue to Agent Setup"}
                </button>
              </div>
            </div>
          )}

          {/* Step 4: Agent Assignment */}
          {step === "agents" && (
            <div className="setup-section">
              <h2 className="setup-section-title">Configure Your Agents</h2>
              <p className="setup-section-desc">
                Assign an AI model to each agent. You can change these anytime in Settings.
              </p>

              <div className="setup-agent-list">
                {AGENTS.map(agent => (
                  <div key={agent.id} className="setup-agent-row">
                    <span className="setup-agent-icon">{agent.icon}</span>
                    <span className="setup-agent-name">{agent.name}</span>
                    <select
                      className="setup-agent-select"
                      value={agentModels[agent.id] || ""}
                      onChange={(e) => setAgentModels(prev => ({ ...prev, [agent.id]: e.target.value }))}
                    >
                      {installedModelList.map(modelId => (
                        <option key={modelId} value={modelId}>{modelId}</option>
                      ))}
                    </select>
                    <span className="setup-check">{"\u2713"}</span>
                  </div>
                ))}
              </div>

              {installedModelList.length === 0 && (
                <p className="setup-section-desc" style={{ color: "#fcd34d" }}>
                  No models installed. Go back and download at least one model.
                </p>
              )}

              <div className="setup-actions">
                <button
                  type="button"
                  className="setup-btn setup-btn-ghost"
                  onClick={() => { void goToModelsStep(); }}
                >
                  Back
                </button>
                <button
                  type="button"
                  className="setup-btn setup-btn-primary"
                  disabled={installedModelList.length === 0}
                  onClick={() => { void finishSetup(); }}
                >
                  Finish Setup
                </button>
              </div>
            </div>
          )}

          {/* Step 5: Complete */}
          {step === "complete" && (
            <div className="setup-section setup-complete">
              <span className="setup-complete-icon">&#x2713;</span>
              <h2 className="setup-section-title">Setup Complete</h2>
              <p className="setup-section-desc">Your NexusOS is configured and ready to use.</p>
              {hardware && (
                <p className="setup-section-desc">
                  Default model: <strong>{hardware.recommended_primary}</strong>
                </p>
              )}
            </div>
          )}

          {error && (
            <div className="setup-error">
              {error}
              <button
                type="button"
                className="setup-error-dismiss"
                onClick={() => setError(null)}
              >
                Dismiss
              </button>
            </div>
          )}
        </div>

        <footer className="setup-wizard-footer">
          <button type="button" className="setup-btn setup-btn-ghost" onClick={onSkip}>
            Skip Setup
          </button>
        </footer>
      </div>
    </div>
  );

  function renderModelCard(m: AvailableModel, showFull: boolean): JSX.Element {
    const dlState = downloadStates[m.id] || (m.installed || installedModels.has(m.id) ? "installed" : "idle");
    const isInstalled = dlState === "installed" || installedModels.has(m.id);

    return (
      <div key={m.id} className={`setup-model-card ${m.recommended ? "recommended" : ""}`}>
        <div className="setup-model-card-header">
          <span className={`setup-model-card-badge ${m.recommended ? "primary" : "fast"}`}>
            {m.recommended ? "\u2605" : ""} {m.tag}
          </span>
          <span className="setup-model-card-size">{m.size_gb} GB</span>
        </div>
        <h3 className="setup-model-card-name">{m.id}</h3>
        {showFull && <p className="setup-model-card-desc">{m.description}</p>}
        {showFull && (
          <div className="setup-model-caps">
            {m.capabilities.map(c => (
              <span key={c} className="setup-cap-badge">{c}</span>
            ))}
            <span className="setup-cap-badge">{m.context} ctx</span>
          </div>
        )}

        {isInstalled ? (
          <span className="setup-model-installed">{"\u2713"} Installed</span>
        ) : dlState === "downloading" ? (
          <div className="setup-model-progress">
            <div className="setup-download-bar">
              <div
                ref={(el) => { barRefs.current[m.id] = el; }}
                className="setup-download-fill"
                style={{ width: "0%", animation: "none" }}
              />
            </div>
            <span
              ref={(el) => { textRefs.current[m.id] = el; }}
              className="setup-download-label"
            >Starting download...</span>
          </div>
        ) : dlState === "error" ? (
          <div className="setup-model-error">
            <span className="setup-model-error-text">{downloadErrors[m.id] || "Download failed"}</span>
            <button
              type="button"
              className="setup-btn setup-btn-download"
              onClick={() => { void handleDownload(m.id); }}
            >
              Retry
            </button>
          </div>
        ) : ollama?.connected ? (
          <button
            type="button"
            className="setup-btn setup-btn-download"
            disabled={anyDownloading}
            onClick={() => { void handleDownload(m.id); }}
          >
            Download ({m.size_gb} GB)
          </button>
        ) : (
          <span className="setup-model-unavailable">Requires Ollama</span>
        )}
      </div>
    );
  }

  function renderRowAction(m: AvailableModel): JSX.Element {
    const dlState = downloadStates[m.id] || (m.installed || installedModels.has(m.id) ? "installed" : "idle");
    const isInstalled = dlState === "installed" || installedModels.has(m.id);

    if (isInstalled) return <span className="setup-check">{"\u2713"}</span>;
    if (dlState === "downloading") {
      return (
        <span className="setup-model-row-dl">
          <span
            ref={(el) => { barRefs.current[m.id] = el; }}
            className="setup-row-bar"
            style={{ width: "0%" }}
          />
          <span
            ref={(el) => { textRefs.current[m.id] = el; }}
            className="setup-row-pct"
          >0%</span>
        </span>
      );
    }
    if (dlState === "error") {
      return (
        <button
          type="button"
          className="setup-btn-sm setup-btn-download"
          onClick={() => { void handleDownload(m.id); }}
        >
          Retry
        </button>
      );
    }
    if (ollama?.connected) {
      return (
        <button
          type="button"
          className="setup-btn-sm setup-btn-download"
          disabled={anyDownloading}
          onClick={() => { void handleDownload(m.id); }}
        >
          Download
        </button>
      );
    }
    return <span className="setup-model-unavailable">\u2014</span>;
  }
}

import { useCallback, useEffect, useRef, useState } from "react";
import "./setup-wizard.css";
import type { HardwareInfo, ModelPullProgress, OllamaStatus } from "../types";

type SetupStep = "welcome" | "ollama" | "models" | "complete";
type DownloadState = "idle" | "downloading" | "installed" | "error";

interface SetupWizardProps {
  onDetectHardware: () => Promise<HardwareInfo>;
  onCheckOllama: (url?: string) => Promise<OllamaStatus>;
  onEnsureOllama: () => Promise<boolean>;
  onIsOllamaInstalled: () => Promise<boolean>;
  onPullModel: (model: string) => Promise<string>;
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

export function SetupWizard({
  onDetectHardware,
  onCheckOllama,
  onEnsureOllama,
  onIsOllamaInstalled,
  onPullModel,
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

  // Download state: only setState for start/finish/error — never for progress ticks
  const [primaryState, setPrimaryState] = useState<DownloadState>("idle");
  const [fastState, setFastState] = useState<DownloadState>("idle");
  const [downloadErrors, setDownloadErrors] = useState<Record<string, string>>({});

  // Refs for direct DOM manipulation — zero re-renders during download
  const primaryBarRef = useRef<HTMLDivElement | null>(null);
  const primaryTextRef = useRef<HTMLSpanElement | null>(null);
  const fastBarRef = useRef<HTMLDivElement | null>(null);
  const fastTextRef = useRef<HTMLSpanElement | null>(null);

  // Track which model is currently downloading (ref, not state)
  const activeDownloadRef = useRef<string | null>(null);

  // Pre-installed models from Ollama
  const [installedModels, setInstalledModels] = useState<Set<string>>(new Set());

  // Event listener for model-pull-progress — updates DOM directly via refs
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;

    import("@tauri-apps/api/event").then(({ listen }) => {
      if (!mounted) return;
      listen<ModelPullProgress>("model-pull-progress", (event) => {
        const { model, status, percent, completed_bytes, total_bytes, error: pullError } = event.payload;

        // Find the right refs for this model
        const isPrimary = hardware?.recommended_primary === model;
        const barEl = isPrimary ? primaryBarRef.current : fastBarRef.current;
        const textEl = isPrimary ? primaryTextRef.current : fastTextRef.current;
        const setModelState = isPrimary ? setPrimaryState : setFastState;

        if (status === "success") {
          // setState ONCE on completion
          setModelState("installed");
          setInstalledModels(prev => new Set([...prev, model]));
          if (barEl) barEl.style.width = "100%";
          if (textEl) textEl.textContent = "Download complete";
          return;
        }

        if (status.includes("error") || pullError) {
          setModelState("error");
          setDownloadErrors(prev => ({ ...prev, [model]: pullError || status }));
          return;
        }

        // DIRECT DOM UPDATE — no React re-render
        if (barEl) {
          barEl.style.width = `${percent}%`;
        }
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
  }, [hardware]);

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

  // Auto-advance when both models are installed
  useEffect(() => {
    if (!hardware || step !== "models") return;
    const primaryDone = primaryState === "installed" || installedModels.has(hardware.recommended_primary);
    const fastDone = fastState === "installed" || installedModels.has(hardware.recommended_fast);
    if (primaryDone && fastDone && activeDownloadRef.current === null) {
      // Brief pause then auto-advance
      const timer = setTimeout(() => {
        setStep("complete");
        finishSetup();
      }, 1200);
      return () => clearTimeout(timer);
    }
  }, [primaryState, fastState, installedModels, step, hardware]);

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

  async function handleDownload(modelId: string): Promise<void> {
    if (activeDownloadRef.current) return;
    activeDownloadRef.current = modelId;

    const isPrimary = hardware?.recommended_primary === modelId;
    const setModelState = isPrimary ? setPrimaryState : setFastState;

    // setState ONCE to show the progress bar
    setModelState("downloading");
    setError(null);

    try {
      await onPullModel(modelId);
      // The event listener handles marking "installed" on success.
      // But if the event didn't fire (mock mode), mark it here too.
      setModelState(prev => prev === "downloading" ? "installed" : prev);
      setInstalledModels(prev => new Set([...prev, modelId]));
    } catch (err) {
      setModelState("error");
      setDownloadErrors(prev => ({
        ...prev,
        [modelId]: err instanceof Error ? err.message : String(err)
      }));
    } finally {
      activeDownloadRef.current = null;
    }
  }

  async function handleDownloadAll(): Promise<void> {
    if (!hardware || !ollama?.connected) return;
    const toDownload = [hardware.recommended_primary, hardware.recommended_fast].filter(
      m => !installedModels.has(m) && primaryState !== "installed" && fastState !== "installed"
    );
    for (const model of toDownload) {
      await handleDownload(model);
    }
  }

  function finishSetup(): void {
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

  const anyDownloading = primaryState === "downloading" || fastState === "downloading";

  function renderModelCard(
    modelId: string,
    desc: string,
    badgeClass: string,
    badgeLabel: string,
    barRef: React.MutableRefObject<HTMLDivElement | null>,
    textRef: React.MutableRefObject<HTMLSpanElement | null>,
    dlState: DownloadState
  ): JSX.Element {
    const isInstalled = dlState === "installed" || installedModels.has(modelId);

    return (
      <div className="setup-model-card">
        <div className="setup-model-card-header">
          <span className={`setup-model-card-badge ${badgeClass}`}>{badgeLabel}</span>
          <h3 className="setup-model-card-name">{modelId}</h3>
        </div>
        <p className="setup-model-card-desc">{desc}</p>

        {isInstalled ? (
          <span className="setup-model-installed">&#x2713; Installed</span>
        ) : dlState === "downloading" ? (
          <div className="setup-model-progress">
            <div className="setup-download-bar">
              <div ref={(el) => { barRef.current = el; }} className="setup-download-fill" style={{ width: "0%", animation: "none" }} />
            </div>
            <span ref={(el) => { textRef.current = el; }} className="setup-download-label">Starting download...</span>
          </div>
        ) : dlState === "error" ? (
          <div className="setup-model-error">
            <span className="setup-model-error-text">{downloadErrors[modelId] || "Download failed"}</span>
            <button
              type="button"
              className="setup-btn setup-btn-download"
              onClick={() => { void handleDownload(modelId); }}
            >
              Retry
            </button>
          </div>
        ) : ollama?.connected ? (
          <button
            type="button"
            className="setup-btn setup-btn-download"
            disabled={anyDownloading}
            onClick={() => { void handleDownload(modelId); }}
          >
            Download
          </button>
        ) : (
          <span className="setup-model-unavailable">Requires Ollama</span>
        )}
      </div>
    );
  }

  return (
    <div className="setup-wizard-overlay">
      <div className="setup-wizard-card">
        <header className="setup-wizard-header">
          <span className="setup-wizard-logo">&#x25C8;</span>
          <h1 className="setup-wizard-title">NEXUS OS</h1>
          <p className="setup-wizard-subtitle">Smart Setup Wizard</p>
        </header>

        <div className="setup-wizard-steps">
          <span className={`setup-step-dot ${step === "welcome" ? "active" : hardware ? "done" : ""}`} />
          <span className="setup-step-line" />
          <span className={`setup-step-dot ${step === "ollama" ? "active" : ollama?.connected ? "done" : ""}`} />
          <span className="setup-step-line" />
          <span className={`setup-step-dot ${step === "models" ? "active" : step === "complete" ? "done" : ""}`} />
          <span className="setup-step-line" />
          <span className={`setup-step-dot ${step === "complete" ? "active" : ""}`} />
        </div>

        <div className="setup-wizard-body">
          {/* Step 1: Welcome + Hardware Scan */}
          {step === "welcome" && (
            <div className="setup-section">
              <h2 className="setup-section-title">Welcome to NexusOS</h2>
              <p className="setup-section-desc">
                {hwScanning
                  ? "Scanning your hardware — GPU, VRAM, and system memory..."
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
                  Ollama Connected — {ollama.models.length} model{ollama.models.length !== 1 ? "s" : ""} installed
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
                    onClick={() => setStep("models")}
                  >
                    {ollama?.connected ? "Continue" : "Skip — Continue Without Ollama"}
                  </button>
                </div>
              )}
            </div>
          )}

          {/* Step 3: Model Recommendations + Download */}
          {step === "models" && hardware && (
            <div className="setup-section">
              <h2 className="setup-section-title">AI Models</h2>
              <p className="setup-section-desc">
                Based on your {hardware.tier} hardware, we recommend these models:
              </p>

              <div className="setup-model-cards">
                {renderModelCard(
                  hardware.recommended_primary,
                  "Full-power model for coding, design, and complex tasks",
                  "primary",
                  "PRIMARY",
                  primaryBarRef,
                  primaryTextRef,
                  primaryState
                )}
                {renderModelCard(
                  hardware.recommended_fast,
                  "Lightweight model for quick responses and background agents",
                  "fast",
                  "FAST",
                  fastBarRef,
                  fastTextRef,
                  fastState
                )}
              </div>

              {/* Download All button */}
              {ollama?.connected
                && !installedModels.has(hardware.recommended_primary)
                && !installedModels.has(hardware.recommended_fast)
                && primaryState === "idle"
                && fastState === "idle" && (
                <button
                  type="button"
                  className="setup-btn setup-btn-download"
                  style={{ width: "100%", marginBottom: "1rem" }}
                  onClick={() => { void handleDownloadAll(); }}
                >
                  Download All Recommended
                </button>
              )}

              {/* Installed models list */}
              {ollama?.connected && ollama.models.length > 0 && (
                <div className="setup-installed-models">
                  <h3 className="setup-sub-title">Installed Models</h3>
                  {ollama.models.map((m) => (
                    <div key={m.name} className="setup-installed-row">
                      <span className="setup-installed-name">{m.name}</span>
                      <span className="setup-installed-size">{formatSize(m.size)}</span>
                    </div>
                  ))}
                </div>
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
                  onClick={() => {
                    setStep("complete");
                    finishSetup();
                  }}
                >
                  {anyDownloading ? "Downloading..." : "Finish Setup"}
                </button>
              </div>
            </div>
          )}

          {/* Step 4: Complete */}
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
}

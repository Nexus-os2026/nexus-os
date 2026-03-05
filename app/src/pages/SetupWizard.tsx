import { useCallback, useEffect, useState } from "react";
import "./setup-wizard.css";
import type { HardwareInfo, OllamaStatus } from "../types";

type SetupStep = "detect" | "ollama" | "models" | "download" | "complete";

interface SetupWizardProps {
  onDetectHardware: () => Promise<HardwareInfo>;
  onCheckOllama: (url?: string) => Promise<OllamaStatus>;
  onPullModel: (model: string) => Promise<string>;
  onComplete: (hw: HardwareInfo, ollama: OllamaStatus) => void;
  onSkip: () => void;
}

export function SetupWizard({
  onDetectHardware,
  onCheckOllama,
  onPullModel,
  onComplete,
  onSkip
}: SetupWizardProps): JSX.Element {
  const [step, setStep] = useState<SetupStep>("detect");
  const [hardware, setHardware] = useState<HardwareInfo | null>(null);
  const [ollama, setOllama] = useState<OllamaStatus | null>(null);
  const [downloading, setDownloading] = useState<string | null>(null);
  const [downloadedModels, setDownloadedModels] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);

  const detectStep = useCallback(async () => {
    try {
      setError(null);
      const hw = await onDetectHardware();
      setHardware(hw);
      setStep("ollama");
    } catch (e) {
      setError(String(e));
    }
  }, [onDetectHardware]);

  useEffect(() => {
    void detectStep();
  }, [detectStep]);

  async function checkOllamaStep(): Promise<void> {
    try {
      setError(null);
      const status = await onCheckOllama();
      setOllama(status);
      if (status.connected) {
        const installed = new Set(status.models.map((m) => m.name));
        setDownloadedModels(installed);
        setStep("models");
      } else {
        setStep("models");
      }
    } catch (e) {
      setError(String(e));
      setStep("models");
    }
  }

  async function pullModel(name: string): Promise<void> {
    if (downloading) return;
    setDownloading(name);
    setError(null);
    try {
      await onPullModel(name);
      setDownloadedModels((prev) => new Set([...prev, name]));
    } catch (e) {
      setError(`Failed to download ${name}: ${String(e)}`);
    } finally {
      setDownloading(null);
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

  function formatSize(bytes: number): string {
    if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
    if (bytes >= 1_000_000) return `${(bytes / 1_000_000).toFixed(0)} MB`;
    return `${bytes} B`;
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
          <span className={`setup-step-dot ${step === "detect" ? "active" : hardware ? "done" : ""}`} />
          <span className="setup-step-line" />
          <span className={`setup-step-dot ${step === "ollama" ? "active" : ollama ? "done" : ""}`} />
          <span className="setup-step-line" />
          <span className={`setup-step-dot ${step === "models" || step === "download" ? "active" : step === "complete" ? "done" : ""}`} />
          <span className="setup-step-line" />
          <span className={`setup-step-dot ${step === "complete" ? "active" : ""}`} />
        </div>

        <div className="setup-wizard-body">
          {step === "detect" && (
            <div className="setup-section">
              <h2 className="setup-section-title">Detecting Hardware</h2>
              <p className="setup-section-desc">Scanning GPU, VRAM, and system memory...</p>
              <div className="setup-spinner" />
            </div>
          )}

          {step === "ollama" && hardware && (
            <div className="setup-section">
              <h2 className="setup-section-title">Hardware Detected</h2>
              <div className="setup-hw-grid">
                <div className="setup-hw-item">
                  <span className="setup-hw-label">GPU</span>
                  <span className="setup-hw-value">{hardware.gpu === "none" ? "No GPU detected" : hardware.gpu}</span>
                </div>
                <div className="setup-hw-item">
                  <span className="setup-hw-label">VRAM</span>
                  <span className="setup-hw-value">{hardware.vram_mb > 0 ? `${hardware.vram_mb} MB` : "N/A"}</span>
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

              <h3 className="setup-sub-title">Checking Ollama Connection...</h3>
              <button type="button" className="setup-btn setup-btn-primary" onClick={() => { void checkOllamaStep(); }}>
                Connect to Ollama
              </button>
            </div>
          )}

          {(step === "models" || step === "download") && hardware && (
            <div className="setup-section">
              <h2 className="setup-section-title">Model Setup</h2>

              {ollama?.connected ? (
                <div className="setup-ollama-badge connected">Ollama Connected</div>
              ) : (
                <div className="setup-ollama-badge disconnected">
                  Ollama Not Running — <a href="https://ollama.ai" target="_blank" rel="noreferrer">Install Ollama</a>
                </div>
              )}

              <div className="setup-model-cards">
                <div className="setup-model-card">
                  <div className="setup-model-card-header">
                    <span className="setup-model-card-badge primary">PRIMARY</span>
                    <h3 className="setup-model-card-name">{hardware.recommended_primary}</h3>
                  </div>
                  <p className="setup-model-card-desc">Full-power model for coding, design, and complex tasks</p>
                  {downloadedModels.has(hardware.recommended_primary) ? (
                    <span className="setup-model-installed">Installed</span>
                  ) : ollama?.connected ? (
                    <button
                      type="button"
                      className="setup-btn setup-btn-download"
                      disabled={downloading !== null}
                      onClick={() => { void pullModel(hardware.recommended_primary); }}
                    >
                      {downloading === hardware.recommended_primary ? "Downloading..." : "Download"}
                    </button>
                  ) : (
                    <span className="setup-model-unavailable">Requires Ollama</span>
                  )}
                </div>

                <div className="setup-model-card">
                  <div className="setup-model-card-header">
                    <span className="setup-model-card-badge fast">FAST</span>
                    <h3 className="setup-model-card-name">{hardware.recommended_fast}</h3>
                  </div>
                  <p className="setup-model-card-desc">Lightweight model for quick responses and background agents</p>
                  {downloadedModels.has(hardware.recommended_fast) ? (
                    <span className="setup-model-installed">Installed</span>
                  ) : ollama?.connected ? (
                    <button
                      type="button"
                      className="setup-btn setup-btn-download"
                      disabled={downloading !== null}
                      onClick={() => { void pullModel(hardware.recommended_fast); }}
                    >
                      {downloading === hardware.recommended_fast ? "Downloading..." : "Download"}
                    </button>
                  ) : (
                    <span className="setup-model-unavailable">Requires Ollama</span>
                  )}
                </div>
              </div>

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

              {downloading && (
                <div className="setup-download-progress">
                  <div className="setup-download-bar">
                    <div className="setup-download-fill" />
                  </div>
                  <span className="setup-download-label">Downloading {downloading}...</span>
                </div>
              )}

              <div className="setup-actions">
                <button type="button" className="setup-btn setup-btn-primary" onClick={finishSetup}>
                  {ollama?.connected ? "Continue" : "Skip & Continue"}
                </button>
              </div>
            </div>
          )}

          {step === "complete" && (
            <div className="setup-section setup-complete">
              <span className="setup-complete-icon">&#x2713;</span>
              <h2 className="setup-section-title">Setup Complete</h2>
              <p className="setup-section-desc">Your NEXUS OS is configured and ready.</p>
            </div>
          )}

          {error && (
            <div className="setup-error">{error}</div>
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

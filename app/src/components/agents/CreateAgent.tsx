import { useMemo, useState } from "react";

interface CapabilityOption {
  id: string;
  label: string;
  risk: "low" | "medium" | "high";
}

interface CreateAgentProps {
  open: boolean;
  onClose: () => void;
  onDeploy: (manifestJson: string) => void;
}

const CAPABILITIES: CapabilityOption[] = [
  { id: "web.search", label: "Web Search", risk: "low" },
  { id: "web.build", label: "Web Build", risk: "medium" },
  { id: "fs.read", label: "Filesystem Read", risk: "medium" },
  { id: "fs.write", label: "Filesystem Write", risk: "medium" },
  { id: "llm.query", label: "LLM Query", risk: "medium" },
  { id: "gpu.render", label: "GPU Render", risk: "medium" },
  { id: "vision.analyze", label: "Vision Analyze", risk: "medium" },
  { id: "process.exec", label: "Process Exec", risk: "high" },
  { id: "social.post", label: "Social Posting", risk: "high" },
  { id: "social.read", label: "Social Read", risk: "low" },
  { id: "messaging.send", label: "Messaging Send", risk: "high" },
  { id: "marketplace.publish", label: "Marketplace Publish", risk: "high" }
];

const CAPABILITY_KEYWORDS: { keywords: RegExp; capabilities: string[] }[] = [
  { keywords: /3d|three[\s.-]?js|r3f|react three fiber|webgl|render|scene|shader|glsl|camera\s*path|catmullrom/i, capabilities: ["gpu.render", "fs.read", "fs.write"] },
  { keywords: /web|react|html|css|site|page|frontend|component|builder/i, capabilities: ["web.build", "fs.read", "fs.write"] },
  { keywords: /search|browse|scrape|crawl|fetch/i, capabilities: ["web.search"] },
  { keywords: /llm|ai\b|generate|prompt|gpt|claude|model|query/i, capabilities: ["llm.query"] },
  { keywords: /post|social|tweet|instagram|share|publish/i, capabilities: ["social.post", "social.read"] },
  { keywords: /message|telegram|notify|alert|chat/i, capabilities: ["messaging.send"] },
  { keywords: /code|compile|test|build|debug|git|terminal/i, capabilities: ["process.exec", "fs.read", "fs.write"] },
  { keywords: /image|screenshot|vision|camera|capture/i, capabilities: ["vision.analyze"] },
  { keywords: /scroll|animation|gsap|orchestrat/i, capabilities: ["gpu.render", "fs.read", "fs.write"] },
  { keywords: /marketplace|plugin|modular|install/i, capabilities: ["marketplace.publish"] },
  { keywords: /upload|file|save|load|read|write/i, capabilities: ["fs.read", "fs.write"] },
];

function detectCapabilities(desc: string): Record<string, boolean> {
  const result: Record<string, boolean> = {};
  for (const cap of CAPABILITIES) {
    result[cap.id] = false;
  }
  for (const rule of CAPABILITY_KEYWORDS) {
    if (rule.keywords.test(desc)) {
      for (const cap of rule.capabilities) {
        result[cap] = true;
      }
    }
  }
  return result;
}

const MODEL_OPTIONS = [
  { value: "claude-sonnet-4-5", label: "claude-sonnet-4-5" },
  { value: "claude-haiku-4-5", label: "claude-haiku-4-5" },
  { value: "gpt-4o", label: "gpt-4o" },
  { value: "gpt-4o-mini", label: "gpt-4o-mini" },
  { value: "qwen3.5:9b", label: "qwen3.5:9b (Local)" },
  { value: "ollama", label: "ollama (Local)" },
  { value: "mock", label: "mock (Testing)" },
];

function riskIcon(risk: CapabilityOption["risk"]): string {
  if (risk === "low") {
    return "◉";
  }
  if (risk === "medium") {
    return "▲";
  }
  return "◆";
}

function inferName(input: string): string {
  const normalized = input
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return normalized.length > 0 ? normalized : "new-agent";
}

export function CreateAgent({ open, onClose, onDeploy }: CreateAgentProps): JSX.Element | null {
  const [step, setStep] = useState(1);
  const [description, setDescription] = useState("");
  const [name, setName] = useState("new-agent");
  const [fuelBudget, setFuelBudget] = useState(10_000);
  const [model, setModel] = useState("claude-sonnet-4-5");
  const [selectedCapabilities, setSelectedCapabilities] = useState<Record<string, boolean>>(() => {
    const init: Record<string, boolean> = {};
    for (const cap of CAPABILITIES) {
      init[cap.id] = false;
    }
    return init;
  });
  const [capsAutoDetected, setCapsAutoDetected] = useState(false);

  const chosenCapabilities = useMemo(
    () => CAPABILITIES.filter((capability) => selectedCapabilities[capability.id]).map((capability) => capability.id),
    [selectedCapabilities]
  );

  const fuelPct = Math.max(5, Math.min(100, Math.round((fuelBudget / 20_000) * 100)));

  function moveNext(): void {
    if (step === 1) {
      // Auto-detect capabilities from description when entering step 2
      const detected = detectCapabilities(description);
      const hasAny = Object.values(detected).some(Boolean);
      if (hasAny) {
        setSelectedCapabilities(detected);
        setCapsAutoDetected(true);
      } else {
        setCapsAutoDetected(false);
      }
    }
    setStep((current) => Math.min(4, current + 1));
  }

  function moveBack(): void {
    setStep((current) => Math.max(1, current - 1));
  }

  function closeDialog(): void {
    setStep(1);
    onClose();
  }

  function deploy(): void {
    const payload = {
      name: inferName(name.trim()),
      version: "2.0.0",
      capabilities: chosenCapabilities,
      fuel_budget: fuelBudget,
      schedule: null,
      llm_model: model.trim() || null,
      description: description.trim()
    };
    onDeploy(JSON.stringify(payload));
    setStep(1);
  }

  if (!open) {
    return null;
  }

  return (
    <div className="create-agent-overlay">
      <section className="create-agent-panel">
        <header className="create-agent-head">
          <h3 className="create-agent-title">CREATE AGENT // FACTORY</h3>
          <button type="button" className="create-agent-close" onClick={closeDialog} aria-label="Close create agent">
            ✕
          </button>
        </header>

        <div className="create-progress">
          <div className={`create-progress-item ${step === 1 ? "active" : ""}`}>1. Describe</div>
          <div className={`create-progress-item ${step === 2 ? "active" : ""}`}>2. Capabilities</div>
          <div className={`create-progress-item ${step === 3 ? "active" : ""}`}>3. Fuel</div>
          <div className={`create-progress-item ${step === 4 ? "active" : ""}`}>4. Deploy</div>
        </div>

        {step === 1 ? (
          <article className="create-step">
            <h4 className="create-step-title">Describe your agent</h4>
            <input
              className="create-input"
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="Agent name"
            />
            <textarea
              className="create-textarea"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              placeholder="Describe mission objectives, constraints, and expected outputs..."
            />
          </article>
        ) : null}

        {step === 2 ? (
          <article className="create-step">
            <h4 className="create-step-title">Review capabilities</h4>
            {capsAutoDetected && (
              <p className="create-autodetect-note">Auto-detected from your description. You can adjust below.</p>
            )}
            <div className="create-capability-list">
              {CAPABILITIES.map((capability) => (
                <div key={capability.id} className="create-capability">
                  <label>
                    <input
                      type="checkbox"
                      checked={selectedCapabilities[capability.id] ?? false}
                      onChange={(event) =>
                        setSelectedCapabilities((previous) => ({
                          ...previous,
                          [capability.id]: event.target.checked
                        }))
                      }
                    />
                    {capability.label}
                  </label>
                  <span className={`create-risk ${capability.risk}`}>
                    {riskIcon(capability.risk)} {capability.risk}
                  </span>
                </div>
              ))}
            </div>
          </article>
        ) : null}

        {step === 3 ? (
          <article className="create-step">
            <h4 className="create-step-title">Set fuel budget</h4>
            <input
              type="range"
              min={1000}
              max={20000}
              step={500}
              className="create-slider"
              value={fuelBudget}
              onChange={(event) => setFuelBudget(Number(event.target.value))}
            />
            <div className="create-gauge-preview">
              <span>Fuel Budget: {fuelBudget}</span>
              <div className="fuel-bar">
                <div className="fuel-bar__track">
                  <div className="fuel-bar__fill" style={{ width: `${fuelPct}%` }} />
                </div>
              </div>
            </div>
            <label className="create-model-label">LLM Model</label>
            <select
              className="create-input"
              value={model}
              onChange={(event) => setModel(event.target.value)}
            >
              {MODEL_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
          </article>
        ) : null}

        {step === 4 ? (
          <article className="create-step">
            <h4 className="create-step-title">Confirm & Deploy</h4>
            <p className="agent-card-last">Name: {inferName(name)}</p>
            <p className="agent-card-last">Capabilities: {chosenCapabilities.join(", ") || "none selected"}</p>
            <p className="agent-card-last">Fuel Budget: {fuelBudget}</p>
            <p className="agent-card-last">Model: {model}</p>
          </article>
        ) : null}

        <footer className="create-actions">
          <button type="button" className="create-btn" onClick={moveBack} disabled={step === 1}>
            Back
          </button>
          {step < 4 ? (
            <button type="button" className="create-btn" onClick={moveNext}>
              Next
            </button>
          ) : (
            <button type="button" className="create-btn deploy" onClick={deploy}>
              Confirm & Deploy
            </button>
          )}
        </footer>
      </section>
    </div>
  );
}

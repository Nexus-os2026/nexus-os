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
  { id: "fs.read", label: "Filesystem Read", risk: "medium" },
  { id: "llm.query", label: "LLM Query", risk: "medium" },
  { id: "social.post", label: "Social Posting", risk: "high" },
  { id: "messaging.send", label: "Messaging Send", risk: "high" }
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
  const [selectedCapabilities, setSelectedCapabilities] = useState<Record<string, boolean>>({
    "web.search": true,
    "llm.query": true,
    "fs.read": true,
    "social.post": false,
    "messaging.send": false
  });

  const chosenCapabilities = useMemo(
    () => CAPABILITIES.filter((capability) => selectedCapabilities[capability.id]).map((capability) => capability.id),
    [selectedCapabilities]
  );

  const fuelPct = Math.max(5, Math.min(100, Math.round((fuelBudget / 20_000) * 100)));

  function moveNext(): void {
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
      version: "0.1.0",
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
            <input
              className="create-input"
              value={model}
              onChange={(event) => setModel(event.target.value)}
              placeholder="LLM model"
            />
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

import { useCallback, useEffect, useRef, useState } from "react";
import { hasDesktopRuntime, marketplacePublish, marketplaceMyAgents } from "../api/backend";
import type { MarketplaceAgent, MarketplacePublishResult } from "../types";
import "./developer-portal.css";

type PublishState = "idle" | "uploading" | "verifying" | "done" | "error";

interface VerificationStep {
  name: string;
  label: string;
  status: "pending" | "running" | "passed" | "failed" | "warning";
  detail?: string;
}

const INITIAL_STEPS: VerificationStep[] = [
  { name: "signature_check", label: "Signature Verification", status: "pending" },
  { name: "manifest_validation", label: "Manifest Validation", status: "pending" },
  { name: "sandbox_test", label: "Sandbox Test", status: "pending" },
  { name: "security_scan", label: "Security Scan", status: "pending" },
  { name: "capability_audit", label: "Capability Audit", status: "pending" },
  { name: "governance_check", label: "Governance Check", status: "pending" },
];

function formatDownloads(count: number): string {
  if (count >= 1000) return `${(count / 1000).toFixed(1)}k`;
  return String(count);
}

function renderStars(rating: number): string {
  const full = Math.floor(rating);
  const half = rating - full >= 0.5;
  return "\u2605".repeat(full) + (half ? "\u00BD" : "") + "\u2606".repeat(5 - full - (half ? 1 : 0));
}

export default function DeveloperPortal(): JSX.Element {
  const [publishState, setPublishState] = useState<PublishState>("idle");
  const [bundleData, setBundleData] = useState<string | null>(null);
  const [bundleName, setBundleName] = useState<string>("");
  const [steps, setSteps] = useState<VerificationStep[]>(INITIAL_STEPS);
  const [publishResult, setPublishResult] = useState<MarketplacePublishResult | null>(null);
  const [publishError, setPublishError] = useState<string>("");
  const [myAgents, setMyAgents] = useState<MarketplaceAgent[]>([]);
  const [authorName, setAuthorName] = useState("developer");
  const [dragOver, setDragOver] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const verifyIntervalRef = useRef<number | null>(null);
  const delayTimerRef = useRef<number | null>(null);
  const isDesktop = hasDesktopRuntime();

  useEffect(() => {
    return () => {
      if (verifyIntervalRef.current) clearInterval(verifyIntervalRef.current);
      if (delayTimerRef.current) clearTimeout(delayTimerRef.current);
    };
  }, []);

  const loadMyAgents = useCallback(async (author: string) => {
    if (!isDesktop || !author) return;
    try {
      const agents = await marketplaceMyAgents(author);
      setMyAgents(agents);
    } catch {
      // silent
    }
  }, [isDesktop]);

  useEffect(() => {
    void loadMyAgents(authorName);
  }, [loadMyAgents, authorName]);

  const readFile = useCallback((file: File) => {
    setBundleName(file.name);
    const reader = new FileReader();
    reader.onload = () => {
      const text = reader.result as string;
      setBundleData(text);
      setPublishState("idle");
      setSteps(INITIAL_STEPS);
      setPublishResult(null);
      setPublishError("");
    };
    reader.readAsText(file);
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const file = e.dataTransfer.files[0];
    if (file) readFile(file);
  }, [readFile]);

  const handleFileSelect = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) readFile(file);
  }, [readFile]);

  const simulateVerification = useCallback((checks: MarketplacePublishResult["checks"]) => {
    const stepNames = INITIAL_STEPS.map((s) => s.name);
    let i = 0;
    if (verifyIntervalRef.current) clearInterval(verifyIntervalRef.current);
    const interval = setInterval(() => {
      if (i >= stepNames.length) {
        clearInterval(interval);
        verifyIntervalRef.current = null;
        return;
      }
      const name = stepNames[i];
      const check = checks.find((c) => c.name === name);
      setSteps((prev) =>
        prev.map((s) => {
          if (s.name === name) {
            const passed = check ? check.passed : true;
            const hasFindings = check && check.findings.length > 0;
            return {
              ...s,
              status: passed ? (hasFindings ? "warning" : "passed") : "failed",
              detail: check?.findings.join("; ") || undefined,
            };
          }
          if (s.name === stepNames[i + 1]) {
            return { ...s, status: "running" };
          }
          return s;
        })
      );
      i++;
    }, 300);
    verifyIntervalRef.current = interval;
    // Start first step
    setSteps((prev) =>
      prev.map((s, idx) => (idx === 0 ? { ...s, status: "running" } : s))
    );
  }, []);

  const handlePublish = useCallback(async () => {
    if (!bundleData) return;
    setPublishState("uploading");
    setSteps(INITIAL_STEPS);
    setPublishResult(null);
    setPublishError("");

    await new Promise((r) => { delayTimerRef.current = window.setTimeout(r, 400); });
    setPublishState("verifying");

    if (isDesktop) {
      try {
        const result = await marketplacePublish(bundleData);
        simulateVerification(result.checks);
        await new Promise((r) => { delayTimerRef.current = window.setTimeout(r, INITIAL_STEPS.length * 300 + 200); });
        setPublishResult(result);
        setPublishState("done");
        void loadMyAgents(authorName);
      } catch (err) {
        setPublishError(err instanceof Error ? err.message : "Publish failed");
        setPublishState("error");
      }
    } else {
      // Browser-mode verification preview (no backend)
      const browserChecks = INITIAL_STEPS.map((s) => ({
        name: s.name,
        passed: true,
        findings: [] as string[],
      }));
      simulateVerification(browserChecks);
      await new Promise((r) => { delayTimerRef.current = window.setTimeout(r, INITIAL_STEPS.length * 300 + 200); });
      setPublishResult({
        package_id: `local-${Date.now()}`,
        name: bundleName.replace(".nexus-agent", ""),
        version: "1.0.0",
        verdict: "approved",
        checks: browserChecks,
      });
      setPublishState("done");
    }
  }, [bundleData, bundleName, isDesktop, simulateVerification, loadMyAgents, authorName]);

  const stepIcon = (status: VerificationStep["status"]): string => {
    switch (status) {
      case "pending": return "\u25CB";
      case "running": return "\u25D4";
      case "passed": return "\u2713";
      case "warning": return "\u26A0";
      case "failed": return "\u2717";
    }
  };

  return (
    <section className="dp-hub">
      <header className="dp-header">
        <h2 className="dp-title">DEVELOPER PORTAL // PUBLISH & MANAGE</h2>
        <p className="dp-subtitle">Upload agent bundles for verification and marketplace listing</p>
      </header>

      <div className="dp-layout">
        <div className="dp-publish-panel">
          <h3 className="dp-section-title">Publish Agent</h3>

          <div
            className={`dp-dropzone ${dragOver ? "dp-dropzone-active" : ""} ${bundleData ? "dp-dropzone-loaded" : ""}`}
            onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
            onDragLeave={() => setDragOver(false)}
            onDrop={handleDrop}
            onClick={() => fileInputRef.current?.click()}
            onKeyDown={() => {}}
          >
            <input
              ref={fileInputRef}
              type="file"
              accept=".nexus-agent,.json"
              onChange={handleFileSelect}
              style={{ display: "none" }}
            />
            {bundleData ? (
              <div className="dp-dropzone-file">
                <span className="dp-file-icon">{"\u2714"}</span>
                <span className="dp-file-name">{bundleName}</span>
                <span className="dp-file-size">{(bundleData.length / 1024).toFixed(1)} KB</span>
              </div>
            ) : (
              <div className="dp-dropzone-empty">
                <span className="dp-drop-icon">{"\u2B06"}</span>
                <span className="dp-drop-text">Drop .nexus-agent bundle here</span>
                <span className="dp-drop-hint">or click to browse</span>
              </div>
            )}
          </div>

          {bundleData && publishState === "idle" && (
            <button type="button" className="dp-publish-btn" onClick={() => void handlePublish()}>
              Submit for Verification
            </button>
          )}

          {(publishState === "uploading" || publishState === "verifying") && (
            <div className="dp-verify-panel">
              <h4 className="dp-verify-title">
                {publishState === "uploading" ? "Uploading..." : "Verification Pipeline"}
              </h4>
              <div className="dp-steps">
                {steps.map((step) => (
                  <div key={step.name} className={`dp-step dp-step-${step.status}`}>
                    <span className="dp-step-icon">{stepIcon(step.status)}</span>
                    <span className="dp-step-label">{step.label}</span>
                    {step.detail && <span className="dp-step-detail">{step.detail}</span>}
                  </div>
                ))}
              </div>
            </div>
          )}

          {publishState === "done" && publishResult && (
            <div className="dp-result">
              <div className={`dp-verdict dp-verdict-${publishResult.verdict}`}>
                <span className="dp-verdict-icon">
                  {publishResult.verdict === "approved" ? "\u2713" : publishResult.verdict === "rejected" ? "\u2717" : "\u26A0"}
                </span>
                <span className="dp-verdict-text">{publishResult.verdict.toUpperCase()}</span>
              </div>
              <div className="dp-result-meta">
                <span>Package: {publishResult.name}</span>
                <span>Version: {publishResult.version}</span>
                <span>ID: {publishResult.package_id}</span>
              </div>
              <div className="dp-steps">
                {steps.map((step) => (
                  <div key={step.name} className={`dp-step dp-step-${step.status}`}>
                    <span className="dp-step-icon">{stepIcon(step.status)}</span>
                    <span className="dp-step-label">{step.label}</span>
                    {step.detail && <span className="dp-step-detail">{step.detail}</span>}
                  </div>
                ))}
              </div>
              <button type="button" className="dp-reset-btn" onClick={() => {
                setPublishState("idle");
                setBundleData(null);
                setBundleName("");
                setSteps(INITIAL_STEPS);
                setPublishResult(null);
              }}>
                Publish Another
              </button>
            </div>
          )}

          {publishState === "error" && (
            <div className="dp-error">
              <span className="dp-error-icon">{"\u2717"}</span>
              <span>{publishError}</span>
              <button type="button" className="dp-reset-btn" onClick={() => setPublishState("idle")}>
                Try Again
              </button>
            </div>
          )}
        </div>

        <div className="dp-agents-panel">
          <h3 className="dp-section-title">My Published Agents</h3>
          <div className="dp-author-input">
            <label htmlFor="dp-author">Author:</label>
            <input
              id="dp-author"
              className="dp-author-field"
              value={authorName}
              onChange={(e) => setAuthorName(e.target.value)}
              onBlur={() => void loadMyAgents(authorName)}
            />
          </div>
          {myAgents.length === 0 ? (
            <p className="dp-no-agents">No published agents yet.</p>
          ) : (
            <div className="dp-agent-list">
              {myAgents.map((agent) => (
                <div key={agent.package_id} className="dp-agent-card">
                  <div className="dp-agent-top">
                    <span className="dp-agent-name">{agent.name}</span>
                    <span className="dp-agent-version">v{agent.version}</span>
                  </div>
                  <p className="dp-agent-desc">{agent.description}</p>
                  <div className="dp-agent-stats">
                    <span>{formatDownloads(agent.downloads)} downloads</span>
                    <span>{renderStars(agent.rating)} {agent.rating.toFixed(1)} ({agent.review_count})</span>
                    {agent.price_cents > 0
                      ? <span className="dp-price">${(agent.price_cents / 100).toFixed(2)}</span>
                      : <span className="dp-free">Free</span>
                    }
                  </div>
                  <div className="dp-agent-tags">
                    {agent.tags.map((tag) => (
                      <span key={tag} className="dp-tag">{tag}</span>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}

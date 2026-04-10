/**
 * DeployPanel — slide-out panel for one-click deployment.
 *
 * Flow: Provider selection -> Credential auth -> Confirm -> Deploy -> Live URL
 *
 * Supports Netlify, Cloudflare Pages, and Vercel.
 * Credentials are stored encrypted on the user's machine — never leave the device.
 * All inline styles per project convention (no CSS classes/variables).
 */

import { useState, useCallback, useEffect } from "react";
import {
  builderDeploy,
  builderDeployStoreCredentials,
  builderDeployCheckCredentials,
  builderDeployRollback,
  type DeployResult,
} from "../../api/backend";

const C = {
  bg: "#0a0e14",
  surface: "#111820",
  surfaceAlt: "#0d1219",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  red: "#ef4444",
  redDim: "rgba(239,68,68,0.10)",
  sans: "system-ui,-apple-system,sans-serif",
};

type Step = "provider" | "auth" | "confirm" | "deploying" | "live" | "error";
type Provider = "netlify" | "cloudflare" | "vercel";

interface DeployPanelProps {
  projectId: string;
  onClose: () => void;
  /** If a previous deploy exists, show Redeploy instead of Deploy */
  lastDeploy?: { provider: string; site_id: string; deploy_id: string; url: string } | null;
  /** Called when user wants to see deploy history */
  onHistory?: () => void;
}

const PROVIDERS: { id: Provider; label: string; desc: string }[] = [
  { id: "netlify", label: "Netlify", desc: "Free tier, instant CDN, auto-SSL" },
  { id: "cloudflare", label: "Cloudflare Pages", desc: "Free tier, global edge network" },
  { id: "vercel", label: "Vercel", desc: "Free tier, optimized for React" },
];

export default function DeployPanel({ projectId, onClose, lastDeploy, onHistory }: DeployPanelProps) {
  const [step, setStep] = useState<Step>(lastDeploy ? "confirm" : "provider");
  const [provider, setProvider] = useState<Provider>(
    (lastDeploy?.provider as Provider) ?? "netlify"
  );
  const [token, setToken] = useState("");
  const [accountId, setAccountId] = useState("");
  const [siteName, setSiteName] = useState("");
  const [result, setResult] = useState<DeployResult | null>(null);
  const [error, setError] = useState("");
  const [progress, setProgress] = useState("");

  // Check credentials when provider is selected
  const checkCreds = useCallback(async (prov: Provider) => {
    try {
      const valid = await builderDeployCheckCredentials(prov);
      if (valid) {
        setStep("confirm");
      } else {
        setStep("auth");
      }
    } catch {
      setStep("auth");
    }
  }, []);

  const handleProviderSelect = useCallback((prov: Provider) => {
    setProvider(prov);
    checkCreds(prov);
  }, [checkCreds]);

  const handleSaveCredentials = useCallback(async () => {
    if (!token.trim()) return;
    try {
      await builderDeployStoreCredentials(
        provider,
        token.trim(),
        provider === "cloudflare" ? accountId.trim() || undefined : undefined
      );
      setToken("");
      setAccountId("");
      setStep("confirm");
    } catch (e: any) {
      setError(e?.toString() ?? "Failed to save credentials");
      setStep("error");
    }
  }, [provider, token, accountId]);

  const handleDeploy = useCallback(async () => {
    setStep("deploying");
    setProgress("Preparing files...");
    try {
      setProgress("Uploading to " + provider + "...");
      const res = await builderDeploy(
        projectId,
        provider,
        lastDeploy?.site_id,
        siteName.trim() || undefined
      );
      setResult(res);
      setStep("live");
    } catch (e: any) {
      setError(e?.toString() ?? "Deploy failed");
      setStep("error");
    }
  }, [projectId, provider, lastDeploy, siteName]);

  const handleRollback = useCallback(async () => {
    if (!lastDeploy) return;
    setStep("deploying");
    setProgress("Rolling back...");
    try {
      await builderDeployRollback(
        projectId,
        lastDeploy.provider,
        lastDeploy.site_id,
        lastDeploy.deploy_id
      );
      setStep("confirm");
    } catch (e: any) {
      setError(e?.toString() ?? "Rollback failed");
      setStep("error");
    }
  }, [projectId, lastDeploy]);

  const copyUrl = useCallback(() => {
    if (result?.url) {
      navigator.clipboard.writeText(result.url).catch(() => {});
    }
  }, [result]);

  const openUrl = useCallback(() => {
    if (result?.url) {
      window.open(result.url, "_blank");
    }
  }, [result]);

  // Panel backdrop + slide-out
  return (
    <div style={{
      position: "fixed", top: 0, right: 0, bottom: 0, left: 0, zIndex: 1000,
      display: "flex", justifyContent: "flex-end",
      background: "rgba(0,0,0,0.5)",
    }} onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div style={{
        width: 380, height: "100%", background: C.surface,
        borderLeft: `1px solid ${C.border}`, padding: 24,
        overflowY: "auto", fontFamily: C.sans,
        display: "flex", flexDirection: "column", gap: 16,
      }}>
        {/* Header */}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span style={{ color: C.text, fontSize: 14, fontWeight: 600 }}>
            {lastDeploy ? "Redeploy" : "Deploy"}
          </span>
          <button type="button" onClick={onClose} style={{
            background: "transparent", border: "none", color: C.dim, fontSize: 16,
            cursor: "pointer", padding: "2px 6px",
          }}>x</button>
        </div>

        {/* Step 1: Provider selection */}
        {step === "provider" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            <span style={{ color: C.muted, fontSize: 11 }}>Choose your hosting:</span>
            {PROVIDERS.map(p => (
              <button type="button" key={p.id} onClick={() => handleProviderSelect(p.id)} style={{
                background: C.surfaceAlt, border: `1px solid ${C.border}`,
                borderRadius: 6, padding: "12px 14px", cursor: "pointer",
                textAlign: "left", transition: "border-color 0.15s",
              }}>
                <div style={{ color: C.text, fontSize: 12, fontWeight: 600 }}>{p.label}</div>
                <div style={{ color: C.muted, fontSize: 10, marginTop: 2 }}>{p.desc}</div>
              </button>
            ))}
            <div style={{ color: C.dim, fontSize: 9, marginTop: 4 }}>
              All free. Your code stays yours.
            </div>
          </div>
        )}

        {/* Step 2: Authentication */}
        {step === "auth" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <span style={{ color: C.muted, fontSize: 11 }}>
              {provider === "netlify" && "Paste your Netlify personal access token:"}
              {provider === "cloudflare" && "Paste your Cloudflare API token and Account ID:"}
              {provider === "vercel" && "Paste your Vercel personal access token:"}
            </span>

            {provider === "netlify" && (
              <div style={{ color: C.dim, fontSize: 9 }}>
                Go to app.netlify.com/user/applications/personal &rarr; New access token
              </div>
            )}
            {provider === "cloudflare" && (
              <div style={{ color: C.dim, fontSize: 9 }}>
                Go to dash.cloudflare.com/profile/api-tokens &rarr; Create Token &rarr; Edit Cloudflare Pages
              </div>
            )}
            {provider === "vercel" && (
              <div style={{ color: C.dim, fontSize: 9 }}>
                Go to vercel.com/account/tokens &rarr; Create Token
              </div>
            )}

            <input
              type="password"
              placeholder="Paste token here..."
              value={token}
              onChange={e => setToken(e.target.value)}
              style={{
                background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4,
                padding: "8px 10px", color: C.text, fontSize: 11, fontFamily: "monospace",
                outline: "none",
              }}
            />

            {provider === "cloudflare" && (
              <input
                type="text"
                placeholder="Account ID (from dashboard URL)"
                value={accountId}
                onChange={e => setAccountId(e.target.value)}
                style={{
                  background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4,
                  padding: "8px 10px", color: C.text, fontSize: 11, fontFamily: "monospace",
                  outline: "none",
                }}
              />
            )}

            <div style={{ color: C.dim, fontSize: 9 }}>
              Your credentials are stored securely on this device and never leave your machine.
            </div>

            <div style={{ display: "flex", gap: 8 }}>
              <button type="button" onClick={() => setStep("provider")} style={{
                background: "transparent", border: `1px solid ${C.border}`,
                borderRadius: 4, padding: "6px 14px", color: C.muted, fontSize: 10,
                cursor: "pointer",
              }}>Back</button>
              <button type="button" onClick={handleSaveCredentials} disabled={!token.trim()} style={{
                background: token.trim() ? C.accent : C.dim,
                border: "none", borderRadius: 4, padding: "6px 14px",
                color: token.trim() ? C.bg : C.muted, fontSize: 10,
                fontWeight: 600, cursor: token.trim() ? "pointer" : "default",
              }}>Connect</button>
            </div>
          </div>
        )}

        {/* Step 3: Confirm deploy */}
        {step === "confirm" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <div style={{
              background: C.accentDim, border: "1px solid rgba(0,212,170,0.2)",
              borderRadius: 6, padding: "10px 12px",
            }}>
              <div style={{ color: C.accent, fontSize: 11, fontWeight: 600 }}>
                Ready to deploy to {provider.charAt(0).toUpperCase() + provider.slice(1)}
              </div>
            </div>

            {!lastDeploy && (
              <div>
                <label style={{ color: C.muted, fontSize: 10, display: "block", marginBottom: 4 }}>
                  Site name (optional):
                </label>
                <input
                  type="text"
                  placeholder={projectId}
                  value={siteName}
                  onChange={e => setSiteName(e.target.value)}
                  style={{
                    background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4,
                    padding: "6px 10px", color: C.text, fontSize: 11, width: "100%",
                    outline: "none", boxSizing: "border-box",
                  }}
                />
              </div>
            )}

            {lastDeploy && (
              <div style={{ color: C.muted, fontSize: 10 }}>
                Site: <span style={{ color: C.text }}>{lastDeploy.url}</span>
              </div>
            )}

            <div style={{ display: "flex", gap: 8 }}>
              {!lastDeploy && (
                <button type="button" onClick={() => setStep("provider")} style={{
                  background: "transparent", border: `1px solid ${C.border}`,
                  borderRadius: 4, padding: "6px 14px", color: C.muted, fontSize: 10,
                  cursor: "pointer",
                }}>Back</button>
              )}
              <button type="button" onClick={handleDeploy} style={{
                background: C.accent, border: "none", borderRadius: 4,
                padding: "6px 18px", color: C.bg, fontSize: 10,
                fontWeight: 600, cursor: "pointer", flex: 1,
              }}>
                {lastDeploy ? "Redeploy Now" : "Deploy Now"}
              </button>
              <button type="button" onClick={onClose} style={{
                background: "transparent", border: `1px solid ${C.border}`,
                borderRadius: 4, padding: "6px 14px", color: C.muted, fontSize: 10,
                cursor: "pointer",
              }}>Cancel</button>
            </div>
          </div>
        )}

        {/* Step 4: Deploying */}
        {step === "deploying" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12, alignItems: "center", paddingTop: 20 }}>
            <div style={{
              width: 32, height: 32, border: `2px solid ${C.border}`,
              borderTopColor: C.accent, borderRadius: "50%",
              animation: "spin 1s linear infinite",
            }} />
            <style>{`@keyframes spin { to { transform: rotate(360deg); } }`}</style>
            <div style={{ color: C.text, fontSize: 12, fontWeight: 500 }}>{progress}</div>
            <div style={{ color: C.dim, fontSize: 9 }}>This usually takes less than 60 seconds</div>
          </div>
        )}

        {/* Step 5: Live */}
        {step === "live" && result && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            <div style={{
              background: C.accentDim, border: "1px solid rgba(0,212,170,0.25)",
              borderRadius: 6, padding: "14px 16px", textAlign: "center",
            }}>
              <div style={{ color: C.accent, fontSize: 13, fontWeight: 600, marginBottom: 6 }}>
                Your site is live!
              </div>
              <a
                href={result.url}
                target="_blank"
                rel="noopener noreferrer"
                style={{ color: C.text, fontSize: 11, wordBreak: "break-all", textDecoration: "underline" }}
              >
                {result.url}
              </a>
            </div>

            <div style={{ display: "flex", gap: 8 }}>
              <button type="button" onClick={copyUrl} style={{
                background: C.surfaceAlt, border: `1px solid ${C.border}`,
                borderRadius: 4, padding: "6px 14px", color: C.muted, fontSize: 10,
                cursor: "pointer", flex: 1,
              }}>Copy URL</button>
              <button type="button" onClick={openUrl} style={{
                background: C.accent, border: "none", borderRadius: 4,
                padding: "6px 14px", color: C.bg, fontSize: 10,
                fontWeight: 600, cursor: "pointer", flex: 1,
              }}>Open in Browser</button>
            </div>

            <div style={{ color: C.dim, fontSize: 9 }}>
              {result.file_count} files deployed in {(result.duration_ms / 1000).toFixed(1)}s
              &middot; Build hash: {result.build_hash.slice(0, 12)}...
            </div>

            {/* Rollback link */}
            {lastDeploy && (
              <button type="button" onClick={handleRollback} style={{
                background: "transparent", border: "none", color: C.dim,
                fontSize: 9, cursor: "pointer", textDecoration: "underline",
                padding: 0, textAlign: "left",
              }}>
                Rollback to previous deploy
              </button>
            )}

            {onHistory && (
              <button type="button" onClick={onHistory} style={{
                background: C.surfaceAlt, border: `1px solid ${C.border}`,
                borderRadius: 4, padding: "6px 14px", color: C.muted, fontSize: 10,
                cursor: "pointer",
              }}>View Deploy History</button>
            )}

            <button type="button" onClick={onClose} style={{
              background: "transparent", border: `1px solid ${C.border}`,
              borderRadius: 4, padding: "6px 14px", color: C.muted, fontSize: 10,
              cursor: "pointer",
            }}>Close</button>
          </div>
        )}

        {/* Error */}
        {step === "error" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <div style={{
              background: C.redDim, border: "1px solid rgba(239,68,68,0.25)",
              borderRadius: 6, padding: "10px 12px",
            }}>
              <div style={{ color: C.red, fontSize: 11, fontWeight: 600, marginBottom: 4 }}>
                Deploy failed
              </div>
              <div style={{ color: C.muted, fontSize: 10, wordBreak: "break-word" }}>{error}</div>
            </div>
            <div style={{ display: "flex", gap: 8 }}>
              <button type="button" onClick={() => setStep("confirm")} style={{
                background: C.surfaceAlt, border: `1px solid ${C.border}`,
                borderRadius: 4, padding: "6px 14px", color: C.muted, fontSize: 10,
                cursor: "pointer",
              }}>Try Again</button>
              <button type="button" onClick={onClose} style={{
                background: "transparent", border: `1px solid ${C.border}`,
                borderRadius: 4, padding: "6px 14px", color: C.muted, fontSize: 10,
                cursor: "pointer",
              }}>Close</button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

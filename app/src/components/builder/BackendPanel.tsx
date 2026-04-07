/**
 * BackendPanel — Multi-provider backend integration flow.
 *
 * Steps: Choose Provider → Connect (if needed) → Describe → Review → Applied
 * Providers: Supabase, SQLite, PocketBase, Firebase
 * Optional — projects work fine as frontend-only.
 */

import { useState, useCallback, useEffect, type FormEvent } from "react";
import {
  builderBackendConnect,
  builderBackendGenerate,
  builderBackendGenerateV2,
  builderBackendApply,
  builderBackendListProviders,
  type BackendGenerationResult,
  type ProviderInfo,
} from "../../api/backend";

const C = {
  bg: "#0a0e14",
  surface: "#111820",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  accentGlow: "rgba(0,212,170,0.25)",
  err: "#f85149",
  sans: "system-ui,-apple-system,sans-serif",
  mono: "'JetBrains Mono',monospace",
};

type Step = "provider" | "connect" | "describe" | "review" | "applied";

interface BackendPanelProps {
  projectId: string;
  onClose: () => void;
}

const PROVIDER_ICONS: Record<string, string> = {
  supabase: "S",
  sqlite: "Q",
  pocketbase: "P",
  firebase: "F",
};

export default function BackendPanel({ projectId, onClose }: BackendPanelProps) {
  const [step, setStep] = useState<Step>("provider");
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [selectedProvider, setSelectedProvider] = useState<string>("supabase");
  const [projectUrl, setProjectUrl] = useState("");
  const [anonKey, setAnonKey] = useState("");
  const [serviceRoleKey, setServiceRoleKey] = useState("");
  const [pbUrl, setPbUrl] = useState("http://127.0.0.1:8090");
  const [description, setDescription] = useState("");
  const [result, setResult] = useState<BackendGenerationResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [showSql, setShowSql] = useState(false);

  // Load providers on mount
  useEffect(() => {
    builderBackendListProviders()
      .then(setProviders)
      .catch(() => {
        // Fallback if command not yet available
        setProviders([
          { id: "supabase", name: "Supabase", description: "Cloud database + auth + storage", requires_credentials: true, cost_hint: "~$0.15" },
          { id: "sqlite", name: "SQLite", description: "Local database, offline-capable", requires_credentials: false, cost_hint: "$0" },
          { id: "pocketbase", name: "PocketBase", description: "Self-hosted backend", requires_credentials: true, cost_hint: "~$0.15" },
          { id: "firebase", name: "Firebase", description: "Google cloud backend", requires_credentials: true, cost_hint: "~$0.15" },
        ]);
      });
  }, []);

  const currentProvider = providers.find((p) => p.id === selectedProvider);

  const handleSelectProvider = (id: string) => {
    setSelectedProvider(id);
    setError(null);
    const prov = providers.find((p) => p.id === id);
    if (prov && !prov.requires_credentials) {
      // Skip connect step for providers that don't need credentials
      setStep("describe");
    } else if (id === "supabase") {
      setStep("connect");
    } else if (id === "pocketbase") {
      setStep("connect");
    } else if (id === "firebase") {
      // Firebase uses env vars, skip connect step
      setStep("describe");
    } else {
      setStep("connect");
    }
  };

  const handleConnect = useCallback(async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);
    try {
      if (selectedProvider === "supabase") {
        await builderBackendConnect(projectId, projectUrl, anonKey, serviceRoleKey || undefined);
      }
      // PocketBase: just validate URL is provided
      setStep("describe");
    } catch (err: any) {
      setError(err?.toString() ?? "Connection failed");
    } finally {
      setLoading(false);
    }
  }, [projectId, projectUrl, anonKey, serviceRoleKey, selectedProvider]);

  const handleGenerate = useCallback(async () => {
    setError(null);
    setLoading(true);
    try {
      let res: BackendGenerationResult;
      if (selectedProvider === "supabase") {
        res = await builderBackendGenerate(projectId, description);
      } else {
        res = await builderBackendGenerateV2(projectId, selectedProvider, description);
      }
      setResult(res);
      setStep("review");
    } catch (err: any) {
      setError(err?.toString() ?? "Generation failed");
    } finally {
      setLoading(false);
    }
  }, [projectId, description, selectedProvider]);

  const handleApply = useCallback(async () => {
    setError(null);
    setLoading(true);
    try {
      await builderBackendApply(projectId);
      setStep("applied");
    } catch (err: any) {
      setError(err?.toString() ?? "Apply failed");
    } finally {
      setLoading(false);
    }
  }, [projectId]);

  const allMigrationSql = result
    ? [...result.migrations, ...result.rls_migrations].map((m) => `-- ${m.filename}\n${m.sql}`).join("\n\n")
    : "";

  const allFileContents = result
    ? result.files.map((f) => `// ${f.path}\n${f.content}`).join("\n\n")
    : "";

  const previewContent = selectedProvider === "supabase" ? allMigrationSql : allFileContents;

  const panelStyle: React.CSSProperties = {
    position: "absolute",
    top: 0,
    right: 0,
    bottom: 0,
    width: 420,
    background: C.surface,
    borderLeft: `1px solid ${C.border}`,
    display: "flex",
    flexDirection: "column",
    zIndex: 40,
    fontFamily: C.sans,
    fontSize: 12,
    color: C.text,
    overflow: "hidden",
  };

  const cardStyle: React.CSSProperties = {
    background: C.bg,
    border: `1px solid ${C.border}`,
    borderRadius: 6,
    padding: "10px 12px",
    marginBottom: 6,
    cursor: "pointer",
    transition: "border-color 0.15s",
  };

  const cardSelectedStyle: React.CSSProperties = {
    ...cardStyle,
    borderColor: C.accent,
    background: C.accentDim,
  };

  return (
    <div style={panelStyle}>
      {/* Header */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "10px 14px", borderBottom: `1px solid ${C.border}` }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span style={{ fontWeight: 600, fontSize: 13 }}>Add Backend</span>
          {step !== "provider" && currentProvider && (
            <span style={{ fontSize: 10, color: C.accent, background: C.accentDim, padding: "2px 6px", borderRadius: 3 }}>
              {currentProvider.name}
            </span>
          )}
        </div>
        <button onClick={onClose} style={{ background: "none", border: "none", color: C.muted, cursor: "pointer", fontSize: 16 }}>x</button>
      </div>

      {/* Content */}
      <div style={{ flex: 1, overflow: "auto", padding: 14 }}>
        {error && (
          <div style={{ background: "rgba(248,81,73,0.08)", border: `1px solid rgba(248,81,73,0.25)`, borderRadius: 4, padding: "6px 10px", marginBottom: 12, color: C.err, fontSize: 11 }}>
            {error}
          </div>
        )}

        {/* Step 1: Provider Selection */}
        {step === "provider" && (
          <div>
            <p style={{ color: C.muted, marginBottom: 14, lineHeight: 1.5 }}>
              Choose your backend provider:
            </p>
            {providers.map((p) => (
              <div
                key={p.id}
                onClick={() => handleSelectProvider(p.id)}
                style={selectedProvider === p.id ? cardSelectedStyle : cardStyle}
                onMouseEnter={(e) => { if (selectedProvider !== p.id) e.currentTarget.style.borderColor = C.dim; }}
                onMouseLeave={(e) => { if (selectedProvider !== p.id) e.currentTarget.style.borderColor = C.border; }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <span style={{ width: 24, height: 24, borderRadius: 4, background: C.accentDim, color: C.accent, display: "flex", alignItems: "center", justifyContent: "center", fontWeight: 700, fontSize: 11, flexShrink: 0 }}>
                    {PROVIDER_ICONS[p.id] ?? "?"}
                  </span>
                  <div style={{ flex: 1 }}>
                    <div style={{ fontWeight: 600, fontSize: 12 }}>{p.name}</div>
                    <div style={{ color: C.muted, fontSize: 10, marginTop: 2 }}>{p.description}</div>
                  </div>
                  <div style={{ fontSize: 9, color: C.dim }}>{p.cost_hint}</div>
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Step 2: Connect (Supabase / PocketBase) */}
        {step === "connect" && selectedProvider === "supabase" && (
          <form onSubmit={handleConnect}>
            <button onClick={() => setStep("provider")} type="button" style={{ background: "none", border: "none", color: C.muted, cursor: "pointer", fontSize: 10, marginBottom: 10, padding: 0 }}>&larr; Back</button>
            <p style={{ color: C.muted, marginBottom: 12, lineHeight: 1.5 }}>
              Connect your Supabase project. Get credentials from Project Settings &rarr; API.
            </p>
            <label style={{ display: "block", color: C.muted, fontSize: 10, marginBottom: 4 }}>Project URL</label>
            <input value={projectUrl} onChange={(e) => setProjectUrl(e.target.value)} placeholder="https://xxxxx.supabase.co" required
              style={{ width: "100%", padding: "6px 8px", background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4, color: C.text, fontSize: 11, fontFamily: C.mono, marginBottom: 10, boxSizing: "border-box" }} />
            <label style={{ display: "block", color: C.muted, fontSize: 10, marginBottom: 4 }}>Anon Key</label>
            <input value={anonKey} onChange={(e) => setAnonKey(e.target.value)} placeholder="eyJhbGciOi..." required type="password"
              style={{ width: "100%", padding: "6px 8px", background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4, color: C.text, fontSize: 11, fontFamily: C.mono, marginBottom: 10, boxSizing: "border-box" }} />
            <label style={{ display: "block", color: C.muted, fontSize: 10, marginBottom: 4 }}>Service Role Key (optional)</label>
            <input value={serviceRoleKey} onChange={(e) => setServiceRoleKey(e.target.value)} placeholder="Optional — for migration execution" type="password"
              style={{ width: "100%", padding: "6px 8px", background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4, color: C.text, fontSize: 11, fontFamily: C.mono, marginBottom: 14, boxSizing: "border-box" }} />
            <button type="submit" disabled={loading || !projectUrl || !anonKey}
              style={{ width: "100%", padding: "7px 0", background: C.accentDim, border: `1px solid ${C.accentGlow}`, borderRadius: 4, color: C.accent, cursor: loading ? "default" : "pointer", fontWeight: 600, fontSize: 11 }}>
              {loading ? "Connecting..." : "Connect"}
            </button>
          </form>
        )}

        {step === "connect" && selectedProvider === "pocketbase" && (
          <form onSubmit={handleConnect}>
            <button onClick={() => setStep("provider")} type="button" style={{ background: "none", border: "none", color: C.muted, cursor: "pointer", fontSize: 10, marginBottom: 10, padding: 0 }}>&larr; Back</button>
            <p style={{ color: C.muted, marginBottom: 12, lineHeight: 1.5 }}>
              Enter your PocketBase server URL. Make sure PocketBase is running.
            </p>
            <label style={{ display: "block", color: C.muted, fontSize: 10, marginBottom: 4 }}>PocketBase URL</label>
            <input value={pbUrl} onChange={(e) => setPbUrl(e.target.value)} placeholder="http://127.0.0.1:8090" required
              style={{ width: "100%", padding: "6px 8px", background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4, color: C.text, fontSize: 11, fontFamily: C.mono, marginBottom: 14, boxSizing: "border-box" }} />
            <button type="submit" disabled={loading || !pbUrl}
              style={{ width: "100%", padding: "7px 0", background: C.accentDim, border: `1px solid ${C.accentGlow}`, borderRadius: 4, color: C.accent, cursor: loading ? "default" : "pointer", fontWeight: 600, fontSize: 11 }}>
              {loading ? "Connecting..." : "Continue"}
            </button>
          </form>
        )}

        {/* Step 3: Describe Data Model */}
        {step === "describe" && (
          <div>
            <button onClick={() => setStep("provider")} style={{ background: "none", border: "none", color: C.muted, cursor: "pointer", fontSize: 10, marginBottom: 10, padding: 0 }}>&larr; Change provider</button>
            <p style={{ color: C.muted, marginBottom: 12, lineHeight: 1.5 }}>
              Describe your data model in plain English.
            </p>
            <textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={"I need user authentication, a products table with\nname/price/image, and a shopping cart for each user"}
              rows={5}
              style={{ width: "100%", padding: "8px", background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4, color: C.text, fontSize: 11, fontFamily: C.sans, resize: "vertical", marginBottom: 10, boxSizing: "border-box" }}
            />
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 14 }}>
              <span style={{ color: C.dim, fontSize: 10 }}>
                {currentProvider?.cost_hint ?? ""}
              </span>
            </div>
            <button
              onClick={handleGenerate}
              disabled={loading || !description.trim()}
              style={{ width: "100%", padding: "7px 0", background: C.accentDim, border: `1px solid ${C.accentGlow}`, borderRadius: 4, color: C.accent, cursor: loading ? "default" : "pointer", fontWeight: 600, fontSize: 11 }}
            >
              {loading ? "Generating..." : `Generate ${currentProvider?.name ?? ""} Backend`}
            </button>
          </div>
        )}

        {/* Step 4: Review */}
        {step === "review" && result && (
          <div>
            <p style={{ fontWeight: 600, marginBottom: 10 }}>Generated Schema</p>

            {/* Tables */}
            {result.schema?.tables?.map((t: any) => (
              <div key={t.name} style={{ background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4, padding: "8px 10px", marginBottom: 6 }}>
                <div style={{ fontWeight: 600, fontSize: 11 }}>{t.name}</div>
                <div style={{ color: C.muted, fontSize: 10, marginTop: 2 }}>
                  {t.columns?.map((c: any) => c.name).join(", ")}
                </div>
                {t.rls_enabled && (
                  <div style={{ color: C.accent, fontSize: 9, marginTop: 3 }}>
                    {selectedProvider === "supabase" ? "RLS enabled" : "Security rules applied"}
                  </div>
                )}
              </div>
            ))}

            {/* Security summary */}
            {result.rls_migrations.length > 0 && (
              <>
                <p style={{ fontWeight: 600, marginTop: 12, marginBottom: 6 }}>Security Policies</p>
                {result.rls_migrations.map((m) => (
                  <div key={m.filename} style={{ color: C.accent, fontSize: 10, marginBottom: 2 }}>
                    {m.description}
                  </div>
                ))}
              </>
            )}

            {/* Files summary */}
            <p style={{ fontWeight: 600, marginTop: 12, marginBottom: 6 }}>Generated Files ({result.files.length})</p>
            <div style={{ color: C.muted, fontSize: 10 }}>
              {result.files.map((f) => f.path).join(", ")}
            </div>

            {/* Cost */}
            <div style={{ marginTop: 12, padding: "6px 10px", background: C.accentDim, borderRadius: 4, fontSize: 10 }}>
              <span style={{ color: C.accent }}>Cost: ${result.cost_usd.toFixed(2)}</span>
              <span style={{ color: C.dim, marginLeft: 8 }}>
                Schema: {result.schema_hash.slice(0, 8)}...
                {result.rls_hash && ` Security: ${result.rls_hash.slice(0, 8)}...`}
              </span>
            </div>

            {/* Actions */}
            <div style={{ display: "flex", gap: 6, marginTop: 14 }}>
              <button
                onClick={() => setShowSql(!showSql)}
                style={{ flex: 1, padding: "6px 0", background: "transparent", border: `1px solid ${C.border}`, borderRadius: 4, color: C.muted, cursor: "pointer", fontSize: 10 }}
              >
                {showSql ? "Hide Preview" : "View Files"}
              </button>
              <button
                onClick={handleApply}
                disabled={loading}
                style={{ flex: 1, padding: "6px 0", background: C.accentDim, border: `1px solid ${C.accentGlow}`, borderRadius: 4, color: C.accent, cursor: loading ? "default" : "pointer", fontWeight: 600, fontSize: 10 }}
              >
                {loading ? "Applying..." : "Apply to Project"}
              </button>
            </div>

            {showSql && (
              <pre style={{ marginTop: 10, padding: 10, background: C.bg, border: `1px solid ${C.border}`, borderRadius: 4, fontSize: 10, fontFamily: C.mono, color: C.muted, overflow: "auto", maxHeight: 300, whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
                {previewContent}
              </pre>
            )}
          </div>
        )}

        {/* Step 5: Applied */}
        {step === "applied" && result && (
          <div>
            <div style={{ color: C.accent, fontWeight: 600, marginBottom: 12, fontSize: 13 }}>
              {currentProvider?.name ?? "Backend"} configured!
            </div>
            <ul style={{ listStyle: "none", padding: 0, margin: 0, color: C.text, fontSize: 11, lineHeight: 2 }}>
              {selectedProvider === "supabase" && <li>{result.migrations.length} tables with RLS policies</li>}
              {selectedProvider === "sqlite" && <li>{result.files.filter((f) => f.path.includes("migrations/")).length} SQLite migrations</li>}
              {selectedProvider === "pocketbase" && <li>PocketBase collection schema generated</li>}
              {selectedProvider === "firebase" && <li>Firestore security rules generated</li>}
              {result.schema?.auth_enabled && <li>Auth components added</li>}
              <li>CRUD hooks generated ({result.files.filter((f) => f.path.includes("hooks/")).length} hooks)</li>
              <li>TypeScript types generated</li>
            </ul>
            <p style={{ color: C.muted, fontSize: 10, marginTop: 14, lineHeight: 1.6 }}>
              {selectedProvider === "supabase" && "Next: Run migrations in Supabase Dashboard \u2192 SQL Editor"}
              {selectedProvider === "sqlite" && "Next: Migrations run automatically on first database load."}
              {selectedProvider === "pocketbase" && "Next: Import pb_schema.json in PocketBase Admin \u2192 Settings."}
              {selectedProvider === "firebase" && "Next: Deploy rules with 'firebase deploy --only firestore:rules'."}
            </p>
            {selectedProvider === "supabase" && (
              <button
                onClick={() => { navigator.clipboard.writeText(allMigrationSql); }}
                style={{ width: "100%", padding: "7px 0", marginTop: 10, background: C.accentDim, border: `1px solid ${C.accentGlow}`, borderRadius: 4, color: C.accent, cursor: "pointer", fontWeight: 600, fontSize: 11 }}
              >
                Copy Migration SQL
              </button>
            )}
            <button
              onClick={onClose}
              style={{ width: "100%", padding: "7px 0", marginTop: 6, background: "transparent", border: `1px solid ${C.border}`, borderRadius: 4, color: C.muted, cursor: "pointer", fontSize: 11 }}
            >
              Done
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

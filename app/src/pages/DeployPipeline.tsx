import { useState, useCallback, useMemo } from "react";
import "./deploy-pipeline.css";

/* ─── types ─── */
type View = "deployments" | "environments" | "domains" | "certs" | "logs";

interface Deployment {
  id: string;
  project: string;
  env: Environment;
  provider: Provider;
  status: DeployStatus;
  version: string;
  commit: string;
  branch: string;
  url: string;
  duration: number;
  startedAt: number;
  agent: string;
  fuelCost: number;
  logs: LogEntry[];
}

type DeployStatus = "building" | "deploying" | "live" | "failed" | "rolled-back" | "queued";
type Provider = "vercel" | "netlify" | "cloudflare" | "self-hosted";
type Environment = "dev" | "staging" | "prod";

interface LogEntry {
  timestamp: number;
  level: "info" | "warn" | "error" | "agent";
  message: string;
}

interface Domain {
  id: string;
  domain: string;
  target: string;
  env: Environment;
  ssl: boolean;
  sslExpiry: string;
  status: "active" | "pending" | "error";
  provider: Provider;
}

interface SSLCert {
  id: string;
  domain: string;
  issuer: string;
  validFrom: string;
  validTo: string;
  status: "valid" | "expiring" | "expired";
  autoRenew: boolean;
}

/* ─── constants ─── */
const PROVIDERS: { id: Provider; name: string; icon: string; color: string }[] = [
  { id: "vercel", name: "Vercel", icon: "▲", color: "#fff" },
  { id: "netlify", name: "Netlify", icon: "◆", color: "#00c7b7" },
  { id: "cloudflare", name: "Cloudflare", icon: "☁", color: "#f6821f" },
  { id: "self-hosted", name: "Self-Hosted", icon: "⬢", color: "#22d3ee" },
];

const ENV_COLORS: Record<Environment, string> = { dev: "#22c55e", staging: "#f59e0b", prod: "#ef4444" };
const STATUS_COLORS: Record<DeployStatus, string> = {
  building: "#f59e0b", deploying: "#3b82f6", live: "#22c55e",
  failed: "#ef4444", "rolled-back": "#a78bfa", queued: "#64748b",
};

const MOCK_LOGS: LogEntry[] = [
  { timestamp: Date.now() - 120000, level: "info", message: "Build started — npm ci" },
  { timestamp: Date.now() - 110000, level: "info", message: "Installing 847 packages..." },
  { timestamp: Date.now() - 95000, level: "info", message: "Dependencies installed in 14.2s" },
  { timestamp: Date.now() - 90000, level: "agent", message: "[DevOps Agent] Detected Vite config — using vite build" },
  { timestamp: Date.now() - 80000, level: "info", message: "Building for production..." },
  { timestamp: Date.now() - 60000, level: "info", message: "✓ 2560 modules transformed" },
  { timestamp: Date.now() - 55000, level: "warn", message: "Chunk size > 500KB — consider code splitting" },
  { timestamp: Date.now() - 50000, level: "agent", message: "[DevOps Agent] Build artifact: 1.15MB gzipped. Within budget." },
  { timestamp: Date.now() - 45000, level: "info", message: "Build completed in 38.4s" },
  { timestamp: Date.now() - 40000, level: "info", message: "Deploying to Vercel edge network..." },
  { timestamp: Date.now() - 20000, level: "agent", message: "[DevOps Agent] 12 edge regions propagated. SSL verified." },
  { timestamp: Date.now() - 10000, level: "info", message: "Deployment live at https://nexus-os.vercel.app" },
];

const INITIAL_DEPLOYMENTS: Deployment[] = [
  {
    id: "dep-1", project: "nexus-os-web", env: "prod", provider: "vercel", status: "live",
    version: "5.0.1", commit: "92eea41", branch: "main",
    url: "https://nexus-os.vercel.app", duration: 42, startedAt: Date.now() - 3600000,
    agent: "DevOps Agent", fuelCost: 35, logs: MOCK_LOGS,
  },
  {
    id: "dep-2", project: "nexus-os-web", env: "staging", provider: "vercel", status: "live",
    version: "5.0.2-rc.1", commit: "b75f033", branch: "staging",
    url: "https://staging.nexus-os.vercel.app", duration: 38, startedAt: Date.now() - 7200000,
    agent: "DevOps Agent", fuelCost: 30, logs: MOCK_LOGS.slice(0, 8),
  },
  {
    id: "dep-3", project: "nexus-os-api", env: "prod", provider: "cloudflare", status: "live",
    version: "5.0.0", commit: "b402779", branch: "main",
    url: "https://api.nexus-os.dev", duration: 28, startedAt: Date.now() - 86400000,
    agent: "DevOps Agent", fuelCost: 25, logs: MOCK_LOGS.slice(0, 6),
  },
  {
    id: "dep-4", project: "nexus-os-docs", env: "prod", provider: "netlify", status: "live",
    version: "3.2.0", commit: "6c9409f", branch: "main",
    url: "https://docs.nexus-os.dev", duration: 22, startedAt: Date.now() - 172800000,
    agent: "Content Agent", fuelCost: 15, logs: MOCK_LOGS.slice(0, 5),
  },
  {
    id: "dep-5", project: "nexus-os-web", env: "dev", provider: "self-hosted", status: "failed",
    version: "5.0.2-dev.3", commit: "a1b2c3d", branch: "feature/chat-hub",
    url: "https://dev.local:3000", duration: 15, startedAt: Date.now() - 1800000,
    agent: "Coder Agent", fuelCost: 10,
    logs: [
      ...MOCK_LOGS.slice(0, 6),
      { timestamp: Date.now() - 1700000, level: "error", message: "Error: Cannot find module './pages/MissingPage'" },
      { timestamp: Date.now() - 1690000, level: "agent", message: "[DevOps Agent] Build failed — missing import detected. Notifying Coder Agent." },
    ],
  },
  {
    id: "dep-6", project: "nexus-os-web", env: "prod", provider: "vercel", status: "rolled-back",
    version: "5.0.0-bad", commit: "deadbeef", branch: "main",
    url: "https://nexus-os.vercel.app", duration: 40, startedAt: Date.now() - 259200000,
    agent: "DevOps Agent", fuelCost: 35,
    logs: [
      ...MOCK_LOGS.slice(0, 10),
      { timestamp: Date.now() - 259100000, level: "error", message: "Health check failed — 502 on /api/health" },
      { timestamp: Date.now() - 259050000, level: "agent", message: "[DevOps Agent] Auto-rollback triggered. Reverting to v4.9.8." },
      { timestamp: Date.now() - 259000000, level: "info", message: "Rollback complete. v4.9.8 restored." },
    ],
  },
];

const INITIAL_DOMAINS: Domain[] = [
  { id: "dom-1", domain: "nexus-os.dev", target: "nexus-os.vercel.app", env: "prod", ssl: true, sslExpiry: "2026-09-15", status: "active", provider: "vercel" },
  { id: "dom-2", domain: "staging.nexus-os.dev", target: "staging.nexus-os.vercel.app", env: "staging", ssl: true, sslExpiry: "2026-09-15", status: "active", provider: "vercel" },
  { id: "dom-3", domain: "api.nexus-os.dev", target: "nexus-api.workers.dev", env: "prod", ssl: true, sslExpiry: "2026-08-20", status: "active", provider: "cloudflare" },
  { id: "dom-4", domain: "docs.nexus-os.dev", target: "nexus-docs.netlify.app", env: "prod", ssl: true, sslExpiry: "2026-07-01", status: "active", provider: "netlify" },
  { id: "dom-5", domain: "dev.nexus-os.local", target: "localhost:3000", env: "dev", ssl: false, sslExpiry: "", status: "pending", provider: "self-hosted" },
];

const INITIAL_CERTS: SSLCert[] = [
  { id: "cert-1", domain: "nexus-os.dev", issuer: "Let's Encrypt", validFrom: "2025-09-15", validTo: "2026-09-15", status: "valid", autoRenew: true },
  { id: "cert-2", domain: "*.nexus-os.dev", issuer: "Let's Encrypt", validFrom: "2025-09-15", validTo: "2026-09-15", status: "valid", autoRenew: true },
  { id: "cert-3", domain: "api.nexus-os.dev", issuer: "Cloudflare", validFrom: "2025-08-20", validTo: "2026-08-20", status: "valid", autoRenew: true },
  { id: "cert-4", domain: "docs.nexus-os.dev", issuer: "Let's Encrypt", validFrom: "2025-07-01", validTo: "2026-07-01", status: "expiring", autoRenew: true },
  { id: "cert-5", domain: "old.nexus-os.dev", issuer: "Let's Encrypt", validFrom: "2024-03-01", validTo: "2025-03-01", status: "expired", autoRenew: false },
];

const PROJECTS = ["nexus-os-web", "nexus-os-api", "nexus-os-docs", "nexus-os-landing"];

/* ─── component ─── */
export default function DeployPipeline() {
  const [view, setView] = useState<View>("deployments");
  const [deployments, setDeployments] = useState<Deployment[]>(INITIAL_DEPLOYMENTS);
  const [domains] = useState<Domain[]>(INITIAL_DOMAINS);
  const [certs, setCerts] = useState<SSLCert[]>(INITIAL_CERTS);
  const [selectedDeploy, setSelectedDeploy] = useState<string | null>("dep-1");
  const [fuelUsed, setFuelUsed] = useState(150);
  const [auditLog, setAuditLog] = useState<string[]>(["Pipeline initialized", "6 deployments loaded"]);
  const [filterEnv, setFilterEnv] = useState<Environment | "all">("all");
  const [filterProvider, setFilterProvider] = useState<Provider | "all">("all");

  // new deploy state
  const [showNewDeploy, setShowNewDeploy] = useState(false);
  const [newProject, setNewProject] = useState(PROJECTS[0]);
  const [newEnv, setNewEnv] = useState<Environment>("dev");
  const [newProvider, setNewProvider] = useState<Provider>("vercel");
  const [newBranch, setNewBranch] = useState("main");

  // HITL state
  const [hitlPending, setHitlPending] = useState<string | null>(null);

  const logAudit = useCallback((msg: string) => setAuditLog(prev => [msg, ...prev].slice(0, 50)), []);

  const activeDeploy = useMemo(() => deployments.find(d => d.id === selectedDeploy), [deployments, selectedDeploy]);

  const filteredDeploys = useMemo(() => {
    return deployments.filter(d => {
      if (filterEnv !== "all" && d.env !== filterEnv) return false;
      if (filterProvider !== "all" && d.provider !== filterProvider) return false;
      return true;
    }).sort((a, b) => b.startedAt - a.startedAt);
  }, [deployments, filterEnv, filterProvider]);

  const formatTime = (ts: number) => {
    const diff = Date.now() - ts;
    if (diff < 60000) return "now";
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
    return `${Math.floor(diff / 86400000)}d ago`;
  };

  /* ─── actions ─── */
  const startDeploy = useCallback(() => {
    if (newEnv === "prod") {
      setHitlPending("deploy-new");
      return;
    }
    executeDeploy();
  }, [newProject, newEnv, newProvider, newBranch]);

  const executeDeploy = useCallback(() => {
    const dep: Deployment = {
      id: `dep-${Date.now()}`, project: newProject, env: newEnv, provider: newProvider,
      status: "building", version: `5.0.${Math.floor(Math.random() * 20)}`,
      commit: Math.random().toString(16).slice(2, 9), branch: newBranch,
      url: "", duration: 0, startedAt: Date.now(),
      agent: "DevOps Agent", fuelCost: newEnv === "prod" ? 35 : newEnv === "staging" ? 25 : 15,
      logs: [{ timestamp: Date.now(), level: "info", message: "Build started..." }],
    };
    setDeployments(prev => [dep, ...prev]);
    setSelectedDeploy(dep.id);
    setShowNewDeploy(false);
    setFuelUsed(f => f + dep.fuelCost);
    logAudit(`Deploy started: ${newProject} → ${newEnv} (${newProvider})`);
    setHitlPending(null);

    // Simulate build → deploy → live
    setTimeout(() => {
      setDeployments(prev => prev.map(d => d.id === dep.id ? {
        ...d, status: "deploying" as DeployStatus,
        logs: [...d.logs,
          { timestamp: Date.now(), level: "info" as const, message: "Build completed in 18.3s" },
          { timestamp: Date.now(), level: "agent" as const, message: "[DevOps Agent] Artifact uploaded. Deploying..." },
        ],
      } : d));
    }, 2000);

    setTimeout(() => {
      const prov = PROVIDERS.find(p => p.id === newProvider);
      const url = newEnv === "prod" ? `https://nexus-os.${prov?.name.toLowerCase()}.app`
        : `https://${newEnv}.nexus-os.${prov?.name.toLowerCase()}.app`;
      setDeployments(prev => prev.map(d => d.id === dep.id ? {
        ...d, status: "live" as DeployStatus, url, duration: 34,
        logs: [...d.logs,
          { timestamp: Date.now(), level: "info" as const, message: `Deployment live at ${url}` },
          { timestamp: Date.now(), level: "agent" as const, message: "[DevOps Agent] Health check passed. All edge regions propagated." },
        ],
      } : d));
      logAudit(`Deploy live: ${newProject} → ${newEnv}`);
    }, 4500);
  }, [newProject, newEnv, newProvider, newBranch, logAudit]);

  const rollback = useCallback((depId: string) => {
    const dep = deployments.find(d => d.id === depId);
    if (!dep) return;
    if (dep.env === "prod") {
      setHitlPending(`rollback-${depId}`);
      return;
    }
    executeRollback(depId);
  }, [deployments]);

  const executeRollback = useCallback((depId: string) => {
    setDeployments(prev => prev.map(d => d.id === depId ? {
      ...d, status: "rolled-back" as DeployStatus,
      logs: [...d.logs,
        { timestamp: Date.now(), level: "agent" as const, message: "[DevOps Agent] Rollback initiated by user." },
        { timestamp: Date.now(), level: "info" as const, message: "Previous version restored." },
      ],
    } : d));
    setFuelUsed(f => f + 10);
    logAudit(`Rolled back deployment ${depId}`);
    setHitlPending(null);
  }, [logAudit]);

  const retryDeploy = useCallback((depId: string) => {
    setDeployments(prev => prev.map(d => d.id === depId ? {
      ...d, status: "building" as DeployStatus,
      logs: [...d.logs, { timestamp: Date.now(), level: "info" as const, message: "Retry initiated..." }],
    } : d));
    setFuelUsed(f => f + 15);
    logAudit(`Retry deployment ${depId}`);
    setTimeout(() => {
      setDeployments(prev => prev.map(d => d.id === depId ? {
        ...d, status: "live" as DeployStatus,
        logs: [...d.logs,
          { timestamp: Date.now(), level: "info" as const, message: "Build succeeded on retry." },
          { timestamp: Date.now(), level: "agent" as const, message: "[DevOps Agent] Issue resolved. Deployment live." },
        ],
      } : d));
    }, 3000);
  }, [logAudit]);

  const renewCert = useCallback((certId: string) => {
    setCerts(prev => prev.map(c => c.id === certId ? {
      ...c, status: "valid" as const, validFrom: "2026-03-10", validTo: "2027-03-10",
    } : c));
    setFuelUsed(f => f + 5);
    logAudit(`SSL certificate renewed: ${certId}`);
  }, [logAudit]);

  const envCounts = useMemo(() => {
    const counts = { dev: 0, staging: 0, prod: 0 };
    deployments.filter(d => d.status === "live").forEach(d => counts[d.env]++);
    return counts;
  }, [deployments]);

  /* ─── render ─── */
  return (
    <div className="dp-container">
      {/* ─── Sidebar ─── */}
      <aside className="dp-sidebar">
        <div className="dp-sidebar-header">
          <h2 className="dp-sidebar-title">Deploy Pipeline</h2>
          <button className="dp-new-btn" onClick={() => setShowNewDeploy(true)}>+ Deploy</button>
        </div>

        {/* views */}
        <div className="dp-views">
          {([["deployments", "⚡", "Deploys"], ["environments", "◎", "Envs"], ["domains", "⊕", "Domains"], ["certs", "🔒", "SSL"], ["logs", "▤", "Logs"]] as const).map(([id, icon, label]) => (
            <button key={id} className={`dp-view-btn ${view === id ? "active" : ""}`} onClick={() => setView(id)}>
              <span>{icon}</span> {label}
            </button>
          ))}
        </div>

        {/* env summary */}
        <div className="dp-env-summary">
          <div className="dp-section-header">Environments</div>
          {(["prod", "staging", "dev"] as Environment[]).map(env => (
            <div key={env} className="dp-env-card" onClick={() => { setFilterEnv(env); setView("deployments"); }}>
              <span className="dp-env-dot" style={{ background: ENV_COLORS[env] }} />
              <span className="dp-env-name">{env.toUpperCase()}</span>
              <span className="dp-env-count">{envCounts[env]} live</span>
            </div>
          ))}
        </div>

        {/* providers */}
        <div className="dp-providers">
          <div className="dp-section-header">Providers</div>
          {PROVIDERS.map(p => (
            <button key={p.id} className={`dp-provider-btn ${filterProvider === p.id ? "active" : ""}`} onClick={() => setFilterProvider(filterProvider === p.id ? "all" : p.id)}>
              <span style={{ color: p.color }}>{p.icon}</span> {p.name}
            </button>
          ))}
        </div>

        {/* audit */}
        <div className="dp-audit">
          <div className="dp-section-header">Activity</div>
          {auditLog.slice(0, 6).map((msg, i) => (
            <div key={i} className="dp-audit-entry">{msg}</div>
          ))}
        </div>
      </aside>

      {/* ─── Main ─── */}
      <div className="dp-main">

        {/* ═══ HITL APPROVAL ═══ */}
        {hitlPending && (
          <div className="dp-hitl-overlay">
            <div className="dp-hitl-dialog">
              <div className="dp-hitl-icon">⛨</div>
              <h3>HITL Approval Required</h3>
              <p className="dp-hitl-msg">
                {hitlPending === "deploy-new"
                  ? `Production deployment of ${newProject} requires human approval.`
                  : `Rolling back production deployment requires human approval.`}
              </p>
              <div className="dp-hitl-meta">
                <span>Environment: <strong style={{ color: ENV_COLORS.prod }}>PRODUCTION</strong></span>
                <span>Governance: Tier 2 — HITL mandatory</span>
              </div>
              <div className="dp-hitl-actions">
                <button className="dp-hitl-approve" onClick={() => {
                  if (hitlPending === "deploy-new") executeDeploy();
                  else executeRollback(hitlPending.replace("rollback-", ""));
                }}>✓ Approve & Execute</button>
                <button className="dp-hitl-deny" onClick={() => { setHitlPending(null); logAudit("HITL denied"); }}>✗ Deny</button>
              </div>
            </div>
          </div>
        )}

        {/* ═══ NEW DEPLOY MODAL ═══ */}
        {showNewDeploy && (
          <div className="dp-hitl-overlay">
            <div className="dp-new-dialog">
              <h3 className="dp-new-title">New Deployment</h3>
              <div className="dp-form-grid">
                <div className="dp-form-group">
                  <label>Project</label>
                  <select value={newProject} onChange={e => setNewProject(e.target.value)}>
                    {PROJECTS.map(p => <option key={p} value={p}>{p}</option>)}
                  </select>
                </div>
                <div className="dp-form-group">
                  <label>Environment</label>
                  <select value={newEnv} onChange={e => setNewEnv(e.target.value as Environment)}>
                    <option value="dev">Development</option>
                    <option value="staging">Staging</option>
                    <option value="prod">Production</option>
                  </select>
                </div>
                <div className="dp-form-group">
                  <label>Provider</label>
                  <select value={newProvider} onChange={e => setNewProvider(e.target.value as Provider)}>
                    {PROVIDERS.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}
                  </select>
                </div>
                <div className="dp-form-group">
                  <label>Branch</label>
                  <input value={newBranch} onChange={e => setNewBranch(e.target.value)} placeholder="main" />
                </div>
              </div>
              {newEnv === "prod" && (
                <div className="dp-form-warn">⛨ Production deploys require HITL approval (Tier 2)</div>
              )}
              <div className="dp-form-cost">
                Fuel cost: ⚡ {newEnv === "prod" ? 35 : newEnv === "staging" ? 25 : 15}
              </div>
              <div className="dp-form-actions">
                <button className="dp-form-deploy" onClick={startDeploy}>🚀 Deploy</button>
                <button className="dp-form-cancel" onClick={() => setShowNewDeploy(false)}>Cancel</button>
              </div>
            </div>
          </div>
        )}

        {/* ═══ DEPLOYMENTS VIEW ═══ */}
        {view === "deployments" && (
          <div className="dp-deploys">
            <div className="dp-deploys-header">
              <h3 className="dp-view-title">⚡ Deployments</h3>
              <div className="dp-filters">
                <select value={filterEnv} onChange={e => setFilterEnv(e.target.value as Environment | "all")}>
                  <option value="all">All Envs</option>
                  <option value="dev">Dev</option>
                  <option value="staging">Staging</option>
                  <option value="prod">Prod</option>
                </select>
                <button className="dp-new-btn" onClick={() => setShowNewDeploy(true)}>+ New Deploy</button>
              </div>
            </div>

            <div className="dp-deploys-grid">
              {/* list */}
              <div className="dp-deploy-list">
                {filteredDeploys.map(dep => {
                  const prov = PROVIDERS.find(p => p.id === dep.provider);
                  return (
                    <div key={dep.id} className={`dp-deploy-item ${selectedDeploy === dep.id ? "active" : ""}`} onClick={() => setSelectedDeploy(dep.id)}>
                      <div className="dp-deploy-status">
                        <span className="dp-status-dot" style={{ background: STATUS_COLORS[dep.status] }} />
                        <span className="dp-status-text" style={{ color: STATUS_COLORS[dep.status] }}>{dep.status}</span>
                      </div>
                      <div className="dp-deploy-info">
                        <div className="dp-deploy-project">{dep.project}</div>
                        <div className="dp-deploy-meta">
                          <span className="dp-deploy-env" style={{ color: ENV_COLORS[dep.env] }}>{dep.env}</span>
                          <span style={{ color: prov?.color }}>{prov?.icon}</span>
                          <span>v{dep.version}</span>
                          <span>{dep.commit.slice(0, 7)}</span>
                          <span>{formatTime(dep.startedAt)}</span>
                        </div>
                      </div>
                      <div className="dp-deploy-actions">
                        {dep.status === "live" && (
                          <button className="dp-act-btn dp-act-rollback" onClick={e => { e.stopPropagation(); rollback(dep.id); }} title="Rollback">↩</button>
                        )}
                        {dep.status === "failed" && (
                          <button className="dp-act-btn dp-act-retry" onClick={e => { e.stopPropagation(); retryDeploy(dep.id); }} title="Retry">↻</button>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>

              {/* detail */}
              {activeDeploy && (
                <div className="dp-deploy-detail">
                  <div className="dp-detail-header">
                    <h4>{activeDeploy.project}</h4>
                    <span className="dp-status-badge" style={{ background: STATUS_COLORS[activeDeploy.status] + "22", color: STATUS_COLORS[activeDeploy.status], borderColor: STATUS_COLORS[activeDeploy.status] + "44" }}>
                      {activeDeploy.status}
                    </span>
                  </div>
                  <div className="dp-detail-grid">
                    <div className="dp-detail-row"><span>Version</span><span>v{activeDeploy.version}</span></div>
                    <div className="dp-detail-row"><span>Commit</span><span className="dp-mono">{activeDeploy.commit}</span></div>
                    <div className="dp-detail-row"><span>Branch</span><span>{activeDeploy.branch}</span></div>
                    <div className="dp-detail-row"><span>Environment</span><span style={{ color: ENV_COLORS[activeDeploy.env] }}>{activeDeploy.env.toUpperCase()}</span></div>
                    <div className="dp-detail-row"><span>Provider</span><span>{PROVIDERS.find(p => p.id === activeDeploy.provider)?.name}</span></div>
                    <div className="dp-detail-row"><span>Duration</span><span>{activeDeploy.duration}s</span></div>
                    <div className="dp-detail-row"><span>Agent</span><span>⬢ {activeDeploy.agent}</span></div>
                    <div className="dp-detail-row"><span>Fuel</span><span>⚡ {activeDeploy.fuelCost}</span></div>
                    {activeDeploy.url && <div className="dp-detail-row"><span>URL</span><span className="dp-url">{activeDeploy.url}</span></div>}
                  </div>

                  {/* deploy logs */}
                  <div className="dp-detail-logs-header">Deploy Logs</div>
                  <div className="dp-detail-logs">
                    {activeDeploy.logs.map((log, i) => (
                      <div key={i} className={`dp-log-line dp-log-${log.level}`}>
                        <span className="dp-log-time">{new Date(log.timestamp).toLocaleTimeString()}</span>
                        <span className="dp-log-level">{log.level === "agent" ? "⬢" : log.level.toUpperCase()}</span>
                        <span className="dp-log-msg">{log.message}</span>
                      </div>
                    ))}
                  </div>

                  <div className="dp-detail-actions">
                    {activeDeploy.status === "live" && (
                      <button className="dp-btn-rollback" onClick={() => rollback(activeDeploy.id)}>↩ Rollback</button>
                    )}
                    {activeDeploy.status === "failed" && (
                      <button className="dp-btn-retry" onClick={() => retryDeploy(activeDeploy.id)}>↻ Retry</button>
                    )}
                  </div>
                </div>
              )}
            </div>
          </div>
        )}

        {/* ═══ ENVIRONMENTS VIEW ═══ */}
        {view === "environments" && (
          <div className="dp-envs">
            <h3 className="dp-view-title">◎ Environment Overview</h3>
            <div className="dp-envs-grid">
              {(["prod", "staging", "dev"] as Environment[]).map(env => {
                const envDeps = deployments.filter(d => d.env === env);
                const liveDeps = envDeps.filter(d => d.status === "live");
                const latest = envDeps.sort((a, b) => b.startedAt - a.startedAt)[0];
                return (
                  <div key={env} className="dp-env-panel">
                    <div className="dp-env-panel-header" style={{ borderLeftColor: ENV_COLORS[env] }}>
                      <span className="dp-env-dot" style={{ background: ENV_COLORS[env] }} />
                      <h4>{env.toUpperCase()}</h4>
                      <span className="dp-env-live-count">{liveDeps.length} live</span>
                    </div>
                    <div className="dp-env-stats">
                      <div className="dp-env-stat"><span>Total deploys</span><span>{envDeps.length}</span></div>
                      <div className="dp-env-stat"><span>Active services</span><span>{liveDeps.length}</span></div>
                      <div className="dp-env-stat"><span>Latest version</span><span>{latest ? `v${latest.version}` : "—"}</span></div>
                      <div className="dp-env-stat"><span>Last deploy</span><span>{latest ? formatTime(latest.startedAt) : "—"}</span></div>
                    </div>
                    <div className="dp-env-services">
                      {liveDeps.map(d => (
                        <div key={d.id} className="dp-env-service" onClick={() => { setSelectedDeploy(d.id); setView("deployments"); }}>
                          <span style={{ color: PROVIDERS.find(p => p.id === d.provider)?.color }}>{PROVIDERS.find(p => p.id === d.provider)?.icon}</span>
                          <span>{d.project}</span>
                          <span className="dp-env-svc-ver">v{d.version}</span>
                        </div>
                      ))}
                    </div>
                    {env === "prod" && <div className="dp-env-gov">⛨ Tier 2 HITL required for deploys & rollbacks</div>}
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* ═══ DOMAINS VIEW ═══ */}
        {view === "domains" && (
          <div className="dp-domains">
            <h3 className="dp-view-title">⊕ Domain Management</h3>
            <div className="dp-table">
              <div className="dp-table-header">
                <span>Domain</span><span>Target</span><span>Env</span><span>SSL</span><span>Provider</span><span>Status</span>
              </div>
              {domains.map(dom => (
                <div key={dom.id} className="dp-table-row">
                  <span className="dp-domain-name">{dom.domain}</span>
                  <span className="dp-mono dp-domain-target">{dom.target}</span>
                  <span style={{ color: ENV_COLORS[dom.env] }}>{dom.env.toUpperCase()}</span>
                  <span>{dom.ssl ? "🔒" : "⚠"}</span>
                  <span>{PROVIDERS.find(p => p.id === dom.provider)?.name}</span>
                  <span className={`dp-domain-status dp-domain-${dom.status}`}>{dom.status}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* ═══ CERTS VIEW ═══ */}
        {view === "certs" && (
          <div className="dp-certs">
            <h3 className="dp-view-title">🔒 SSL Certificates</h3>
            <div className="dp-cert-grid">
              {certs.map(cert => (
                <div key={cert.id} className={`dp-cert-card dp-cert-${cert.status}`}>
                  <div className="dp-cert-header">
                    <span className="dp-cert-domain">{cert.domain}</span>
                    <span className={`dp-cert-status dp-cert-st-${cert.status}`}>{cert.status}</span>
                  </div>
                  <div className="dp-cert-details">
                    <div className="dp-cert-row"><span>Issuer</span><span>{cert.issuer}</span></div>
                    <div className="dp-cert-row"><span>Valid from</span><span>{cert.validFrom}</span></div>
                    <div className="dp-cert-row"><span>Valid to</span><span>{cert.validTo}</span></div>
                    <div className="dp-cert-row"><span>Auto-renew</span><span>{cert.autoRenew ? "✓ On" : "✗ Off"}</span></div>
                  </div>
                  {(cert.status === "expiring" || cert.status === "expired") && (
                    <button className="dp-cert-renew" onClick={() => renewCert(cert.id)}>↻ Renew Now (⚡ 5)</button>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* ═══ LOGS VIEW ═══ */}
        {view === "logs" && (
          <div className="dp-logs-view">
            <h3 className="dp-view-title">▤ All Deploy Logs</h3>
            <div className="dp-logs-list">
              {deployments.sort((a, b) => b.startedAt - a.startedAt).flatMap(dep =>
                dep.logs.map((log, i) => ({
                  ...log,
                  project: dep.project,
                  env: dep.env,
                  depId: dep.id,
                  key: `${dep.id}-${i}`,
                }))
              ).sort((a, b) => b.timestamp - a.timestamp).slice(0, 60).map(log => (
                <div key={log.key} className={`dp-log-line dp-log-${log.level}`} onClick={() => { setSelectedDeploy(log.depId); setView("deployments"); }}>
                  <span className="dp-log-time">{new Date(log.timestamp).toLocaleTimeString()}</span>
                  <span className="dp-log-proj">{log.project}</span>
                  <span className="dp-log-env" style={{ color: ENV_COLORS[log.env] }}>{log.env}</span>
                  <span className="dp-log-level">{log.level === "agent" ? "⬢" : log.level.toUpperCase()}</span>
                  <span className="dp-log-msg">{log.message}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>

      {/* ─── Status Bar ─── */}
      <div className="dp-status-bar">
        <span className="dp-status-item">{deployments.filter(d => d.status === "live").length} live</span>
        <span className="dp-status-item">{deployments.filter(d => d.status === "building" || d.status === "deploying").length} in progress</span>
        <span className="dp-status-item">{deployments.filter(d => d.status === "failed").length} failed</span>
        <span className="dp-status-item">{domains.length} domains</span>
        <span className="dp-status-item">{certs.filter(c => c.status === "valid").length}/{certs.length} certs valid</span>
        <span className="dp-status-item dp-status-right">⚡ {fuelUsed} fuel</span>
      </div>
    </div>
  );
}

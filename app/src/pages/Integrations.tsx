import { useState, useEffect, useCallback } from "react";
import {
  MessageSquare,
  Users,
  Ticket,
  Wrench,
  Github,
  Gitlab,
  Webhook,
  RefreshCw,
  Settings,
  Zap,
  CheckCircle,
  XCircle,
  Activity,
  PlugZap,
} from "lucide-react";
import { integrationsList, integrationTest, integrationConfigure } from "../api/backend";
import "./integrations.css";

// ── Types ──────────────────────────────────────────────────────────

interface Provider {
  id: string;
  name: string;
  provider_type: string;
  description: string;
  icon: string;
  category: string;
  configured: boolean;
  healthy: boolean;
  events: string[];
}

interface TestResult {
  provider: string;
  success: boolean;
  message: string;
}

// ── Icon Map ───────────────────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const ICON_MAP: Record<string, any> = {
  MessageSquare,
  Users,
  Ticket,
  Wrench,
  Github,
  Gitlab,
  Webhook,
};

type Tab = "marketplace" | "routing" | "health" | "configure";
type Category = "all" | "messaging" | "ticketing" | "devops" | "custom";

const CATEGORIES: { id: Category; label: string }[] = [
  { id: "all", label: "All" },
  { id: "messaging", label: "Messaging" },
  { id: "ticketing", label: "Ticketing" },
  { id: "devops", label: "DevOps" },
  { id: "custom", label: "Custom" },
];

const EVENT_KINDS = [
  "agent_started",
  "agent_completed",
  "agent_error",
  "hitl_required",
  "hitl_decision",
  "security_event",
  "fuel_exhausted",
  "genome_evolved",
  "audit_chain_break",
  "backup_completed",
  "system_alert",
];

// ── Component ──────────────────────────────────────────────────────

export default function Integrations() {
  const [tab, setTab] = useState<Tab>("marketplace");
  const [category, setCategory] = useState<Category>("all");
  const [providers, setProviders] = useState<Provider[]>([]);
  const [testing, setTesting] = useState<string | null>(null);
  const [testResults, setTestResults] = useState<Record<string, TestResult>>({});
  const [configuring, setConfiguring] = useState<string | null>(null);
  const [configValues, setConfigValues] = useState<Record<string, string>>({});
  const [loadError, setLoadError] = useState<string | null>(null);

  const load = useCallback(() => {
    setLoadError(null);
    integrationsList()
      .then((data: Provider[]) => { setProviders(data); setLoadError(null); })
      .catch(() => { setProviders([]); setLoadError("Failed to load integrations — check backend connection."); });
  }, []);

  useEffect(() => { load(); }, [load]);

  const filtered = category === "all"
    ? providers
    : providers.filter((p) => p.category === category);

  const handleTest = async (id: string) => {
    setTesting(id);
    try {
      const result: TestResult = await integrationTest(id);
      setTestResults((prev) => ({ ...prev, [id]: result }));
    } catch {
      setTestResults((prev) => ({
        ...prev,
        [id]: { provider: id, success: false, message: "Connection test failed" },
      }));
    }
    setTesting(null);
  };

  const handleConfigure = async (id: string) => {
    try {
      await integrationConfigure(id, configValues as unknown as Record<string, unknown>);
      setConfiguring(null);
      setConfigValues({});
      load();
    } catch {
      // keep panel open on error
    }
  };

  // ── Render helpers ─────────────────────────────────────────────

  const renderIcon = (iconName: string) => {
    const Icon = ICON_MAP[iconName] || PlugZap;
    return <Icon size={20} />;
  };

  const renderBadge = (cat: string) => (
    <span className={`intg-badge intg-badge--${cat}`}>{cat}</span>
  );

  const renderCard = (p: Provider) => (
    <div key={p.id} className={`intg-card ${p.configured ? "intg-card--configured" : ""}`}>
      <div className="intg-card-header">
        <div className="intg-card-icon">{renderIcon(p.icon)}</div>
        <div>
          <div className="intg-card-title">{p.name}</div>
          <div className="intg-card-category">{renderBadge(p.category)}</div>
        </div>
      </div>
      <div className="intg-card-desc">{p.description}</div>
      <div className="intg-card-footer">
        <div className="intg-status">
          <span className={`intg-dot ${p.configured ? "intg-dot--green" : "intg-dot--red"}`} />
          {p.configured ? "Configured" : "Not configured"}
        </div>
        <div style={{ display: "flex", gap: "0.4rem" }}>
          <button
            className="intg-btn intg-btn--sm"
            onClick={() => handleTest(p.id)}
            disabled={testing === p.id}
          >
            {testing === p.id ? <RefreshCw size={12} className="spin" /> : <Zap size={12} />}
            {" "}Test
          </button>
          <button
            className="intg-btn intg-btn--sm intg-btn--primary"
            onClick={() => setConfiguring(configuring === p.id ? null : p.id)}
          >
            <Settings size={12} /> Configure
          </button>
        </div>
      </div>
      {testResults[p.id] && (
        <div className={`intg-test-result ${testResults[p.id].success ? "intg-test-result--pass" : "intg-test-result--fail"}`}>
          {testResults[p.id].success ? <CheckCircle size={14} /> : <XCircle size={14} />}
          {" "}{testResults[p.id].message}
        </div>
      )}
      {configuring === p.id && (
        <div className="intg-config-panel" style={{ marginTop: "0.5rem", padding: "1rem" }}>
          <h3>Configure {p.name}</h3>
          {getConfigFields(p.id).map((field) => (
            <div className="intg-config-row" key={field.key}>
              <span className="intg-config-label">{field.label}</span>
              <input
                className="intg-input"
                type={field.secret ? "password" : "text"}
                placeholder={field.placeholder}
                value={configValues[field.key] || ""}
                onChange={(e) =>
                  setConfigValues((prev) => ({ ...prev, [field.key]: e.target.value }))
                }
              />
            </div>
          ))}
          <div style={{ marginTop: "0.75rem", display: "flex", gap: "0.5rem" }}>
            <button className="intg-btn intg-btn--primary" onClick={() => handleConfigure(p.id)}>
              Save Configuration
            </button>
            <button className="intg-btn" onClick={() => setConfiguring(null)}>
              Cancel
            </button>
          </div>
        </div>
      )}
    </div>
  );

  const renderMarketplace = () => (
    <>
      <div className="intg-filters">
        {CATEGORIES.map((c) => (
          <button
            key={c.id}
            className={`intg-filter-btn ${category === c.id ? "intg-filter-btn--active" : ""}`}
            onClick={() => setCategory(c.id)}
          >
            {c.label}
          </button>
        ))}
      </div>
      {loadError ? (
        <div className="intg-config-panel" style={{ textAlign: "center", padding: "2rem" }}>
          <XCircle size={24} style={{ color: "var(--nexus-red, #ef4444)", marginBottom: "0.5rem" }} />
          <p style={{ color: "var(--text-secondary)", marginBottom: "1rem" }}>{loadError}</p>
          <button className="intg-btn intg-btn--primary" onClick={load}>
            <RefreshCw size={14} /> Retry
          </button>
        </div>
      ) : (
        <div className="intg-grid">{filtered.map(renderCard)}</div>
      )}
    </>
  );

  const renderRouting = () => (
    <div className="intg-config-panel">
      <h3>Event Routing Matrix</h3>
      <p style={{ fontSize: "0.82rem", color: "var(--text-secondary)", marginBottom: "1rem" }}>
        Which events are forwarded to which integrations.
      </p>
      <div style={{ overflowX: "auto" }}>
        <table className="intg-events-table">
          <thead>
            <tr>
              <th>Event</th>
              {providers.map((p) => (
                <th key={p.id}>{p.name}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {EVENT_KINDS.map((ek) => (
              <tr key={ek}>
                <td style={{ fontFamily: "var(--font-mono, monospace)", fontSize: "0.78rem" }}>{ek}</td>
                {providers.map((p) => (
                  <td key={p.id} style={{ textAlign: "center" }}>
                    {p.events.includes(ek) || p.events.includes("*") ? (
                      <CheckCircle size={14} style={{ color: "#4af7d3" }} />
                    ) : (
                      <span style={{ color: "rgba(150,150,150,0.3)" }}>-</span>
                    )}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );

  const renderHealth = () => (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1rem" }}>
        <h3 style={{ fontFamily: "var(--font-display)", fontSize: "1rem", margin: 0 }}>Provider Health Status</h3>
        <button className="intg-btn intg-btn--sm" onClick={load}>
          <RefreshCw size={12} /> Refresh
        </button>
      </div>
      <div className="intg-health-grid">
        {providers.map((p) => (
          <div key={p.id} className={`intg-health-card ${p.healthy ? "intg-health-card--ok" : "intg-health-card--err"}`}>
            <div className="intg-card-icon">{renderIcon(p.icon)}</div>
            <div style={{ fontWeight: 600, fontSize: "0.9rem" }}>{p.name}</div>
            <div className="intg-status">
              <span className={`intg-dot ${p.healthy ? "intg-dot--green" : "intg-dot--red"}`} />
              {p.healthy ? "Healthy" : "Unreachable"}
            </div>
            <button
              className="intg-btn intg-btn--sm"
              onClick={() => handleTest(p.id)}
              disabled={testing === p.id}
            >
              {testing === p.id ? "Testing..." : "Test Connection"}
            </button>
          </div>
        ))}
      </div>
    </div>
  );

  // ── Main Render ────────────────────────────────────────────────

  return (
    <div className="intg-shell">
      <h1><PlugZap size={22} style={{ verticalAlign: "middle", marginRight: 8 }} />Integrations</h1>
      <div className="intg-subtitle">
        Connect Nexus OS to enterprise tools &mdash; all integrations are capability-gated, PII-redacted, rate-limited, and audited.
      </div>

      <div className="intg-tabs">
        {(
          [
            { id: "marketplace" as Tab, label: "Marketplace", icon: PlugZap },
            { id: "routing" as Tab, label: "Event Routing", icon: Activity },
            { id: "health" as Tab, label: "Health Status", icon: Activity },
          ] as const
        ).map((t) => (
          <button
            key={t.id}
            className={`intg-tab ${tab === t.id ? "intg-tab--active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            <t.icon size={13} style={{ verticalAlign: "middle", marginRight: 4 }} />
            {t.label}
          </button>
        ))}
      </div>

      {tab === "marketplace" && renderMarketplace()}
      {tab === "routing" && renderRouting()}
      {tab === "health" && renderHealth()}
    </div>
  );
}

// ── Config Fields per Provider ─────────────────────────────────────

interface ConfigField {
  key: string;
  label: string;
  placeholder: string;
  secret?: boolean;
}

function getConfigFields(providerId: string): ConfigField[] {
  switch (providerId) {
    case "slack":
      return [
        { key: "webhook_url", label: "Webhook URL", placeholder: "https://hooks.slack.com/services/...", secret: true },
        { key: "bot_token", label: "Bot Token (optional)", placeholder: "xoxb-...", secret: true },
        { key: "default_channel", label: "Default Channel", placeholder: "#nexus-alerts" },
      ];
    case "teams":
      return [
        { key: "webhook_url", label: "Webhook URL", placeholder: "https://outlook.office.com/webhook/...", secret: true },
      ];
    case "jira":
      return [
        { key: "base_url", label: "Base URL", placeholder: "https://your-org.atlassian.net" },
        { key: "email", label: "Email", placeholder: "admin@company.com" },
        { key: "api_token", label: "API Token", placeholder: "ATT...", secret: true },
        { key: "default_project", label: "Default Project", placeholder: "NEXUS" },
      ];
    case "servicenow":
      return [
        { key: "instance_url", label: "Instance URL", placeholder: "https://your-instance.service-now.com" },
        { key: "username", label: "Username", placeholder: "admin" },
        { key: "password", label: "Password", placeholder: "********", secret: true },
      ];
    case "github":
      return [
        { key: "token", label: "Personal Access Token", placeholder: "ghp_...", secret: true },
        { key: "default_owner", label: "Default Owner", placeholder: "nexaiceo" },
        { key: "default_repo", label: "Default Repo", placeholder: "nexus-os" },
      ];
    case "gitlab":
      return [
        { key: "base_url", label: "Base URL", placeholder: "https://gitlab.com" },
        { key: "token", label: "Access Token", placeholder: "glpat-...", secret: true },
        { key: "default_project_id", label: "Default Project", placeholder: "nexaiceo/nexus-os" },
      ];
    case "webhook":
      return [
        { key: "url", label: "Webhook URL", placeholder: "https://your-api.com/webhook" },
        { key: "method", label: "HTTP Method", placeholder: "POST" },
        { key: "secret", label: "HMAC Secret (optional)", placeholder: "your-secret", secret: true },
      ];
    default:
      return [];
  }
}


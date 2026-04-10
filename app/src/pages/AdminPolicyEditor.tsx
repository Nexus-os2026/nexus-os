import { useCallback, useEffect, useState } from "react";
import {
  adminPolicyGet,
  adminPolicyUpdate,
  adminPolicyHistory,
} from "../api/backend";
import "./admin.css";

interface Policy {
  scope: string;
  max_autonomy_level: number;
  allowed_providers: string[];
  fuel_limit_per_agent: number;
  fuel_limit_per_workspace: number;
  require_hitl_above_tier: number;
  allow_self_modify: boolean;
  allow_internet_access: boolean;
  pii_redaction_enabled: boolean;
}

interface PolicyChange {
  id: string;
  timestamp: string;
  user: string;
  scope: string;
  field: string;
  old_value: string;
  new_value: string;
}

type Template = "strict" | "balanced" | "permissive";

const TEMPLATES: Record<Template, Policy> = {
  strict: {
    scope: "global",
    max_autonomy_level: 1,
    allowed_providers: ["ollama"],
    fuel_limit_per_agent: 1000,
    fuel_limit_per_workspace: 10000,
    require_hitl_above_tier: 0,
    allow_self_modify: false,
    allow_internet_access: false,
    pii_redaction_enabled: true,
  },
  balanced: {
    scope: "global",
    max_autonomy_level: 3,
    allowed_providers: ["ollama", "openai", "claude"],
    fuel_limit_per_agent: 10000,
    fuel_limit_per_workspace: 100000,
    require_hitl_above_tier: 1,
    allow_self_modify: false,
    allow_internet_access: true,
    pii_redaction_enabled: true,
  },
  permissive: {
    scope: "global",
    max_autonomy_level: 5,
    allowed_providers: ["ollama", "openai", "claude", "deepseek", "gemini", "nvidia"],
    fuel_limit_per_agent: 100000,
    fuel_limit_per_workspace: 1000000,
    require_hitl_above_tier: 2,
    allow_self_modify: true,
    allow_internet_access: true,
    pii_redaction_enabled: true,
  },
};

const AUTONOMY_LABELS = ["L0 Inert", "L1 Suggest", "L2 Approve", "L3 Report", "L4 Bounded", "L5 Full"];

export default function AdminPolicyEditor() {
  const [tab, setTab] = useState<"editor" | "templates" | "history">("editor");
  const [policy, setPolicy] = useState<Policy>(TEMPLATES.balanced);
  const [scope, setScope] = useState("global");
  const [history, setHistory] = useState<PolicyChange[]>([]);
  const [saving, setSaving] = useState(false);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const data = await adminPolicyGet(scope);
      setPolicy(data);
      const hist = await adminPolicyHistory(scope);
      setHistory(hist);
    } catch {
      // fallback: keep current state (balanced template on first load)
    } finally {
      setLoading(false);
    }
  }, [scope]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleSave = async () => {
    setSaving(true);
    try {
      await adminPolicyUpdate(scope, policy as unknown as Record<string, unknown>);
    } catch {
      /* no-op */
    } finally {
      setSaving(false);
    }
  };

  const applyTemplate = (t: Template) => {
    setPolicy({ ...TEMPLATES[t], scope });
  };

  return (
    <div className="admin-shell">
      <h1>Policy Editor</h1>
      <p className="admin-subtitle">Configure global and workspace-level governance policies</p>

      <div className="admin-tabs">
        {(["editor", "templates", "history"] as const).map((t) => (
          <button type="button" key={t} className={`admin-tab ${tab === t ? "admin-tab--active" : ""}`} onClick={() => setTab(t)}>
            {t === "editor" ? "Edit Policy" : t === "templates" ? "Templates" : "History"}
          </button>
        ))}
      </div>

      {tab === "templates" && (
        <div className="admin-grid-3">
          {(Object.keys(TEMPLATES) as Template[]).map((t) => (
            <div
              key={t}
              className={`admin-policy-card ${policy.max_autonomy_level === TEMPLATES[t].max_autonomy_level ? "admin-policy-card--active" : ""}`}
              onClick={() => applyTemplate(t)}
            >
              <div style={{ fontSize: "1rem", fontWeight: 600, marginBottom: "0.4rem", color: "var(--text-primary)", textTransform: "capitalize" }}>{t}</div>
              <div style={{ fontSize: "0.78rem", color: "var(--text-secondary)", lineHeight: 1.5 }}>
                Max autonomy: {AUTONOMY_LABELS[TEMPLATES[t].max_autonomy_level]}<br />
                Providers: {TEMPLATES[t].allowed_providers.join(", ")}<br />
                HITL above Tier {TEMPLATES[t].require_hitl_above_tier}<br />
                Self-modify: {TEMPLATES[t].allow_self_modify ? "Yes" : "No"}<br />
                Internet: {TEMPLATES[t].allow_internet_access ? "Yes" : "No"}
              </div>
            </div>
          ))}
        </div>
      )}

      {tab === "editor" && (
        <div className="admin-card">
          <div style={{ display: "flex", gap: "0.6rem", marginBottom: "1rem", alignItems: "center" }}>
            <label style={{ fontSize: "0.78rem", color: "var(--text-secondary)" }}>Scope:</label>
            <select className="admin-select" value={scope} onChange={(e) => setScope(e.target.value)}>
              <option value="global">Global</option>
              <option value="workspace:default">workspace:default</option>
              <option value="workspace:prod">workspace:prod</option>
              <option value="workspace:staging">workspace:staging</option>
            </select>
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "1rem" }}>
            <FieldRow label="Max Autonomy Level">
              <select className="admin-select" value={policy.max_autonomy_level} onChange={(e) => setPolicy((p) => ({ ...p, max_autonomy_level: Number(e.target.value) }))}>
                {AUTONOMY_LABELS.map((l, i) => <option key={i} value={i}>{l}</option>)}
              </select>
            </FieldRow>

            <FieldRow label="Fuel Limit / Agent">
              <input className="admin-input" type="number" value={policy.fuel_limit_per_agent} onChange={(e) => setPolicy((p) => ({ ...p, fuel_limit_per_agent: Number(e.target.value) }))} />
            </FieldRow>

            <FieldRow label="Fuel Limit / Workspace">
              <input className="admin-input" type="number" value={policy.fuel_limit_per_workspace} onChange={(e) => setPolicy((p) => ({ ...p, fuel_limit_per_workspace: Number(e.target.value) }))} />
            </FieldRow>

            <FieldRow label="HITL Required Above Tier">
              <select className="admin-select" value={policy.require_hitl_above_tier} onChange={(e) => setPolicy((p) => ({ ...p, require_hitl_above_tier: Number(e.target.value) }))}>
                {[0, 1, 2, 3].map((t) => <option key={t} value={t}>Tier {t}</option>)}
              </select>
            </FieldRow>

            <FieldRow label="Allowed Providers">
              <input className="admin-input" value={policy.allowed_providers.join(", ")} onChange={(e) => setPolicy((p) => ({ ...p, allowed_providers: e.target.value.split(",").map((s) => s.trim()).filter(Boolean) }))} />
            </FieldRow>

            <FieldRow label="Toggles">
              <div style={{ display: "flex", gap: "1rem", fontSize: "0.82rem" }}>
                <label style={{ cursor: "pointer" }}>
                  <input type="checkbox" checked={policy.allow_self_modify} onChange={(e) => setPolicy((p) => ({ ...p, allow_self_modify: e.target.checked }))} />
                  {" "}Self-Modify
                </label>
                <label style={{ cursor: "pointer" }}>
                  <input type="checkbox" checked={policy.allow_internet_access} onChange={(e) => setPolicy((p) => ({ ...p, allow_internet_access: e.target.checked }))} />
                  {" "}Internet
                </label>
                <label style={{ cursor: "pointer" }}>
                  <input type="checkbox" checked={policy.pii_redaction_enabled} onChange={(e) => setPolicy((p) => ({ ...p, pii_redaction_enabled: e.target.checked }))} />
                  {" "}PII Redaction
                </label>
              </div>
            </FieldRow>
          </div>

          <div style={{ marginTop: "1rem", display: "flex", justifyContent: "flex-end" }}>
            <button type="button" className="admin-btn admin-btn--accent" onClick={() => void handleSave()} disabled={saving || loading}>
              {saving ? "Saving..." : "Save Policy"}
            </button>
          </div>
        </div>
      )}

      {tab === "history" && (
        <div className="admin-card">
          <table className="admin-table">
            <thead>
              <tr>
                <th>Timestamp</th>
                <th>User</th>
                <th>Scope</th>
                <th>Field</th>
                <th>Old Value</th>
                <th>New Value</th>
              </tr>
            </thead>
            <tbody>
              {history.length === 0 && <tr><td colSpan={6} className="admin-empty">No policy changes recorded</td></tr>}
              {history.map((h) => (
                <tr key={h.id}>
                  <td>{new Date(h.timestamp).toLocaleString()}</td>
                  <td style={{ color: "var(--text-primary)" }}>{h.user}</td>
                  <td>{h.scope}</td>
                  <td style={{ fontFamily: "var(--font-mono)", fontSize: "0.75rem" }}>{h.field}</td>
                  <td style={{ color: "var(--nexus-danger)" }}>{h.old_value}</td>
                  <td style={{ color: "var(--nexus-accent)" }}>{h.new_value}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function FieldRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <label style={{ display: "block", fontSize: "0.72rem", color: "var(--text-muted)", marginBottom: "0.25rem", textTransform: "uppercase", letterSpacing: "0.06em" }}>
        {label}
      </label>
      {children}
    </div>
  );
}

import { useCallback, useEffect, useState } from "react";
import {
  workspaceList,
  workspaceCreate,
  workspaceGet,
  workspaceAddMember,
  workspaceRemoveMember,
  workspaceSetPolicy,
  workspaceUsage,
} from "../api/backend";
import "./admin.css";

// ── Types ──────────────────────────────────────────────────────────────────

export interface WorkspaceMember {
  user_id: string;
  role: string;
  joined_at?: string;
}

export interface WorkspacePolicy {
  max_autonomy_level: number;
  allowed_capabilities: string[];
  require_hitl_tier: string;
  data_isolation: string;
}

export interface WorkspaceSummary {
  id: string;
  name: string;
  member_count: number;
  agent_limit: number;
  fuel_budget: number;
  data_isolation: string;
  created_at: string;
  status: "active" | "suspended" | "archived";
}

export interface WorkspaceDetail extends WorkspaceSummary {
  members: WorkspaceMember[];
  policy: WorkspacePolicy;
}

export interface WorkspaceUsageData {
  workspace_id: string;
  fuel_used: number;
  fuel_remaining: number;
  agents_deployed: number;
  actions_last_24h: number;
  period_start: string;
  period_end: string;
}

// ── Helpers ────────────────────────────────────────────────────────────────

const MEMBER_ROLES = ["Admin", "Operator", "Viewer", "Auditor"] as const;
type MemberRole = typeof MEMBER_ROLES[number];

function formatFuel(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function fuelPct(used: number, budget: number): number {
  if (budget <= 0) return 0;
  return Math.min(100, Math.round((used / budget) * 100));
}

function roleBadgeClass(role: string): string {
  return `admin-badge admin-badge--${role.toLowerCase()}`;
}

function statusDotClass(status: WorkspaceSummary["status"]): string {
  if (status === "active") return "admin-dot admin-dot--running";
  if (status === "suspended") return "admin-dot admin-dot--idle";
  return "admin-dot admin-dot--stopped";
}

function normalizeWorkspace(raw: any): WorkspaceSummary {
  return {
    id: raw.id ?? raw.workspace_id ?? "",
    name: raw.name ?? "Unnamed",
    member_count: raw.member_count ?? (raw.members?.length ?? 0),
    agent_limit: raw.agent_limit ?? 0,
    fuel_budget: raw.fuel_budget ?? 0,
    data_isolation: raw.data_isolation ?? raw.policy?.data_isolation ?? "shared",
    created_at: raw.created_at ?? "",
    status: raw.status ?? "active",
  };
}

function normalizeDetail(raw: any): WorkspaceDetail {
  const base = normalizeWorkspace(raw);
  return {
    ...base,
    members: Array.isArray(raw.members) ? raw.members : [],
    policy: raw.policy ?? {
      max_autonomy_level: 3,
      allowed_capabilities: [],
      require_hitl_tier: "Tier1",
      data_isolation: base.data_isolation,
    },
  };
}

function normalizeUsage(raw: any, workspaceId: string): WorkspaceUsageData {
  return {
    workspace_id: raw.workspace_id ?? workspaceId,
    fuel_used: raw.fuel_used ?? raw.fuel_consumed ?? 0,
    fuel_remaining: raw.fuel_remaining ?? raw.fuel_budget_remaining ?? 0,
    agents_deployed: raw.agents_deployed ?? raw.active_agents ?? 0,
    actions_last_24h: raw.actions_last_24h ?? 0,
    period_start: raw.period_start ?? "",
    period_end: raw.period_end ?? "",
  };
}

// ── Sub-components ─────────────────────────────────────────────────────────

interface UsagePanelProps {
  usage: WorkspaceUsageData;
  fuelBudget: number;
}

function UsagePanel({ usage, fuelBudget }: UsagePanelProps) {
  const pct = fuelPct(usage.fuel_used, fuelBudget || usage.fuel_used + usage.fuel_remaining);
  const barClass = pct >= 90 ? "admin-bar__fill--warn" : pct >= 70 ? "admin-bar__fill--accent" : "admin-bar__fill--ok";

  return (
    <div className="admin-card">
      <div className="admin-card__title">Usage</div>
      <div className="admin-metrics" style={{ marginBottom: "0.5rem" }}>
        <div className="admin-metric">
          <span className="admin-metric__label">Fuel Used</span>
          <span className="admin-metric__value">{formatFuel(usage.fuel_used)}</span>
          <span className="admin-metric__sub">of {formatFuel(fuelBudget || usage.fuel_used + usage.fuel_remaining)} budget</span>
        </div>
        <div className="admin-metric">
          <span className="admin-metric__label">Fuel Remaining</span>
          <span className="admin-metric__value">{formatFuel(usage.fuel_remaining)}</span>
          <span className="admin-metric__sub">{100 - pct}% left</span>
        </div>
        <div className="admin-metric">
          <span className="admin-metric__label">Agents Deployed</span>
          <span className="admin-metric__value">{usage.agents_deployed}</span>
        </div>
        <div className="admin-metric">
          <span className="admin-metric__label">Actions (24h)</span>
          <span className="admin-metric__value">{usage.actions_last_24h.toLocaleString()}</span>
        </div>
      </div>
      <div style={{ marginBottom: "0.3rem" }}>
        <div style={{ display: "flex", justifyContent: "space-between", fontSize: "0.72rem", color: "var(--text-muted)", marginBottom: "0.3rem" }}>
          <span>Fuel consumption</span>
          <span>{pct}%</span>
        </div>
        <div className="admin-bar">
          <div className={`admin-bar__fill ${barClass}`} style={{ width: `${pct}%` }} />
        </div>
      </div>
    </div>
  );
}

// ── Main Component ─────────────────────────────────────────────────────────

export default function Workspaces() {
  // List view state
  const [workspaces, setWorkspaces] = useState<WorkspaceSummary[]>([]);
  const [listLoading, setListLoading] = useState(true);
  const [listError, setListError] = useState<string | null>(null);

  // Create workspace form
  const [showCreate, setShowCreate] = useState(false);
  const [createName, setCreateName] = useState("");
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  // Detail view state
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detail, setDetail] = useState<WorkspaceDetail | null>(null);
  const [usage, setUsage] = useState<WorkspaceUsageData | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);

  // Add member form
  const [showAddMember, setShowAddMember] = useState(false);
  const [addMemberForm, setAddMemberForm] = useState({ userId: "", role: "Viewer" as MemberRole });
  const [addingMember, setAddingMember] = useState(false);
  const [addMemberError, setAddMemberError] = useState<string | null>(null);

  // Policy editor state
  const [showPolicyEditor, setShowPolicyEditor] = useState(false);
  const [policyJson, setPolicyJson] = useState("");
  const [savingPolicy, setSavingPolicy] = useState(false);
  const [policyError, setPolicyError] = useState<string | null>(null);

  // ── Data fetching ────────────────────────────────────────────────────────

  const fetchWorkspaces = useCallback(async () => {
    setListLoading(true);
    setListError(null);
    try {
      const raw = await workspaceList();
      setWorkspaces((Array.isArray(raw) ? raw : []).map(normalizeWorkspace));
    } catch (err: any) {
      setListError(err?.message ?? "Failed to load workspaces");
    } finally {
      setListLoading(false);
    }
  }, []);

  const fetchDetail = useCallback(async (id: string) => {
    setDetailLoading(true);
    setDetailError(null);
    setDetail(null);
    setUsage(null);
    try {
      const [rawDetail, rawUsage] = await Promise.all([
        workspaceGet(id),
        workspaceUsage(id),
      ]);
      const det = normalizeDetail(rawDetail);
      setDetail(det);
      setUsage(normalizeUsage(rawUsage, id));
      setPolicyJson(JSON.stringify(det.policy, null, 2));
    } catch (err: any) {
      setDetailError(err?.message ?? "Failed to load workspace details");
    } finally {
      setDetailLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchWorkspaces();
  }, [fetchWorkspaces]);

  useEffect(() => {
    if (selectedId) {
      void fetchDetail(selectedId);
      setShowAddMember(false);
      setShowPolicyEditor(false);
      setAddMemberForm({ userId: "", role: "Viewer" });
      setAddMemberError(null);
      setPolicyError(null);
    } else {
      setDetail(null);
      setUsage(null);
    }
  }, [selectedId, fetchDetail]);

  // ── Actions ──────────────────────────────────────────────────────────────

  const handleCreate = async () => {
    if (!createName.trim()) return;
    setCreating(true);
    setCreateError(null);
    try {
      const raw = await workspaceCreate(createName.trim());
      const ws = normalizeWorkspace(raw);
      setWorkspaces((prev) => [...prev, ws]);
      setCreateName("");
      setShowCreate(false);
    } catch (err: any) {
      setCreateError(err?.message ?? "Failed to create workspace");
    } finally {
      setCreating(false);
    }
  };

  const handleAddMember = async () => {
    if (!detail || !addMemberForm.userId.trim()) return;
    setAddingMember(true);
    setAddMemberError(null);
    try {
      await workspaceAddMember(detail.id, addMemberForm.userId.trim(), addMemberForm.role);
      const newMember: WorkspaceMember = {
        user_id: addMemberForm.userId.trim(),
        role: addMemberForm.role,
        joined_at: new Date().toISOString(),
      };
      setDetail((prev) =>
        prev ? { ...prev, members: [...prev.members, newMember], member_count: prev.member_count + 1 } : prev
      );
      setWorkspaces((prev) =>
        prev.map((w) => (w.id === detail.id ? { ...w, member_count: w.member_count + 1 } : w))
      );
      setAddMemberForm({ userId: "", role: "Viewer" });
      setShowAddMember(false);
    } catch (err: any) {
      setAddMemberError(err?.message ?? "Failed to add member");
    } finally {
      setAddingMember(false);
    }
  };

  const handleRemoveMember = async (userId: string) => {
    if (!detail) return;
    try {
      await workspaceRemoveMember(detail.id, userId);
      setDetail((prev) =>
        prev
          ? { ...prev, members: prev.members.filter((m) => m.user_id !== userId), member_count: Math.max(0, prev.member_count - 1) }
          : prev
      );
      setWorkspaces((prev) =>
        prev.map((w) => (w.id === detail.id ? { ...w, member_count: Math.max(0, w.member_count - 1) } : w))
      );
    } catch (err: any) {
      setDetailError(err?.message ?? "Failed to remove member");
    }
  };

  const handleSavePolicy = async () => {
    if (!detail) return;
    setSavingPolicy(true);
    setPolicyError(null);
    try {
      // Validate JSON before sending
      JSON.parse(policyJson);
      await workspaceSetPolicy(detail.id, policyJson);
      const updated = JSON.parse(policyJson) as WorkspacePolicy;
      setDetail((prev) => (prev ? { ...prev, policy: updated } : prev));
      setShowPolicyEditor(false);
    } catch (err: any) {
      setPolicyError(err?.message ?? "Invalid JSON or failed to save policy");
    } finally {
      setSavingPolicy(false);
    }
  };

  // ── Render: list view ────────────────────────────────────────────────────

  function renderList() {
    return (
      <div className="admin-shell">
        <h1>Workspaces</h1>
        <p className="admin-subtitle">Multi-tenant workspace management — isolation, policies, and fuel budgets</p>

        {/* Controls */}
        <div style={{ display: "flex", gap: "0.6rem", marginBottom: "1rem", alignItems: "center" }}>
          <button
            className="admin-btn admin-btn--accent"
            onClick={() => setShowCreate(!showCreate)}
          >
            + Create Workspace
          </button>
          <button className="admin-btn" onClick={() => void fetchWorkspaces()}>
            Refresh
          </button>
        </div>

        {/* Create form */}
        {showCreate && (
          <div className="admin-card" style={{ marginBottom: "1rem" }}>
            <div className="admin-card__title">New Workspace</div>
            <div style={{ display: "flex", gap: "0.6rem", alignItems: "flex-end", flexWrap: "wrap" }}>
              <div style={{ flex: 1, minWidth: 220 }}>
                <label style={{ fontSize: "0.72rem", color: "var(--text-muted)", display: "block", marginBottom: "0.25rem" }}>
                  Workspace Name
                </label>
                <input
                  className="admin-input"
                  placeholder="e.g. production, staging, team-alpha"
                  value={createName}
                  onChange={(e) => setCreateName(e.target.value)}
                  onKeyDown={(e) => { if (e.key === "Enter") void handleCreate(); }}
                  disabled={creating}
                />
              </div>
              <button
                className="admin-btn admin-btn--accent"
                onClick={() => void handleCreate()}
                disabled={creating || !createName.trim()}
              >
                {creating ? "Creating..." : "Create"}
              </button>
              <button
                className="admin-btn"
                onClick={() => { setShowCreate(false); setCreateName(""); setCreateError(null); }}
              >
                Cancel
              </button>
            </div>
            {createError && (
              <div className="admin-alert admin-alert--danger" style={{ marginTop: "0.6rem" }}>
                {createError}
              </div>
            )}
          </div>
        )}

        {/* Error state */}
        {listError && (
          <div className="admin-alert admin-alert--danger" style={{ marginBottom: "1rem" }}>
            {listError}
          </div>
        )}

        {/* Workspace grid */}
        {listLoading ? (
          <div className="admin-card">
            <p style={{ textAlign: "center", padding: "2rem", color: "var(--text-muted)" }}>
              Loading workspaces...
            </p>
          </div>
        ) : workspaces.length === 0 ? (
          <div className="admin-card">
            <div className="admin-empty">
              No workspaces found. Create one to get started.
            </div>
          </div>
        ) : (
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fill, minmax(320px, 1fr))",
              gap: "1rem",
              marginBottom: "1rem",
            }}
          >
            {workspaces.map((ws) => (
              <button
                key={ws.id}
                className="admin-card"
                style={{
                  cursor: "pointer",
                  textAlign: "left",
                  background: "var(--bg-card, #0b1526)",
                  border: "1px solid var(--border, rgba(90,142,190,0.18))",
                  borderRadius: "var(--radius-panel, 1.35rem)",
                  padding: "1.2rem",
                  width: "100%",
                  transition: "border-color 0.2s",
                }}
                onMouseEnter={(e) => (e.currentTarget.style.borderColor = "var(--nexus-accent, #4af7d3)")}
                onMouseLeave={(e) => (e.currentTarget.style.borderColor = "var(--border, rgba(90,142,190,0.18))")}
                onClick={() => setSelectedId(ws.id)}
              >
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: "0.75rem" }}>
                  <span
                    style={{
                      fontFamily: "var(--font-display, 'Orbitron', sans-serif)",
                      fontSize: "0.9rem",
                      color: "var(--text-primary, #eef7ff)",
                      fontWeight: 600,
                    }}
                  >
                    {ws.name}
                  </span>
                  <span>
                    <span className={statusDotClass(ws.status)} />
                    <span
                      style={{ fontSize: "0.72rem", color: "var(--text-muted)", textTransform: "uppercase" }}
                    >
                      {ws.status}
                    </span>
                  </span>
                </div>

                <div
                  style={{
                    display: "grid",
                    gridTemplateColumns: "1fr 1fr",
                    gap: "0.5rem 1rem",
                    fontSize: "0.78rem",
                  }}
                >
                  <div>
                    <div style={{ color: "var(--text-muted)", fontSize: "0.68rem", textTransform: "uppercase", letterSpacing: "0.06em" }}>
                      Members
                    </div>
                    <div style={{ color: "var(--nexus-accent, #4af7d3)", fontWeight: 600 }}>
                      {ws.member_count}
                    </div>
                  </div>
                  <div>
                    <div style={{ color: "var(--text-muted)", fontSize: "0.68rem", textTransform: "uppercase", letterSpacing: "0.06em" }}>
                      Agent Limit
                    </div>
                    <div style={{ color: "var(--text-secondary)", fontWeight: 600 }}>
                      {ws.agent_limit > 0 ? ws.agent_limit : "Unlimited"}
                    </div>
                  </div>
                  <div>
                    <div style={{ color: "var(--text-muted)", fontSize: "0.68rem", textTransform: "uppercase", letterSpacing: "0.06em" }}>
                      Fuel Budget
                    </div>
                    <div style={{ color: "var(--text-secondary)", fontWeight: 600 }}>
                      {ws.fuel_budget > 0 ? formatFuel(ws.fuel_budget) : "Unmetered"}
                    </div>
                  </div>
                  <div>
                    <div style={{ color: "var(--text-muted)", fontSize: "0.68rem", textTransform: "uppercase", letterSpacing: "0.06em" }}>
                      Isolation
                    </div>
                    <div style={{ color: "var(--text-secondary)", fontWeight: 600, textTransform: "capitalize" }}>
                      {ws.data_isolation}
                    </div>
                  </div>
                </div>

                {ws.created_at && (
                  <div
                    style={{ marginTop: "0.75rem", fontSize: "0.68rem", color: "var(--text-muted)", borderTop: "1px solid var(--border, rgba(90,142,190,0.12))", paddingTop: "0.5rem" }}
                  >
                    Created {new Date(ws.created_at).toLocaleDateString()}
                  </div>
                )}
              </button>
            ))}
          </div>
        )}
      </div>
    );
  }

  // ── Render: detail view ──────────────────────────────────────────────────

  function renderDetail() {
    return (
      <div className="admin-shell">
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: "0.75rem", marginBottom: "0.25rem" }}>
          <button
            className="admin-btn admin-btn--sm"
            onClick={() => setSelectedId(null)}
            style={{ flexShrink: 0 }}
          >
            Back
          </button>
          <h1 style={{ margin: 0 }}>{detail?.name ?? "Workspace"}</h1>
          {detail && <span className={statusDotClass(detail.status)} />}
        </div>
        <p className="admin-subtitle">
          ID: <code style={{ fontFamily: "monospace", fontSize: "0.78rem" }}>{selectedId}</code>
        </p>

        {detailError && (
          <div className="admin-alert admin-alert--danger" style={{ marginBottom: "1rem" }}>
            {detailError}
          </div>
        )}

        {detailLoading && (
          <div className="admin-card">
            <p style={{ textAlign: "center", padding: "2rem", color: "var(--text-muted)" }}>
              Loading workspace details...
            </p>
          </div>
        )}

        {!detailLoading && detail && (
          <>
            {/* Usage */}
            {usage && <UsagePanel usage={usage} fuelBudget={detail.fuel_budget} />}

            {/* Summary metrics */}
            <div className="admin-metrics" style={{ marginBottom: "1rem" }}>
              <div className="admin-metric">
                <span className="admin-metric__label">Agent Limit</span>
                <span className="admin-metric__value">{detail.agent_limit > 0 ? detail.agent_limit : "--"}</span>
                <span className="admin-metric__sub">{detail.agent_limit > 0 ? "max agents" : "unlimited"}</span>
              </div>
              <div className="admin-metric">
                <span className="admin-metric__label">Fuel Budget</span>
                <span className="admin-metric__value">{detail.fuel_budget > 0 ? formatFuel(detail.fuel_budget) : "--"}</span>
                <span className="admin-metric__sub">{detail.fuel_budget > 0 ? "fuel units" : "unmetered"}</span>
              </div>
              <div className="admin-metric">
                <span className="admin-metric__label">Data Isolation</span>
                <span className="admin-metric__value" style={{ fontSize: "1rem", textTransform: "capitalize" }}>
                  {detail.data_isolation}
                </span>
              </div>
              <div className="admin-metric">
                <span className="admin-metric__label">Members</span>
                <span className="admin-metric__value">{detail.member_count}</span>
              </div>
            </div>

            <div className="admin-grid-2">
              {/* Members panel */}
              <div>
                <div className="admin-card">
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.8rem" }}>
                    <div className="admin-card__title" style={{ margin: 0 }}>Members</div>
                    <button
                      className="admin-btn admin-btn--accent admin-btn--sm"
                      onClick={() => { setShowAddMember(!showAddMember); setAddMemberError(null); }}
                    >
                      + Add Member
                    </button>
                  </div>

                  {/* Add member form */}
                  {showAddMember && (
                    <div
                      style={{
                        background: "var(--bg-elevated, #101f35)",
                        border: "1px solid var(--border, rgba(90,142,190,0.18))",
                        borderRadius: "8px",
                        padding: "0.75rem",
                        marginBottom: "0.75rem",
                      }}
                    >
                      <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
                        <div>
                          <label style={{ fontSize: "0.68rem", color: "var(--text-muted)", display: "block", marginBottom: "0.2rem", textTransform: "uppercase", letterSpacing: "0.06em" }}>
                            User ID
                          </label>
                          <input
                            className="admin-input"
                            placeholder="e.g. oidc:alice or local:nexus"
                            value={addMemberForm.userId}
                            onChange={(e) => setAddMemberForm((f) => ({ ...f, userId: e.target.value }))}
                            disabled={addingMember}
                          />
                        </div>
                        <div>
                          <label style={{ fontSize: "0.68rem", color: "var(--text-muted)", display: "block", marginBottom: "0.2rem", textTransform: "uppercase", letterSpacing: "0.06em" }}>
                            Role
                          </label>
                          <select
                            className="admin-select"
                            value={addMemberForm.role}
                            onChange={(e) => setAddMemberForm((f) => ({ ...f, role: e.target.value as MemberRole }))}
                            disabled={addingMember}
                            style={{ width: "100%" }}
                          >
                            {MEMBER_ROLES.map((r) => <option key={r} value={r}>{r}</option>)}
                          </select>
                        </div>
                        <div style={{ display: "flex", gap: "0.5rem" }}>
                          <button
                            className="admin-btn admin-btn--accent admin-btn--sm"
                            onClick={() => void handleAddMember()}
                            disabled={addingMember || !addMemberForm.userId.trim()}
                          >
                            {addingMember ? "Adding..." : "Add"}
                          </button>
                          <button
                            className="admin-btn admin-btn--sm"
                            onClick={() => { setShowAddMember(false); setAddMemberError(null); }}
                          >
                            Cancel
                          </button>
                        </div>
                        {addMemberError && (
                          <div className="admin-alert admin-alert--danger">{addMemberError}</div>
                        )}
                      </div>
                    </div>
                  )}

                  {/* Members table */}
                  {detail.members.length === 0 ? (
                    <div className="admin-empty" style={{ padding: "1.5rem" }}>No members yet</div>
                  ) : (
                    <table className="admin-table">
                      <thead>
                        <tr>
                          <th>User ID</th>
                          <th>Role</th>
                          <th>Actions</th>
                        </tr>
                      </thead>
                      <tbody>
                        {detail.members.map((m) => (
                          <tr key={m.user_id}>
                            <td style={{ fontFamily: "monospace", fontSize: "0.78rem" }}>{m.user_id}</td>
                            <td>
                              <span className={roleBadgeClass(m.role)}>{m.role}</span>
                            </td>
                            <td>
                              <button
                                className="admin-btn admin-btn--danger admin-btn--sm"
                                onClick={() => void handleRemoveMember(m.user_id)}
                              >
                                Remove
                              </button>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  )}
                </div>
              </div>

              {/* Policy panel */}
              <div>
                <div className="admin-card">
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.8rem" }}>
                    <div className="admin-card__title" style={{ margin: 0 }}>Policy</div>
                    <button
                      className="admin-btn admin-btn--sm"
                      onClick={() => {
                        setShowPolicyEditor(!showPolicyEditor);
                        setPolicyError(null);
                        if (!showPolicyEditor) {
                          setPolicyJson(JSON.stringify(detail.policy, null, 2));
                        }
                      }}
                    >
                      {showPolicyEditor ? "Close Editor" : "Edit Policy"}
                    </button>
                  </div>

                  {!showPolicyEditor ? (
                    <table className="admin-table">
                      <tbody>
                        <tr>
                          <td style={{ color: "var(--text-muted)", width: "55%" }}>Max Autonomy Level</td>
                          <td style={{ color: "var(--nexus-accent, #4af7d3)", fontWeight: 600 }}>
                            L{detail.policy.max_autonomy_level}
                          </td>
                        </tr>
                        <tr>
                          <td style={{ color: "var(--text-muted)" }}>HITL Requirement</td>
                          <td>{detail.policy.require_hitl_tier}</td>
                        </tr>
                        <tr>
                          <td style={{ color: "var(--text-muted)" }}>Data Isolation</td>
                          <td style={{ textTransform: "capitalize" }}>{detail.policy.data_isolation}</td>
                        </tr>
                        <tr>
                          <td style={{ color: "var(--text-muted)" }}>Allowed Capabilities</td>
                          <td>
                            {detail.policy.allowed_capabilities.length === 0
                              ? <span style={{ color: "var(--text-muted)", fontStyle: "italic" }}>all</span>
                              : detail.policy.allowed_capabilities.join(", ")}
                          </td>
                        </tr>
                      </tbody>
                    </table>
                  ) : (
                    <div>
                      <textarea
                        style={{
                          width: "100%",
                          minHeight: "180px",
                          background: "var(--bg-elevated, #101f35)",
                          border: "1px solid var(--border, rgba(90,142,190,0.18))",
                          borderRadius: "8px",
                          padding: "0.6rem",
                          fontSize: "0.78rem",
                          fontFamily: "monospace",
                          color: "var(--text-primary, #eef7ff)",
                          resize: "vertical",
                          boxSizing: "border-box",
                        }}
                        value={policyJson}
                        onChange={(e) => setPolicyJson(e.target.value)}
                        disabled={savingPolicy}
                      />
                      <div style={{ display: "flex", gap: "0.5rem", marginTop: "0.5rem" }}>
                        <button
                          className="admin-btn admin-btn--accent admin-btn--sm"
                          onClick={() => void handleSavePolicy()}
                          disabled={savingPolicy}
                        >
                          {savingPolicy ? "Saving..." : "Save Policy"}
                        </button>
                        <button
                          className="admin-btn admin-btn--sm"
                          onClick={() => { setShowPolicyEditor(false); setPolicyError(null); }}
                        >
                          Cancel
                        </button>
                      </div>
                      {policyError && (
                        <div className="admin-alert admin-alert--danger" style={{ marginTop: "0.5rem" }}>
                          {policyError}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              </div>
            </div>
          </>
        )}
      </div>
    );
  }

  // ── Root render ──────────────────────────────────────────────────────────

  return selectedId ? renderDetail() : renderList();
}

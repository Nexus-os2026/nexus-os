import { useCallback, useEffect, useState } from "react";
import type { LucideIcon } from "lucide-react";
import {
  Folder,
  Globe,
  Brain,
  Shield,
  Share2,
  MessageCircle,
  HelpCircle,
  Circle,
} from "lucide-react";
import {
  bulkUpdatePermissions,
  getAgentPermissions,
  getCapabilityRequest,
  getPermissionHistory,
  hasDesktopRuntime,
  setAgentLlmProvider as setAgentLlmProviderApi,
  updateAgentPermission,
} from "../api/backend";
import type {
  CapabilityRequest,
  FilesystemPermissionScope,
  Permission,
  PermissionCategory,
  PermissionHistoryEntry,
  PermissionRiskLevel,
  PermissionUpdate,
} from "../types";
import { CapabilityRequestModal } from "../components/agents/CapabilityRequestModal";
import "./permission-dashboard.css";

// ── Icon mapping ──

const CATEGORY_ICONS: Record<string, LucideIcon> = {
  folder: Folder,
  globe: Globe,
  brain: Brain,
  shield: Shield,
  share: Share2,
  chat: MessageCircle,
  circle: Circle,
};

// ── Plain English tooltips for every capability ──

const CAPABILITY_TOOLTIPS: Record<string, string> = {
  "fs.read":
    "Lets the agent look at files on your computer. It can read documents, configs, and other data — but it cannot change or delete anything with this permission alone.",
  "fs.write":
    "Lets the agent create, edit, and delete files on your computer. This is high risk because changes can be destructive and hard to undo.",
  "web.search":
    "Lets the agent search the internet for information, similar to how you would use a search engine. It can see search results but does not share your personal data.",
  "web.read":
    "Lets the agent open and read web pages. It can fetch articles, documentation, or any public page — but it cannot log into your accounts or submit forms.",
  "llm.query":
    "Lets the agent send questions to an AI language model (like ChatGPT or Claude). This may send data to a cloud service and could incur usage costs on your account.",
  "process.exec":
    "Lets the agent run programs and system commands on your computer. This is the most powerful permission — a misbehaving agent could install software, change system settings, or access any data.",
  "audit.read":
    "Lets the agent view the history of actions taken by all agents. This is low risk — it's read-only access to activity logs, useful for monitoring and reporting.",
  "social.post":
    "Lets the agent publish posts on social media platforms on your behalf. Anything it posts will be visible to your followers and the public.",
  "social.x.post":
    "Lets the agent post on X (formerly Twitter) using your account. Posts will appear as if you wrote them, so review what the agent plans to say.",
  "social.x.read":
    "Lets the agent read your X (Twitter) timeline, mentions, and other public posts. It cannot post, like, or interact — only read.",
  "messaging.send":
    "Lets the agent send messages through Telegram, WhatsApp, Discord, or Slack on your behalf. Recipients will see the messages as if they came from you.",
};

const RISK_LABELS: Record<PermissionRiskLevel, { label: string; className: string }> = {
  low: { label: "Low", className: "risk-low" },
  medium: { label: "Medium", className: "risk-medium" },
  high: { label: "High", className: "risk-high" },
  critical: { label: "Critical", className: "risk-critical" },
};

const ACTION_STYLES: Record<string, { label: string; className: string }> = {
  granted: { label: "Granted", className: "action-granted" },
  revoked: { label: "Revoked", className: "action-revoked" },
  escalated: { label: "Escalated", className: "action-escalated" },
  downgraded: { label: "Downgraded", className: "action-downgraded" },
  locked_by_admin: { label: "Locked", className: "action-locked" },
  unlocked_by_admin: { label: "Unlocked", className: "action-unlocked" },
};

// ── Mock data for non-desktop mode ──

function mockCategories(): PermissionCategory[] {
  return [
    {
      id: "filesystem", display_name: "Filesystem", icon: "folder",
      permissions: [
        { capability_key: "fs.read", display_name: "Read files", description: "Allows the agent to read files from the filesystem", risk_level: "medium", enabled: true, granted_by: "manifest", granted_at: Date.now() / 1000, can_user_toggle: true, filesystem_scopes: [{ path_pattern: "/src/**", permission: "ReadOnly" as const }, { path_pattern: "*.rs", permission: "ReadOnly" as const }] },
        { capability_key: "fs.write", display_name: "Write files", description: "Allows the agent to create or modify files on the filesystem. Changes can be destructive.", risk_level: "high", enabled: false, granted_by: "", granted_at: 0, can_user_toggle: true, filesystem_scopes: [{ path_pattern: "/output/", permission: "ReadWrite" as const }, { path_pattern: "/src/secret.rs", permission: "Deny" as const }] },
      ],
    },
    {
      id: "network", display_name: "Network", icon: "globe",
      permissions: [
        { capability_key: "web.search", display_name: "Web search", description: "Allows the agent to search the web for information", risk_level: "medium", enabled: true, granted_by: "manifest", granted_at: Date.now() / 1000, can_user_toggle: true },
        { capability_key: "web.read", display_name: "Read web pages", description: "Allows the agent to fetch and read web page content", risk_level: "medium", enabled: false, granted_by: "", granted_at: 0, can_user_toggle: true },
      ],
    },
    {
      id: "ai", display_name: "AI / LLM", icon: "brain",
      permissions: [
        { capability_key: "llm.query", display_name: "Query AI model", description: "Allows the agent to send prompts to an AI language model. May incur costs and expose data.", risk_level: "medium", enabled: true, granted_by: "manifest", granted_at: Date.now() / 1000, can_user_toggle: true },
      ],
    },
    {
      id: "system", display_name: "System", icon: "shield",
      permissions: [
        { capability_key: "process.exec", display_name: "Execute processes", description: "Allows the agent to run system commands and processes. This is a powerful capability.", risk_level: "critical", enabled: false, granted_by: "", granted_at: 0, can_user_toggle: true },
        { capability_key: "audit.read", display_name: "Read audit logs", description: "Allows the agent to read the audit trail and event history", risk_level: "low", enabled: true, granted_by: "manifest", granted_at: Date.now() / 1000, can_user_toggle: true },
      ],
    },
    {
      id: "social", display_name: "Social Media", icon: "share",
      permissions: [
        { capability_key: "social.post", display_name: "Post to social media", description: "Allows the agent to publish posts on social media platforms", risk_level: "high", enabled: false, granted_by: "", granted_at: 0, can_user_toggle: true },
        { capability_key: "social.x.post", display_name: "Post to X (Twitter)", description: "Allows the agent to publish posts on X (formerly Twitter)", risk_level: "high", enabled: false, granted_by: "", granted_at: 0, can_user_toggle: true },
        { capability_key: "social.x.read", display_name: "Read X (Twitter)", description: "Allows the agent to read posts and timelines on X", risk_level: "low", enabled: false, granted_by: "", granted_at: 0, can_user_toggle: true },
      ],
    },
    {
      id: "messaging", display_name: "Messaging", icon: "chat",
      permissions: [
        { capability_key: "messaging.send", display_name: "Send messages", description: "Allows the agent to send messages via Telegram, WhatsApp, Discord, or Slack", risk_level: "high", enabled: false, granted_by: "", granted_at: 0, can_user_toggle: true },
      ],
    },
  ];
}

function mockHistory(): PermissionHistoryEntry[] {
  const now = Date.now() / 1000;
  return [
    { capability_key: "fs.read", action: "granted", changed_by: "manifest", timestamp: now - 86400 * 2, reason: null },
    { capability_key: "llm.query", action: "granted", changed_by: "manifest", timestamp: now - 86400 * 2, reason: null },
    { capability_key: "web.search", action: "granted", changed_by: "user", timestamp: now - 86400, reason: "Needed for research" },
    { capability_key: "fs.write", action: "revoked", changed_by: "adaptive-governance", timestamp: now - 3600, reason: "Risk assessment triggered automatic revocation" },
  ];
}

// ── Props ──

interface PermissionDashboardProps {
  agentId: string;
  agentName: string;
  fuelRemaining?: number;
  fuelBudget?: number;
  memoryUsageBytes?: number;
  onBack: () => void;
}

// ── Component ──

export function PermissionDashboard({
  agentId,
  agentName,
  fuelRemaining = 8000,
  fuelBudget = 10000,
  memoryUsageBytes,
  onBack,
}: PermissionDashboardProps) {
  const [categories, setCategories] = useState<PermissionCategory[]>([]);
  const [history, setHistory] = useState<PermissionHistoryEntry[]>([]);
  const [requests, setRequests] = useState<CapabilityRequest[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [pendingToggles, setPendingToggles] = useState<Set<string>>(new Set());
  const [historyOpen, setHistoryOpen] = useState(false);
  const [historyFilter, setHistoryFilter] = useState<string>("all");
  const [confirmAction, setConfirmAction] = useState<{ label: string; updates: PermissionUpdate[]; reason: string } | null>(null);
  const [requestModalCap, setRequestModalCap] = useState<CapabilityRequest | null>(null);
  const [agentLlmProvider, setAgentLlmProvider] = useState("");
  const [agentLocalOnly, setAgentLocalOnly] = useState(false);
  const isDesktop = hasDesktopRuntime();

  const loadData = useCallback(async () => {
    try {
      if (isDesktop) {
        const [cats, hist, reqs] = await Promise.all([
          getAgentPermissions(agentId),
          getPermissionHistory(agentId),
          getCapabilityRequest(agentId),
        ]);
        setCategories(cats);
        setHistory(hist);
        setRequests(reqs);
      } else {
        setCategories(mockCategories());
        setHistory(mockHistory());
        setRequests([]);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [agentId, isDesktop]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // ── Toggle handler ──

  const handleToggle = async (capKey: string, newValue: boolean) => {
    // Optimistic update
    setPendingToggles((prev) => new Set(prev).add(capKey));
    setCategories((prev) =>
      prev.map((cat) => ({
        ...cat,
        permissions: cat.permissions.map((p) =>
          p.capability_key === capKey ? { ...p, enabled: newValue } : p
        ),
      }))
    );

    try {
      if (isDesktop) {
        await updateAgentPermission(agentId, capKey, newValue);
      }
      // Refresh history
      if (isDesktop) {
        const hist = await getPermissionHistory(agentId);
        setHistory(hist);
      } else {
        setHistory((prev) => [
          { capability_key: capKey, action: newValue ? "granted" : "revoked", changed_by: "user", timestamp: Date.now() / 1000, reason: null },
          ...prev,
        ]);
      }
    } catch (err) {
      // Revert optimistic update
      setCategories((prev) =>
        prev.map((cat) => ({
          ...cat,
          permissions: cat.permissions.map((p) =>
            p.capability_key === capKey ? { ...p, enabled: !newValue } : p
          ),
        }))
      );
      setError(String(err));
    } finally {
      setPendingToggles((prev) => {
        const next = new Set(prev);
        next.delete(capKey);
        return next;
      });
    }
  };

  // ── Bulk actions ──

  const executeBulkAction = async (updates: PermissionUpdate[], reason: string) => {
    setConfirmAction(null);
    try {
      if (isDesktop) {
        await bulkUpdatePermissions(agentId, updates, reason);
      }
      // Optimistic update
      const updateMap = new Map(updates.map((u) => [u.capability_key, u.enabled]));
      setCategories((prev) =>
        prev.map((cat) => ({
          ...cat,
          permissions: cat.permissions.map((p) =>
            updateMap.has(p.capability_key) ? { ...p, enabled: updateMap.get(p.capability_key)! } : p
          ),
        }))
      );
      if (isDesktop) {
        const hist = await getPermissionHistory(agentId);
        setHistory(hist);
      }
    } catch (err) {
      setError(String(err));
      loadData();
    }
  };

  // ── Render helpers ──

  const [openTooltip, setOpenTooltip] = useState<string | null>(null);

  const FS_SCOPE_BADGE: Record<string, { label: string; className: string }> = {
    ReadOnly: { label: "Read", className: "fs-scope-readonly" },
    ReadWrite: { label: "Read/Write", className: "fs-scope-readwrite" },
    Deny: { label: "Deny", className: "fs-scope-deny" },
  };

  const renderFsScopes = (scopes?: FilesystemPermissionScope[]) => {
    const isFs = true; // caller only invokes for fs.* capabilities
    if (!isFs) return null;
    if (!scopes || scopes.length === 0) {
      return (
        <div className="fs-scope-section">
          <span className="fs-scope-note">Unrestricted (no path scopes configured)</span>
        </div>
      );
    }
    return (
      <div className="fs-scope-section">
        <div className="fs-scope-note">Default: deny unlisted paths</div>
        <div className="fs-scope-list">
          {scopes.map((s, i) => {
            const badge = FS_SCOPE_BADGE[s.permission] || { label: s.permission, className: "" };
            return (
              <div className="fs-scope-entry" key={`${s.path_pattern}-${i}`}>
                <code className="fs-scope-pattern">{s.path_pattern}</code>
                <span className={`fs-scope-badge ${badge.className}`}>{badge.label}</span>
              </div>
            );
          })}
        </div>
      </div>
    );
  };

  const renderToggle = (perm: Permission) => {
    const isPending = pendingToggles.has(perm.capability_key);
    const risk = RISK_LABELS[perm.risk_level];
    const tooltip = CAPABILITY_TOOLTIPS[perm.capability_key];
    const isTooltipOpen = openTooltip === perm.capability_key;
    const isFsCap = perm.capability_key === "fs.read" || perm.capability_key === "fs.write";

    return (
      <div className="perm-row" key={perm.capability_key}>
        <div className="perm-info">
          <span className="perm-name">{perm.display_name}</span>
          <span className={`perm-risk-badge ${risk.className}`}>{risk.label}</span>
          {!perm.can_user_toggle && <span className="perm-locked-badge" title="Locked by admin">{"\u{1F512}"}</span>}
          {tooltip && (
            <button
              className="perm-help-btn"
              onClick={() => setOpenTooltip(isTooltipOpen ? null : perm.capability_key)}
              title="What does this mean?"
            >
              <HelpCircle size={14} />
            </button>
          )}
        </div>
        {isTooltipOpen && tooltip ? (
          <div className="perm-explain-tooltip">
            <span className="perm-explain-label">What does this mean?</span>
            <p className="perm-explain-text">{tooltip}</p>
          </div>
        ) : (
          <div className="perm-tooltip">{perm.description}</div>
        )}
        <label className={`perm-toggle ${isPending ? "perm-toggle-pending" : ""}`}>
          <input
            type="checkbox"
            checked={perm.enabled}
            disabled={!perm.can_user_toggle || isPending}
            onChange={() => handleToggle(perm.capability_key, !perm.enabled)}
          />
          <span className="perm-toggle-slider" />
        </label>
        {isFsCap && renderFsScopes(perm.filesystem_scopes)}
      </div>
    );
  };

  const renderCategory = (cat: PermissionCategory) => {
    const IconComponent = CATEGORY_ICONS[cat.icon] || CATEGORY_ICONS.circle;
    return (
      <div className="perm-category-card" key={cat.id}>
        <div className="perm-category-header">
          <span className="perm-category-icon"><IconComponent size={18} /></span>
          <span className="perm-category-name">{cat.display_name}</span>
        </div>
        <div className="perm-category-body">
          {cat.permissions.map(renderToggle)}
        </div>
      </div>
    );
  };

  // ── Resource bars ──

  const fuelPercent = fuelBudget ? Math.round((fuelRemaining / fuelBudget) * 100) : 0;
  const fuelColor = fuelPercent > 50 ? "bar-green" : fuelPercent > 20 ? "bar-yellow" : "bar-red";
  const memMb = memoryUsageBytes ? Math.round(memoryUsageBytes / (1024 * 1024)) : 0;
  const memLimit = 256;
  const memPercent = Math.min(Math.round((memMb / memLimit) * 100), 100);
  const memColor = memPercent > 80 ? "bar-red" : memPercent > 50 ? "bar-yellow" : "bar-green";

  // ── History filter ──

  const filteredHistory = historyFilter === "all"
    ? history
    : history.filter((h) => {
        const catMap: Record<string, string[]> = {
          filesystem: ["fs.read", "fs.write"],
          network: ["web.search", "web.read"],
          ai: ["llm.query"],
          system: ["process.exec", "audit.read"],
          social: ["social.post", "social.x.post", "social.x.read"],
          messaging: ["messaging.send"],
        };
        return catMap[historyFilter]?.includes(h.capability_key);
      });

  const formatRelativeTime = (ts: number) => {
    const diff = Math.floor(Date.now() / 1000 - ts);
    if (diff < 60) return "just now";
    if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
    if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
    return `${Math.floor(diff / 86400)}d ago`;
  };

  // ── Main render ──

  if (loading) {
    return (
      <div className="perm-dashboard">
        <div className="perm-loading">Loading permissions...</div>
      </div>
    );
  }

  return (
    <div className="perm-dashboard">
      {/* Header */}
      <div className="perm-header">
        <button className="perm-back-btn" onClick={onBack}>&larr; Back</button>
        <h2 className="perm-title">{agentName}</h2>
        <span className="perm-subtitle">Permission Dashboard</span>
      </div>

      {error && (
        <div className="perm-error">
          {error}
          <button onClick={() => setError(null)}>&times;</button>
        </div>
      )}

      {/* Capability Request Banner */}
      {requests.length > 0 && (
        <div className="perm-request-banner">
          <span>This agent is requesting new capabilities</span>
          {requests.map((req) => (
            <button key={req.requested_capability} className="perm-request-btn" onClick={() => setRequestModalCap(req)}>
              Review: {req.requested_capability}
            </button>
          ))}
        </div>
      )}

      {/* Bulk Controls */}
      <div className="perm-bulk-bar">
        <button
          className="perm-bulk-btn perm-bulk-network"
          onClick={() => setConfirmAction({
            label: "Revoke All Network",
            updates: [{ capability_key: "web.search", enabled: false }, { capability_key: "web.read", enabled: false }],
            reason: "User revoked all network access",
          })}
        >
          Revoke All Network
        </button>
        <button
          className="perm-bulk-btn perm-bulk-readonly"
          onClick={() => setConfirmAction({
            label: "Read-Only Mode",
            updates: [
              { capability_key: "fs.write", enabled: false },
              { capability_key: "social.post", enabled: false },
              { capability_key: "social.x.post", enabled: false },
              { capability_key: "messaging.send", enabled: false },
              { capability_key: "process.exec", enabled: false },
            ],
            reason: "User enabled read-only mode",
          })}
        >
          Read-Only Mode
        </button>
        <button
          className="perm-bulk-btn perm-bulk-minimal"
          onClick={() => setConfirmAction({
            label: "Minimal Mode",
            updates: [
              { capability_key: "fs.read", enabled: false },
              { capability_key: "fs.write", enabled: false },
              { capability_key: "web.search", enabled: false },
              { capability_key: "web.read", enabled: false },
              { capability_key: "llm.query", enabled: false },
              { capability_key: "process.exec", enabled: false },
              { capability_key: "social.post", enabled: false },
              { capability_key: "social.x.post", enabled: false },
              { capability_key: "social.x.read", enabled: false },
              { capability_key: "messaging.send", enabled: false },
            ],
            reason: "User enabled minimal mode",
          })}
        >
          Minimal Mode
        </button>
      </div>

      {/* Confirmation Modal */}
      {confirmAction && (
        <div className="perm-modal-backdrop" onClick={() => setConfirmAction(null)}>
          <div className="perm-modal" onClick={(e) => e.stopPropagation()}>
            <h3>Confirm: {confirmAction.label}</h3>
            <p>This will update {confirmAction.updates.length} permissions. Continue?</p>
            <div className="perm-modal-actions">
              <button className="perm-modal-cancel" onClick={() => setConfirmAction(null)}>Cancel</button>
              <button className="perm-modal-confirm" onClick={() => executeBulkAction(confirmAction.updates, confirmAction.reason)}>Apply</button>
            </div>
          </div>
        </div>
      )}

      {/* Capability Request Modal */}
      {requestModalCap && (
        <CapabilityRequestModal
          request={requestModalCap}
          onApprove={async (capKey) => {
            await handleToggle(capKey, true);
            setRequestModalCap(null);
            loadData();
          }}
          onDeny={() => setRequestModalCap(null)}
        />
      )}

      {/* Permission Categories */}
      <div className="perm-categories-grid">
        {categories.map(renderCategory)}
      </div>

      {/* Resources Section */}
      <div className="perm-resources-section">
        <h3 className="perm-section-title">Resources</h3>
        <div className="perm-resources-grid">
          <div className="perm-resource-card">
            <div className="perm-resource-label">Fuel Budget</div>
            <div className="perm-resource-bar">
              <div className={`perm-resource-fill ${fuelColor}`} style={{ width: `${fuelPercent}%` }} />
            </div>
            <div className="perm-resource-text">{fuelRemaining.toLocaleString()} / {(fuelBudget || 0).toLocaleString()}</div>
          </div>
          <div className="perm-resource-card">
            <div className="perm-resource-label">Memory Usage</div>
            <div className="perm-resource-bar">
              <div className={`perm-resource-fill ${memColor}`} style={{ width: `${memPercent}%` }} />
            </div>
            <div className="perm-resource-text">{memMb}MB / {memLimit}MB</div>
          </div>
        </div>
      </div>

      {/* LLM Provider Assignment */}
      <div className="perm-resources-section">
        <h3 className="perm-section-title">LLM Provider</h3>
        <div className="perm-resources-grid">
          <div className="perm-resource-card" style={{ gridColumn: "1 / -1" }}>
            <div style={{ display: "flex", gap: "1rem", flexWrap: "wrap", alignItems: "center" }}>
              <div style={{ flex: 1, minWidth: 150 }}>
                <div className="perm-resource-label">Provider Assignment</div>
                <select
                  style={{ width: "100%", padding: "4px 8px", marginTop: 4, background: "rgba(255,255,255,0.05)", color: "inherit", border: "1px solid rgba(255,255,255,0.12)", borderRadius: 4 }}
                  value={agentLlmProvider}
                  onChange={(e) => {
                    setAgentLlmProvider(e.target.value);
                    if (isDesktop) {
                      void setAgentLlmProviderApi(agentId, e.target.value, agentLocalOnly, 0, 0);
                    }
                  }}
                >
                  <option value="">Auto (global routing strategy)</option>
                  <option value="ollama">Ollama (local)</option>
                  <option value="openai">OpenAI</option>
                  <option value="deepseek">DeepSeek</option>
                  <option value="gemini">Google Gemini</option>
                  <option value="claude">Anthropic Claude</option>
                </select>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <label style={{ fontSize: "0.78rem", display: "flex", alignItems: "center", gap: 4, cursor: "pointer" }}>
                  <input
                    type="checkbox"
                    checked={agentLocalOnly}
                    onChange={(e) => {
                      setAgentLocalOnly(e.target.checked);
                      if (isDesktop) {
                        void setAgentLlmProviderApi(agentId, agentLlmProvider, e.target.checked, 0, 0);
                      }
                    }}
                  />
                  Local only (no cloud)
                </label>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Permission History */}
      <div className="perm-history-section">
        <button
          className="perm-history-toggle"
          onClick={() => setHistoryOpen(!historyOpen)}
        >
          {historyOpen ? "\u25BC" : "\u25B6"} Permission History ({history.length})
        </button>

        {historyOpen && (
          <>
            <div className="perm-history-filter">
              <select value={historyFilter} onChange={(e) => setHistoryFilter(e.target.value)}>
                <option value="all">All Categories</option>
                <option value="filesystem">Filesystem</option>
                <option value="network">Network</option>
                <option value="ai">AI / LLM</option>
                <option value="system">System</option>
                <option value="social">Social Media</option>
                <option value="messaging">Messaging</option>
              </select>
            </div>
            <div className="perm-history-timeline">
              {filteredHistory.length === 0 && (
                <div className="perm-history-empty">No permission changes recorded</div>
              )}
              {filteredHistory.map((entry, idx) => {
                const actionStyle = ACTION_STYLES[entry.action] || { label: entry.action, className: "" };
                return (
                  <div className="perm-history-entry" key={`${entry.capability_key}-${entry.timestamp}-${idx}`}>
                    <div className="perm-history-line" />
                    <div className="perm-history-dot" />
                    <div className="perm-history-content">
                      <span className="perm-history-time">{formatRelativeTime(entry.timestamp)}</span>
                      <span className="perm-history-cap">{entry.capability_key}</span>
                      <span className={`perm-history-action ${actionStyle.className}`}>{actionStyle.label}</span>
                      <span className="perm-history-by">by {entry.changed_by}</span>
                      {entry.reason && <span className="perm-history-reason">{entry.reason}</span>}
                    </div>
                  </div>
                );
              })}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

export default PermissionDashboard;

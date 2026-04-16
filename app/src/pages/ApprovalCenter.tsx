import { useCallback, useEffect, useRef, useState } from "react";
import {
  Shield,
  CheckCircle,
  XCircle,
  Clock,
  Folder,
  Terminal as TerminalIcon,
  Globe,
  Cloud,
  AlertTriangle,
} from "lucide-react";
import {
  approveConsentRequest,
  batchApproveConsents,
  batchDenyConsents,
  denyConsentRequest,
  getConsentHistory,
  hasDesktopRuntime,
  hitlStats,
  listPendingConsents,
  reviewConsentBatch,
} from "../api/backend";
import type { HitlStats } from "../api/backend";
import type { ConsentNotification } from "../types";

// ── Icon mapping by operation type ──

const OP_ICONS: Record<string, typeof Folder> = {
  file: Folder,
  fs: Folder,
  shell: TerminalIcon,
  process: TerminalIcon,
  terminal: TerminalIcon,
  web: Globe,
  api: Cloud,
  llm: Cloud,
  social: Globe,
};

function getOpIcon(opType: string): typeof Folder {
  const key = opType.split(".")[0].toLowerCase();
  return OP_ICONS[key] ?? Shield;
}

// ── Risk level badge ──

const RISK_COLORS: Record<string, { bg: string; border: string; text: string }> = {
  Low: { bg: "rgba(74, 222, 128, 0.1)", border: "#4ade80", text: "#4ade80" },
  Medium: { bg: "rgba(250, 204, 21, 0.1)", border: "#facc15", text: "#facc15" },
  High: { bg: "rgba(251, 146, 60, 0.1)", border: "#fb923c", text: "#fb923c" },
  Critical: { bg: "rgba(248, 113, 113, 0.1)", border: "#f87171", text: "#f87171" },
};

function riskColor(level: string): { bg: string; border: string; text: string } {
  const base = level.split(":")[0];
  return RISK_COLORS[base] ?? RISK_COLORS.Low;
}

// ── Countdown hook ──

function useCountdown(targetIso: string): string {
  const [remaining, setRemaining] = useState("");

  useEffect(() => {
    function update() {
      const target = new Date(targetIso).getTime();
      const now = Date.now();
      const diff = target - now;
      if (diff <= 0) {
        setRemaining("Expired");
        return;
      }
      const mins = Math.floor(diff / 60000);
      const secs = Math.floor((diff % 60000) / 1000);
      setRemaining(`${mins}m ${secs.toString().padStart(2, "0")}s`);
    }
    update();
    const interval = setInterval(update, 1000);
    return () => clearInterval(interval);
  }, [targetIso]);

  return remaining;
}

function useReviewCountdown(requestedAt: string, minReviewSeconds?: number | null): number {
  const [remaining, setRemaining] = useState(0);

  useEffect(() => {
    if (!minReviewSeconds || minReviewSeconds <= 0) {
      setRemaining(0);
      return;
    }
    const reviewWindowSeconds = minReviewSeconds;

    function update() {
      const created = new Date(requestedAt).getTime();
      const deadline = created + reviewWindowSeconds * 1000;
      const diff = Math.max(0, Math.ceil((deadline - Date.now()) / 1000));
      setRemaining(diff);
    }

    update();
    const interval = setInterval(update, 250);
    return () => clearInterval(interval);
  }, [requestedAt, minReviewSeconds]);

  return remaining;
}

// ── Pending Card ──

function PendingCard({
  item,
  onApprove,
  onDeny,
  onBatchApprove,
  onBatchDeny,
  onReviewEach,
}: {
  item: ConsentNotification;
  onApprove: (id: string) => void;
  onDeny: (id: string, reason?: string) => void;
  onBatchApprove: (goalId: string) => void;
  onBatchDeny: (goalId: string, reason?: string) => void;
  onReviewEach: (consentId: string) => void;
}) {
  const [showDenyReason, setShowDenyReason] = useState(false);
  const [denyReason, setDenyReason] = useState("");
  const countdown = useCountdown(item.auto_deny_at);
  const reviewRemaining = useReviewCountdown(item.requested_at, item.min_review_seconds);
  const Icon = getOpIcon(item.operation_type);
  const risk = riskColor(item.risk_level);
  const isBatch = Boolean(item.goal_id && (item.batch_action_count ?? 0) > 1);

  return (
    <div
      className="nexus-pending-card"
      style={{
        background: "var(--bg-secondary, #1e293b)",
        border: `1px solid ${risk.border}33`,
        borderRadius: 10,
        padding: "1rem 1.2rem",
        marginBottom: "0.75rem",
      }}
    >
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.6rem" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
          <Icon size={16} style={{ color: risk.text }} />
          <span style={{ color: "var(--text-primary, #e2e8f0)", fontWeight: 600, fontSize: "0.95rem" }}>
            {item.agent_name}
          </span>
          <span style={{ color: "var(--text-secondary, #64748b)", fontSize: "0.8rem" }}>
            {item.agent_id.slice(0, 8)}
          </span>
        </div>
        <span
          style={{
            background: risk.bg,
            border: `1px solid ${risk.border}`,
            color: risk.text,
            padding: "0.15rem 0.5rem",
            borderRadius: 6,
            fontSize: "0.75rem",
            fontWeight: 600,
          }}
        >
          {item.risk_level}
        </span>
      </div>

      {/* Operation summary */}
      <div style={{ color: "var(--text-primary, #e2e8f0)", fontSize: "0.9rem", marginBottom: "0.5rem" }}>
        <span style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.8rem" }}>
          {item.operation_type}
        </span>{" "}
        &mdash; {item.operation_summary}
      </div>

      {/* Side effects */}
      {item.side_effects_preview.length > 0 && (
        <div style={{ marginBottom: "0.5rem" }}>
          <div style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.75rem", marginBottom: "0.25rem" }}>
            Side effects:
          </div>
          <ul style={{ margin: 0, paddingLeft: "1.2rem", color: "var(--text-primary, #e2e8f0)", fontSize: "0.85rem" }}>
            {item.side_effects_preview.map((effect, i) => (
              <li key={i}>{effect}</li>
            ))}
          </ul>
        </div>
      )}

      {isBatch && item.batch_actions.length > 0 && (
        <div style={{ marginBottom: "0.75rem" }}>
          <div style={{ color: "var(--text-secondary, #94a3b8)", fontSize: "0.75rem", marginBottom: "0.25rem" }}>
            Planned actions:
          </div>
          <ol style={{ margin: 0, paddingLeft: "1.2rem", color: "var(--text-primary, #e2e8f0)", fontSize: "0.85rem" }}>
            {item.batch_actions.map((action, index) => (
              <li key={`${item.consent_id}-${index}`}>{action}</li>
            ))}
          </ol>
        </div>
      )}

      {/* Meta row */}
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          fontSize: "0.8rem",
          color: "var(--text-secondary, #94a3b8)",
          marginBottom: "0.75rem",
        }}
      >
        <span>Fuel cost: {item.fuel_cost_estimate}</span>
        <span style={{ display: "flex", alignItems: "center", gap: "0.3rem", color: countdown === "Expired" ? "#f87171" : risk.text }}>
          <Clock size={12} />
          {countdown}
        </span>
      </div>
      {reviewRemaining > 0 && (
        <div style={{ color: "#facc15", fontSize: "0.8rem", marginBottom: "0.75rem" }}>
          Mandatory review: {reviewRemaining}s
        </div>
      )}

      {/* Action buttons */}
      <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
        <button type="button"
          disabled={reviewRemaining > 0}
          onClick={() => {
            if (isBatch && item.goal_id) {
              onBatchApprove(item.goal_id);
              return;
            }
            onApprove(item.consent_id);
          }}
          style={{
            background: reviewRemaining > 0 ? "rgba(148, 163, 184, 0.12)" : "rgba(74, 222, 128, 0.15)",
            border: reviewRemaining > 0 ? "1px solid #64748b" : "1px solid #4ade80",
            color: reviewRemaining > 0 ? "#94a3b8" : "#4ade80",
            padding: "0.4rem 1rem",
            borderRadius: 6,
            cursor: reviewRemaining > 0 ? "not-allowed" : "pointer",
            fontFamily: "var(--font-mono, monospace)",
            fontSize: "0.85rem",
            display: "flex",
            alignItems: "center",
            gap: "0.3rem",
          }}
        >
          <CheckCircle size={14} /> {isBatch ? "Approve All" : "Approve"}
        </button>
        {isBatch && item.review_each_available && (
          <button type="button"
            onClick={() => onReviewEach(item.consent_id)}
            style={{
              background: "rgba(96, 165, 250, 0.15)",
              border: "1px solid #60a5fa",
              color: "#60a5fa",
              padding: "0.4rem 1rem",
              borderRadius: 6,
              cursor: "pointer",
              fontFamily: "var(--font-mono, monospace)",
              fontSize: "0.85rem",
            }}
          >
            Review Each
          </button>
        )}
        {!showDenyReason ? (
          <button type="button"
            onClick={() => setShowDenyReason(true)}
            style={{
              background: "rgba(248, 113, 113, 0.15)",
              border: "1px solid #f87171",
              color: "#f87171",
              padding: "0.4rem 1rem",
              borderRadius: 6,
              cursor: "pointer",
              fontFamily: "var(--font-mono, monospace)",
              fontSize: "0.85rem",
              display: "flex",
              alignItems: "center",
              gap: "0.3rem",
            }}
          >
            <XCircle size={14} /> {isBatch ? "Deny All" : "Deny"}
          </button>
        ) : (
          <div style={{ display: "flex", gap: "0.3rem", flex: 1 }}>
            <input
              type="text"
              placeholder="Reason (optional)"
              value={denyReason}
              onChange={(e) => setDenyReason(e.target.value)}
              style={{
                flex: 1,
                background: "var(--bg-primary, #0f172a)",
                border: "1px solid var(--border, #334155)",
                borderRadius: 6,
                padding: "0.35rem 0.6rem",
                color: "var(--text-primary, #e2e8f0)",
                fontSize: "0.8rem",
                fontFamily: "var(--font-mono, monospace)",
              }}
            />
            <button type="button"
              onClick={() => {
                if (isBatch && item.goal_id) {
                  onBatchDeny(item.goal_id, denyReason || undefined);
                } else {
                  onDeny(item.consent_id, denyReason || undefined);
                }
                setShowDenyReason(false);
                setDenyReason("");
              }}
              style={{
                background: "rgba(248, 113, 113, 0.15)",
                border: "1px solid #f87171",
                color: "#f87171",
                padding: "0.35rem 0.8rem",
                borderRadius: 6,
                cursor: "pointer",
                fontFamily: "var(--font-mono, monospace)",
                fontSize: "0.8rem",
              }}
            >
              Deny
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Pulse animation style (injected once) ──

const PULSE_STYLE_ID = "nexus-approval-pulse";
function ensurePulseStyle() {
  if (document.getElementById(PULSE_STYLE_ID)) return;
  const style = document.createElement("style");
  style.id = PULSE_STYLE_ID;
  style.textContent = `
    @keyframes nexus-pending-pulse {
      0%, 100% { box-shadow: 0 0 0 0 rgba(250, 204, 21, 0.25); }
      50% { box-shadow: 0 0 12px 4px rgba(250, 204, 21, 0.15); }
    }
    .nexus-pending-card { animation: nexus-pending-pulse 2s ease-in-out infinite; }
  `;
  document.head.appendChild(style);
}

// ── Stats Bar ──

function StatsBar({ stats }: { stats: HitlStats | null }) {
  if (!stats) return null;
  const items = [
    { label: "Pending", value: String(stats.pending_count), color: stats.pending_count > 0 ? "#facc15" : "#4ade80" },
    { label: "Approval Rate", value: `${Math.round(stats.approval_rate * 100)}%`, color: "#60a5fa" },
    { label: "Avg Response", value: stats.avg_response_time_ms > 0 ? `${(stats.avg_response_time_ms / 1000).toFixed(1)}s` : "—", color: "#a78bfa" },
    { label: "Today", value: String(stats.total_decisions_today), color: "#34d399" },
  ];
  return (
    <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: "0.75rem", marginBottom: "1.5rem" }}>
      {items.map((item) => (
        <div
          key={item.label}
          style={{
            background: "var(--bg-secondary, #1e293b)",
            border: "1px solid var(--border, #334155)",
            borderRadius: 10,
            padding: "0.75rem 1rem",
            textAlign: "center",
          }}
        >
          <div style={{ fontSize: "1.5rem", fontWeight: 700, color: item.color, fontFamily: "var(--font-mono, monospace)" }}>
            {item.value}
          </div>
          <div style={{ fontSize: "0.75rem", color: "var(--text-secondary, #94a3b8)", marginTop: "0.2rem" }}>
            {item.label}
          </div>
        </div>
      ))}
    </div>
  );
}

// ── Main Component ──

type LedgerTab = "all" | "pending" | "approved" | "denied";

const SURFACE_LABELS: Record<string, string> = {
  chat: "Chat",
  agents: "Agents",
  mobile: "Mobile",
  approvals: "Approvals",
  system: "System",
  unknown: "Unknown",
};

function surfaceBadgeColor(surface: string): { bg: string; border: string; text: string } {
  switch (surface) {
    case "chat": return { bg: "rgba(96, 165, 250, 0.12)", border: "#60a5fa44", text: "#93c5fd" };
    case "agents": return { bg: "rgba(168, 85, 247, 0.12)", border: "#a855f744", text: "#c084fc" };
    case "mobile": return { bg: "rgba(34, 211, 238, 0.12)", border: "#22d3ee44", text: "#67e8f9" };
    case "system": return { bg: "rgba(251, 191, 36, 0.12)", border: "#fbbf2444", text: "#fcd34d" };
    default: return { bg: "rgba(148, 163, 184, 0.12)", border: "#94a3b844", text: "#94a3b8" };
  }
}

function statusColor(status: string): string {
  switch (status) {
    case "approved": return "#4ade80";
    case "denied": return "#f87171";
    case "expired": return "#fb923c";
    case "cancelled": return "#94a3b8";
    default: return "#facc15";
  }
}

function formatTimestamp(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString(undefined, {
      month: "short", day: "numeric",
      hour: "2-digit", minute: "2-digit", second: "2-digit",
    });
  } catch {
    return iso;
  }
}

export default function ApprovalCenter(): JSX.Element {
  const [pending, setPending] = useState<ConsentNotification[]>([]);
  const [history, setHistory] = useState<ConsentNotification[]>([]);
  const [stats, setStats] = useState<HitlStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<LedgerTab>("all");
  const unlistenRef = useRef<(() => void)[]>([]);
  const isDesktop = hasDesktopRuntime();

  // Inject pulse animation CSS
  useEffect(() => { ensurePulseStyle(); }, []);

  const loadData = useCallback(async () => {
    if (!isDesktop) {
      setLoading(false);
      return;
    }
    try {
      const [p, h, s] = await Promise.all([
        listPendingConsents(),
        getConsentHistory(50),
        hitlStats().catch(() => null),
      ]);
      setPending(p);
      setHistory(h);
      if (s) setStats(s);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [isDesktop]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // Polling fallback — refresh pending every 2 seconds
  useEffect(() => {
    if (!isDesktop) return;
    const interval = setInterval(() => {
      listPendingConsents()
        .then((p) => setPending(p))
        .catch((e) => { if (import.meta.env.DEV) console.warn("[ApprovalCenter]", e); });
      hitlStats()
        .then((s) => setStats(s))
        .catch((e) => { if (import.meta.env.DEV) console.warn("[ApprovalCenter]", e); });
    }, 2000);
    return () => clearInterval(interval);
  }, [isDesktop]);

  // Listen for real-time consent events
  useEffect(() => {
    if (!isDesktop) return;

    import("@tauri-apps/api/event").then((mod) => {
      // New consent request
      mod
        .listen<ConsentNotification>("consent-request-pending", (event) => {
          setPending((prev) => {
            const next = prev.filter((item) => item.consent_id !== event.payload.consent_id);
            return [event.payload, ...next];
          });
          // Desktop notification
          if (typeof Notification !== "undefined" && Notification.permission === "granted") {
            new Notification("Nexus OS \u2014 Agent Approval Required", {
              body: `${event.payload.agent_name} wants to: ${event.payload.operation_summary}`,
            });
          }
        })
        .then((fn) => {
          unlistenRef.current.push(fn);
        });

      // Consent resolved
      mod
        .listen<{ consent_id: string; status: string }>(
          "consent-resolved",
          (event) => {
            setPending((prev) =>
              prev.filter((p) => p.consent_id !== event.payload.consent_id)
            );
            // Refresh history
            getConsentHistory(20)
              .then((h) => setHistory(h))
              .catch((e) => { if (import.meta.env.DEV) console.warn("[ApprovalCenter]", e); });
          }
        )
        .then((fn) => {
          unlistenRef.current.push(fn);
        });
    });

    // Request notification permission
    if (typeof Notification !== "undefined" && Notification.permission === "default") {
      Notification.requestPermission();
    }

    return () => {
      for (const fn of unlistenRef.current) fn();
      unlistenRef.current = [];
    };
  }, [isDesktop]);

  const handleApprove = async (consentId: string) => {
    try {
      await approveConsentRequest(consentId, "user");
      await loadData();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleDeny = async (consentId: string, reason?: string) => {
    try {
      await denyConsentRequest(consentId, "user", reason);
      await loadData();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleBatchApprove = async (goalId: string) => {
    try {
      await batchApproveConsents(goalId, "user");
      await loadData();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleBatchDeny = async (goalId: string, reason?: string) => {
    try {
      await batchDenyConsents(goalId, "user", reason);
      await loadData();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleReviewEach = async (consentId: string) => {
    try {
      await reviewConsentBatch(consentId, "user");
      await loadData();
    } catch (err) {
      setError(String(err));
    }
  };

  if (loading) {
    return (
      <div style={{ padding: "2rem", textAlign: "center", color: "var(--text-secondary, #94a3b8)" }}>
        Loading approval queue...
      </div>
    );
  }

  return (
    <div style={{ padding: "1.5rem", maxWidth: 900, margin: "0 auto" }}>
      {/* Header */}
      <div style={{ display: "flex", alignItems: "center", gap: "0.6rem", marginBottom: "1.5rem" }}>
        <Shield size={22} style={{ color: "var(--accent, #60a5fa)" }} />
        <h2
          style={{
            fontFamily: "var(--font-display, monospace)",
            color: "var(--text-primary, #e2e8f0)",
            margin: 0,
            fontSize: "1.3rem",
          }}
        >
          Approval Center
        </h2>
        {pending.length > 0 && (
          <span
            style={{
              background: "#f87171",
              color: "#fff",
              borderRadius: 10,
              padding: "0.1rem 0.5rem",
              fontSize: "0.75rem",
              fontWeight: 700,
            }}
          >
            {pending.length}
          </span>
        )}
      </div>

      {/* Stats Bar */}
      {isDesktop && <StatsBar stats={stats} />}

      {error && (
        <div
          style={{
            background: "rgba(248, 113, 113, 0.1)",
            border: "1px solid #f87171",
            borderRadius: 8,
            padding: "0.6rem 1rem",
            color: "#f87171",
            fontSize: "0.85rem",
            marginBottom: "1rem",
            display: "flex",
            alignItems: "center",
            gap: "0.4rem",
          }}
        >
          <AlertTriangle size={14} /> {error}
        </div>
      )}

      {!isDesktop && (
        <div
          style={{
            background: "var(--bg-secondary, #1e293b)",
            border: "1px solid var(--border, #334155)",
            borderRadius: 10,
            padding: "2rem",
            textAlign: "center",
            color: "var(--text-secondary, #94a3b8)",
          }}
        >
          <Shield size={40} style={{ marginBottom: "0.75rem", opacity: 0.4 }} />
          <p style={{ margin: 0 }}>Desktop runtime required for live consent approvals.</p>
          <p style={{ margin: "0.5rem 0 0", fontSize: "0.85rem" }}>
            In the Tauri desktop app, pending agent requests will appear here in real-time.
          </p>
        </div>
      )}

      {/* Tab bar */}
      {isDesktop && (
        <div style={{ display: "flex", gap: "0.25rem", marginBottom: "1.25rem" }}>
          {(["all", "pending", "approved", "denied"] as LedgerTab[]).map((tab) => {
            const active = activeTab === tab;
            let count = 0;
            if (tab === "all") count = pending.length + history.length;
            else if (tab === "pending") count = pending.length;
            else count = history.filter((h) => h.status === tab).length;
            return (
              <button
                key={tab}
                type="button"
                onClick={() => setActiveTab(tab)}
                style={{
                  background: active ? "rgba(96, 165, 250, 0.15)" : "transparent",
                  border: active ? "1px solid #60a5fa44" : "1px solid transparent",
                  color: active ? "#93c5fd" : "#94a3b8",
                  padding: "0.35rem 0.9rem",
                  borderRadius: 6,
                  cursor: "pointer",
                  fontFamily: "var(--font-mono, monospace)",
                  fontSize: "0.8rem",
                  fontWeight: active ? 600 : 400,
                  textTransform: "capitalize",
                }}
              >
                {tab}{count > 0 ? ` (${count})` : ""}
              </button>
            );
          })}
        </div>
      )}

      {/* Pending section — visible on "all" and "pending" tabs */}
      {isDesktop && (activeTab === "all" || activeTab === "pending") && (
        <div style={{ marginBottom: "2rem" }}>
          <h3
            style={{
              fontFamily: "var(--font-display, monospace)",
              color: "var(--text-primary, #e2e8f0)",
              fontSize: "1rem",
              marginBottom: "0.75rem",
            }}
          >
            Pending Requests ({pending.length})
          </h3>
          {pending.length === 0 ? (
            <div
              style={{
                background: "var(--bg-secondary, #1e293b)",
                border: "1px solid var(--border, #334155)",
                borderRadius: 10,
                padding: "1.5rem",
                textAlign: "center",
                color: "var(--text-secondary, #94a3b8)",
                fontSize: "0.9rem",
              }}
            >
              <CheckCircle size={24} style={{ marginBottom: "0.5rem", opacity: 0.4 }} />
              <p style={{ margin: 0 }}>No pending approval requests</p>
            </div>
          ) : (
            pending.map((item) => (
              <PendingCard
                key={item.consent_id}
                item={item}
                onApprove={(id) => void handleApprove(id)}
                onDeny={(id, reason) => void handleDeny(id, reason)}
                onBatchApprove={(goalId) => void handleBatchApprove(goalId)}
                onBatchDeny={(goalId, reason) => void handleBatchDeny(goalId, reason)}
                onReviewEach={(consentId) => void handleReviewEach(consentId)}
              />
            ))
          )}
        </div>
      )}

      {/* Ledger / History section */}
      {isDesktop && (() => {
        const filteredHistory = activeTab === "all"
          ? history
          : activeTab === "pending"
            ? []
            : history.filter((h) => h.status === activeTab);
        if (activeTab === "pending" || filteredHistory.length === 0) return null;
        return (
          <div>
            <h3
              style={{
                fontFamily: "var(--font-display, monospace)",
                color: "var(--text-primary, #e2e8f0)",
                fontSize: "1rem",
                marginBottom: "0.75rem",
              }}
            >
              {activeTab === "all" ? "Approval Ledger" : `${activeTab.charAt(0).toUpperCase() + activeTab.slice(1)} Decisions`}
            </h3>
            <div
              style={{
                background: "var(--bg-secondary, #1e293b)",
                border: "1px solid var(--border, #334155)",
                borderRadius: 10,
                overflow: "hidden",
              }}
            >
              {/* Table header */}
              <div style={{
                display: "grid",
                gridTemplateColumns: "auto 1fr auto auto auto auto",
                gap: "0.5rem",
                padding: "0.5rem 1rem",
                fontSize: "0.7rem",
                color: "#64748b",
                fontWeight: 600,
                textTransform: "uppercase",
                borderBottom: "1px solid var(--border, #334155)",
                letterSpacing: "0.05em",
              }}>
                <span>Status</span>
                <span>Action</span>
                <span>Source</span>
                <span>Risk</span>
                <span>Created</span>
                <span>Resolved</span>
              </div>
              {filteredHistory.map((item, i) => {
                const rc = riskColor(item.risk_level);
                const sc = surfaceBadgeColor(item.source_surface);
                return (
                  <div
                    key={item.consent_id + i}
                    style={{
                      display: "grid",
                      gridTemplateColumns: "auto 1fr auto auto auto auto",
                      gap: "0.5rem",
                      padding: "0.55rem 1rem",
                      borderBottom: i < filteredHistory.length - 1 ? "1px solid var(--border, #334155)" : "none",
                      alignItems: "center",
                    }}
                  >
                    {/* Status icon + label */}
                    <div style={{ display: "flex", alignItems: "center", gap: "0.3rem" }}>
                      {item.status === "approved" ? (
                        <CheckCircle size={13} style={{ color: "#4ade80" }} />
                      ) : item.status === "denied" ? (
                        <XCircle size={13} style={{ color: "#f87171" }} />
                      ) : item.status === "expired" ? (
                        <Clock size={13} style={{ color: "#fb923c" }} />
                      ) : (
                        <Clock size={13} style={{ color: "#facc15" }} />
                      )}
                      <span style={{
                        color: statusColor(item.status),
                        fontSize: "0.72rem",
                        fontWeight: 600,
                        textTransform: "uppercase",
                      }}>
                        {item.status}
                      </span>
                    </div>
                    {/* Action summary */}
                    <div style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      <span style={{ color: "var(--text-primary, #e2e8f0)", fontSize: "0.82rem" }}>
                        {item.agent_name}
                      </span>
                      <span style={{ color: "var(--text-secondary, #64748b)", fontSize: "0.78rem", marginLeft: "0.4rem" }}>
                        {item.operation_summary}
                      </span>
                    </div>
                    {/* Source surface badge */}
                    <span style={{
                      background: sc.bg,
                      border: `1px solid ${sc.border}`,
                      color: sc.text,
                      padding: "0.1rem 0.4rem",
                      borderRadius: 4,
                      fontSize: "0.68rem",
                      fontWeight: 500,
                      whiteSpace: "nowrap",
                    }}>
                      {SURFACE_LABELS[item.source_surface] ?? item.source_surface}
                    </span>
                    {/* Risk badge */}
                    <span style={{
                      background: rc.bg,
                      border: `1px solid ${rc.border}`,
                      color: rc.text,
                      padding: "0.1rem 0.4rem",
                      borderRadius: 4,
                      fontSize: "0.68rem",
                      fontWeight: 600,
                    }}>
                      {item.risk_level}
                    </span>
                    {/* Created timestamp */}
                    <span style={{ color: "#94a3b8", fontSize: "0.72rem", whiteSpace: "nowrap" }}>
                      {formatTimestamp(item.requested_at)}
                    </span>
                    {/* Resolved timestamp + by */}
                    <span style={{ color: "#94a3b8", fontSize: "0.72rem", whiteSpace: "nowrap" }}>
                      {item.resolved_at ? (
                        <>
                          {formatTimestamp(item.resolved_at)}
                          {item.resolved_by && (
                            <span style={{ color: "#64748b", marginLeft: "0.3rem" }}>
                              by {item.resolved_by}
                            </span>
                          )}
                        </>
                      ) : (
                        "\u2014"
                      )}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        );
      })()}
    </div>
  );
}

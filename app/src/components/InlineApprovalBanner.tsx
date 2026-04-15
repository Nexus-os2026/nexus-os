import { useCallback, useEffect, useRef, useState } from "react";
import { Shield, Check, X, ChevronDown, ChevronUp } from "lucide-react";
import {
  approveConsentRequest,
  batchApproveConsents,
  denyConsentRequest,
  hasDesktopRuntime,
  listPendingConsents,
} from "../api/backend";
import type { ConsentNotification } from "../types";

const RISK_COLORS: Record<string, string> = {
  Critical: "#ef4444",
  High: "#f97316",
  Medium: "#eab308",
  Low: "#22c55e",
};

function riskColor(level: string): string {
  return RISK_COLORS[level] ?? "#64748b";
}

function timeLeft(autoDenyAt: string): string {
  const ms = new Date(autoDenyAt).getTime() - Date.now();
  if (ms <= 0) return "expired";
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}s`;
  return `${Math.floor(secs / 60)}m ${secs % 60}s`;
}

interface InlineApprovalBannerProps {
  currentPage?: string;
}

export function InlineApprovalBanner({
  currentPage,
}: InlineApprovalBannerProps = {}): JSX.Element | null {
  const [pending, setPending] = useState<ConsentNotification[]>([]);
  const [expanded, setExpanded] = useState(false);
  const [acting, setActing] = useState<string | null>(null);
  const pollRef = useRef<number>(0);

  // Poll for pending consents
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    const poll = () => {
      listPendingConsents()
        .then(setPending)
        .catch(() => {});
    };
    poll();
    pollRef.current = window.setInterval(poll, 2000);
    return () => clearInterval(pollRef.current);
  }, []);

  // Listen for real-time events
  useEffect(() => {
    if (!hasDesktopRuntime()) return;
    let cancelled = false;
    const cleanups: (() => void)[] = [];

    (async () => {
      try {
        const mod = await import("@tauri-apps/api/event");
        if (cancelled) return;

        const u1 = await mod.listen<ConsentNotification>(
          "consent-request-pending",
          (event) => {
            setPending((prev) => {
              const filtered = prev.filter(
                (c) => c.consent_id !== event.payload.consent_id,
              );
              return [event.payload, ...filtered];
            });
          },
        );
        cleanups.push(u1);

        const u2 = await mod.listen<{ consent_id: string }>(
          "consent-resolved",
          (event) => {
            setPending((prev) =>
              prev.filter((c) => c.consent_id !== event.payload.consent_id),
            );
          },
        );
        cleanups.push(u2);
      } catch {
        // Event bridge unavailable — polling handles it
      }
    })();

    return () => {
      cancelled = true;
      cleanups.forEach((fn) => fn());
    };
  }, []);

  const handleApprove = useCallback(
    async (consent: ConsentNotification) => {
      setActing(consent.consent_id);
      try {
        await approveConsentRequest(consent.consent_id, "user");
        setPending((prev) =>
          prev.filter((c) => c.consent_id !== consent.consent_id),
        );
      } catch (e) {
        console.error("[approval] approve failed:", e);
      } finally {
        setActing(null);
      }
    },
    [],
  );

  const handleApproveAll = useCallback(
    async (consent: ConsentNotification) => {
      if (!consent.goal_id) return handleApprove(consent);
      setActing(consent.consent_id);
      try {
        await batchApproveConsents(consent.goal_id, "user");
        setPending((prev) =>
          prev.filter((c) => c.goal_id !== consent.goal_id),
        );
      } catch (e) {
        console.error("[approval] batch approve failed:", e);
      } finally {
        setActing(null);
      }
    },
    [handleApprove],
  );

  const handleDeny = useCallback(
    async (consent: ConsentNotification) => {
      setActing(consent.consent_id);
      try {
        await denyConsentRequest(consent.consent_id, "user", "User denied");
        setPending((prev) =>
          prev.filter((c) => c.consent_id !== consent.consent_id),
        );
      } catch (e) {
        console.error("[approval] deny failed:", e);
      } finally {
        setActing(null);
      }
    },
    [],
  );

  // Countdown timer
  const [, setTick] = useState(0);
  useEffect(() => {
    if (pending.length === 0) return;
    const t = window.setInterval(() => setTick((n) => n + 1), 1000);
    return () => clearInterval(t);
  }, [pending.length]);

  // Fix G9: when the user is on the Chat page, AiChatHub's consent-pending
  // listener already surfaces every pending consent inline in the active
  // conversation, so suppress the floating banner to avoid duplicate UX.
  // Banner remains the canonical surface on every non-chat page.
  const onChatPage = currentPage === "ai-chat-hub" || currentPage === "chat";
  const visiblePending = onChatPage ? [] : pending;

  if (visiblePending.length === 0) return null;

  const top = visiblePending[0];
  const rest = visiblePending.slice(1);
  const color = riskColor(top.risk_level);

  return (
    <div
      style={{
        position: "fixed",
        bottom: 32,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 9999,
        width: "min(640px, calc(100vw - 48px))",
        animation: "slideUp 0.25s ease-out",
      }}
    >
      <style>{`
        @keyframes slideUp {
          from { opacity: 0; transform: translateX(-50%) translateY(20px); }
          to { opacity: 1; transform: translateX(-50%) translateY(0); }
        }
      `}</style>

      {/* Stacked indicator */}
      {rest.length > 0 && expanded && (
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            gap: 6,
            marginBottom: 6,
          }}
        >
          {rest.map((c) => (
            <div
              key={c.consent_id}
              style={{
                background: "rgba(10, 22, 40, 0.95)",
                border: `1px solid ${riskColor(c.risk_level)}33`,
                borderRadius: 10,
                padding: "10px 16px",
                backdropFilter: "blur(16px)",
                display: "flex",
                alignItems: "center",
                gap: 10,
              }}
            >
              <Shield
                size={14}
                style={{ color: riskColor(c.risk_level), flexShrink: 0 }}
              />
              <div style={{ flex: 1, minWidth: 0 }}>
                <div
                  style={{
                    fontSize: 13,
                    color: "#e2e8f0",
                    fontWeight: 600,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {c.agent_name}: {c.operation_summary}
                </div>
              </div>
              <div style={{ display: "flex", gap: 6, flexShrink: 0 }}>
                <button type="button"
                  onClick={() => handleApprove(c)}
                  disabled={acting === c.consent_id}
                  style={{
                    background: "rgba(34,197,94,0.15)",
                    border: "1px solid rgba(34,197,94,0.3)",
                    borderRadius: 6,
                    padding: "4px 12px",
                    color: "#22c55e",
                    cursor: "pointer",
                    fontSize: 12,
                    fontWeight: 600,
                  }}
                >
                  Approve
                </button>
                <button type="button"
                  onClick={() => handleDeny(c)}
                  disabled={acting === c.consent_id}
                  style={{
                    background: "rgba(239,68,68,0.1)",
                    border: "1px solid rgba(239,68,68,0.2)",
                    borderRadius: 6,
                    padding: "4px 12px",
                    color: "#ef4444",
                    cursor: "pointer",
                    fontSize: 12,
                    fontWeight: 600,
                  }}
                >
                  Deny
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Primary banner */}
      <div
        style={{
          background: "rgba(10, 22, 40, 0.97)",
          border: `1px solid ${color}44`,
          borderRadius: 12,
          padding: "14px 20px",
          backdropFilter: "blur(20px)",
          boxShadow: `0 8px 32px rgba(0,0,0,0.5), 0 0 0 1px rgba(0,0,0,0.2), inset 0 1px 0 rgba(255,255,255,0.03)`,
        }}
      >
        {/* Header row */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 12,
          }}
        >
          <div
            style={{
              width: 32,
              height: 32,
              borderRadius: 8,
              background: `${color}18`,
              border: `1px solid ${color}33`,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              flexShrink: 0,
            }}
          >
            <Shield size={16} style={{ color }} />
          </div>

          <div style={{ flex: 1, minWidth: 0 }}>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
              }}
            >
              <span
                style={{
                  fontSize: 14,
                  fontWeight: 600,
                  color: "#e2e8f0",
                }}
              >
                {top.agent_name}
              </span>
              <span
                style={{
                  fontSize: 10,
                  fontWeight: 700,
                  color,
                  background: `${color}18`,
                  padding: "1px 6px",
                  borderRadius: 4,
                  textTransform: "uppercase",
                }}
              >
                {top.risk_level}
              </span>
              <span
                style={{
                  fontSize: 11,
                  color: "#64748b",
                  fontFamily: "monospace",
                }}
              >
                {timeLeft(top.auto_deny_at)}
              </span>
            </div>
            <div
              style={{
                fontSize: 13,
                color: "#94a3b8",
                marginTop: 2,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {top.operation_summary}
            </div>
            {top.fuel_cost_estimate > 0 && (
              <span
                style={{
                  fontSize: 11,
                  color: "#475569",
                  marginTop: 2,
                  display: "inline-block",
                }}
              >
                ~{top.fuel_cost_estimate} fuel
                {top.side_effects_preview.length > 0 &&
                  ` \u00B7 ${top.side_effects_preview.slice(0, 2).join(", ")}`}
              </span>
            )}
          </div>

          {/* Action buttons */}
          <div
            style={{
              display: "flex",
              gap: 8,
              flexShrink: 0,
              alignItems: "center",
            }}
          >
            <button type="button"
              onClick={() => handleApprove(top)}
              disabled={acting === top.consent_id}
              style={{
                background: "rgba(34,197,94,0.15)",
                border: "1px solid rgba(34,197,94,0.35)",
                borderRadius: 8,
                padding: "8px 16px",
                color: "#22c55e",
                cursor: "pointer",
                fontSize: 13,
                fontWeight: 600,
                fontFamily: "monospace",
                display: "flex",
                alignItems: "center",
                gap: 6,
              }}
            >
              <Check size={14} /> Approve
            </button>
            {top.goal_id && (
              <button type="button"
                onClick={() => handleApproveAll(top)}
                disabled={acting === top.consent_id}
                style={{
                  background: "rgba(6,182,212,0.1)",
                  border: "1px solid rgba(6,182,212,0.25)",
                  borderRadius: 8,
                  padding: "8px 12px",
                  color: "#06b6d4",
                  cursor: "pointer",
                  fontSize: 12,
                  fontWeight: 600,
                  fontFamily: "monospace",
                }}
              >
                Approve All
              </button>
            )}
            <button type="button"
              onClick={() => handleDeny(top)}
              disabled={acting === top.consent_id}
              style={{
                background: "rgba(239,68,68,0.1)",
                border: "1px solid rgba(239,68,68,0.25)",
                borderRadius: 8,
                padding: "8px 16px",
                color: "#ef4444",
                cursor: "pointer",
                fontSize: 13,
                fontWeight: 600,
                fontFamily: "monospace",
                display: "flex",
                alignItems: "center",
                gap: 6,
              }}
            >
              <X size={14} /> Deny
            </button>
          </div>
        </div>

        {/* Queue indicator */}
        {rest.length > 0 && (
          <button type="button"
            onClick={() => setExpanded(!expanded)}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              marginTop: 8,
              background: "none",
              border: "none",
              color: "#64748b",
              cursor: "pointer",
              fontSize: 11,
              padding: 0,
            }}
          >
            {expanded ? (
              <ChevronDown size={12} />
            ) : (
              <ChevronUp size={12} />
            )}
            {rest.length} more pending approval
            {rest.length !== 1 ? "s" : ""}
          </button>
        )}
      </div>
    </div>
  );
}

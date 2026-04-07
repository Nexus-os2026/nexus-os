/**
 * CollabToolbar — collaboration status bar for the builder.
 *
 * Shows current mode (Solo/Hosting/Connected) with action buttons.
 * All inline styles per project convention.
 */

import { useState, useCallback } from "react";
import {
  builderCollabStartHosting,
  builderCollabLeave,
  builderCollabInvite,
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
  green: "#22c55e",
  greenDim: "rgba(34,197,94,0.12)",
  blue: "#3b82f6",
  blueDim: "rgba(59,130,246,0.12)",
  sans: "system-ui,-apple-system,sans-serif",
};

type CollabMode = "solo" | "hosting" | "connected";

interface CollabToolbarProps {
  projectId: string;
  mode: CollabMode;
  connectedCount: number;
  hostName?: string;
  onModeChange: (mode: CollabMode, session?: any) => void;
}

export default function CollabToolbar({
  projectId,
  mode,
  connectedCount,
  hostName,
  onModeChange,
}: CollabToolbarProps) {
  const [loading, setLoading] = useState(false);
  const [inviteLink, setInviteLink] = useState("");

  const startHosting = useCallback(async () => {
    setLoading(true);
    try {
      const session = await builderCollabStartHosting(projectId);
      onModeChange("hosting", session);
    } catch (e: any) {
      console.error("Failed to start hosting:", e);
    }
    setLoading(false);
  }, [projectId, onModeChange]);

  const stopHosting = useCallback(async () => {
    try {
      await builderCollabLeave(projectId);
      onModeChange("solo");
      setInviteLink("");
    } catch (e: any) {
      console.error("Failed to stop hosting:", e);
    }
  }, [projectId, onModeChange]);

  const copyInvite = useCallback(async () => {
    try {
      const link = await builderCollabInvite(projectId, "editor");
      setInviteLink(link);
      await navigator.clipboard.writeText(link);
    } catch (e: any) {
      console.error("Failed to generate invite:", e);
    }
  }, [projectId]);

  const leaveSession = useCallback(async () => {
    try {
      await builderCollabLeave(projectId);
      onModeChange("solo");
    } catch (e: any) {
      console.error("Failed to leave:", e);
    }
  }, [projectId, onModeChange]);

  // Solo mode
  if (mode === "solo") {
    return (
      <button
        onClick={startHosting}
        disabled={loading}
        style={{
          background: C.blueDim,
          border: "1px solid rgba(59,130,246,0.2)",
          borderRadius: 4,
          padding: "4px 10px",
          color: C.blue,
          fontSize: 10,
          cursor: loading ? "default" : "pointer",
          fontWeight: 500,
          fontFamily: C.sans,
          opacity: loading ? 0.6 : 1,
        }}
      >
        {loading ? "Starting..." : "Share Project"}
      </button>
    );
  }

  // Hosting mode
  if (mode === "hosting") {
    return (
      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
        <div style={{
          display: "flex", alignItems: "center", gap: 4,
          background: C.greenDim, borderRadius: 4, padding: "3px 8px",
        }}>
          <span style={{ width: 6, height: 6, borderRadius: "50%", background: C.green }} />
          <span style={{ color: C.green, fontSize: 10, fontWeight: 600 }}>
            Hosting
          </span>
          {connectedCount > 0 && (
            <span style={{ color: C.muted, fontSize: 9 }}>
              | {connectedCount} connected
            </span>
          )}
        </div>
        <button onClick={copyInvite} style={{
          background: "transparent", border: `1px solid ${C.border}`,
          borderRadius: 3, padding: "2px 8px", color: C.muted, fontSize: 9,
          cursor: "pointer", fontFamily: C.sans,
        }}>
          {inviteLink ? "Copied!" : "Invite Link"}
        </button>
        <button onClick={stopHosting} style={{
          background: "transparent", border: `1px solid ${C.border}`,
          borderRadius: 3, padding: "2px 8px", color: C.dim, fontSize: 9,
          cursor: "pointer", fontFamily: C.sans,
        }}>
          Stop
        </button>
      </div>
    );
  }

  // Connected mode
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
      <div style={{
        display: "flex", alignItems: "center", gap: 4,
        background: C.greenDim, borderRadius: 4, padding: "3px 8px",
      }}>
        <span style={{ width: 6, height: 6, borderRadius: "50%", background: C.green }} />
        <span style={{ color: C.green, fontSize: 10, fontWeight: 600 }}>
          Connected{hostName ? ` to ${hostName}` : ""}
        </span>
      </div>
      <button onClick={leaveSession} style={{
        background: "transparent", border: `1px solid ${C.border}`,
        borderRadius: 3, padding: "2px 8px", color: C.dim, fontSize: 9,
        cursor: "pointer", fontFamily: C.sans,
      }}>
        Leave
      </button>
    </div>
  );
}

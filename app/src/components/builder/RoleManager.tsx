/**
 * RoleManager — panel for project owner to manage collaborator roles.
 *
 * Shows participant list with role management and invite link generation.
 * All inline styles per project convention.
 */

import { useState, useCallback } from "react";
import {
  builderCollabSetRole,
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
  amber: "#f59e0b",
  sans: "system-ui,-apple-system,sans-serif",
};

const ROLE_ICONS: Record<string, string> = {
  Owner: "\uD83D\uDC51",     // crown
  Editor: "\u270F\uFE0F",    // pencil
  Commenter: "\uD83D\uDCAC", // speech bubble
  Viewer: "\uD83D\uDC41",    // eye
};

interface Participant {
  public_key: string;
  display_name: string;
  color: string;
  role: string;
}

interface RoleManagerProps {
  projectId: string;
  participants: Participant[];
  isOwner: boolean;
  onRoleChanged?: () => void;
}

export default function RoleManager({
  projectId,
  participants,
  isOwner,
  onRoleChanged,
}: RoleManagerProps) {
  const [inviteLink, setInviteLink] = useState("");
  const [changingRole, setChangingRole] = useState<string | null>(null);

  const copyInvite = useCallback(async () => {
    try {
      const link = await builderCollabInvite(projectId, "editor");
      setInviteLink(link);
      await navigator.clipboard.writeText(link);
      setTimeout(() => setInviteLink(""), 3000);
    } catch (e: any) {
      console.error("Invite:", e);
    }
  }, [projectId]);

  const changeRole = useCallback(async (publicKey: string, role: string) => {
    setChangingRole(publicKey);
    try {
      await builderCollabSetRole(projectId, publicKey, role);
      onRoleChanged?.();
    } catch (e: any) {
      console.error("Role change:", e);
    }
    setChangingRole(null);
  }, [projectId, onRoleChanged]);

  return (
    <div style={{
      background: C.surfaceAlt, border: `1px solid ${C.border}`, borderRadius: 6,
      padding: 12, fontFamily: C.sans, minWidth: 260,
    }}>
      <div style={{ color: C.text, fontSize: 11, fontWeight: 600, marginBottom: 10 }}>
        Collaborators
      </div>

      {participants.map((p) => (
        <div key={p.public_key} style={{
          display: "flex", alignItems: "center", gap: 8,
          padding: "6px 0", borderBottom: `1px solid ${C.border}`,
        }}>
          {/* Color dot */}
          <span style={{
            width: 8, height: 8, borderRadius: "50%", background: p.color, flexShrink: 0,
          }} />

          {/* Name + key */}
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ color: C.text, fontSize: 10, fontWeight: 500 }}>
              {ROLE_ICONS[p.role] || ""} {p.display_name}
              {p.role === "Owner" && (
                <span style={{ color: C.amber, fontSize: 8, marginLeft: 4 }}>(you)</span>
              )}
            </div>
            <div style={{ color: C.dim, fontSize: 8, fontFamily: "monospace", overflow: "hidden", textOverflow: "ellipsis" }}>
              {p.public_key.slice(0, 8)}...{p.public_key.slice(-4)}
            </div>
          </div>

          {/* Role selector (Owner only, not for self) */}
          {isOwner && p.role !== "Owner" && (
            <select
              value={p.role.toLowerCase()}
              onChange={(e) => changeRole(p.public_key, e.target.value)}
              disabled={changingRole === p.public_key}
              style={{
                background: C.surface, border: `1px solid ${C.border}`,
                borderRadius: 3, color: C.muted, fontSize: 8,
                padding: "2px 4px", cursor: "pointer",
              }}
            >
              <option value="editor">Editor</option>
              <option value="commenter">Commenter</option>
              <option value="viewer">Viewer</option>
            </select>
          )}
        </div>
      ))}

      {/* Invite button */}
      {isOwner && (
        <button onClick={copyInvite} style={{
          width: "100%", marginTop: 8,
          background: C.accentDim, border: `1px solid rgba(0,212,170,0.2)`,
          borderRadius: 4, padding: "6px 0", color: C.accent, fontSize: 10,
          cursor: "pointer", fontWeight: 500,
        }}>
          {inviteLink ? "Copied!" : "Copy Invite Link"}
        </button>
      )}
    </div>
  );
}

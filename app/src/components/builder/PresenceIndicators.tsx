/**
 * PresenceIndicators — shows which sections other collaborators are editing.
 *
 * Renders colored dots next to sections being edited by remote users,
 * with a tooltip showing who. All inline styles per project convention.
 */

import { useState, useEffect } from "react";

const C = {
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  surface: "#111820",
  border: "#1a2332",
  sans: "system-ui,-apple-system,sans-serif",
};

interface RemoteUser {
  display_name: string;
  color: string;
  selected_section: string | null;
}

interface PresenceIndicatorsProps {
  /** Remote users from Yjs awareness */
  remoteUsers: RemoteUser[];
  /** List of section IDs in the current template */
  sectionIds: string[];
}

export default function PresenceIndicators({
  remoteUsers,
  sectionIds,
}: PresenceIndicatorsProps) {
  if (remoteUsers.length === 0) return null;

  // Group users by section
  const bySection: Record<string, RemoteUser[]> = {};
  for (const user of remoteUsers) {
    if (user.selected_section) {
      if (!bySection[user.selected_section]) {
        bySection[user.selected_section] = [];
      }
      bySection[user.selected_section].push(user);
    }
  }

  const activeSections = Object.keys(bySection);
  if (activeSections.length === 0) return null;

  return (
    <div style={{
      display: "flex", flexDirection: "column", gap: 2,
      padding: "4px 8px", fontFamily: C.sans,
    }}>
      {activeSections.map((sectionId) => {
        const users = bySection[sectionId];
        return (
          <div key={sectionId} style={{
            display: "flex", alignItems: "center", gap: 6, fontSize: 9,
          }}>
            {/* User dots */}
            <div style={{ display: "flex", gap: -2 }}>
              {users.map((u, i) => (
                <span
                  key={i}
                  title={u.display_name}
                  style={{
                    width: 8, height: 8, borderRadius: "50%",
                    background: u.color, border: `1px solid ${C.surface}`,
                    marginLeft: i > 0 ? -2 : 0,
                  }}
                />
              ))}
            </div>
            {/* Section + names */}
            <span style={{ color: C.muted }}>
              {users.map((u) => u.display_name).join(", ")}
              {" "}editing <span style={{ color: C.text }}>{sectionId}</span>
            </span>
          </div>
        );
      })}
    </div>
  );
}

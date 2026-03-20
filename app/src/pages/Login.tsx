import { useCallback, useEffect, useState } from "react";
import {
  authLogin,
  authSessionInfo,
  authLogout,
  adminUsersList,
  authConfigGet,
} from "../api/backend";
import "./admin.css";

// ── Types ────────────────────────────────────────────────────────────

interface SessionInfo {
  session_id: string;
  user_id: string;
  name: string;
  email: string;
  role: string;
  authenticated_at: string;
  expires_at: string;
}

interface ActiveSession {
  id: string;
  email: string;
  name: string;
  role: string;
  workspace_ids: string[];
  last_active: string;
  status: string;
  created_at: string;
}

interface AuthConfig {
  provider: string;
  issuer_url: string;
  session_duration_secs: number;
  require_mfa: boolean;
  allowed_domains: string[];
}

// ── Fallback data (used only when Tauri is not available) ─────────────

const FALLBACK_SESSION: SessionInfo = {
  session_id: "",
  user_id: "",
  name: "",
  email: "",
  role: "Viewer",
  authenticated_at: new Date().toISOString(),
  expires_at: new Date(Date.now() + 86_400_000).toISOString(),
};

const FALLBACK_CONFIG: AuthConfig = {
  provider: "local",
  issuer_url: "",
  session_duration_secs: 86400,
  require_mfa: false,
  allowed_domains: [],
};

// ── Helpers ──────────────────────────────────────────────────────────

function roleBadgeClass(role: string): string {
  const r = role.toLowerCase();
  if (r === "admin") return "admin-badge admin-badge--admin";
  if (r === "operator") return "admin-badge admin-badge--operator";
  if (r === "auditor") return "admin-badge admin-badge--auditor";
  return "admin-badge admin-badge--viewer";
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

// ── Component ────────────────────────────────────────────────────────

export default function Login() {
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [sessions, setSessions] = useState<ActiveSession[]>([]);
  const [authConfig, setAuthConfig] = useState<AuthConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [loggingOut, setLoggingOut] = useState(false);

  // ── Initial load ──────────────────────────────────────────────────

  const initialize = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      // 1. Create / resume local session
      const loginResult = await authLogin();
      const sessionId: string = loginResult?.session_id ?? loginResult?.id ?? "";

      // 2. Fetch full session info
      let sessionInfo: SessionInfo;
      if (sessionId) {
        sessionInfo = await authSessionInfo(sessionId);
      } else {
        // loginResult itself may already be the session object
        sessionInfo = loginResult as SessionInfo;
      }
      setSession(sessionInfo ?? FALLBACK_SESSION);

      // 3. Fetch all active sessions for the sessions table
      const userList = await adminUsersList();
      setSessions(Array.isArray(userList) ? userList : []);

      // 4. Fetch auth provider config
      const config = await authConfigGet();
      setAuthConfig(config ?? FALLBACK_CONFIG);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      // Fall back to safe defaults so the UI is still useful
      setSession(FALLBACK_SESSION);
      setAuthConfig(FALLBACK_CONFIG);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void initialize();
  }, [initialize]);

  // ── Logout ────────────────────────────────────────────────────────

  const handleLogout = useCallback(async () => {
    if (!session) return;
    setLoggingOut(true);
    try {
      await authLogout(session.session_id);
      setSession(null);
      setSessions([]);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(`Logout failed: ${msg}`);
    } finally {
      setLoggingOut(false);
    }
  }, [session]);

  // ── Render ────────────────────────────────────────────────────────

  return (
    <div className="admin-shell">
      <h1>Session &amp; Auth</h1>
      <p className="admin-subtitle">
        Current session, active users, and authentication configuration
      </p>

      {error && (
        <div className="admin-alert admin-alert--warn" style={{ marginBottom: "1rem" }}>
          <span>{error}</span>
        </div>
      )}

      {loading ? (
        <div className="admin-empty">Initializing session...</div>
      ) : (
        <>
          {/* ── Current Session ── */}
          <div className="admin-grid-2" style={{ marginBottom: "1rem" }}>
            <div className="admin-card">
              <div className="admin-card__title">Current Session</div>

              {session ? (
                <div style={{ display: "flex", flexDirection: "column", gap: "0.65rem" }}>
                  <SessionRow label="Name" value={session.name} />
                  <SessionRow label="Email" value={session.email} />
                  <SessionRow
                    label="Role"
                    value={
                      <span className={roleBadgeClass(session.role)}>
                        {session.role}
                      </span>
                    }
                  />
                  <SessionRow
                    label="Session ID"
                    value={
                      <span style={{ fontFamily: "var(--font-mono, monospace)", fontSize: "0.72rem" }}>
                        {session.session_id}
                      </span>
                    }
                  />
                  <SessionRow
                    label="Authenticated At"
                    value={formatDate(session.authenticated_at)}
                  />
                  <SessionRow
                    label="Expires At"
                    value={formatDate(session.expires_at)}
                  />

                  <div style={{ marginTop: "0.5rem" }}>
                    <button
                      className="admin-btn admin-btn--danger"
                      onClick={() => void handleLogout()}
                      disabled={loggingOut}
                    >
                      {loggingOut ? "Logging out..." : "Logout"}
                    </button>
                  </div>
                </div>
              ) : (
                <div className="admin-empty">No active session</div>
              )}
            </div>

            {/* ── Auth Config ── */}
            <div className="admin-card">
              <div className="admin-card__title">Auth Configuration</div>

              {authConfig ? (
                <div style={{ display: "flex", flexDirection: "column", gap: "0.65rem" }}>
                  <SessionRow label="Provider" value={authConfig.provider} />
                  <SessionRow
                    label="Issuer URL"
                    value={
                      <span style={{ fontFamily: "var(--font-mono, monospace)", fontSize: "0.72rem", wordBreak: "break-all" }}>
                        {authConfig.issuer_url}
                      </span>
                    }
                  />
                  <SessionRow
                    label="Session Duration"
                    value={formatDuration(authConfig.session_duration_secs)}
                  />
                  <SessionRow
                    label="Require MFA"
                    value={
                      <span
                        style={{
                          color: authConfig.require_mfa
                            ? "var(--nexus-accent, #4af7d3)"
                            : "var(--text-muted, #5e7491)",
                        }}
                      >
                        {authConfig.require_mfa ? "Yes" : "No"}
                      </span>
                    }
                  />
                  {authConfig.allowed_domains.length > 0 && (
                    <SessionRow
                      label="Allowed Domains"
                      value={authConfig.allowed_domains.join(", ")}
                    />
                  )}
                </div>
              ) : (
                <div className="admin-empty">No auth config available</div>
              )}
            </div>
          </div>

          {/* ── Active Sessions Table ── */}
          <div className="admin-card">
            <div className="admin-card__title">Active Users</div>

            {sessions.length === 0 ? (
              <div className="admin-empty">No active sessions</div>
            ) : (
              <table className="admin-table">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Email</th>
                    <th>Role</th>
                    <th>Workspaces</th>
                    <th>Last Active</th>
                    <th>Status</th>
                    <th>Created</th>
                  </tr>
                </thead>
                <tbody>
                  {sessions.map((u) => (
                    <tr key={u.id}>
                      <td style={{ color: "var(--text-primary)", fontWeight: 500 }}>
                        {u.name}
                      </td>
                      <td>{u.email}</td>
                      <td>
                        <span className={roleBadgeClass(u.role)}>{u.role}</span>
                      </td>
                      <td>
                        {Array.isArray(u.workspace_ids)
                          ? u.workspace_ids.join(", ")
                          : "-"}
                      </td>
                      <td>{formatDate(u.last_active)}</td>
                      <td>
                        <span
                          className="admin-dot"
                          style={{
                            background:
                              u.status === "active"
                                ? "var(--nexus-accent, #4af7d3)"
                                : "var(--text-muted, #5e7491)",
                            boxShadow:
                              u.status === "active"
                                ? "0 0 6px rgba(74,247,211,0.4)"
                                : "none",
                          }}
                        />
                        {u.status}
                      </td>
                      <td>{new Date(u.created_at).toLocaleDateString()}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </>
      )}
    </div>
  );
}

// ── Sub-component ─────────────────────────────────────────────────────

function SessionRow({
  label,
  value,
}: {
  label: string;
  value: React.ReactNode;
}) {
  return (
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "flex-start",
        gap: "1rem",
        fontSize: "0.82rem",
      }}
    >
      <span
        style={{
          color: "var(--text-muted, #5e7491)",
          textTransform: "uppercase",
          letterSpacing: "0.06em",
          fontSize: "0.7rem",
          flexShrink: 0,
          paddingTop: "0.1rem",
        }}
      >
        {label}
      </span>
      <span style={{ color: "var(--text-primary, #eef7ff)", textAlign: "right" }}>
        {value}
      </span>
    </div>
  );
}

import { useCallback, useEffect, useState } from "react";
import {
  adminUsersList,
  adminUserCreate,
  adminUserUpdateRole,
  adminUserDeactivate,
} from "../api/backend";
import "./admin.css";

export interface UserDetail {
  id: string;
  email: string;
  name: string;
  role: "Admin" | "Operator" | "Viewer" | "Auditor";
  workspace_ids: string[];
  last_active: string;
  status: "active" | "inactive";
  created_at: string;
}

const ROLES = ["Admin", "Operator", "Viewer", "Auditor"] as const;

function roleBadgeClass(role: string): string {
  return `admin-badge admin-badge--${role.toLowerCase()}`;
}

export default function AdminUsers() {
  const [users, setUsers] = useState<UserDetail[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [createForm, setCreateForm] = useState({ email: "", name: "", role: "Viewer" as UserDetail["role"] });
  const [filter, setFilter] = useState("");

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await adminUsersList();
      setUsers(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load users");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const [createError, setCreateError] = useState<string | null>(null);

  const handleCreate = async () => {
    setCreateError(null);
    try {
      const user = await adminUserCreate(createForm.email, createForm.name, createForm.role);
      setUsers((prev) => [...prev, user]);
      setShowCreate(false);
      setCreateForm({ email: "", name: "", role: "Viewer" });
    } catch (err) {
      setCreateError(err instanceof Error ? err.message : "Failed to create user");
    }
  };

  const handleRoleChange = async (userId: string, role: UserDetail["role"]) => {
    try {
      await adminUserUpdateRole(userId, role);
      setUsers((prev) => prev.map((u) => (u.id === userId ? { ...u, role } : u)));
    } catch {
      /* no-op */
    }
  };

  const handleDeactivate = async (userId: string) => {
    try {
      await adminUserDeactivate(userId);
      setUsers((prev) => prev.map((u) => (u.id === userId ? { ...u, status: "inactive" as const } : u)));
    } catch {
      /* no-op */
    }
  };

  const filtered = filter
    ? users.filter((u) => u.name.toLowerCase().includes(filter.toLowerCase()) || u.email.toLowerCase().includes(filter.toLowerCase()))
    : users;

  return (
    <div className="admin-shell">
      <h1>User Management</h1>
      <p className="admin-subtitle">Manage users, roles, and workspace access</p>

      {error && (
        <div className="admin-error" style={{ marginBottom: "1rem", display: "flex", alignItems: "center", gap: "0.75rem" }}>
          <span>{error}</span>
          <button className="admin-btn admin-btn--sm" onClick={() => void refresh()}>Retry</button>
        </div>
      )}

      <div style={{ display: "flex", gap: "0.6rem", marginBottom: "1rem" }}>
        <input
          className="admin-input"
          style={{ maxWidth: 280 }}
          placeholder="Filter users..."
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
        <button className="admin-btn admin-btn--accent" onClick={() => setShowCreate(!showCreate)}>
          + Add User
        </button>
      </div>

      {showCreate && (
        <div className="admin-card" style={{ marginBottom: "1rem" }}>
          <div className="admin-card__title">Create User</div>
          <div style={{ display: "flex", gap: "0.6rem", flexWrap: "wrap", alignItems: "flex-end" }}>
            <div style={{ flex: 1, minWidth: 180 }}>
              <label style={{ fontSize: "0.72rem", color: "var(--text-muted)", display: "block", marginBottom: "0.25rem" }}>Email</label>
              <input className="admin-input" value={createForm.email} onChange={(e) => setCreateForm((f) => ({ ...f, email: e.target.value }))} />
            </div>
            <div style={{ flex: 1, minWidth: 180 }}>
              <label style={{ fontSize: "0.72rem", color: "var(--text-muted)", display: "block", marginBottom: "0.25rem" }}>Name</label>
              <input className="admin-input" value={createForm.name} onChange={(e) => setCreateForm((f) => ({ ...f, name: e.target.value }))} />
            </div>
            <div style={{ minWidth: 120 }}>
              <label style={{ fontSize: "0.72rem", color: "var(--text-muted)", display: "block", marginBottom: "0.25rem" }}>Role</label>
              <select className="admin-select" value={createForm.role} onChange={(e) => setCreateForm((f) => ({ ...f, role: e.target.value as UserDetail["role"] }))}>
                {ROLES.map((r) => <option key={r} value={r}>{r}</option>)}
              </select>
            </div>
            <button className="admin-btn admin-btn--accent" onClick={() => void handleCreate()}>Create</button>
          </div>
          {createError && (
            <div className="admin-error" style={{ marginTop: "0.5rem" }}>{createError}</div>
          )}
        </div>
      )}

      <div className="admin-card">
        <table className="admin-table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Email</th>
              <th>Role</th>
              <th>Workspaces</th>
              <th>Last Active</th>
              <th>Status</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {loading && (
              <tr><td colSpan={7} style={{ textAlign: "center", padding: "2rem" }}>Loading...</td></tr>
            )}
            {!loading && !error && filtered.length === 0 && (
              <tr><td colSpan={7} className="admin-empty">No users found</td></tr>
            )}
            {filtered.map((u) => (
              <tr key={u.id}>
                <td style={{ color: "var(--text-primary)", fontWeight: 500 }}>{u.name}</td>
                <td>{u.email}</td>
                <td>
                  <select
                    className="admin-select"
                    value={u.role}
                    onChange={(e) => void handleRoleChange(u.id, e.target.value as UserDetail["role"])}
                    style={{ padding: "0.2rem 0.4rem", fontSize: "0.75rem" }}
                  >
                    {ROLES.map((r) => <option key={r} value={r}>{r}</option>)}
                  </select>
                </td>
                <td>{u.workspace_ids.join(", ")}</td>
                <td>{new Date(u.last_active).toLocaleDateString()}</td>
                <td>
                  <span className={roleBadgeClass(u.status === "active" ? u.role : "viewer")} style={u.status !== "active" ? { opacity: 0.5 } : {}}>
                    {u.status}
                  </span>
                </td>
                <td>
                  {u.status === "active" && (
                    <button className="admin-btn admin-btn--danger admin-btn--sm" onClick={() => void handleDeactivate(u.id)}>
                      Deactivate
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

import { useCallback, useEffect, useRef, useState } from "react";
import {
  telemetryStatus,
  telemetryHealth,
  telemetryConfigGet,
  telemetryConfigUpdate,
} from "../api/backend";
import "./admin.css";

// ── Types ───────────────────────────────────────────────────────────────────

interface TelemetryStatusData {
  status: "Healthy" | "Degraded" | "Unhealthy" | string;
  version: string;
  uptime: string;
  agents_active: number;
  audit_chain_valid: boolean;
}

interface TelemetryHealthData {
  status: "Healthy" | "Degraded" | "Unhealthy" | string;
  version: string;
  uptime: string;
  agents_active: number;
  audit_chain_valid: boolean;
}

interface TelemetryConfig {
  enabled: boolean;
  otlp_endpoint: string;
  service_name: string;
  sample_rate: number;
  log_format: string;
  log_level: string;
}

// ── Helpers ─────────────────────────────────────────────────────────────────

function statusColor(s: string): string {
  const lower = s.toLowerCase();
  if (lower === "healthy") return "var(--nexus-accent)";
  if (lower === "degraded") return "var(--nexus-amber)";
  return "var(--nexus-danger)";
}

function statusDotClass(s: string): string {
  const lower = s.toLowerCase();
  if (lower === "healthy") return "admin-dot--running";
  if (lower === "degraded") return "admin-dot--idle";
  return "admin-dot--error";
}

function StatRow({
  label,
  value,
  color,
}: {
  label: string;
  value: string;
  color?: string;
}) {
  return (
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        padding: "0.4rem 0",
        fontSize: "0.82rem",
        borderBottom: "1px solid rgba(90,142,190,0.08)",
      }}
    >
      <span style={{ color: "var(--text-secondary)" }}>{label}</span>
      <span
        style={{
          color: color ?? "var(--text-primary)",
          fontFamily: "var(--font-mono)",
        }}
      >
        {value}
      </span>
    </div>
  );
}

// ── Component ────────────────────────────────────────────────────────────────

export default function Telemetry() {
  const [statusData, setStatusData] = useState<TelemetryStatusData | null>(
    null
  );
  const [healthData, setHealthData] = useState<TelemetryHealthData | null>(
    null
  );
  const [config, setConfig] = useState<TelemetryConfig | null>(null);
  const [editConfig, setEditConfig] = useState<TelemetryConfig | null>(null);
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saveSuccess, setSaveSuccess] = useState(false);
  const saveSuccessTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (saveSuccessTimerRef.current) clearTimeout(saveSuccessTimerRef.current);
    };
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [s, h, c] = await Promise.all([
        telemetryStatus(),
        telemetryHealth(),
        telemetryConfigGet(),
      ]);
      setStatusData(s as TelemetryStatusData);
      setHealthData(h as TelemetryHealthData);
      setConfig(c as TelemetryConfig);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to fetch telemetry data"
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  function startEdit() {
    if (!config) return;
    setEditConfig({ ...config });
    setEditing(true);
    setSaveError(null);
    setSaveSuccess(false);
  }

  function cancelEdit() {
    setEditing(false);
    setEditConfig(null);
    setSaveError(null);
  }

  async function saveConfig() {
    if (!editConfig) return;
    setSaving(true);
    setSaveError(null);
    setSaveSuccess(false);
    try {
      await telemetryConfigUpdate(JSON.stringify(editConfig));
      setConfig({ ...editConfig });
      setEditing(false);
      setEditConfig(null);
      setSaveSuccess(true);
      if (saveSuccessTimerRef.current) clearTimeout(saveSuccessTimerRef.current);
      saveSuccessTimerRef.current = setTimeout(() => setSaveSuccess(false), 3000);
    } catch (err) {
      setSaveError(
        err instanceof Error ? err.message : "Failed to save telemetry config"
      );
    } finally {
      setSaving(false);
    }
  }

  function patchEdit(field: keyof TelemetryConfig, value: unknown) {
    if (!editConfig) return;
    setEditConfig({ ...editConfig, [field]: value });
  }

  const displayStatus = healthData?.status ?? statusData?.status ?? "—";
  const displayVersion = healthData?.version ?? statusData?.version ?? "—";
  const displayUptime = healthData?.uptime ?? statusData?.uptime ?? "—";
  const displayAgents =
    healthData?.agents_active ?? statusData?.agents_active ?? 0;
  const displayChain =
    healthData?.audit_chain_valid ?? statusData?.audit_chain_valid ?? false;

  return (
    <div className="admin-shell">
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          marginBottom: "0.25rem",
        }}
      >
        <h1>Telemetry</h1>
        <button type="button"
          className="admin-btn admin-btn--accent"
          onClick={() => void refresh()}
          disabled={loading}
        >
          {loading ? "Refreshing..." : "Refresh"}
        </button>
      </div>
      <p className="admin-subtitle">
        Observability, health status, and telemetry pipeline configuration
        {loading && " — loading..."}
      </p>

      {error && (
        <div className="admin-alert admin-alert--danger" style={{ marginBottom: "1rem" }}>
          {error}
        </div>
      )}

      {saveSuccess && (
        <div className="admin-alert admin-alert--info" style={{ marginBottom: "1rem" }}>
          Telemetry configuration saved successfully.
        </div>
      )}

      {/* ── Health Overview ── */}
      <div className="admin-metrics">
        <div className="admin-metric">
          <div className="admin-metric__label">Health Status</div>
          <div
            className="admin-metric__value"
            style={{
              fontSize: "1.1rem",
              color: statusColor(displayStatus),
              display: "flex",
              alignItems: "center",
              gap: "0.4rem",
            }}
          >
            <span className={`admin-dot ${statusDotClass(displayStatus)}`} />
            {displayStatus}
          </div>
          <div className="admin-metric__sub">Current system health</div>
        </div>

        <div className="admin-metric">
          <div className="admin-metric__label">Version</div>
          <div
            className="admin-metric__value"
            style={{ fontSize: "1.1rem", fontFamily: "var(--font-mono)" }}
          >
            {displayVersion}
          </div>
          <div className="admin-metric__sub">Telemetry service version</div>
        </div>

        <div className="admin-metric">
          <div className="admin-metric__label">Uptime</div>
          <div
            className="admin-metric__value"
            style={{ fontSize: "1.1rem", fontFamily: "var(--font-mono)" }}
          >
            {displayUptime}
          </div>
          <div className="admin-metric__sub">Service uptime</div>
        </div>

        <div className="admin-metric">
          <div className="admin-metric__label">Agents Active</div>
          <div className="admin-metric__value">{displayAgents}</div>
          <div className="admin-metric__sub">Currently running agents</div>
        </div>

        <div className="admin-metric">
          <div className="admin-metric__label">Audit Chain</div>
          <div
            className="admin-metric__value"
            style={{
              fontSize: "1.1rem",
              color: displayChain
                ? "var(--nexus-accent)"
                : "var(--nexus-danger)",
            }}
          >
            {displayChain ? "Valid" : "Invalid"}
          </div>
          <div className="admin-metric__sub">Hash-chain integrity</div>
        </div>
      </div>

      <div className="admin-grid-2">
        {/* ── Status Detail ── */}
        <div className="admin-card">
          <div className="admin-card__title">Status Detail</div>
          {statusData ? (
            <>
              <StatRow
                label="Status"
                value={statusData.status}
                color={statusColor(statusData.status)}
              />
              <StatRow label="Version" value={statusData.version} />
              <StatRow label="Uptime" value={statusData.uptime} />
              <StatRow
                label="Agents Active"
                value={String(statusData.agents_active)}
              />
              <StatRow
                label="Audit Chain Valid"
                value={statusData.audit_chain_valid ? "Yes" : "No"}
                color={
                  statusData.audit_chain_valid
                    ? "var(--nexus-accent)"
                    : "var(--nexus-danger)"
                }
              />
            </>
          ) : (
            <div className="admin-empty">
              {loading ? "Loading status..." : "No status data available"}
            </div>
          )}
        </div>

        {/* ── Health Detail ── */}
        <div className="admin-card">
          <div className="admin-card__title">Health Detail</div>
          {healthData ? (
            <>
              <StatRow
                label="Status"
                value={healthData.status}
                color={statusColor(healthData.status)}
              />
              <StatRow label="Version" value={healthData.version} />
              <StatRow label="Uptime" value={healthData.uptime} />
              <StatRow
                label="Agents Active"
                value={String(healthData.agents_active)}
              />
              <StatRow
                label="Audit Chain Valid"
                value={healthData.audit_chain_valid ? "Yes" : "No"}
                color={
                  healthData.audit_chain_valid
                    ? "var(--nexus-accent)"
                    : "var(--nexus-danger)"
                }
              />
            </>
          ) : (
            <div className="admin-empty">
              {loading ? "Loading health..." : "No health data available"}
            </div>
          )}
        </div>
      </div>

      {/* ── Telemetry Config ── */}
      <div className="admin-card">
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            marginBottom: "0.8rem",
          }}
        >
          <div className="admin-card__title" style={{ marginBottom: 0 }}>
            Telemetry Configuration
          </div>
          {!editing && config && (
            <button type="button" className="admin-btn admin-btn--sm" onClick={startEdit}>
              Edit
            </button>
          )}
        </div>

        {saveError && (
          <div
            className="admin-alert admin-alert--danger"
            style={{ marginBottom: "0.8rem" }}
          >
            {saveError}
          </div>
        )}

        {!editing && config && (
          <>
            <StatRow
              label="Enabled"
              value={config.enabled ? "Yes" : "No"}
              color={
                config.enabled ? "var(--nexus-accent)" : "var(--text-muted)"
              }
            />
            <StatRow label="OTLP Endpoint" value={config.otlp_endpoint} />
            <StatRow label="Service Name" value={config.service_name} />
            <StatRow
              label="Sample Rate"
              value={String(config.sample_rate)}
            />
            <StatRow label="Log Format" value={config.log_format} />
            <StatRow label="Log Level" value={config.log_level} />
          </>
        )}

        {!editing && !config && (
          <div className="admin-empty">
            {loading ? "Loading configuration..." : "No configuration available"}
          </div>
        )}

        {editing && editConfig && (
          <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
            {/* Enabled toggle */}
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                fontSize: "0.82rem",
                padding: "0.2rem 0",
              }}
            >
              <label style={{ color: "var(--text-secondary)" }}>Enabled</label>
              <button type="button"
                className={`admin-btn admin-btn--sm ${editConfig.enabled ? "admin-btn--accent" : ""}`}
                onClick={() => patchEdit("enabled", !editConfig.enabled)}
              >
                {editConfig.enabled ? "On" : "Off"}
              </button>
            </div>

            {/* OTLP Endpoint */}
            <div style={{ display: "flex", flexDirection: "column", gap: "0.3rem" }}>
              <label
                style={{
                  fontSize: "0.75rem",
                  color: "var(--text-muted)",
                  textTransform: "uppercase",
                  letterSpacing: "0.06em",
                }}
              >
                OTLP Endpoint
              </label>
              <input
                className="admin-input"
                type="text"
                value={editConfig.otlp_endpoint}
                onChange={(e) => patchEdit("otlp_endpoint", e.target.value)}
                placeholder="http://localhost:4317"
              />
            </div>

            {/* Service Name */}
            <div style={{ display: "flex", flexDirection: "column", gap: "0.3rem" }}>
              <label
                style={{
                  fontSize: "0.75rem",
                  color: "var(--text-muted)",
                  textTransform: "uppercase",
                  letterSpacing: "0.06em",
                }}
              >
                Service Name
              </label>
              <input
                className="admin-input"
                type="text"
                value={editConfig.service_name}
                onChange={(e) => patchEdit("service_name", e.target.value)}
                placeholder="nexus-os"
              />
            </div>

            {/* Sample Rate */}
            <div style={{ display: "flex", flexDirection: "column", gap: "0.3rem" }}>
              <label
                style={{
                  fontSize: "0.75rem",
                  color: "var(--text-muted)",
                  textTransform: "uppercase",
                  letterSpacing: "0.06em",
                }}
              >
                Sample Rate
              </label>
              <input
                className="admin-input"
                type="number"
                min={0}
                max={1}
                step={0.01}
                value={editConfig.sample_rate}
                onChange={(e) =>
                  patchEdit("sample_rate", parseFloat(e.target.value) || 0)
                }
              />
            </div>

            {/* Log Format */}
            <div style={{ display: "flex", flexDirection: "column", gap: "0.3rem" }}>
              <label
                style={{
                  fontSize: "0.75rem",
                  color: "var(--text-muted)",
                  textTransform: "uppercase",
                  letterSpacing: "0.06em",
                }}
              >
                Log Format
              </label>
              <select
                className="admin-select"
                value={editConfig.log_format}
                onChange={(e) => patchEdit("log_format", e.target.value)}
                style={{ width: "100%" }}
              >
                <option value="json">json</option>
                <option value="text">text</option>
                <option value="compact">compact</option>
              </select>
            </div>

            {/* Log Level */}
            <div style={{ display: "flex", flexDirection: "column", gap: "0.3rem" }}>
              <label
                style={{
                  fontSize: "0.75rem",
                  color: "var(--text-muted)",
                  textTransform: "uppercase",
                  letterSpacing: "0.06em",
                }}
              >
                Log Level
              </label>
              <select
                className="admin-select"
                value={editConfig.log_level}
                onChange={(e) => patchEdit("log_level", e.target.value)}
                style={{ width: "100%" }}
              >
                <option value="trace">trace</option>
                <option value="debug">debug</option>
                <option value="info">info</option>
                <option value="warn">warn</option>
                <option value="error">error</option>
              </select>
            </div>

            {/* Actions */}
            <div style={{ display: "flex", gap: "0.5rem", marginTop: "0.25rem" }}>
              <button type="button"
                className="admin-btn admin-btn--accent"
                onClick={() => void saveConfig()}
                disabled={saving}
              >
                {saving ? "Saving..." : "Save"}
              </button>
              <button type="button"
                className="admin-btn"
                onClick={cancelEdit}
                disabled={saving}
              >
                Cancel
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

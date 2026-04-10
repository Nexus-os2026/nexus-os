import { useCallback, useEffect, useState } from "react";
import {
  meteringUsageReport,
  meteringCostBreakdown,
  meteringExportCsv,
  meteringSetBudgetAlert,
  meteringBudgetAlerts,
} from "../api/backend";
import "./admin.css";

type Period = "hour" | "day" | "week" | "month";

interface UsageReport {
  workspace_id: string;
  period: string;
  total_llm_tokens: number;
  total_fuel_consumed: number;
  total_compute_seconds: number;
  total_api_calls: number;
  total_storage_bytes: number;
  top_agents: TopAgent[];
}

interface TopAgent {
  agent_id: string;
  llm_tokens: number;
  fuel_consumed: number;
  api_calls: number;
}

interface CostLineItem {
  category: string;
  quantity: number;
  unit_cost: number;
  total_cost: number;
}

interface BudgetAlert {
  id: string;
  workspace_id: string;
  threshold: number;
  current_spend: number;
  triggered: boolean;
  created_at: string;
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function formatBytes(n: number): string {
  if (n >= 1_073_741_824) return `${(n / 1_073_741_824).toFixed(2)} GB`;
  if (n >= 1_048_576) return `${(n / 1_048_576).toFixed(1)} MB`;
  if (n >= 1_024) return `${(n / 1_024).toFixed(1)} KB`;
  return `${n} B`;
}

function formatSeconds(n: number): string {
  if (n >= 3600) return `${(n / 3600).toFixed(1)}h`;
  if (n >= 60) return `${(n / 60).toFixed(1)}m`;
  return `${n}s`;
}

function formatFuel(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function formatCost(n: number): string {
  return `$${n.toFixed(4)}`;
}

export default function UsageBilling() {
  const [period, setPeriod] = useState<Period>("day");
  const [report, setReport] = useState<UsageReport | null>(null);
  const [costItems, setCostItems] = useState<CostLineItem[]>([]);
  const [alerts, setAlerts] = useState<BudgetAlert[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportMsg, setExportMsg] = useState<string | null>(null);
  const [newThreshold, setNewThreshold] = useState("");
  const [settingAlert, setSettingAlert] = useState(false);
  const [alertMsg, setAlertMsg] = useState<string | null>(null);

  const fetchAll = useCallback(async (p: Period) => {
    setLoading(true);
    setError(null);
    try {
      const [rep, cost, budgetAlerts] = await Promise.all([
        meteringUsageReport("default", p),
        meteringCostBreakdown("default", p),
        meteringBudgetAlerts("default"),
      ]);
      setReport(rep as UsageReport);
      setCostItems(Array.isArray(cost) ? (cost as CostLineItem[]) : []);
      setAlerts(Array.isArray(budgetAlerts) ? (budgetAlerts as BudgetAlert[]) : []);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchAll(period);
  }, [period, fetchAll]);

  async function handleExport() {
    setExporting(true);
    setExportMsg(null);
    try {
      const csv = await meteringExportCsv("default", period);
      const blob = new Blob([csv], { type: "text/csv" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `nexus-usage-${period}-${Date.now()}.csv`;
      a.click();
      URL.revokeObjectURL(url);
      setExportMsg("Export downloaded.");
    } catch (err) {
      setExportMsg(`Export failed: ${String(err)}`);
    } finally {
      setExporting(false);
    }
  }

  async function handleSetAlert() {
    const val = parseFloat(newThreshold);
    if (isNaN(val) || val <= 0) {
      setAlertMsg("Enter a valid positive threshold.");
      return;
    }
    setSettingAlert(true);
    setAlertMsg(null);
    try {
      await meteringSetBudgetAlert("default", val);
      setAlertMsg("Budget alert set.");
      setNewThreshold("");
      const updated = await meteringBudgetAlerts("default");
      setAlerts(Array.isArray(updated) ? (updated as BudgetAlert[]) : []);
    } catch (err) {
      setAlertMsg(`Failed: ${String(err)}`);
    } finally {
      setSettingAlert(false);
    }
  }

  const PERIODS: { label: string; value: Period }[] = [
    { label: "Hour", value: "hour" },
    { label: "Day", value: "day" },
    { label: "Week", value: "week" },
    { label: "Month", value: "month" },
  ];

  return (
    <div className="admin-shell">
      <h1>Usage &amp; Billing</h1>
      <p className="admin-subtitle">
        Workspace metering, cost breakdown, and budget alerts
      </p>

      {/* ── Period Selector ── */}
      <div style={{ display: "flex", alignItems: "center", gap: "0.75rem", marginBottom: "1.5rem", flexWrap: "wrap" }}>
        <span style={{ fontSize: "0.78rem", color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.06em" }}>
          Period
        </span>
        <div className="admin-tabs" style={{ marginBottom: 0, borderBottom: "none", padding: 0 }}>
          {PERIODS.map((p) => (
            <button type="button"
              key={p.value}
              className={`admin-tab${period === p.value ? " admin-tab--active" : ""}`}
              onClick={() => setPeriod(p.value)}
            >
              {p.label}
            </button>
          ))}
        </div>
        <div style={{ flex: 1 }} />
        <button type="button"
          className="admin-btn admin-btn--accent"
          onClick={handleExport}
          disabled={exporting}
        >
          {exporting ? "Exporting..." : "Export CSV"}
        </button>
        {exportMsg && (
          <span style={{ fontSize: "0.78rem", color: "var(--text-secondary)" }}>{exportMsg}</span>
        )}
      </div>

      {error && (
        <div className="admin-alert admin-alert--danger" style={{ marginBottom: "1rem" }}>
          <span>Error loading usage data: {error}</span>
        </div>
      )}

      {/* ── Usage Metrics ── */}
      {loading ? (
        <div className="admin-empty">Loading usage data...</div>
      ) : report ? (
        <>
          <div className="admin-metrics">
            <div className="admin-metric">
              <span className="admin-metric__label">LLM Tokens</span>
              <span className="admin-metric__value">{formatTokens(report.total_llm_tokens)}</span>
              <span className="admin-metric__sub">this {period}</span>
            </div>
            <div className="admin-metric">
              <span className="admin-metric__label">Fuel Consumed</span>
              <span className="admin-metric__value">{formatFuel(report.total_fuel_consumed)}</span>
              <span className="admin-metric__sub">units</span>
            </div>
            <div className="admin-metric">
              <span className="admin-metric__label">Compute Time</span>
              <span className="admin-metric__value">{formatSeconds(report.total_compute_seconds)}</span>
              <span className="admin-metric__sub">wall-clock</span>
            </div>
            <div className="admin-metric">
              <span className="admin-metric__label">API Calls</span>
              <span className="admin-metric__value">{formatTokens(report.total_api_calls)}</span>
              <span className="admin-metric__sub">requests</span>
            </div>
            <div className="admin-metric">
              <span className="admin-metric__label">Storage</span>
              <span className="admin-metric__value">{formatBytes(report.total_storage_bytes)}</span>
              <span className="admin-metric__sub">consumed</span>
            </div>
          </div>

          <div className="admin-grid-2">
            {/* ── Cost Breakdown ── */}
            <div className="admin-card">
              <div className="admin-card__title">Cost Breakdown</div>
              {costItems.length === 0 ? (
                <div className="admin-empty" style={{ padding: "1.5rem" }}>No cost data for this period.</div>
              ) : (
                <table className="admin-table">
                  <thead>
                    <tr>
                      <th>Category</th>
                      <th>Quantity</th>
                      <th>Unit Cost</th>
                      <th>Total</th>
                    </tr>
                  </thead>
                  <tbody>
                    {costItems.map((item, i) => (
                      <tr key={i}>
                        <td style={{ color: "var(--text-primary)", textTransform: "capitalize" }}>
                          {item.category.replace(/_/g, " ")}
                        </td>
                        <td>{item.quantity.toLocaleString()}</td>
                        <td style={{ fontFamily: "var(--font-mono, monospace)" }}>{formatCost(item.unit_cost)}</td>
                        <td style={{ color: "var(--nexus-accent)", fontWeight: 600 }}>{formatCost(item.total_cost)}</td>
                      </tr>
                    ))}
                    <tr>
                      <td colSpan={3} style={{ fontWeight: 600, color: "var(--text-primary)", paddingTop: "0.75rem" }}>
                        Total
                      </td>
                      <td style={{ color: "var(--nexus-accent)", fontWeight: 700, paddingTop: "0.75rem", fontSize: "1rem" }}>
                        {formatCost(costItems.reduce((sum, x) => sum + x.total_cost, 0))}
                      </td>
                    </tr>
                  </tbody>
                </table>
              )}
            </div>

            {/* ── Top Agents ── */}
            <div className="admin-card">
              <div className="admin-card__title">Top Agents by Usage</div>
              {(!report.top_agents || report.top_agents.length === 0) ? (
                <div className="admin-empty" style={{ padding: "1.5rem" }}>No agent usage recorded.</div>
              ) : (
                <table className="admin-table">
                  <thead>
                    <tr>
                      <th>Agent</th>
                      <th>Tokens</th>
                      <th>Fuel</th>
                      <th>API Calls</th>
                    </tr>
                  </thead>
                  <tbody>
                    {report.top_agents.map((agent, i) => (
                      <tr key={i}>
                        <td style={{ color: "var(--text-primary)", fontFamily: "var(--font-mono, monospace)", fontSize: "0.75rem" }}>
                          {agent.agent_id}
                        </td>
                        <td>{formatTokens(agent.llm_tokens)}</td>
                        <td>{formatFuel(agent.fuel_consumed)}</td>
                        <td>{agent.api_calls.toLocaleString()}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          </div>
        </>
      ) : null}

      {/* ── Budget Alerts ── */}
      <div className="admin-card">
        <div className="admin-card__title">Budget Alerts</div>

        {/* Set New Alert */}
        <div style={{ display: "flex", gap: "0.75rem", alignItems: "center", marginBottom: "1rem", flexWrap: "wrap" }}>
          <input
            className="admin-input"
            style={{ maxWidth: 220 }}
            type="number"
            min="0"
            step="0.01"
            placeholder="Threshold in USD (e.g. 50.00)"
            value={newThreshold}
            onChange={(e) => setNewThreshold(e.target.value)}
          />
          <button type="button"
            className="admin-btn admin-btn--accent"
            onClick={handleSetAlert}
            disabled={settingAlert}
          >
            {settingAlert ? "Setting..." : "Set Alert"}
          </button>
          {alertMsg && (
            <span style={{ fontSize: "0.78rem", color: "var(--text-secondary)" }}>{alertMsg}</span>
          )}
        </div>

        {/* Active Alerts */}
        {alerts.length === 0 ? (
          <div className="admin-empty" style={{ padding: "1rem" }}>No budget alerts configured.</div>
        ) : (
          <table className="admin-table">
            <thead>
              <tr>
                <th>Threshold</th>
                <th>Current Spend</th>
                <th>Status</th>
                <th>Created</th>
              </tr>
            </thead>
            <tbody>
              {alerts.map((alert) => {
                const pct = alert.threshold > 0 ? Math.min((alert.current_spend / alert.threshold) * 100, 100) : 0;
                return (
                  <tr key={alert.id}>
                    <td style={{ color: "var(--text-primary)", fontWeight: 600 }}>{formatCost(alert.threshold)}</td>
                    <td>
                      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                        <span>{formatCost(alert.current_spend)}</span>
                        <div className="admin-bar" style={{ width: 80, flex: "none" }}>
                          <div
                            className={`admin-bar__fill ${pct >= 90 ? "admin-bar__fill--warn" : "admin-bar__fill--accent"}`}
                            style={{ width: `${pct}%` }}
                          />
                        </div>
                        <span style={{ fontSize: "0.7rem", color: "var(--text-muted)" }}>{pct.toFixed(0)}%</span>
                      </div>
                    </td>
                    <td>
                      {alert.triggered ? (
                        <span className="admin-badge admin-badge--admin" style={{ background: "rgba(255,109,122,0.12)", color: "var(--nexus-danger)" }}>
                          TRIGGERED
                        </span>
                      ) : (
                        <span className="admin-badge admin-badge--operator">OK</span>
                      )}
                    </td>
                    <td style={{ fontSize: "0.75rem" }}>
                      {new Date(alert.created_at).toLocaleDateString()}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

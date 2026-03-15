import { useCallback, useEffect, useState } from "react";
import { getTrustOverview, hasDesktopRuntime } from "../api/backend";
import type { TrustOverviewAgent } from "../types";
import "./trust-dashboard.css";

const AUTONOMY_LABELS = ["L0 Inert", "L1 Suggest", "L2 Act+Approve", "L3 Act+Report", "L4 Autonomous", "L5 Full"];

function trustColor(score: number): string {
  if (score >= 0.7) return "#22c55e";
  if (score >= 0.4) return "#eab308";
  return "#ef4444";
}

export default function TrustDashboard(): JSX.Element {
  const [agents, setAgents] = useState<TrustOverviewAgent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const isDesktop = hasDesktopRuntime();

  const loadData = useCallback(async () => {
    if (!isDesktop) {
      setLoading(false);
      return;
    }
    try {
      const data = await getTrustOverview();
      setAgents(data);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [isDesktop]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  // Refresh every 10 seconds
  useEffect(() => {
    if (!isDesktop) return;
    const interval = setInterval(loadData, 10000);
    return () => clearInterval(interval);
  }, [isDesktop, loadData]);

  if (loading) {
    return (
      <section className="td-hub">
        <header className="td-header">
          <h2 className="td-title">TRUST DASHBOARD // ADAPTIVE GOVERNANCE</h2>
          <p className="td-subtitle">Loading...</p>
        </header>
      </section>
    );
  }

  if (!isDesktop) {
    return (
      <section className="td-hub">
        <header className="td-header">
          <h2 className="td-title">TRUST DASHBOARD // ADAPTIVE GOVERNANCE</h2>
          <p className="td-subtitle">Desktop runtime required</p>
        </header>
      </section>
    );
  }

  return (
    <section className="td-hub">
      <header className="td-header">
        <h2 className="td-title">TRUST DASHBOARD // ADAPTIVE GOVERNANCE</h2>
        <p className="td-subtitle">
          {agents.length} agent{agents.length !== 1 ? "s" : ""} tracked
          {error && <span className="td-error"> | {error}</span>}
        </p>
      </header>

      {agents.length === 0 && (
        <div className="td-empty">
          No agents registered. Create agents to see trust scores here.
        </div>
      )}

      <div className="td-grid">
        {agents.map((agent) => {
          const pct = Math.round(agent.trust_score * 100);
          const color = trustColor(agent.trust_score);
          const level = agent.autonomy_level;
          const canPromote = agent.trust_score >= 0.85 && level < 5;
          const shouldDemote = agent.trust_score < 0.3 && level > 0;
          return (
            <article key={agent.id} className="td-card">
              <div className="td-card-top">
                <h3 className="td-card-name">{agent.name}</h3>
                <div className="td-indicators">
                  {canPromote && <span className="td-promo-badge">PROMO</span>}
                  {shouldDemote && <span className="td-demo-badge">DEMO</span>}
                  <span className={`td-status-badge td-status-${agent.status.toLowerCase()}`}>
                    {agent.status}
                  </span>
                </div>
              </div>

              <div className="td-score-row">
                <div className="td-score-ring" style={{ borderColor: color }}>
                  <span className="td-score-pct" style={{ color }}>{pct}%</span>
                </div>
                <div className="td-score-details">
                  <div className="td-detail-row">
                    <span className="td-label">Trust Score</span>
                    <span className="td-value" style={{ color }}>{agent.trust_score.toFixed(2)}</span>
                  </div>
                  <div className="td-detail-row">
                    <span className="td-label">Autonomy</span>
                    <span className="td-value">{AUTONOMY_LABELS[level] ?? `L${level}`}</span>
                  </div>
                  {agent.did && (
                    <div className="td-detail-row">
                      <span className="td-label">DID</span>
                      <span className="td-value td-did">{agent.did.slice(0, 24)}...</span>
                    </div>
                  )}
                </div>
              </div>

              <div className="td-stats">
                <div className="td-stat">
                  <span className="td-stat-value">{agent.total_tasks}</span>
                  <span className="td-stat-label">Tasks</span>
                </div>
                <div className="td-stat">
                  <span className="td-stat-value">{agent.total_tasks > 0 ? Math.round(agent.success_rate * 100) : 0}%</span>
                  <span className="td-stat-label">Success</span>
                </div>
                <div className="td-stat">
                  <span className="td-stat-value" style={{ color: agent.violations > 0 ? "#ef4444" : "#22c55e" }}>
                    {agent.violations}
                  </span>
                  <span className="td-stat-label">Violations</span>
                </div>
                <div className="td-stat">
                  <span className="td-stat-value">{agent.fuel_remaining.toLocaleString()}</span>
                  <span className="td-stat-label">Fuel</span>
                </div>
              </div>

              {agent.badges.length > 0 && (
                <div className="td-badges">
                  {agent.badges.map((badge) => (
                    <span key={badge} className="td-badge">{badge}</span>
                  ))}
                </div>
              )}
            </article>
          );
        })}
      </div>
    </section>
  );
}

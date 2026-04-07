import { useCallback, useEffect, useState } from "react";
import {
  getTrustOverview,
  hasDesktopRuntime,
  reputationRegister,
  reputationGet,
  reputationTop,
  reputationRateAgent,
  reputationRecordTask,
  reputationExport,
  reputationImport,
} from "../api/backend";
import type { TrustOverviewAgent } from "../types";
import "./trust-dashboard.css";

const AUTONOMY_LABELS = ["L0 Inert", "L1 Suggest", "L2 Act+Approve", "L3 Act+Report", "L4 Autonomous", "L5 Full"];

type Tab = "trust" | "reputation";

function trustColor(score: number): string {
  if (score >= 0.7) return "#22c55e";
  if (score >= 0.4) return "#eab308";
  return "#ef4444";
}

/* ---------- Reputation sub-types (parsed from JSON strings) ---------- */

interface ReputationEntry {
  did: string;
  name: string;
  score: number;
  total_tasks: number;
  successful_tasks: number;
  ratings_count: number;
  average_rating: number;
  [key: string]: unknown;
}

/* ---------- Reputation Section ---------- */

function ReputationSection({ isDesktop }: { isDesktop: boolean }): JSX.Element {
  // Leaderboard
  const [leaderboard, setLeaderboard] = useState<ReputationEntry[]>([]);
  const [lbLoading, setLbLoading] = useState(false);
  const [lbError, setLbError] = useState<string | null>(null);

  // Agent detail lookup
  const [lookupDid, setLookupDid] = useState("");
  const [agentDetail, setAgentDetail] = useState<ReputationEntry | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);

  // Rate agent form
  const [rateDid, setRateDid] = useState("");
  const [raterDid, setRaterDid] = useState("");
  const [rateScore, setRateScore] = useState(3);
  const [rateComment, setRateComment] = useState("");
  const [rateLoading, setRateLoading] = useState(false);
  const [rateMsg, setRateMsg] = useState<string | null>(null);

  // Record task form
  const [taskDid, setTaskDid] = useState("");
  const [taskSuccess, setTaskSuccess] = useState(true);
  const [taskLoading, setTaskLoading] = useState(false);
  const [taskMsg, setTaskMsg] = useState<string | null>(null);

  // Register form
  const [regDid, setRegDid] = useState("");
  const [regName, setRegName] = useState("");
  const [regLoading, setRegLoading] = useState(false);
  const [regMsg, setRegMsg] = useState<string | null>(null);

  // Import
  const [importJson, setImportJson] = useState("");
  const [importLoading, setImportLoading] = useState(false);
  const [importMsg, setImportMsg] = useState<string | null>(null);

  // Export
  const [exportDid, setExportDid] = useState("");
  const [exportLoading, setExportLoading] = useState(false);
  const [exportResult, setExportResult] = useState<string | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);

  const fetchLeaderboard = useCallback(async () => {
    if (!isDesktop) return;
    setLbLoading(true);
    setLbError(null);
    try {
      const raw = await reputationTop(10);
      const parsed = JSON.parse(raw);
      setLeaderboard(Array.isArray(parsed) ? parsed : []);
    } catch (err) {
      setLbError(String(err));
    } finally {
      setLbLoading(false);
    }
  }, [isDesktop]);

  useEffect(() => {
    fetchLeaderboard();
  }, [fetchLeaderboard]);

  const handleLookup = async () => {
    if (!lookupDid.trim()) return;
    setDetailLoading(true);
    setDetailError(null);
    setAgentDetail(null);
    try {
      const raw = await reputationGet(lookupDid.trim());
      setAgentDetail(JSON.parse(raw));
    } catch (err) {
      setDetailError(String(err));
    } finally {
      setDetailLoading(false);
    }
  };

  const handleRate = async () => {
    if (!rateDid.trim() || !raterDid.trim()) return;
    setRateLoading(true);
    setRateMsg(null);
    try {
      const result = await reputationRateAgent(
        rateDid.trim(),
        raterDid.trim(),
        rateScore,
        rateComment.trim() || undefined,
      );
      setRateMsg(result);
      fetchLeaderboard();
    } catch (err) {
      setRateMsg(`Error: ${String(err)}`);
    } finally {
      setRateLoading(false);
    }
  };

  const handleRecordTask = async () => {
    if (!taskDid.trim()) return;
    setTaskLoading(true);
    setTaskMsg(null);
    try {
      const result = await reputationRecordTask(taskDid.trim(), taskSuccess);
      setTaskMsg(result);
      fetchLeaderboard();
    } catch (err) {
      setTaskMsg(`Error: ${String(err)}`);
    } finally {
      setTaskLoading(false);
    }
  };

  const handleRegister = async () => {
    if (!regDid.trim() || !regName.trim()) return;
    setRegLoading(true);
    setRegMsg(null);
    try {
      const result = await reputationRegister(regDid.trim(), regName.trim());
      setRegMsg(result);
      fetchLeaderboard();
    } catch (err) {
      setRegMsg(`Error: ${String(err)}`);
    } finally {
      setRegLoading(false);
    }
  };

  const handleImport = async () => {
    if (!importJson.trim()) return;
    setImportLoading(true);
    setImportMsg(null);
    try {
      const result = await reputationImport(importJson.trim());
      setImportMsg(result);
      fetchLeaderboard();
    } catch (err) {
      setImportMsg(`Error: ${String(err)}`);
    } finally {
      setImportLoading(false);
    }
  };

  const handleExport = async (did: string) => {
    setExportDid(did);
    setExportLoading(true);
    setExportError(null);
    setExportResult(null);
    try {
      const result = await reputationExport(did);
      setExportResult(result);
    } catch (err) {
      setExportError(String(err));
    } finally {
      setExportLoading(false);
    }
  };

  if (!isDesktop) {
    return <div className="td-empty">Reputation system requires desktop runtime.</div>;
  }

  return (
    <div className="td-rep-section">
      {/* --- Leaderboard --- */}
      <div className="td-rep-panel">
        <h3 className="td-rep-heading">Leaderboard (Top 10)</h3>
        {lbLoading && <p className="td-rep-status">Loading...</p>}
        {lbError && <p className="td-rep-status td-error">{lbError}</p>}
        {!lbLoading && !lbError && leaderboard.length === 0 && (
          <p className="td-rep-status">No agents registered yet.</p>
        )}
        {leaderboard.length > 0 && (
          <table className="td-rep-table">
            <thead>
              <tr>
                <th>#</th>
                <th>Name</th>
                <th>DID</th>
                <th>Score</th>
                <th>Tasks</th>
                <th>Avg Rating</th>
                <th>Export</th>
              </tr>
            </thead>
            <tbody>
              {leaderboard.map((entry, i) => (
                <tr key={entry.did}>
                  <td>{i + 1}</td>
                  <td>{entry.name}</td>
                  <td className="td-did">{entry.did.length > 20 ? `${entry.did.slice(0, 20)}...` : entry.did}</td>
                  <td style={{ color: trustColor(entry.score) }}>{entry.score.toFixed(2)}</td>
                  <td>{entry.total_tasks}</td>
                  <td>{entry.average_rating.toFixed(1)}</td>
                  <td>
                    <button
                      className="td-rep-btn td-rep-btn-sm"
                      onClick={() => handleExport(entry.did)}
                      disabled={exportLoading && exportDid === entry.did}
                    >
                      {exportLoading && exportDid === entry.did ? "..." : "Export"}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* --- Export result display --- */}
      {exportResult && (
        <div className="td-rep-panel">
          <h3 className="td-rep-heading">Export Result ({exportDid.slice(0, 20)}...)</h3>
          <textarea className="td-rep-textarea" readOnly value={exportResult} rows={6} />
          <button className="td-rep-btn" onClick={() => setExportResult(null)}>Dismiss</button>
        </div>
      )}
      {exportError && (
        <div className="td-rep-panel">
          <p className="td-error">Export error: {exportError}</p>
        </div>
      )}

      <div className="td-rep-forms">
        {/* --- Agent Detail Lookup --- */}
        <div className="td-rep-panel">
          <h3 className="td-rep-heading">Agent Reputation Detail</h3>
          <div className="td-rep-form-row">
            <input
              className="td-rep-input"
              placeholder="Agent DID"
              value={lookupDid}
              onChange={(e) => setLookupDid(e.target.value)}
            />
            <button className="td-rep-btn" onClick={handleLookup} disabled={detailLoading || !lookupDid.trim()}>
              {detailLoading ? "Loading..." : "Lookup"}
            </button>
          </div>
          {detailError && <p className="td-error">{detailError}</p>}
          {agentDetail && (
            <div className="td-rep-detail">
              <div className="td-detail-row"><span className="td-label">Name</span><span className="td-value">{agentDetail.name}</span></div>
              <div className="td-detail-row"><span className="td-label">DID</span><span className="td-value td-did">{agentDetail.did}</span></div>
              <div className="td-detail-row"><span className="td-label">Score</span><span className="td-value" style={{ color: trustColor(agentDetail.score) }}>{agentDetail.score.toFixed(2)}</span></div>
              <div className="td-detail-row"><span className="td-label">Tasks</span><span className="td-value">{agentDetail.total_tasks} ({agentDetail.successful_tasks} success)</span></div>
              <div className="td-detail-row"><span className="td-label">Ratings</span><span className="td-value">{agentDetail.ratings_count} (avg {agentDetail.average_rating.toFixed(1)})</span></div>
            </div>
          )}
        </div>

        {/* --- Register Agent --- */}
        <div className="td-rep-panel">
          <h3 className="td-rep-heading">Register Agent</h3>
          <div className="td-rep-form-row">
            <input className="td-rep-input" placeholder="DID" value={regDid} onChange={(e) => setRegDid(e.target.value)} />
            <input className="td-rep-input" placeholder="Name" value={regName} onChange={(e) => setRegName(e.target.value)} />
            <button className="td-rep-btn" onClick={handleRegister} disabled={regLoading || !regDid.trim() || !regName.trim()}>
              {regLoading ? "..." : "Register"}
            </button>
          </div>
          {regMsg && <p className="td-rep-status">{regMsg}</p>}
        </div>

        {/* --- Rate Agent --- */}
        <div className="td-rep-panel">
          <h3 className="td-rep-heading">Rate Agent</h3>
          <div className="td-rep-form-row">
            <input className="td-rep-input" placeholder="Target DID" value={rateDid} onChange={(e) => setRateDid(e.target.value)} />
            <input className="td-rep-input" placeholder="Rater DID" value={raterDid} onChange={(e) => setRaterDid(e.target.value)} />
          </div>
          <div className="td-rep-form-row">
            <label className="td-rep-label">
              Score (0-5):
              <input
                className="td-rep-input td-rep-input-sm"
                type="number"
                min={0}
                max={5}
                step={1}
                value={rateScore}
                onChange={(e) => setRateScore(Math.min(5, Math.max(0, Number(e.target.value))))}
              />
            </label>
            <input
              className="td-rep-input"
              placeholder="Comment (optional)"
              value={rateComment}
              onChange={(e) => setRateComment(e.target.value)}
            />
            <button className="td-rep-btn" onClick={handleRate} disabled={rateLoading || !rateDid.trim() || !raterDid.trim()}>
              {rateLoading ? "..." : "Rate"}
            </button>
          </div>
          {rateMsg && <p className="td-rep-status">{rateMsg}</p>}
        </div>

        {/* --- Record Task --- */}
        <div className="td-rep-panel">
          <h3 className="td-rep-heading">Record Task</h3>
          <div className="td-rep-form-row">
            <input className="td-rep-input" placeholder="Agent DID" value={taskDid} onChange={(e) => setTaskDid(e.target.value)} />
            <label className="td-rep-label">
              <input
                type="checkbox"
                checked={taskSuccess}
                onChange={(e) => setTaskSuccess(e.target.checked)}
              />
              Success
            </label>
            <button className="td-rep-btn" onClick={handleRecordTask} disabled={taskLoading || !taskDid.trim()}>
              {taskLoading ? "..." : "Record"}
            </button>
          </div>
          {taskMsg && <p className="td-rep-status">{taskMsg}</p>}
        </div>

        {/* --- Import --- */}
        <div className="td-rep-panel">
          <h3 className="td-rep-heading">Import Reputation</h3>
          <textarea
            className="td-rep-textarea"
            placeholder="Paste exported JSON here..."
            rows={4}
            value={importJson}
            onChange={(e) => setImportJson(e.target.value)}
          />
          <button className="td-rep-btn" onClick={handleImport} disabled={importLoading || !importJson.trim()}>
            {importLoading ? "Importing..." : "Import"}
          </button>
          {importMsg && <p className="td-rep-status">{importMsg}</p>}
        </div>
      </div>
    </div>
  );
}

/* ---------- Main component ---------- */

export default function TrustDashboard(): JSX.Element {
  const [agents, setAgents] = useState<TrustOverviewAgent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<Tab>("trust");
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
    const demoAgents = [
      { name: "Researcher", level: 3, trust: 0.82 },
      { name: "Trader", level: 4, trust: 0.65 },
      { name: "Writer", level: 2, trust: 0.91 },
    ];
    return (
      <section className="td-hub">
        <header className="td-header">
          <h2 className="td-title">TRUST DASHBOARD // ADAPTIVE GOVERNANCE</h2>
          <p className="td-subtitle">Desktop runtime required</p>
        </header>
        <nav className="td-tabs">
          <button className="td-tab td-tab-active">Trust Overview</button>
          <button className="td-tab">Reputation</button>
        </nav>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 16, marginBottom: 24 }}>
          {demoAgents.map(a => (
            <div key={a.name} style={{ padding: 20, borderRadius: 12, background: "rgba(255,255,255,0.02)", border: "1px solid rgba(255,255,255,0.05)" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
                <span style={{ fontSize: 14, fontWeight: 600, color: "#e2e8f0" }}>{a.name}</span>
                <span style={{ fontSize: 11, color: "#64748b", fontFamily: "monospace" }}>{AUTONOMY_LABELS[a.level]}</span>
              </div>
              <div style={{ height: 6, borderRadius: 3, background: "rgba(255,255,255,0.05)", marginBottom: 8 }}>
                <div style={{ height: "100%", borderRadius: 3, background: trustColor(a.trust), width: `${a.trust * 100}%`, opacity: 0.3 }} />
              </div>
              <div style={{ fontSize: 12, color: "#475569", fontFamily: "monospace" }}>Trust: <span style={{ color: "#334155" }}>—</span></div>
            </div>
          ))}
        </div>
        <div style={{ display: "flex", flexDirection: "column", alignItems: "center", padding: "24px 0", textAlign: "center" }}>
          <div style={{ fontSize: 14, color: "#64748b", maxWidth: 440, lineHeight: 1.6 }}>Connect to the desktop runtime to enable live trust scoring, reputation tracking, and adaptive governance.</div>
        </div>
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

      {/* Tab bar */}
      <nav className="td-tabs">
        <button
          className={`td-tab ${activeTab === "trust" ? "td-tab-active" : ""}`}
          onClick={() => setActiveTab("trust")}
        >
          Trust Overview
        </button>
        <button
          className={`td-tab ${activeTab === "reputation" ? "td-tab-active" : ""}`}
          onClick={() => setActiveTab("reputation")}
        >
          Reputation
        </button>
      </nav>

      {/* Trust Overview tab */}
      {activeTab === "trust" && (
        <>
          {agents.length === 0 && (
            <div className="td-empty" style={{ textAlign: "center", padding: "2rem 1rem" }}>
              <p style={{ fontSize: "1rem", color: "#94a3b8" }}>No agents are being tracked yet. Start an agent to begin trust scoring.</p>
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
                      {shouldDemote && <span className="td-demo-badge">DEMOTE</span>}
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
        </>
      )}

      {/* Reputation tab */}
      {activeTab === "reputation" && <ReputationSection isDesktop={isDesktop} />}
    </section>
  );
}

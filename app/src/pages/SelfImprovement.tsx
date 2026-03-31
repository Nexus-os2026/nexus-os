import { useCallback, useEffect, useState } from "react";
import {
  selfImproveGetStatus,
  selfImproveGetSignals,
  selfImproveGetOpportunities,
  selfImproveGetProposals,
  selfImproveGetHistory,
  selfImproveRunCycle,
  selfImproveApproveProposal,
  selfImproveRejectProposal,
  selfImproveRollback,
  selfImproveGetInvariants,
  selfImproveGetConfig,
  selfImproveUpdateConfig,
  selfImproveGetGuardianStatus,
} from "../api/backend";
import {
  ActionButton,
  EmptyState,
  Panel,
  commandHeaderMetaStyle,
  commandInsetStyle,
  commandLabelStyle,
  commandMutedStyle,
  commandPageStyle,
  commandScrollStyle,
} from "./commandCenterUi";

interface PipelineStatus {
  pipeline_state: string;
  signals_count: number;
  opportunities_count: number;
  pending_proposals: number;
  monitoring_count: number;
  committed_count: number;
  rolled_back_count: number;
  rejected_count: number;
  fuel_budget: number;
  enabled_domains: string[];
}

interface Signal {
  id: string;
  metric_name: string;
  domain: string;
  source: string;
  current_value: number;
  baseline_value: number;
  deviation_sigma: number;
}

interface Opportunity {
  id: string;
  domain: string;
  classification: string;
  severity: string;
  blast_radius: string;
  confidence: number;
  estimated_impact: number;
}

interface Proposal {
  id: string;
  domain: string;
  description: string;
  fuel_cost: number;
}

interface AppliedImprovement {
  id: string;
  proposal_id: string;
  status: string;
  applied_at: number;
  canary_deadline: number;
}

interface InvariantStatus {
  id: number;
  name: string;
  status: string;
}

interface GuardianStatus {
  has_baseline: boolean;
  baseline_hash: string;
  switch_threshold: number;
  current_drift: number;
  drift_bound: number;
  headroom: number;
  decision: string;
}

interface SelfImproveConfig {
  sigma_threshold: number;
  canary_duration_minutes: number;
  fuel_budget: number;
  enabled_domains: string[];
  max_proposals_per_cycle: number;
}

const DOMAIN_COLORS: Record<string, string> = {
  PromptOptimization: "#a78bfa",
  ConfigTuning: "#38bdf8",
  GovernancePolicy: "#fbbf24",
  SchedulingPolicy: "#34d399",
  RoutingStrategy: "#f472b6",
  CodePatch: "#ef4444",
};

const SEVERITY_COLORS: Record<string, string> = {
  Critical: "#ef4444",
  High: "#f97316",
  Medium: "#fbbf24",
  Low: "#34d399",
};

export default function SelfImprovement() {
  const [status, setStatus] = useState<PipelineStatus | null>(null);
  const [signals, setSignals] = useState<Signal[]>([]);
  const [opportunities, setOpportunities] = useState<Opportunity[]>([]);
  const [proposals, setProposals] = useState<Proposal[]>([]);
  const [history, setHistory] = useState<AppliedImprovement[]>([]);
  const [invariants, setInvariants] = useState<InvariantStatus[]>([]);
  const [config, setConfig] = useState<SelfImproveConfig | null>(null);
  const [guardian, setGuardian] = useState<GuardianStatus | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cycleResult, setCycleResult] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [s, sig, opp, prop, hist, inv, cfg, grd] = await Promise.all([
        selfImproveGetStatus(),
        selfImproveGetSignals(),
        selfImproveGetOpportunities(),
        selfImproveGetProposals(),
        selfImproveGetHistory(),
        selfImproveGetInvariants(),
        selfImproveGetConfig(),
        selfImproveGetGuardianStatus(),
      ]);
      setStatus(s as unknown as PipelineStatus);
      setSignals(Array.isArray(sig) ? (sig as unknown as Signal[]) : []);
      setOpportunities(Array.isArray(opp) ? (opp as unknown as Opportunity[]) : []);
      setProposals(Array.isArray(prop) ? (prop as unknown as Proposal[]) : []);
      setHistory(Array.isArray(hist) ? (hist as unknown as AppliedImprovement[]) : []);
      setInvariants(Array.isArray(inv) ? (inv as unknown as InvariantStatus[]) : []);
      setConfig(cfg as SelfImproveConfig);
      setGuardian(grd as unknown as GuardianStatus);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleRunCycle = async () => {
    setWorking(true);
    setCycleResult(null);
    try {
      const result = await selfImproveRunCycle();
      setCycleResult(JSON.stringify(result, null, 2));
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setWorking(false);
    }
  };

  const handleApprove = async (proposalId: string) => {
    setWorking(true);
    try {
      await selfImproveApproveProposal(proposalId);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setWorking(false);
    }
  };

  const handleReject = async (proposalId: string) => {
    setWorking(true);
    try {
      await selfImproveRejectProposal(proposalId, "User rejected via dashboard");
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setWorking(false);
    }
  };

  const handleRollback = async (improvementId: string) => {
    setWorking(true);
    try {
      await selfImproveRollback(improvementId);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setWorking(false);
    }
  };

  const handleConfigSave = async () => {
    if (!config) return;
    setWorking(true);
    try {
      await selfImproveUpdateConfig(config);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setWorking(false);
    }
  };

  return (
    <div style={commandPageStyle}>
      <h1 style={{ fontSize: "1.5rem", fontWeight: 700, color: "#f8fafc", marginBottom: 4 }}>
        Governed Self-Improvement
      </h1>
      <div style={commandHeaderMetaStyle}>
        <span>5-Stage Pipeline</span>
        <span>10 Hard Invariants</span>
        <span>Tier3 HITL Required</span>
      </div>

      {error && (
        <div style={{ marginTop: 12, padding: 10, background: "rgba(239,68,68,0.15)", border: "1px solid rgba(239,68,68,0.3)", borderRadius: 8, color: "#fca5a5", fontSize: "0.85rem" }}>
          {error}
        </div>
      )}

      {/* SECTION 1: Pipeline Status + Invariants */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginTop: 20 }}>
        <Panel title="Pipeline Status">
          {status ? (
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
              {[
                ["Signals", status.signals_count],
                ["Opportunities", status.opportunities_count],
                ["Pending", status.pending_proposals],
                ["Monitoring", status.monitoring_count],
                ["Committed", status.committed_count],
                ["Rolled Back", status.rolled_back_count],
                ["Rejected", status.rejected_count],
                ["Fuel Budget", status.fuel_budget],
              ].map(([label, value]) => (
                <div key={String(label)} style={commandInsetStyle}>
                  <div style={commandLabelStyle}>{String(label)}</div>
                  <div style={{ fontSize: "1.3rem", fontWeight: 700, color: "#f8fafc" }}>{String(value)}</div>
                </div>
              ))}
            </div>
          ) : (
            <EmptyState text="Loading status..." />
          )}
        </Panel>

        <Panel title="10 Hard Invariants">
          {invariants.length > 0 ? (
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 6, ...commandScrollStyle, maxHeight: 280 }}>
              {invariants.map((inv) => (
                <div key={inv.id} style={{ ...commandInsetStyle, display: "flex", alignItems: "center", gap: 8, padding: 8 }}>
                  <div style={{ width: 8, height: 8, borderRadius: "50%", background: inv.status === "passing" ? "#34d399" : "#ef4444", flexShrink: 0 }} />
                  <div style={{ fontSize: "0.75rem", color: "#cbd5e1" }}>{inv.name}</div>
                </div>
              ))}
            </div>
          ) : (
            <EmptyState text="Loading invariants..." />
          )}
        </Panel>
      </div>

      {/* SECTION 2: Signals & Opportunities + Run Cycle */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginTop: 16 }}>
        <Panel title={`Signals (${signals.length})`}>
          <div style={{ ...commandScrollStyle, maxHeight: 260 }}>
            {signals.length === 0 ? (
              <EmptyState text="No signals detected — system healthy" />
            ) : (
              signals.map((s) => (
                <div key={s.id} style={{ ...commandInsetStyle, marginBottom: 6 }}>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: DOMAIN_COLORS[s.domain] || "#94a3b8", fontSize: "0.8rem", fontWeight: 600 }}>{s.domain}</span>
                    <span style={{ color: Math.abs(s.deviation_sigma) > 3 ? "#ef4444" : "#fbbf24", fontSize: "0.8rem", fontFamily: "monospace" }}>{s.deviation_sigma.toFixed(1)}\u03C3</span>
                  </div>
                  <div style={commandMutedStyle}>{s.metric_name}</div>
                </div>
              ))
            )}
          </div>
        </Panel>

        <Panel title={`Opportunities (${opportunities.length})`}>
          <div style={{ ...commandScrollStyle, maxHeight: 200 }}>
            {opportunities.length === 0 ? (
              <EmptyState text="No opportunities identified" />
            ) : (
              opportunities.map((o) => (
                <div key={o.id} style={{ ...commandInsetStyle, marginBottom: 6 }}>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: DOMAIN_COLORS[o.domain] || "#94a3b8", fontSize: "0.8rem", fontWeight: 600 }}>{o.domain}</span>
                    <span style={{ color: SEVERITY_COLORS[o.severity] || "#94a3b8", fontSize: "0.75rem" }}>{o.severity}</span>
                  </div>
                  <div style={commandMutedStyle}>{o.classification} &middot; impact: {o.estimated_impact.toFixed(1)} &middot; conf: {(o.confidence * 100).toFixed(0)}%</div>
                </div>
              ))
            )}
          </div>
          <div style={{ marginTop: 10 }}>
            <ActionButton onClick={handleRunCycle} disabled={working}>{working ? "Running..." : "Run Improvement Cycle"}</ActionButton>
          </div>
          {cycleResult && (
            <pre style={{ marginTop: 8, fontSize: "0.72rem", color: "#94a3b8", whiteSpace: "pre-wrap", maxHeight: 120, overflow: "auto" }}>{cycleResult}</pre>
          )}
        </Panel>
      </div>

      {/* SECTION 3: Proposals + History */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 16, marginTop: 16 }}>
        <Panel title={`Pending Proposals (${proposals.length})`}>
          <div style={{ ...commandScrollStyle, maxHeight: 300 }}>
            {proposals.length === 0 ? (
              <EmptyState text="No pending proposals" />
            ) : (
              proposals.map((p) => (
                <div key={p.id} style={{ ...commandInsetStyle, marginBottom: 8 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 4 }}>
                    <span style={{ color: DOMAIN_COLORS[p.domain] || "#94a3b8", fontSize: "0.8rem", fontWeight: 600 }}>{p.domain}</span>
                    <span style={commandMutedStyle}>fuel: {p.fuel_cost}</span>
                  </div>
                  <div style={{ color: "#cbd5e1", fontSize: "0.82rem", marginBottom: 8 }}>{p.description}</div>
                  <div style={{ display: "flex", gap: 8 }}>
                    <ActionButton onClick={() => handleApprove(p.id)} disabled={working}>Approve (Tier3)</ActionButton>
                    <ActionButton onClick={() => handleReject(p.id)} disabled={working}>Reject</ActionButton>
                  </div>
                </div>
              ))
            )}
          </div>
        </Panel>

        <Panel title={`History (${history.length})`}>
          <div style={{ ...commandScrollStyle, maxHeight: 300 }}>
            {history.length === 0 ? (
              <EmptyState text="No improvements applied yet" />
            ) : (
              history.map((h) => (
                <div key={h.id} style={{ ...commandInsetStyle, marginBottom: 6 }}>
                  <div style={{ display: "flex", justifyContent: "space-between" }}>
                    <span style={{ color: h.status === "Committed" ? "#34d399" : h.status === "RolledBack" ? "#ef4444" : "#fbbf24", fontSize: "0.8rem", fontWeight: 600 }}>{h.status}</span>
                    <span style={commandMutedStyle}>{new Date(h.applied_at * 1000).toLocaleString()}</span>
                  </div>
                  {(h.status === "Monitoring" || h.status === "Applied") && (
                    <div style={{ marginTop: 6 }}>
                      <ActionButton onClick={() => handleRollback(h.id)} disabled={working}>Rollback</ActionButton>
                    </div>
                  )}
                </div>
              ))
            )}
          </div>
        </Panel>
      </div>

      {/* SECTION 4: Configuration */}
      {config && (
        <Panel title="Pipeline Configuration" style={{ marginTop: 16 }}>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 12 }}>
            <div style={commandInsetStyle}>
              <div style={commandLabelStyle}>Sigma Threshold</div>
              <input
                type="number"
                step="0.5"
                value={config.sigma_threshold}
                onChange={(e) => setConfig({ ...config, sigma_threshold: parseFloat(e.target.value) || 2.0 })}
                style={{ width: "100%", background: "rgba(0,0,0,0.3)", border: "1px solid rgba(148,163,184,0.2)", borderRadius: 6, padding: 6, color: "#f8fafc", marginTop: 4 }}
              />
            </div>
            <div style={commandInsetStyle}>
              <div style={commandLabelStyle}>Canary Duration (min)</div>
              <input
                type="number"
                value={config.canary_duration_minutes}
                onChange={(e) => setConfig({ ...config, canary_duration_minutes: parseInt(e.target.value) || 30 })}
                style={{ width: "100%", background: "rgba(0,0,0,0.3)", border: "1px solid rgba(148,163,184,0.2)", borderRadius: 6, padding: 6, color: "#f8fafc", marginTop: 4 }}
              />
            </div>
            <div style={commandInsetStyle}>
              <div style={commandLabelStyle}>Fuel Budget</div>
              <input
                type="number"
                value={config.fuel_budget}
                onChange={(e) => setConfig({ ...config, fuel_budget: parseInt(e.target.value) || 5000 })}
                style={{ width: "100%", background: "rgba(0,0,0,0.3)", border: "1px solid rgba(148,163,184,0.2)", borderRadius: 6, padding: 6, color: "#f8fafc", marginTop: 4 }}
              />
            </div>
          </div>
          <div style={{ marginTop: 12 }}>
            <div style={commandLabelStyle}>Enabled Domains</div>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 8, marginTop: 6 }}>
              {["PromptOptimization", "ConfigTuning", "GovernancePolicy", "SchedulingPolicy", "RoutingStrategy", "CodePatch"].map((domain) => {
                const enabled = config.enabled_domains.includes(domain);
                const isCodePatch = domain === "CodePatch";
                return (
                  <button
                    key={domain}
                    onClick={() => {
                      if (isCodePatch) return;
                      setConfig({
                        ...config,
                        enabled_domains: enabled
                          ? config.enabled_domains.filter((d) => d !== domain)
                          : [...config.enabled_domains, domain],
                      });
                    }}
                    title={isCodePatch ? "Requires code-self-modify feature flag (Phase 5)" : ""}
                    style={{
                      padding: "4px 10px",
                      borderRadius: 6,
                      border: `1px solid ${enabled ? (DOMAIN_COLORS[domain] || "#94a3b8") : "rgba(148,163,184,0.2)"}`,
                      background: enabled ? `${DOMAIN_COLORS[domain] || "#94a3b8"}22` : "transparent",
                      color: isCodePatch ? "#475569" : enabled ? DOMAIN_COLORS[domain] || "#94a3b8" : "#64748b",
                      fontSize: "0.75rem",
                      cursor: isCodePatch ? "not-allowed" : "pointer",
                      opacity: isCodePatch ? 0.5 : 1,
                    }}
                  >
                    {isCodePatch ? "\uD83D\uDD12 " : ""}{domain}
                  </button>
                );
              })}
            </div>
          </div>
          <div style={{ marginTop: 12 }}>
            <ActionButton onClick={handleConfigSave} disabled={working}>Save Configuration</ActionButton>
          </div>
        </Panel>
      )}

      {/* SECTION 5: Guardian & Metrics */}
      {guardian && (
        <Panel title="Simplex Guardian" style={{ marginTop: 16 }}>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr 1fr", gap: 10 }}>
            <div style={commandInsetStyle}>
              <div style={commandLabelStyle}>Status</div>
              <div style={{ fontSize: "1rem", fontWeight: 700, color: guardian.decision === "continue_active" ? "#34d399" : "#ef4444" }}>
                {guardian.decision === "continue_active" ? "Active" : "SWITCHING"}
              </div>
            </div>
            <div style={commandInsetStyle}>
              <div style={commandLabelStyle}>Current Drift</div>
              <div style={{ fontSize: "1rem", fontWeight: 700, color: "#f8fafc", fontFamily: "monospace" }}>{guardian.current_drift.toFixed(4)}</div>
            </div>
            <div style={commandInsetStyle}>
              <div style={commandLabelStyle}>Drift Bound (D*)</div>
              <div style={{ fontSize: "1rem", fontWeight: 700, color: "#f8fafc", fontFamily: "monospace" }}>{guardian.drift_bound === Infinity ? "\u221E" : guardian.drift_bound.toFixed(4)}</div>
            </div>
            <div style={commandInsetStyle}>
              <div style={commandLabelStyle}>Headroom</div>
              <div style={{ fontSize: "1rem", fontWeight: 700, color: guardian.headroom > 0.5 ? "#34d399" : "#fbbf24", fontFamily: "monospace" }}>{guardian.headroom === Infinity ? "\u221E" : guardian.headroom.toFixed(4)}</div>
            </div>
          </div>
          <div style={{ marginTop: 8, fontSize: "0.75rem", color: "#64748b" }}>
            Baseline: {guardian.baseline_hash ? guardian.baseline_hash.slice(0, 16) + "..." : "none"} | Threshold: {guardian.switch_threshold.toFixed(2)}
          </div>
        </Panel>
      )}

      {/* SECTION 6: Success Rate Summary */}
      {status && (
        <Panel title="Improvement Metrics" style={{ marginTop: 16 }}>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 10 }}>
            <div style={commandInsetStyle}>
              <div style={commandLabelStyle}>Total Applied</div>
              <div style={{ fontSize: "1.3rem", fontWeight: 700, color: "#38bdf8" }}>{(status.committed_count || 0) + (status.monitoring_count || 0) + (status.rolled_back_count || 0)}</div>
            </div>
            <div style={commandInsetStyle}>
              <div style={commandLabelStyle}>Committed</div>
              <div style={{ fontSize: "1.3rem", fontWeight: 700, color: "#34d399" }}>{status.committed_count}</div>
            </div>
            <div style={commandInsetStyle}>
              <div style={commandLabelStyle}>Rolled Back</div>
              <div style={{ fontSize: "1.3rem", fontWeight: 700, color: "#ef4444" }}>{status.rolled_back_count}</div>
            </div>
            <div style={commandInsetStyle}>
              <div style={commandLabelStyle}>Success Rate</div>
              <div style={{ fontSize: "1.3rem", fontWeight: 700, color: "#f8fafc" }}>
                {(() => {
                  const total = (status.committed_count || 0) + (status.rolled_back_count || 0);
                  return total > 0 ? `${((status.committed_count || 0) / total * 100).toFixed(0)}%` : "N/A";
                })()}
              </div>
            </div>
          </div>
        </Panel>
      )}
    </div>
  );
}

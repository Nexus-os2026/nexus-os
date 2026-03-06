import "./trust-dashboard.css";

interface AgentTrust {
  id: string;
  name: string;
  trustScore: number;
  autonomyLevel: number;
  totalRuns: number;
  successRate: number;
  violations: number;
  promotionPending: boolean;
  demotionPending: boolean;
}

const MOCK_TRUST: AgentTrust[] = [
  { id: "a0000000-0000-4000-8000-000000000001", name: "Coder", trustScore: 0.94, autonomyLevel: 3, totalRuns: 142, successRate: 0.96, violations: 0, promotionPending: true, demotionPending: false },
  { id: "a0000000-0000-4000-8000-000000000002", name: "Designer", trustScore: 0.78, autonomyLevel: 2, totalRuns: 67, successRate: 0.88, violations: 1, promotionPending: false, demotionPending: false },
  { id: "a0000000-0000-4000-8000-000000000003", name: "Screen Poster", trustScore: 0.65, autonomyLevel: 2, totalRuns: 89, successRate: 0.82, violations: 2, promotionPending: false, demotionPending: false },
  { id: "a0000000-0000-4000-8000-000000000004", name: "Web Builder", trustScore: 0.85, autonomyLevel: 3, totalRuns: 54, successRate: 0.91, violations: 0, promotionPending: false, demotionPending: false },
  { id: "a0000000-0000-4000-8000-000000000005", name: "Workflow Studio", trustScore: 0.22, autonomyLevel: 1, totalRuns: 31, successRate: 0.58, violations: 5, promotionPending: false, demotionPending: true },
  { id: "a0000000-0000-4000-8000-000000000006", name: "Self-Improve", trustScore: 0.91, autonomyLevel: 4, totalRuns: 203, successRate: 0.95, violations: 0, promotionPending: true, demotionPending: false },
];

const AUTONOMY_LABELS = ["L0 Inert", "L1 Suggest", "L2 Act+Approve", "L3 Act+Report", "L4 Autonomous", "L5 Full"];

function trustColor(score: number): string {
  if (score >= 0.7) return "#22c55e";
  if (score >= 0.4) return "#eab308";
  return "#ef4444";
}

export default function TrustDashboard(): JSX.Element {
  return (
    <section className="td-hub">
      <header className="td-header">
        <h2 className="td-title">TRUST DASHBOARD // ADAPTIVE GOVERNANCE</h2>
        <p className="td-subtitle">{MOCK_TRUST.length} agents tracked</p>
      </header>

      <div className="td-grid">
        {MOCK_TRUST.map((agent) => {
          const pct = Math.round(agent.trustScore * 100);
          const color = trustColor(agent.trustScore);
          return (
            <article key={agent.id} className="td-card">
              <div className="td-card-top">
                <h3 className="td-card-name">{agent.name}</h3>
                <div className="td-indicators">
                  {agent.promotionPending && <span className="td-promo-badge">PROMO</span>}
                  {agent.demotionPending && <span className="td-demo-badge">DEMO</span>}
                </div>
              </div>

              <div className="td-score-row">
                <div className="td-score-ring" style={{ borderColor: color }}>
                  <span className="td-score-pct" style={{ color }}>{pct}%</span>
                </div>
                <div className="td-score-details">
                  <div className="td-detail-row">
                    <span className="td-label">Trust Score</span>
                    <span className="td-value" style={{ color }}>{agent.trustScore.toFixed(2)}</span>
                  </div>
                  <div className="td-detail-row">
                    <span className="td-label">Autonomy</span>
                    <span className="td-value">{AUTONOMY_LABELS[agent.autonomyLevel]}</span>
                  </div>
                </div>
              </div>

              <div className="td-stats">
                <div className="td-stat">
                  <span className="td-stat-value">{agent.totalRuns}</span>
                  <span className="td-stat-label">Runs</span>
                </div>
                <div className="td-stat">
                  <span className="td-stat-value">{Math.round(agent.successRate * 100)}%</span>
                  <span className="td-stat-label">Success</span>
                </div>
                <div className="td-stat">
                  <span className="td-stat-value" style={{ color: agent.violations > 0 ? "#ef4444" : "#22c55e" }}>
                    {agent.violations}
                  </span>
                  <span className="td-stat-label">Violations</span>
                </div>
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}

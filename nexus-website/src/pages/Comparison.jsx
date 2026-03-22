import { COMPARISON_CAPABILITIES, COMPETITORS } from '../data/constants';
import ThreeScene from '../components/ThreeScene';
import { createComparisonShieldScene } from '../components/sceneFactories';
import { useScrollRevealChildren } from '../hooks/useScrollReveal';

const COMPETITOR_COLUMNS = [
  { key: 'nexus', name: 'Nexus OS', stars: null },
  { key: 'langgraph', name: COMPETITORS.langgraph.name, stars: COMPETITORS.langgraph.stars },
  { key: 'crewai', name: COMPETITORS.crewai.name, stars: COMPETITORS.crewai.stars },
  { key: 'autogen', name: COMPETITORS.autogen.name, stars: COMPETITORS.autogen.stars },
  { key: 'openai', name: COMPETITORS.openai_agents.name, stars: COMPETITORS.openai_agents.stars },
];

function formatStars(stars) {
  if (stars == null) {
    return 'LOCAL-FIRST';
  }
  return stars >= 1000 ? `${(stars / 1000).toFixed(1)}K` : String(stars);
}

function renderCellValue(value) {
  if (typeof value === 'boolean') {
    return <span className="cell-value" style={{ color: value ? '#00ff88' : '#5a6675' }}>{value ? '✓' : '✗'}</span>;
  }
  return <span className="cell-value">{value}</span>;
}

export default function Comparison() {
  const revealRef = useScrollRevealChildren({ stagger: 80, threshold: 0.12 });

  return (
    <div className="page-shell">
      <section className="section" ref={revealRef}>
        <div className="section-header fade-up">
          <div className="section-kicker">Tactical Advantage // Comparison Matrix</div>
          <h1 className="section-title" style={{ fontSize: 'clamp(2.4rem, 6vw, 4.8rem)' }}>Why Nexus Wins</h1>
          <p className="section-copy">
            THE MATRIX BELOW COMPARES GOVERNANCE, LOCAL-FIRST OPERATION, CRYPTOGRAPHIC IDENTITY, AND HARDENED EXECUTION AGAINST POPULAR AGENT FRAMEWORKS.
          </p>
        </div>

        <div className="fade-up comparison-command-layout" style={{
          display: 'grid',
          gridTemplateColumns: 'minmax(0, 1fr) 360px',
          gap: 18,
          alignItems: 'start',
        }}>
          <div className="holo-panel" style={{ overflowX: 'auto' }}>
            <table className="comparison-table">
              <thead>
                <tr>
                  <th>Capability</th>
                  {COMPETITOR_COLUMNS.map((column) => (
                    <th key={column.key} className={column.key === 'nexus' ? 'is-nexus' : ''}>
                      <div>{column.name}</div>
                      <div style={{ marginTop: 6, color: '#5a7a8a', fontFamily: "'JetBrains Mono', monospace", fontSize: '0.7rem' }}>
                        {formatStars(column.stars)}
                      </div>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {COMPARISON_CAPABILITIES.map((capability) => (
                  <tr key={capability.name}>
                    <td style={{ color: '#e2eef7', fontFamily: "'Rajdhani', sans-serif", fontWeight: 600 }}>{capability.name}</td>
                    {COMPETITOR_COLUMNS.map((column) => (
                      <td key={`${capability.name}-${column.key}`} className={column.key === 'nexus' ? 'is-nexus' : ''}>
                        {renderCellValue(capability[column.key])}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <div className="holo-panel" style={{ display: 'grid', gap: 18 }}>
            <div className="panel-kicker">Rotating Shield</div>
            <ThreeScene
              setup={createComparisonShieldScene()}
              height={320}
              fallback={<div className="mobile-3d-icon"><span>NEXUS</span></div>}
              ariaLabel="Comparison shield"
            />
            <div className="agent-meta-cell">
              <div className="agent-meta-label">Win Condition</div>
              <div className="agent-meta-value">LOCAL-FIRST + GOVERNANCE KERNEL + AIR-GAPPED OPERATION</div>
            </div>
            <div className="agent-meta-cell">
              <div className="agent-meta-label">Command Bias</div>
              <div className="agent-meta-value">BUILT FOR CONTROLLED AUTONOMY, NOT JUST ORCHESTRATION</div>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

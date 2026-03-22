import { startTransition, useDeferredValue, useMemo, useState } from 'react';
import { AGENT_CATEGORIES, STATS } from '../data/constants';
import ThreeScene from '../components/ThreeScene';
import { createAgentSphereScene } from '../components/sceneFactories';
import { useScrollRevealChildren } from '../hooks/useScrollReveal';

const SUPPLEMENTAL_AGENTS = [
  { name: 'approval-agent', level: 'L2', description: 'Human approval workflow coordinator', category: 'Security' },
  { name: 'browser-agent', level: 'L3', description: 'Governed browser automation and capture', category: 'Technical' },
  { name: 'voice-agent', level: 'L2', description: 'Voice ingress, wake-word, and TTS bridge', category: 'Communication' },
  { name: 'observer-agent', level: 'L2', description: 'Runtime telemetry and anomaly observation', category: 'Specialized' },
  { name: 'fleet-agent', level: 'L4', description: 'Distributed roster and remote node control', category: 'Specialized' },
  { name: 'recovery-agent', level: 'L3', description: 'Rollback, replay, and checkpoint recovery', category: 'Technical' },
  { name: 'policy-agent', level: 'L4', description: 'Governance policy synthesis and drift checks', category: 'Security' },
];

const LEVEL_COLORS = {
  L0: '#5a6675',
  L1: '#6f8396',
  L2: '#00d4ff',
  L3: '#1b9cff',
  L4: '#00ff88',
  L5: '#ff6a00',
  L6: '#ff0044',
};

const LEVEL_COLORS_THREE = {
  L0: 0x5a6675,
  L1: 0x6f8396,
  L2: 0x00d4ff,
  L3: 0x1b9cff,
  L4: 0x00ff88,
  L5: 0xff6a00,
  L6: 0xff0044,
};

const LEVEL_LABELS = {
  L0: 'INERT',
  L1: 'SUGGEST',
  L2: 'ACT WITH APPROVAL',
  L3: 'ACT THEN REPORT',
  L4: 'AUTONOMOUS BOUNDED',
  L5: 'FULL AUTONOMY',
  L6: 'TRANSCENDENT',
};

const FUEL_BY_LEVEL = {
  L0: '0 FUEL',
  L1: '250 FUEL',
  L2: '1,500 FUEL',
  L3: '5,000 FUEL',
  L4: '12,000 FUEL',
  L5: '22,000 FUEL',
  L6: '50,000 FUEL',
};

const CATEGORY_CAPABILITIES = {
  Cognitive: ['strategic planning', 'memory synthesis', 'temporal forking'],
  Creative: ['design generation', 'copy systems', 'web composition'],
  Technical: ['code mutation', 'deployment ops', 'database control'],
  Security: ['audit trails', 'identity proofs', 'firewall scans'],
  Communication: ['connector messaging', 'webhook routing', 'email actions'],
  Specialized: ['research loops', 'scheduler orchestration', 'distributed control'],
};

function buildAgentRecord(agent, category, index) {
  const genomeNumber = `${9 + (index % 4)}.${(index % 7) + 1}.${(index % 3) + 2}`;
  const status = agent.level === 'L0' || index % 5 === 0 ? 'standby' : 'active';

  return {
    ...agent,
    category,
    status,
    autonomyLabel: LEVEL_LABELS[agent.level],
    fuelBudget: FUEL_BY_LEVEL[agent.level],
    genomeVersion: `GENOME-${genomeNumber}`,
    capabilities: CATEGORY_CAPABILITIES[category] || ['governed execution', 'audit logging', 'local inference'],
  };
}

export default function Agents() {
  const revealRef = useScrollRevealChildren({ stagger: 70, threshold: 0.12 });
  const [search, setSearch] = useState('');
  const [levelFilter, setLevelFilter] = useState('All');
  const [categoryFilter, setCategoryFilter] = useState('All');
  const [expandedName, setExpandedName] = useState(null);
  const deferredSearch = useDeferredValue(search);

  const allAgents = useMemo(() => {
    const records = [];

    AGENT_CATEGORIES.forEach((category) => {
      category.agents.forEach((agent) => {
        records.push(buildAgentRecord(agent, category.name, records.length));
      });
    });

    SUPPLEMENTAL_AGENTS.forEach((agent) => {
      records.push(buildAgentRecord(agent, agent.category, records.length));
    });

    return records;
  }, []);

  const categories = useMemo(
    () => ['All', ...new Set(allAgents.map((agent) => agent.category))],
    [allAgents],
  );

  const filteredAgents = useMemo(() => {
    const query = deferredSearch.trim().toLowerCase();
    return allAgents.filter((agent) => {
      const matchesLevel = levelFilter === 'All' || agent.level === levelFilter;
      const matchesCategory = categoryFilter === 'All' || agent.category === categoryFilter;
      const matchesQuery = !query
        || agent.name.toLowerCase().includes(query)
        || agent.description.toLowerCase().includes(query)
        || agent.capabilities.join(' ').includes(query);
      return matchesLevel && matchesCategory && matchesQuery;
    });
  }, [allAgents, categoryFilter, deferredSearch, levelFilter]);

  const sceneSetup = useMemo(
    () => createAgentSphereScene(allAgents, { colors: LEVEL_COLORS_THREE }),
    [allAgents],
  );

  return (
    <div className="page-shell">
      <section className="section" ref={revealRef}>
        <div className="section-header fade-up">
          <div className="section-kicker">Unit Roster // Tactical Registry</div>
          <h1 className="section-title" style={{ fontSize: 'clamp(2.4rem, 6vw, 4.8rem)' }}>Agent Command Roster</h1>
          <p className="section-copy">
            SEARCH THE REGISTRY, FILTER BY AUTONOMY BAND, AND EXPAND ANY UNIT FOR CAPABILITIES, FUEL BUDGET, AND GENOME VERSION.
          </p>
        </div>

        <div className="fade-up agents-command-layout" style={{
          display: 'grid',
          gridTemplateColumns: 'minmax(0, 1.1fr) 420px',
          gap: 18,
          alignItems: 'start',
        }}>
          <div className="holo-panel" style={{ display: 'grid', gap: 18 }}>
            <div className="pill-row">
              <span className="status-pill success">{`${STATS.agents} REGISTERED UNITS`}</span>
              <span className="status-pill">L0-L6 GOVERNANCE BANDS</span>
            </div>

            <div style={{ position: 'relative' }}>
              <span style={{
                position: 'absolute',
                left: 16,
                top: '50%',
                transform: 'translateY(-50%)',
                color: '#4e7182',
                fontFamily: "'JetBrains Mono', monospace",
                zIndex: 1,
              }}>{'>_'}</span>
              <input
                className="scan-input"
                value={search}
                onChange={(event) => startTransition(() => setSearch(event.target.value))}
                placeholder="SCAN AGENT REGISTRY..."
              />
            </div>

            <div style={{ display: 'grid', gap: 14 }}>
              <div>
                <div className="panel-kicker" style={{ marginBottom: 10 }}>Autonomy Filter</div>
                <div className="pill-row">
                  {['All', 'L0', 'L1', 'L2', 'L3', 'L4', 'L5', 'L6'].map((level) => (
                    <button
                      key={level}
                      className={`status-pill ${levelFilter === level ? 'success' : ''}`}
                      style={{
                        cursor: 'pointer',
                        color: level === 'All' ? '#00d4ff' : LEVEL_COLORS[level] || '#00d4ff',
                        background: levelFilter === level ? `${LEVEL_COLORS[level] || '#00d4ff'}14` : 'rgba(0, 212, 255, 0.06)',
                      }}
                      onClick={() => setLevelFilter(level)}
                    >
                      {level}
                    </button>
                  ))}
                </div>
              </div>

              <div>
                <div className="panel-kicker" style={{ marginBottom: 10 }}>Division Filter</div>
                <div className="pill-row">
                  {categories.map((category) => (
                    <button
                      key={category}
                      className={`status-pill ${categoryFilter === category ? 'success' : ''}`}
                      style={{ cursor: 'pointer' }}
                      onClick={() => setCategoryFilter(category)}
                    >
                      {category}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          </div>

          <div className="holo-panel" style={{ display: 'grid', gap: 18 }}>
            <div className="panel-kicker">Rotating Agent Sphere</div>
            <ThreeScene
              setup={sceneSetup}
              height={340}
              fallback={<div className="mobile-3d-icon"><span>ROSTER</span></div>}
              ariaLabel="Agent roster sphere"
            />
            <div className="data-readout-frame">
              <div className="data-readout">
                <span>ACTIVE FILTER: {levelFilter.toUpperCase()} / {categoryFilter.toUpperCase()}</span>
                <span>RESULTS: {filteredAgents.length} UNIT(S)</span>
                <span>SCAN STATUS: {deferredSearch ? 'QUERY LOCKED' : 'PASSIVE MONITORING'}</span>
              </div>
            </div>
          </div>
        </div>

        <div className="fade-up" style={{ marginTop: 18 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 18 }}>
            <div className="hud-rule" />
            <span className="panel-kicker" style={{ whiteSpace: 'nowrap' }}>{filteredAgents.length} Units Located</span>
            <div className="hud-rule" />
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))', gap: 18 }}>
            {filteredAgents.map((agent, index) => {
              const expanded = expandedName === agent.name;
              const tint = LEVEL_COLORS[agent.level];

              return (
                <button
                  key={agent.name}
                  className="holo-panel"
                  onClick={() => setExpandedName(expanded ? null : agent.name)}
                  style={{ display: 'grid', gap: 16, textAlign: 'left', cursor: 'pointer' }}
                >
                  <div className="agent-header-row">
                    <div className="agent-avatar" style={{ color: tint }}>{agent.name.charAt(0).toUpperCase()}</div>
                    <div style={{ flex: 1 }}>
                      <div className="agent-name">{agent.name}</div>
                      <div className="agent-subtitle">{agent.category.toUpperCase()}</div>
                    </div>
                    <div style={{ display: 'grid', gap: 8, justifyItems: 'end' }}>
                      <span className={`status-pill ${agent.status === 'active' ? 'success' : ''}`} style={{
                        color: agent.status === 'active' ? '#00ff88' : '#6f8396',
                        background: agent.status === 'active' ? 'rgba(0, 255, 136, 0.08)' : 'rgba(111, 131, 150, 0.1)',
                      }}>
                        {agent.status}
                      </span>
                      <span className="status-pill warning" style={{ color: tint, background: `${tint}18` }}>{agent.level}</span>
                    </div>
                  </div>

                  <p className="panel-copy" style={{ fontSize: '0.9rem' }}>{agent.description}</p>

                  <div className="agent-meta-grid">
                    <div className="agent-meta-cell">
                      <div className="agent-meta-label">Autonomy</div>
                      <div className="agent-meta-value">{agent.autonomyLabel}</div>
                    </div>
                    <div className="agent-meta-cell">
                      <div className="agent-meta-label">Fuel Budget</div>
                      <div className="agent-meta-value">{agent.fuelBudget}</div>
                    </div>
                  </div>

                  {expanded ? (
                    <div style={{ display: 'grid', gap: 12 }}>
                      <div className="agent-meta-cell">
                        <div className="agent-meta-label">Genome Version</div>
                        <div className="agent-meta-value">{agent.genomeVersion}</div>
                      </div>
                      <div className="agent-meta-cell">
                        <div className="agent-meta-label">Capabilities</div>
                        <div className="agent-meta-value">{agent.capabilities.join(' / ')}</div>
                      </div>
                    </div>
                  ) : null}

                  <div className="agent-subtitle">{`UNIT-${String(index + 1).padStart(2, '0')} // CLICK TO ${expanded ? 'COLLAPSE' : 'EXPAND'}`}</div>
                </button>
              );
            })}
          </div>
        </div>
      </section>
    </div>
  );
}

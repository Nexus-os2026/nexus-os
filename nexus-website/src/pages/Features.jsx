import { startTransition, useMemo, useState } from 'react';
import { FEATURES_DEEP } from '../data/constants';
import CodeBlock from '../components/CodeBlock';
import ThreeScene from '../components/ThreeScene';
import { createModelScene } from '../components/sceneFactories';
import { useScrollRevealChildren } from '../hooks/useScrollReveal';

const MODEL_BY_FEATURE = {
  governance: 'governance',
  darwin: 'darwin',
  wasm: 'sandbox',
  flash: 'flash',
  mcp: 'protocol',
  scheduler: 'scheduler',
  ghost: 'identity',
  connectors: 'integrations',
  enterprise: 'enterprise',
  persistence: 'persistence',
};

export default function Features() {
  const [selectedId, setSelectedId] = useState(FEATURES_DEEP[0].id);
  const revealRef = useScrollRevealChildren({ stagger: 90, threshold: 0.12 });
  const selectedFeature = useMemo(
    () => FEATURES_DEEP.find((feature) => feature.id === selectedId) || FEATURES_DEEP[0],
    [selectedId],
  );
  const selectedIndex = FEATURES_DEEP.findIndex((feature) => feature.id === selectedFeature.id);
  const sceneSetup = useMemo(
    () => createModelScene(MODEL_BY_FEATURE[selectedFeature.id] || selectedFeature.id, { cameraZ: 6.2, scale: 1.18 }),
    [selectedFeature.id],
  );

  return (
    <div className="page-shell">
      <section className="section" ref={revealRef}>
        <div className="section-header fade-up">
          <div className="section-kicker">System Specifications // Armor Bay Layout</div>
          <h1 className="section-title" style={{ fontSize: 'clamp(2.4rem, 6vw, 4.8rem)' }}>Feature Command Deck</h1>
          <p className="section-copy">
            LEFT: feature index. CENTER: operational detail and implementation trace. RIGHT: live rotating tactical model.
          </p>
        </div>

        <div className="fade-up feature-command-layout" style={{
          display: 'grid',
          gridTemplateColumns: '280px minmax(0, 1fr) 360px',
          gap: 18,
          alignItems: 'start',
        }}>
          <aside className="holo-panel" style={{ position: 'sticky', top: 104 }}>
            <div className="panel-kicker" style={{ marginBottom: 16 }}>Feature Index</div>
            <div className="sidebar-list">
              {FEATURES_DEEP.map((feature, index) => {
                const active = feature.id === selectedFeature.id;
                return (
                  <button
                    key={feature.id}
                    className={`sidebar-button ${active ? 'is-active' : ''}`}
                    onClick={() => startTransition(() => setSelectedId(feature.id))}
                  >
                    <span className="sidebar-index">{`4.${index + 1}`}</span>
                    <div>
                      <div className="panel-title" style={{ fontSize: '0.86rem', marginBottom: 6 }}>{feature.title}</div>
                      <div className="panel-copy" style={{ fontSize: '0.74rem' }}>{feature.subtitle}</div>
                    </div>
                  </button>
                );
              })}
            </div>
          </aside>

          <div className="holo-panel" style={{ display: 'grid', gap: 24 }}>
            <div className="pill-row">
              <span className="status-pill warning">{`SECTION 4.${selectedIndex + 1}`}</span>
              <span className="status-pill">SPEC // {selectedFeature.id.toUpperCase()}</span>
            </div>
            <div>
              <div className="panel-title" style={{ fontSize: '1.7rem', marginBottom: 10 }}>{selectedFeature.title}</div>
              <div className="panel-kicker" style={{ marginBottom: 14 }}>{selectedFeature.subtitle}</div>
              <p className="panel-copy">{selectedFeature.description}</p>
            </div>

            <div className="metric-grid compact">
              {selectedFeature.stats.map((stat) => (
                <div key={stat.label} className="metric-card compact">
                  <span className="stat-label">{stat.label}</span>
                  <span className="stat-value" style={{ fontSize: '1.75rem' }}>{stat.value}</span>
                </div>
              ))}
            </div>

            <CodeBlock
              code={selectedFeature.code}
              language="rust"
              label={`${selectedFeature.id.toUpperCase()} // IMPLEMENTATION`}
            />
          </div>

          <div className="holo-panel" style={{ display: 'grid', gap: 18 }}>
            <div className="panel-kicker">Rotating Showcase</div>
            <ThreeScene
              setup={sceneSetup}
              height={360}
              fallback={<div className="mobile-3d-icon"><span>{selectedFeature.id}</span></div>}
              ariaLabel={`${selectedFeature.title} showcase model`}
            />
            <div className="hud-rule" />
            <div style={{ display: 'grid', gap: 12 }}>
              <div className="agent-meta-cell">
                <div className="agent-meta-label">Selected Module</div>
                <div className="agent-meta-value">{selectedFeature.title}</div>
              </div>
              <div className="agent-meta-cell">
                <div className="agent-meta-label">Operational Intent</div>
                <div className="agent-meta-value">{selectedFeature.subtitle}</div>
              </div>
              <div className="agent-meta-cell">
                <div className="agent-meta-label">Visualization Mode</div>
                <div className="agent-meta-value">360 DEGREE ROTATION + CURSOR TILT</div>
              </div>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

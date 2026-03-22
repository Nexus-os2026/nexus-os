import { useMemo, useState } from 'react';
import { ARCHITECTURE_LAYERS } from '../data/constants';
import ThreeScene from '../components/ThreeScene';
import { createArchitectureBlueprintScene } from '../components/sceneFactories';
import { useScrollRevealChildren } from '../hooks/useScrollReveal';

export default function Architecture() {
  const [activeIndex, setActiveIndex] = useState(0);
  const revealRef = useScrollRevealChildren({ stagger: 90, threshold: 0.12 });
  const activeLayer = ARCHITECTURE_LAYERS[activeIndex];
  const sceneSetup = useMemo(
    () => createArchitectureBlueprintScene(ARCHITECTURE_LAYERS, { activeIndex }),
    [activeIndex],
  );

  return (
    <div className="page-shell">
      <section className="section" ref={revealRef}>
        <div className="section-header fade-up">
          <div className="section-kicker">3D System Blueprint // Layer Cake</div>
          <h1 className="section-title" style={{ fontSize: 'clamp(2.4rem, 6vw, 4.8rem)' }}>Architecture Stack</h1>
          <p className="section-copy">
            DRAG THE BLUEPRINT TO ROTATE. EACH SLAB MAPS TO A GOVERNED SYSTEM LAYER, WITH LIVE HUD DETAIL LOCKED TO THE CURRENT SELECTION.
          </p>
        </div>

        <div className="fade-up architecture-command-layout" style={{
          display: 'grid',
          gridTemplateColumns: '280px minmax(0, 1fr) 320px',
          gap: 18,
          alignItems: 'start',
        }}>
          <aside className="holo-panel" style={{ position: 'sticky', top: 104 }}>
            <div className="panel-kicker" style={{ marginBottom: 16 }}>Layer Index</div>
            <div className="sidebar-list">
              {ARCHITECTURE_LAYERS.map((layer, index) => (
                <button
                  key={layer.name}
                  className={`sidebar-button ${index === activeIndex ? 'is-active' : ''}`}
                  onClick={() => setActiveIndex(index)}
                >
                  <span className="sidebar-index">{`L${index}`}</span>
                  <div>
                    <div className="panel-title" style={{ fontSize: '0.86rem', marginBottom: 6 }}>{layer.name}</div>
                    <div className="panel-copy" style={{ fontSize: '0.74rem' }}>{layer.tech}</div>
                  </div>
                </button>
              ))}
            </div>
          </aside>

          <div className="holo-panel" style={{ display: 'grid', gap: 18 }}>
            <div className="pill-row">
              <span className="status-pill">AUTO-ROTATE ENABLED</span>
              <span className="status-pill warning">DRAG TO REORIENT</span>
            </div>
            <ThreeScene
              setup={sceneSetup}
              height={520}
              fallback={<div className="mobile-3d-icon"><span>STACK</span></div>}
              ariaLabel="Architecture blueprint"
            />
            <div className="data-readout-frame">
              <div className="data-readout">
                <span>ACTIVE LAYER: {activeLayer.name.toUpperCase()}</span>
                <span>PRIMARY TECH: {activeLayer.tech.toUpperCase()}</span>
                <span>CRATE GROUPS: {activeLayer.crates.join(' / ').toUpperCase()}</span>
              </div>
            </div>
          </div>

          <div className="holo-panel" style={{ display: 'grid', gap: 18 }}>
            <div className="panel-kicker">HUD Overlay</div>
            <div className="panel-title" style={{ fontSize: '1.2rem' }}>{activeLayer.name}</div>
            <p className="panel-copy">{activeLayer.description}</p>
            <div className="agent-meta-cell">
              <div className="agent-meta-label">Technology Stack</div>
              <div className="agent-meta-value">{activeLayer.tech}</div>
            </div>
            <div className="agent-meta-cell">
              <div className="agent-meta-label">Crates</div>
              <div className="agent-meta-value">{activeLayer.crates.join(', ')}</div>
            </div>
            <div className="agent-meta-cell">
              <div className="agent-meta-label">Selection</div>
              <div className="agent-meta-value">{`L${activeIndex} / ${ARCHITECTURE_LAYERS.length - 1}`}</div>
            </div>
          </div>
        </div>

        <div className="fade-up" style={{ marginTop: 18 }}>
          <div className="metric-grid">
            {ARCHITECTURE_LAYERS.map((layer, index) => (
              <button
                key={layer.name}
                className={`holo-panel ${index === activeIndex ? 'glow-active' : ''}`}
                onClick={() => setActiveIndex(index)}
                style={{ display: 'grid', gap: 12, textAlign: 'left', cursor: 'pointer' }}
              >
                <div className="panel-kicker">{`L${index}`}</div>
                <div className="panel-title" style={{ fontSize: '1rem' }}>{layer.name}</div>
                <p className="panel-copy" style={{ fontSize: '0.92rem' }}>{layer.description}</p>
              </button>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}

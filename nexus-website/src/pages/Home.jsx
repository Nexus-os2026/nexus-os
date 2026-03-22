import { useEffect, useMemo, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import {
  GITLAB_LANGUAGES,
  GOVERNANCE_PIPELINE,
  STATS,
  VERSION,
} from '../data/constants';
import ThreeScene from '../components/ThreeScene';
import { createModelScene, createNeuralCoreScene } from '../components/sceneFactories';
import { useReducedMotion } from '../hooks/useReducedMotion';
import { useScrollRevealChildren } from '../hooks/useScrollReveal';

const HOME_FEATURES = [
  {
    id: 'governance',
    model: 'governance',
    title: 'Governance Kernel',
    stat: '09 SECURITY GATES',
    description: 'Capability ACL, fuel metering, human approval, sandboxing, and hash-chained audit trails control every action.',
  },
  {
    id: 'darwin',
    model: 'darwin',
    title: 'Darwin Core',
    stat: '47 ACTIVE GENOMES',
    description: 'Adversarial arenas, genetic crossover, and multi-agent selection evolve better behavior instead of guessing at it.',
  },
  {
    id: 'sandbox',
    model: 'sandbox',
    title: 'WASM Sandbox',
    stat: '~5MS BOOT',
    description: 'Execution stays boxed inside memory-limited Wasmtime containers with host access stripped down to capability grants.',
  },
  {
    id: 'flash',
    model: 'flash',
    title: 'Local Inference',
    stat: '93 MODEL SLOTS',
    description: 'Inference routing stays on-machine, governed, auditable, and ready for disconnected environments.',
  },
  {
    id: 'protocol',
    model: 'protocol',
    title: 'MCP + A2A Mesh',
    stat: 'MUTUAL CAPABILITY',
    description: 'Inter-agent traffic is authenticated, encrypted, and policy-checked before any payload crosses a boundary.',
  },
  {
    id: 'persistence',
    model: 'persistence',
    title: 'Persistent State',
    stat: 'CRASH-SAFE WAL',
    description: 'SQLite WAL persistence and integrity chains keep the command surface tamper-evident and recoverable under load.',
  },
];

const HOME_METRICS = [
  { label: 'AGENTS ONLINE', value: String(STATS.agents), meta: 'UNIT ROSTER VERIFIED' },
  { label: 'TEST COVERAGE', value: STATS.tests, meta: 'REGRESSION FIREWALL' },
  { label: 'RUST LINES', value: STATS.rustLines, meta: 'KERNEL + CRATES' },
  { label: 'COMMANDS', value: String(STATS.commands), meta: 'SURFACE AREA LOCKED' },
  { label: 'GEN-3 SYSTEMS', value: String(STATS.gen3Systems), meta: 'LIVE SUBSYSTEMS' },
  { label: 'CRASH SITES', value: String(STATS.panics), meta: 'NO PANIC EVENTS' },
];

function parseMetricValue(value) {
  const cleaned = value.replaceAll(',', '');
  if (cleaned.endsWith('K')) {
    return Number.parseFloat(cleaned) * 1000;
  }
  return Number.parseInt(cleaned, 10);
}

function formatMetricValue(value, template) {
  if (template.endsWith('K')) {
    return `${Math.round(value / 1000)}K`;
  }
  if (template.includes(',')) {
    return Math.round(value).toLocaleString();
  }
  return String(Math.round(value));
}

function useVisibilityState(threshold = 0.2) {
  const ref = useRef(null);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const node = ref.current;
    if (!node) {
      return undefined;
    }

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setVisible(true);
          observer.unobserve(node);
        }
      },
      { threshold },
    );

    observer.observe(node);
    return () => observer.disconnect();
  }, [threshold]);

  return [ref, visible];
}

function AnimatedMetric({ value, active }) {
  const [displayValue, setDisplayValue] = useState('0');

  useEffect(() => {
    if (!active) {
      return undefined;
    }

    const target = parseMetricValue(value);
    const start = performance.now();
    const duration = 1600;
    let frameId;

    const tick = (timestamp) => {
      const progress = Math.min((timestamp - start) / duration, 1);
      const eased = 1 - Math.pow(1 - progress, 3);
      setDisplayValue(formatMetricValue(target * eased, value));
      if (progress < 1) {
        frameId = requestAnimationFrame(tick);
      } else {
        setDisplayValue(value);
      }
    };

    frameId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frameId);
  }, [active, value]);

  return <span style={{ animation: active ? 'count-flicker 1.8s ease' : 'none' }}>{displayValue}</span>;
}

function SystemReadout() {
  const reducedMotion = useReducedMotion();
  const frameText = `AGENTS ONLINE: 53    | TEST COVERAGE: 3,521
GOVERNANCE: ACTIVE   | CRASH SITES: 0
KERNEL: RUST ${VERSION}  | DARWIN CORE: OPERATIONAL`;
  const [count, setCount] = useState(reducedMotion ? frameText.length : 0);

  useEffect(() => {
    if (reducedMotion) {
      return undefined;
    }

    if (count >= frameText.length) {
      return undefined;
    }

    const timer = window.setTimeout(() => setCount((value) => value + 2), 16);
    return () => window.clearTimeout(timer);
  }, [count, frameText.length, reducedMotion]);

  return (
    <div className="data-readout-frame">
      <div className="data-readout">
        <span style={{ color: '#5a7a8a' }}>┌─ SYSTEM STATUS ──────────────────────────────────────┐</span>
        <span style={{ whiteSpace: 'pre-wrap' }}>
          {frameText.slice(0, count)}
          {!reducedMotion && count < frameText.length ? <span style={{ animation: 'typewriter-blink 0.8s step-end infinite' }}>_</span> : null}
        </span>
        <span style={{ color: '#5a7a8a' }}>└──────────────────────────────────────────────────────┘</span>
      </div>
    </div>
  );
}

function FeatureScene({ model }) {
  const setup = useMemo(() => createModelScene(model, { cameraZ: 4.4, scale: 0.85, stars: false }), [model]);

  return (
    <ThreeScene
      setup={setup}
      height={138}
      className="glow-border"
      fallback={<div className="mobile-3d-icon"><span>{model}</span></div>}
      ariaLabel={`${model} tactical model`}
    />
  );
}

export default function Home() {
  const reducedMotion = useReducedMotion();
  const heroScene = useMemo(() => createNeuralCoreScene(), []);
  const featuresRef = useScrollRevealChildren({ stagger: 110, threshold: 0.12 });
  const pipelineRef = useScrollRevealChildren({ stagger: 90, threshold: 0.12 });
  const metricsRevealRef = useScrollRevealChildren({ stagger: 80, threshold: 0.12 });
  const [metricsSectionRef, metricsVisible] = useVisibilityState(0.18);
  const [activeStep, setActiveStep] = useState(0);

  useEffect(() => {
    if (reducedMotion) {
      return undefined;
    }

    const timer = window.setInterval(() => {
      setActiveStep((value) => (value + 1) % GOVERNANCE_PIPELINE.length);
    }, 1150);

    return () => window.clearInterval(timer);
  }, [reducedMotion]);

  return (
    <div className="page-shell">
      <section style={{ position: 'relative', minHeight: 'calc(100vh - 88px)', overflow: 'hidden' }}>
        <div style={{ position: 'absolute', inset: 0 }}>
          <ThreeScene
            setup={heroScene}
            height="100%"
            className="hero-scene"
            disableOnMobile={false}
            fallback={<div className="mobile-3d-icon"><span>NEXUS CORE</span></div>}
            ariaLabel="Nexus neural core visualization"
          />
        </div>
        <div style={{ position: 'absolute', inset: 0, background: 'linear-gradient(90deg, rgba(7, 13, 22, 0.92) 0%, rgba(7, 13, 22, 0.74) 48%, rgba(7, 13, 22, 0.5) 100%)' }} />

        <div className="section home-hero-grid" style={{
          position: 'relative',
          zIndex: 1,
          minHeight: 'calc(100vh - 88px)',
          display: 'grid',
          gridTemplateColumns: 'minmax(0, 760px) minmax(260px, 340px)',
          alignItems: 'center',
          gap: 32,
        }}>
          <div style={{ display: 'grid', gap: 24 }}>
            <div className="pill-row">
              <span className="status-pill">MILITARY-GRADE COMMAND INTERFACE</span>
              <span className="status-pill success">DARWIN CORE ONLINE</span>
            </div>
            <div>
              <div className="section-kicker" style={{ marginBottom: 16 }}>Entry Vector // Home Command</div>
              <h1 className="section-title" style={{ fontSize: 'clamp(4.2rem, 11vw, 7.2rem)' }}>NEXUS OS</h1>
            </div>
            <p className="section-copy" style={{ maxWidth: 740 }}>
              GOVERNED AI AGENT OPERATING SYSTEM. BUILT LIKE A CLASSIFIED CONSOLE, NOT A STARTUP LANDING PAGE.
            </p>
            <div className="section-actions">
              <Link to="/features" className="nexus-button nexus-button-primary">Inspect Systems</Link>
              <Link to="/docs" className="nexus-button">Open Database</Link>
            </div>
            <SystemReadout />
          </div>

          <div className="holo-panel" style={{ display: 'grid', gap: 16 }}>
            <div className="panel-kicker">Live Telemetry</div>
            <div className="metric-grid compact">
              <div className="metric-card compact">
                <span className="stat-label">Kernel</span>
                <span className="stat-value">{VERSION}</span>
                <span className="stat-meta">RUST GOVERNANCE STACK</span>
              </div>
              <div className="metric-card compact">
                <span className="stat-label">Audit</span>
                <span className="stat-value">SHA-256</span>
                <span className="stat-meta">CHAINED INTEGRITY LOG</span>
              </div>
              <div className="metric-card compact">
                <span className="stat-label">Sandbox</span>
                <span className="stat-value">WASM</span>
                <span className="stat-meta">ISOLATED EXECUTION</span>
              </div>
              <div className="metric-card compact">
                <span className="stat-label">Privacy</span>
                <span className="stat-value">LOCAL</span>
                <span className="stat-meta">AIR-GAPPABLE MODE</span>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="section" ref={featuresRef}>
        <div className="section-header fade-up">
          <div className="section-kicker">Subsystems // Feature Grid</div>
          <h2 className="section-title" style={{ fontSize: 'clamp(2.2rem, 5vw, 4rem)' }}>Holographic Modules</h2>
          <p className="section-copy">
            Every panel acts like a mission console: angular geometry, cyan edge glow, scan-line overlays, and live rotational models.
          </p>
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: 18 }}>
          {HOME_FEATURES.map((feature) => (
            <div key={feature.id} className="holo-panel fade-up" style={{ display: 'grid', gap: 18 }}>
              <FeatureScene model={feature.model} />
              <div className="panel-kicker">{feature.stat}</div>
              <div className="panel-title" style={{ fontSize: '1.12rem' }}>{feature.title}</div>
              <p className="panel-copy" style={{ fontSize: '0.98rem' }}>{feature.description}</p>
            </div>
          ))}
        </div>
      </section>

      <section className="section" ref={pipelineRef} style={{ paddingTop: 24 }}>
        <div className="section-header fade-up">
          <div className="section-kicker">Governance Flow // Horizontal Pipeline</div>
          <h2 className="section-title" style={{ fontSize: 'clamp(2rem, 4vw, 3.2rem)' }}>Execution Approval Corridor</h2>
          <p className="section-copy">
            Requests move left to right through a cyan command rail. Completed nodes lock green, the active node burns orange, and waiting steps idle in tactical blue.
          </p>
        </div>
        <div className="holo-panel fade-up" style={{ padding: 24 }}>
          <div style={{ position: 'relative', overflowX: 'auto', paddingBottom: 8 }}>
            <div style={{
              position: 'absolute',
              top: 57,
              left: 48,
              right: 48,
              height: 2,
              background: 'linear-gradient(90deg, rgba(0, 212, 255, 0.8), rgba(0, 212, 255, 0.2))',
              boxShadow: '0 0 16px rgba(0, 212, 255, 0.18)',
            }} />
            <div style={{ display: 'grid', gridTemplateColumns: `repeat(${GOVERNANCE_PIPELINE.length}, minmax(180px, 1fr))`, gap: 18, minWidth: 1080 }}>
              {GOVERNANCE_PIPELINE.map((step, index) => {
                const state = index < activeStep ? 'complete' : index === activeStep ? 'active' : 'idle';
                const tint = state === 'complete' ? '#00ff88' : state === 'active' ? '#ff6a00' : '#00d4ff';

                return (
                  <div key={step.step} className="fade-up" style={{ position: 'relative', display: 'grid', gap: 14 }}>
                    <div style={{
                      position: 'relative',
                      width: 104,
                      height: 104,
                      margin: '0 auto',
                      display: 'grid',
                      placeItems: 'center',
                      clipPath: 'polygon(25% 6%, 75% 6%, 98% 50%, 75% 94%, 25% 94%, 2% 50%)',
                      border: `1px solid ${tint}`,
                      background: `radial-gradient(circle at center, ${tint}26, rgba(13, 21, 32, 0.92))`,
                      boxShadow: state === 'active' ? `0 0 20px ${tint}55` : `0 0 12px ${tint}20`,
                      color: tint,
                      fontFamily: "'Orbitron', sans-serif",
                      fontSize: '0.78rem',
                      letterSpacing: '0.14em',
                    }}>
                      {state === 'complete' ? '✓' : step.step}
                    </div>
                    <div style={{ display: 'grid', gap: 6, textAlign: 'center' }}>
                      <div className="panel-title" style={{ fontSize: '0.9rem' }}>{step.name}</div>
                      <p className="panel-copy" style={{ fontSize: '0.84rem' }}>{step.description}</p>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </section>

      <section className="section" ref={metricsSectionRef}>
        <div ref={metricsRevealRef}>
          <div className="section-header fade-up">
            <div className="section-kicker">Data Readouts // Mission Metrics</div>
            <h2 className="section-title" style={{ fontSize: 'clamp(2rem, 4vw, 3.1rem)' }}>Operational Statistics</h2>
            <p className="section-copy">
              Count-up telemetry, flicker-scanned values, and source-language composition presented like a live command brief.
            </p>
          </div>

          <div className="home-metrics-layout" style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1.2fr) minmax(320px, 0.8fr)', gap: 18 }}>
            <div className="holo-panel fade-up">
              <div className="metric-grid">
                {HOME_METRICS.map((metric) => (
                  <div key={metric.label} className="metric-card">
                    <span className="stat-label">{metric.label}</span>
                    <span className="stat-value"><AnimatedMetric value={metric.value} active={metricsVisible} /></span>
                    <span className="stat-meta">{metric.meta}</span>
                  </div>
                ))}
              </div>
            </div>

            <div className="holo-panel fade-up" style={{ display: 'grid', gap: 18 }}>
              <div className="panel-kicker">Language Signal</div>
              <div className="panel-title" style={{ fontSize: '1.05rem' }}>Repository Composition</div>
              <div style={{ display: 'grid', gap: 14 }}>
                {Object.entries(GITLAB_LANGUAGES).map(([language, share]) => (
                  <div key={language} style={{ display: 'grid', gap: 8 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12 }}>
                      <span className="stat-label" style={{ color: '#9ec8da' }}>{language}</span>
                      <span className="stat-meta">{share.toFixed(2)}%</span>
                    </div>
                    <div style={{ height: 10, border: '1px solid rgba(0, 212, 255, 0.12)', background: 'rgba(7, 13, 22, 0.72)' }}>
                      <div style={{
                        width: `${share}%`,
                        height: '100%',
                        background: language === 'Rust'
                          ? 'linear-gradient(90deg, #00d4ff, #00ff88)'
                          : language === 'CSS'
                            ? 'linear-gradient(90deg, #1a3a5c, #00d4ff)'
                            : 'linear-gradient(90deg, rgba(0, 212, 255, 0.55), rgba(255, 106, 0, 0.6))',
                        boxShadow: '0 0 14px rgba(0, 212, 255, 0.14)',
                      }} />
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

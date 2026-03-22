import { useEffect, useMemo, useState } from 'react';
import CodeBlock from '../components/CodeBlock';
import ThreeScene from '../components/ThreeScene';
import { createDocsArchiveScene } from '../components/sceneFactories';
import { useScrollRevealChildren } from '../hooks/useScrollReveal';

const DOC_SECTIONS = [
  {
    id: 'getting-started',
    label: 'Getting Started',
    number: '4.1',
    title: 'Boot The Command Surface',
    body: [
      'Clone the repository, install website dependencies, and launch the command interface locally.',
      'The website is intentionally framed like a technical database: heavy typography, scan-line overlays, and terminal-wrapped code examples.',
    ],
    code: `cd nexus-website
npm install
npm run dev`,
    language: 'bash',
  },
  {
    id: 'architecture',
    label: 'Architecture',
    number: '4.2',
    title: 'System Blueprint',
    body: [
      'The platform is arranged as a governed stack: presentation, orchestration, agents, evolution, kernel, sandbox, infrastructure, and provider routing.',
      'Each layer stays visible in the architecture page through the rotating layer cake and HUD side panels.',
    ],
    code: `// Every action descends through a governed stack
let permit = kernel.check_capability(&agent_id, &cap)?;
kernel.reserve_fuel(&agent_id, cost)?;
let result = kernel.execute_sandboxed(action).await?;
kernel.audit_trail.append_event(event)?;`,
    language: 'rust',
  },
  {
    id: 'sdk',
    label: 'SDK',
    number: '4.3',
    title: 'Governed Agent Interface',
    body: [
      'Agents declare manifest capabilities up front. Runtime access is granted by policy, not by optimistic convention.',
      'This keeps the website narrative aligned with the product itself: autonomy only exists inside declared boundaries.',
    ],
    code: `pub struct AgentManifest {
    pub id: String,
    pub capabilities: Vec<Capability>,
    pub autonomy_level: AutonomyLevel,
}`,
    language: 'rust',
  },
  {
    id: 'security',
    label: 'Security',
    number: '4.4',
    title: 'Defense In Depth',
    body: [
      'Output firewalls, PII redaction, human approval gates, capability ACLs, and Wasmtime isolation form the core defense lattice.',
      'The docs surface mirrors that posture with framed terminal panels, numbered sections, and immutable command styling.',
    ],
    code: `let output = firewall.scan(result)?;
let redacted = pii_gateway.scrub(output)?;
audit_trail.append(entry(redacted))?;`,
    language: 'rust',
  },
  {
    id: 'deployment',
    label: 'Deployment',
    number: '4.5',
    title: 'Field Deployment',
    body: [
      'The stack supports air-gapped and local-first usage, so the website leans into a deployment-console aesthetic instead of a marketing-site one.',
      'Use Vite for local preview, then package production assets after a full build.',
    ],
    code: `npm run build
npm run preview -- --host 127.0.0.1 --port 4173`,
    language: 'bash',
  },
  {
    id: 'api-reference',
    label: 'API Reference',
    number: '4.6',
    title: 'Operational Primitives',
    body: [
      'Core operations revolve around capability checks, fuel reservation, sandbox execution, and audit commits.',
      'Code examples below stay concise so the docs page reads like an operator database, not a prose-heavy blog.',
    ],
    code: `pub async fn governed_inference(
    &self,
    agent_id: &AgentId,
    request: InferenceRequest,
) -> Result<InferenceOutput>`,
    language: 'rust',
  },
  {
    id: 'compliance',
    label: 'Compliance',
    number: '4.7',
    title: 'Control Evidence',
    body: [
      'EU AI Act readiness, SOC 2 control mapping, and append-only audit logs sit directly inside the product story.',
      'That is why the docs page is styled like a technical evidence room rather than a lightweight marketing wiki.',
    ],
    code: `Article 14 // HUMAN OVERSIGHT ........ IMPLEMENTED
CC6       // LOGICAL ACCESS ......... IMPLEMENTED
CC8       // CHANGE MANAGEMENT ...... IMPLEMENTED`,
    language: 'bash',
  },
];

export default function Docs() {
  const revealRef = useScrollRevealChildren({ stagger: 90, threshold: 0.12 });
  const sceneSetup = useMemo(() => createDocsArchiveScene(), []);
  const [activeSection, setActiveSection] = useState(DOC_SECTIONS[0].id);

  useEffect(() => {
    const sectionNodes = DOC_SECTIONS
      .map((section) => document.getElementById(section.id))
      .filter(Boolean);

    if (sectionNodes.length === 0) {
      return undefined;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        const visibleEntry = entries
          .filter((entry) => entry.isIntersecting)
          .sort((left, right) => right.intersectionRatio - left.intersectionRatio)[0];

        if (visibleEntry) {
          setActiveSection(visibleEntry.target.id);
        }
      },
      { threshold: [0.15, 0.35, 0.6], rootMargin: '-20% 0px -45% 0px' },
    );

    sectionNodes.forEach((node) => observer.observe(node));
    return () => observer.disconnect();
  }, []);

  const scrollToSection = (id) => {
    setActiveSection(id);
    document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  };

  return (
    <div className="page-shell">
      <section className="section" ref={revealRef}>
        <div className="fade-up docs-hero-layout" style={{
          display: 'grid',
          gridTemplateColumns: 'minmax(0, 1fr) 360px',
          gap: 18,
          alignItems: 'start',
          marginBottom: 24,
        }}>
          <div className="section-header" style={{ marginBottom: 0 }}>
            <div className="section-kicker">Technical Database // Reference Room</div>
            <h1 className="section-title" style={{ fontSize: 'clamp(2.4rem, 6vw, 4.8rem)' }}>Documentation Archive</h1>
            <p className="section-copy">
              HALO-STYLE MENU STACK ON THE LEFT. NUMBERED TECHNICAL SECTIONS IN THE CENTER. LIVE ROTATING ARCHIVE CORE ON THE RIGHT.
            </p>
          </div>

          <div className="holo-panel" style={{ display: 'grid', gap: 18 }}>
            <div className="panel-kicker">Archive Core</div>
            <ThreeScene
              setup={sceneSetup}
              height={320}
              fallback={<div className="mobile-3d-icon"><span>DOCS</span></div>}
              ariaLabel="Documentation archive visualization"
            />
            <div className="data-readout-frame">
              <div className="data-readout">
                <span>ACTIVE SECTION: {activeSection.toUpperCase()}</span>
                <span>REFERENCE FRAMES: {DOC_SECTIONS.length}</span>
                <span>TERMINAL PANELS: ENABLED</span>
              </div>
            </div>
          </div>
        </div>

        <div className="docs-layout fade-up">
          <aside className="holo-panel docs-sidebar">
            <div className="panel-kicker" style={{ marginBottom: 14 }}>Menu Stack</div>
            <div className="sidebar-list">
              {DOC_SECTIONS.map((section) => (
                <button
                  key={section.id}
                  className={`sidebar-button ${activeSection === section.id ? 'is-active' : ''}`}
                  onClick={() => scrollToSection(section.id)}
                >
                  <span className="sidebar-index">{section.number}</span>
                  <div>
                    <div className="panel-title" style={{ fontSize: '0.86rem', marginBottom: 6 }}>{section.label}</div>
                    <div className="panel-copy" style={{ fontSize: '0.74rem' }}>{section.title}</div>
                  </div>
                </button>
              ))}
            </div>
          </aside>

          <div className="docs-content">
            {DOC_SECTIONS.map((section) => (
              <section key={section.id} id={section.id} className="doc-section holo-panel">
                <div className="doc-rule-title">
                  <span>SECTION {section.number}</span>
                </div>
                <div className="panel-title" style={{ fontSize: '1.4rem', marginBottom: 16 }}>{section.title}</div>
                <div style={{ display: 'grid', gap: 14, marginBottom: 18 }}>
                  {section.body.map((paragraph) => (
                    <p key={paragraph} className="panel-copy" style={{ fontSize: '0.98rem' }}>{paragraph}</p>
                  ))}
                </div>
                <CodeBlock
                  code={section.code}
                  language={section.language}
                  label={`${section.label.toUpperCase()} // ${section.number}`}
                />
              </section>
            ))}
          </div>
        </div>
      </section>
    </div>
  );
}

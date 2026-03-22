import React, { useEffect, useRef } from 'react';
import { GITLAB_PROJECT, COMMIT_COUNT, VERSION } from '../data/constants';
import CodeBlock from '../components/CodeBlock';

function useFadeUp() {
  const ref = useRef(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const obs = new IntersectionObserver(([e]) => {
      if (e.isIntersecting) { el.classList.add('visible'); obs.unobserve(el); }
    }, { threshold: 0.15 });
    obs.observe(el);
    return () => obs.disconnect();
  }, []);
  return ref;
}

function FadeSection({ children, style }) {
  const ref = useFadeUp();
  return <div ref={ref} className="fade-up" style={style}>{children}</div>;
}

const contributionSteps = [
  {
    title: 'Prerequisites',
    items: [
      'Rust 1.94+ (rustup recommended)',
      'Node.js 22+ and npm',
      'Python 3.11+ (for voice and E2E tests)',
      'Platform: Linux, macOS, or Windows',
    ],
  },
  {
    title: 'Build from Source',
    code: `# Clone the repository
git clone https://gitlab.com/nexaiceo/nexus-os.git
cd nexus-os

# Build the Rust workspace (33 crates)
cargo build --workspace

# Build the desktop app
cd app && npm install && npm run build

# Run all tests
cargo test --workspace --all-features`,
  },
  {
    title: 'Code Style',
    items: [
      'cargo fmt --all -- --check (formatting)',
      'cargo clippy --workspace -D warnings (linting)',
      'Edition 2021, no unsafe code',
      'Public types: derive Debug, Clone, Serialize, Deserialize',
      'Errors: thiserror or custom enums',
      'UUID v4 for all identifiers',
    ],
  },
  {
    title: 'PR Process',
    items: [
      'Branch from main with descriptive name',
      'Keep changes focused and minimal',
      'Include tests for new functionality',
      'CI must pass: fmt, clippy, test, build',
      'One approval required for merge',
    ],
  },
];

const contributionAreas = [
  { title: 'Bug Fixes', description: 'Find and fix issues. Check the issue tracker for good first issues.', color: 'var(--accent-orange)' },
  { title: 'New Agents', description: 'Create governed agents with TOML manifests and capability declarations.', color: 'var(--accent-cyan)' },
  { title: 'Connectors', description: 'Build integration adapters for enterprise platforms.', color: 'var(--accent-purple)' },
  { title: 'Documentation', description: 'Improve docs, add examples, write tutorials.', color: 'var(--accent-green)' },
  { title: 'Tests', description: 'Expand test coverage. Currently 3,521 tests across the workspace.', color: 'var(--accent-cyan)' },
  { title: 'Performance', description: 'Profile and optimize hot paths. Benchmark with Criterion.', color: 'var(--accent-orange)' },
];

export default function Community() {
  const stats = [
    { label: 'Stars', value: GITLAB_PROJECT.star_count.toString() },
    { label: 'Forks', value: GITLAB_PROJECT.forks_count.toString() },
    { label: 'Commits', value: COMMIT_COUNT.toString() },
    { label: 'Last Updated', value: new Date(GITLAB_PROJECT.last_activity_at).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' }) },
  ];

  return (
    <div>
      {/* Header */}
      <section style={{ padding: '120px 24px 80px', textAlign: 'center' }}>
        <FadeSection>
          <h1 className="section-title" style={{ fontSize: '3rem' }}>Community</h1>
          <p className="section-subtitle" style={{ margin: '0 auto', maxWidth: 600 }}>
            Nexus OS is open source under the MIT license. Every contribution strengthens the governed AI ecosystem.
          </p>
        </FadeSection>
      </section>

      {/* Stats */}
      <section style={{ padding: '0 24px 80px' }}>
        <div style={{ maxWidth: 800, margin: '0 auto', display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 16 }}>
          {stats.map((s, i) => (
            <FadeSection key={i}>
              <div className="glass-card" style={{ padding: 24, textAlign: 'center' }}>
                <div style={{ fontFamily: 'var(--font-stat)', fontSize: '1.5rem', color: 'var(--accent-cyan)', marginBottom: 8, letterSpacing: '0.05em' }}>
                  {s.value}
                </div>
                <div style={{ color: 'var(--text-secondary)', fontSize: '0.8rem', textTransform: 'uppercase', letterSpacing: '0.1em' }}>
                  {s.label}
                </div>
              </div>
            </FadeSection>
          ))}
        </div>
      </section>

      {/* Star CTA */}
      <section style={{ padding: '0 24px 80px', textAlign: 'center' }}>
        <FadeSection>
          <a
            href={GITLAB_PROJECT.web_url}
            target="_blank"
            rel="noopener noreferrer"
            className="btn-primary"
            style={{ fontSize: '1rem', padding: '16px 40px' }}
          >
            Star on GitLab
          </a>
        </FadeSection>
      </section>

      {/* Contribution Areas */}
      <section className="section" style={{ background: 'var(--bg-secondary)' }}>
        <FadeSection>
          <h2 className="section-title" style={{ textAlign: 'center', marginBottom: 48 }}>Ways to Contribute</h2>
        </FadeSection>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: 20 }}>
          {contributionAreas.map((area, i) => (
            <FadeSection key={i}>
              <div className="glass-card" style={{ padding: 28, borderLeft: `3px solid ${area.color}` }}>
                <h3 style={{ fontFamily: 'var(--font-hero)', fontWeight: 600, fontSize: '1.1rem', marginBottom: 8, color: area.color }}>
                  {area.title}
                </h3>
                <p style={{ color: 'var(--text-secondary)', fontSize: '0.875rem', lineHeight: 1.6 }}>
                  {area.description}
                </p>
              </div>
            </FadeSection>
          ))}
        </div>
      </section>

      {/* How to Contribute */}
      <section className="section">
        <FadeSection>
          <h2 className="section-title" style={{ textAlign: 'center', marginBottom: 48 }}>Getting Started</h2>
        </FadeSection>
        <div style={{ maxWidth: 800, margin: '0 auto', display: 'flex', flexDirection: 'column', gap: 32 }}>
          {contributionSteps.map((step, i) => (
            <FadeSection key={i}>
              <div className="glass-card" style={{ padding: 32 }}>
                <h3 style={{ fontFamily: 'var(--font-hero)', fontWeight: 600, fontSize: '1.15rem', marginBottom: 16, color: 'var(--accent-cyan)' }}>
                  {step.title}
                </h3>
                {step.items && (
                  <ul style={{ listStyle: 'none', padding: 0, display: 'flex', flexDirection: 'column', gap: 8 }}>
                    {step.items.map((item, j) => (
                      <li key={j} style={{ display: 'flex', alignItems: 'flex-start', gap: 10, color: 'var(--text-secondary)', fontSize: '0.9rem' }}>
                        <span style={{ color: 'var(--accent-cyan)', marginTop: 2, flexShrink: 0 }}>
                          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                            <path d="M5 12h14M12 5l7 7-7 7" strokeLinecap="round" strokeLinejoin="round" />
                          </svg>
                        </span>
                        <span style={{ fontFamily: 'var(--font-mono)', fontSize: '0.85rem' }}>{item}</span>
                      </li>
                    ))}
                  </ul>
                )}
                {step.code && <CodeBlock code={step.code} />}
              </div>
            </FadeSection>
          ))}
        </div>
      </section>

      {/* Links */}
      <section style={{ padding: '80px 24px 120px', textAlign: 'center' }}>
        <FadeSection>
          <div style={{ display: 'flex', gap: 16, justifyContent: 'center', flexWrap: 'wrap', marginBottom: 48 }}>
            <a href={`${GITLAB_PROJECT.web_url}/-/issues`} target="_blank" rel="noopener noreferrer" className="btn-outline">
              Feature Requests
            </a>
            <a href={`${GITLAB_PROJECT.web_url}/-/merge_requests`} target="_blank" rel="noopener noreferrer" className="btn-outline">
              Merge Requests
            </a>
            <a href={`${GITLAB_PROJECT.web_url}/-/releases`} target="_blank" rel="noopener noreferrer" className="btn-outline">
              Releases
            </a>
          </div>
          <p style={{ color: 'var(--text-dim)', fontSize: '0.85rem' }}>
            Built by Suresh Karicheti
          </p>
          <p style={{ color: 'var(--text-dim)', fontSize: '0.75rem', marginTop: 8 }}>
            {VERSION} | MIT License | {COMMIT_COUNT} commits
          </p>
        </FadeSection>
      </section>
    </div>
  );
}

import React, { useEffect, useState } from 'react';
import { EU_AI_ACT_ARTICLES, SOC2_CONTROLS, SECURITY_LAYERS } from '../data/constants';
import AnimatedIcon from '../components/AnimatedIcon';

const glassCard = {
  background: 'rgba(255,255,255,0.03)',
  border: '1px solid rgba(255,255,255,0.08)',
  borderRadius: 16,
  padding: 32,
  backdropFilter: 'blur(12px)',
};

function FadeUp({ children, delay = 0, style = {} }) {
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    const t = setTimeout(() => setVisible(true), delay);
    return () => clearTimeout(t);
  }, [delay]);
  return (
    <div style={{
      opacity: visible ? 1 : 0,
      transform: visible ? 'translateY(0)' : 'translateY(24px)',
      transition: 'opacity 0.6s ease, transform 0.6s ease',
      ...style,
    }}>{children}</div>
  );
}

const enterpriseFeatures = [
  {
    icon: 'key',
    title: 'Authentication (OIDC/SSO)',
    description: 'Enterprise single sign-on with OpenID Connect. Integrate with Okta, Azure AD, Auth0, or any OIDC-compliant provider. Six RBAC roles from viewer to super-admin.',
  },
  {
    icon: 'layers',
    title: 'Multi-Tenancy',
    description: 'Complete data isolation between tenants. Per-tenant governance policies, fuel budgets, and audit trails. AES-256-GCM encryption at rest.',
  },
  {
    icon: 'eye',
    title: 'Telemetry (OpenTelemetry)',
    description: 'Full observability with OpenTelemetry integration. Distributed tracing across agents, metrics dashboards, and structured logging.',
  },
  {
    icon: 'plug',
    title: 'Integrations',
    description: '9 pre-built connectors: Slack, Teams, Discord, GitHub, GitLab, Jira, Telegram, email, webhooks. All governed with capability checks and audit logging.',
  },
  {
    icon: 'cpu',
    title: 'Metering & Usage',
    description: 'Per-agent and per-tenant resource metering. Track fuel consumption, inference tokens, API calls, and storage usage. Configurable quotas and alerts.',
  },
];

const deploymentModes = [
  {
    title: 'Desktop',
    description: 'Native Tauri 2.0 app for macOS, Windows, and Linux. Full governance runs locally. Zero cloud dependency.',
    icon: 'code',
    features: ['Native performance', 'Offline capable', 'Auto-updates'],
  },
  {
    title: 'Server',
    description: 'Docker and Helm deployments for team environments. REST API, multi-user access, centralized governance.',
    icon: 'database',
    features: ['Docker + Helm', 'Multi-user', 'REST API'],
  },
  {
    title: 'Hybrid',
    description: 'Desktop agents connect to a server-side kernel. Local inference with centralized audit trails and governance.',
    icon: 'network',
    features: ['Distributed agents', 'Central audit', 'Edge inference'],
  },
  {
    title: 'Air-Gapped',
    description: 'Complete offline operation. Flash inference with local GGUF models. No network required. Zero data exfiltration.',
    icon: 'lock',
    features: ['Zero network', 'Local LLM', 'SCIF-ready'],
  },
];

export default function Enterprise() {
  return (
    <div style={{
      minHeight: '100vh',
      background: 'var(--bg-primary, #0a0a0f)',
      color: 'var(--text-primary, #e2e8f0)',
    }}>

      {/* Hero */}
      <FadeUp>
        <section style={{
          textAlign: 'center',
          padding: '100px 24px 60px',
          maxWidth: 900,
          margin: '0 auto',
        }}>
          <div style={{ marginBottom: 24 }}>
            <AnimatedIcon name="building" size={64} color="var(--accent-cyan, #00f5ff)" />
          </div>
          <h1 style={{
            fontSize: 'clamp(2rem, 5vw, 3.25rem)',
            fontWeight: 800,
            marginBottom: 20,
            background: 'linear-gradient(135deg, var(--accent-cyan, #00f5ff), var(--accent-purple, #7c3aed))',
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent',
            lineHeight: 1.15,
          }}>Enterprise-Grade AI Governance</h1>
          <p style={{
            fontSize: '1.15rem',
            color: 'var(--text-secondary, #94a3b8)',
            maxWidth: 680,
            margin: '0 auto',
            lineHeight: 1.7,
          }}>
            Deploy governed AI agents across your organization with OIDC authentication,
            multi-tenant isolation, compliance frameworks, and full observability.
            Air-gappable. SOC 2 aligned. EU AI Act conformant.
          </p>
        </section>
      </FadeUp>

      {/* Feature Blocks */}
      <FadeUp delay={100}>
        <section style={{ maxWidth: 1100, margin: '0 auto', padding: '0 24px 80px' }}>
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
            gap: 24,
          }}>
            {enterpriseFeatures.map((f, i) => (
              <div key={i} style={{
                ...glassCard,
                transition: 'border-color 0.3s ease, transform 0.3s ease',
              }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.borderColor = 'rgba(0,245,255,0.25)';
                  e.currentTarget.style.transform = 'translateY(-4px)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.borderColor = 'rgba(255,255,255,0.08)';
                  e.currentTarget.style.transform = 'translateY(0)';
                }}
              >
                <div style={{ marginBottom: 16 }}>
                  <AnimatedIcon name={f.icon} size={36} color="var(--accent-cyan, #00f5ff)" />
                </div>
                <h3 style={{ fontSize: '1.15rem', fontWeight: 700, marginBottom: 10, color: 'var(--text-primary)' }}>{f.title}</h3>
                <p style={{ color: 'var(--text-secondary, #94a3b8)', lineHeight: 1.65, fontSize: '0.9rem' }}>{f.description}</p>
              </div>
            ))}
          </div>
        </section>
      </FadeUp>

      {/* Security Layers Visual */}
      <FadeUp delay={200}>
        <section style={{ maxWidth: 800, margin: '0 auto', padding: '0 24px 80px' }}>
          <h2 style={{
            fontSize: '1.75rem',
            fontWeight: 700,
            textAlign: 'center',
            marginBottom: 40,
            color: 'var(--text-primary)',
          }}>8 Layers of Defense</h2>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 0 }}>
            {SECURITY_LAYERS.map((layer, i) => {
              const opacity = 0.4 + (i * 0.08);
              return (
                <div key={i} style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 20,
                  padding: '16px 24px',
                  background: `rgba(0,245,255,${0.02 + i * 0.01})`,
                  borderLeft: '3px solid',
                  borderColor: `rgba(0,245,255,${opacity})`,
                  borderBottom: i < SECURITY_LAYERS.length - 1 ? '1px solid rgba(255,255,255,0.04)' : 'none',
                  borderRadius: i === 0 ? '12px 12px 0 0' : i === SECURITY_LAYERS.length - 1 ? '0 0 12px 12px' : 0,
                }}>
                  <div style={{
                    width: 40,
                    height: 40,
                    borderRadius: 10,
                    background: `rgba(0,245,255,${0.05 + i * 0.02})`,
                    border: `1px solid rgba(0,245,255,${0.15 + i * 0.05})`,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    fontSize: '0.85rem',
                    fontWeight: 800,
                    color: 'var(--accent-cyan, #00f5ff)',
                    flexShrink: 0,
                  }}>L{layer.layer}</div>
                  <div style={{ flex: 1 }}>
                    <div style={{ fontWeight: 600, marginBottom: 4, color: 'var(--text-primary)' }}>{layer.name}</div>
                    <div style={{ fontSize: '0.85rem', color: 'var(--text-secondary, #94a3b8)', lineHeight: 1.5 }}>{layer.description}</div>
                  </div>
                </div>
              );
            })}
          </div>
        </section>
      </FadeUp>

      {/* Compliance: EU AI Act */}
      <FadeUp delay={300}>
        <section style={{ maxWidth: 1100, margin: '0 auto', padding: '0 24px 80px' }}>
          <h2 style={{
            fontSize: '1.75rem',
            fontWeight: 700,
            textAlign: 'center',
            marginBottom: 12,
            color: 'var(--text-primary)',
          }}>EU AI Act Conformity</h2>
          <p style={{
            textAlign: 'center',
            color: 'var(--text-secondary, #94a3b8)',
            marginBottom: 40,
            maxWidth: 600,
            margin: '0 auto 40px',
          }}>Articles 9-15 addressed through governance-by-design architecture.</p>
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
            gap: 16,
          }}>
            {EU_AI_ACT_ARTICLES.map((a, i) => (
              <div key={i} style={{
                ...glassCard,
                padding: 24,
              }}>
                <div style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  marginBottom: 10,
                }}>
                  <span style={{
                    fontSize: '0.75rem',
                    fontWeight: 700,
                    color: 'var(--accent-cyan, #00f5ff)',
                    textTransform: 'uppercase',
                    letterSpacing: '0.05em',
                  }}>{a.article}</span>
                  <span style={{
                    fontSize: '0.7rem',
                    fontWeight: 600,
                    color: a.status === 'Implemented' ? '#00ff88' : '#ffaa00',
                    background: a.status === 'Implemented' ? 'rgba(0,255,136,0.1)' : 'rgba(255,170,0,0.1)',
                    padding: '3px 10px',
                    borderRadius: 12,
                  }}>{a.status}</span>
                </div>
                <h4 style={{ fontWeight: 600, marginBottom: 8, fontSize: '0.95rem', color: 'var(--text-primary)' }}>{a.title}</h4>
                <p style={{ fontSize: '0.8rem', color: 'var(--text-secondary, #94a3b8)', lineHeight: 1.55 }}>{a.description}</p>
              </div>
            ))}
          </div>
        </section>
      </FadeUp>

      {/* SOC 2 Controls */}
      <FadeUp delay={350}>
        <section style={{ maxWidth: 1100, margin: '0 auto', padding: '0 24px 80px' }}>
          <h2 style={{
            fontSize: '1.75rem',
            fontWeight: 700,
            textAlign: 'center',
            marginBottom: 40,
            color: 'var(--text-primary)',
          }}>SOC 2 Trust Service Criteria</h2>
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))',
            gap: 12,
          }}>
            {SOC2_CONTROLS.map((c, i) => (
              <div key={i} style={{
                ...glassCard,
                padding: 20,
                display: 'flex',
                alignItems: 'center',
                gap: 14,
              }}>
                <div style={{
                  width: 10,
                  height: 10,
                  borderRadius: '50%',
                  background: c.status === 'Implemented' ? '#00ff88' : '#ffaa00',
                  flexShrink: 0,
                }} />
                <div>
                  <div style={{ fontWeight: 700, fontSize: '0.8rem', color: 'var(--accent-cyan, #00f5ff)' }}>{c.id}</div>
                  <div style={{ fontSize: '0.85rem', color: 'var(--text-primary)' }}>{c.title}</div>
                </div>
              </div>
            ))}
          </div>
        </section>
      </FadeUp>

      {/* Deployment Matrix */}
      <FadeUp delay={400}>
        <section style={{ maxWidth: 1100, margin: '0 auto', padding: '0 24px 80px' }}>
          <h2 style={{
            fontSize: '1.75rem',
            fontWeight: 700,
            textAlign: 'center',
            marginBottom: 40,
            color: 'var(--text-primary)',
          }}>Deployment Options</h2>
          <div style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
            gap: 20,
          }}>
            {deploymentModes.map((mode, i) => (
              <div key={i} style={{
                ...glassCard,
                textAlign: 'center',
                transition: 'border-color 0.3s ease, transform 0.3s ease',
              }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.borderColor = 'rgba(124,58,237,0.3)';
                  e.currentTarget.style.transform = 'translateY(-4px)';
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.borderColor = 'rgba(255,255,255,0.08)';
                  e.currentTarget.style.transform = 'translateY(0)';
                }}
              >
                <div style={{ marginBottom: 16 }}>
                  <AnimatedIcon name={mode.icon} size={40} color="var(--accent-purple, #7c3aed)" />
                </div>
                <h3 style={{ fontSize: '1.1rem', fontWeight: 700, marginBottom: 10, color: 'var(--text-primary)' }}>{mode.title}</h3>
                <p style={{ fontSize: '0.85rem', color: 'var(--text-secondary, #94a3b8)', lineHeight: 1.6, marginBottom: 16 }}>{mode.description}</p>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                  {mode.features.map((feat, j) => (
                    <span key={j} style={{
                      fontSize: '0.75rem',
                      color: 'var(--accent-cyan, #00f5ff)',
                      background: 'rgba(0,245,255,0.06)',
                      padding: '4px 12px',
                      borderRadius: 12,
                    }}>{feat}</span>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </section>
      </FadeUp>

      {/* Contact */}
      <FadeUp delay={450}>
        <section style={{
          maxWidth: 700,
          margin: '0 auto',
          padding: '0 24px 100px',
          textAlign: 'center',
        }}>
          <div style={{
            ...glassCard,
            padding: 48,
            background: 'linear-gradient(135deg, rgba(0,245,255,0.04), rgba(124,58,237,0.04))',
            border: '1px solid rgba(0,245,255,0.12)',
          }}>
            <AnimatedIcon name="mail" size={48} color="var(--accent-cyan, #00f5ff)" />
            <h2 style={{
              fontSize: '1.5rem',
              fontWeight: 700,
              marginTop: 20,
              marginBottom: 12,
              color: 'var(--text-primary)',
            }}>Ready for Enterprise?</h2>
            <p style={{
              color: 'var(--text-secondary, #94a3b8)',
              marginBottom: 28,
              lineHeight: 1.65,
            }}>
              Contact us for a tailored deployment plan, compliance review, and dedicated support.
            </p>
            <a
              href="mailto:enterprise@nexus-os.dev"
              style={{
                display: 'inline-block',
                background: 'linear-gradient(135deg, var(--accent-cyan, #00f5ff), var(--accent-purple, #7c3aed))',
                color: '#0a0a0f',
                fontWeight: 700,
                padding: '14px 36px',
                borderRadius: 10,
                textDecoration: 'none',
                fontSize: '0.95rem',
                transition: 'transform 0.2s ease, box-shadow 0.2s ease',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.transform = 'translateY(-2px)';
                e.currentTarget.style.boxShadow = '0 8px 30px rgba(0,245,255,0.3)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.transform = 'translateY(0)';
                e.currentTarget.style.boxShadow = 'none';
              }}
            >enterprise@nexus-os.dev</a>
          </div>
        </section>
      </FadeUp>
    </div>
  );
}

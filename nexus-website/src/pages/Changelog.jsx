import React, { useEffect, useRef } from 'react';
import { CHANGELOG_DATA } from '../data/constants';

const sectionColors = {
  Added: {
    color: 'var(--accent-green)',
    bg: 'rgba(0,255,136,0.1)',
    dotColor: 'var(--accent-green)',
  },
  Changed: {
    color: 'var(--accent-cyan)',
    bg: 'rgba(0,245,255,0.1)',
    dotColor: 'var(--accent-cyan)',
  },
  Fixed: {
    color: 'var(--accent-orange)',
    bg: 'rgba(255,107,43,0.1)',
    dotColor: 'var(--accent-orange)',
  },
  Removed: {
    color: 'var(--text-dim)',
    bg: 'rgba(74,85,104,0.15)',
    dotColor: 'var(--text-dim)',
  },
};

function AnimatedDot({ color }) {
  return (
    <span style={{
      display: 'inline-block',
      width: 6,
      height: 6,
      borderRadius: '50%',
      background: color,
      marginRight: 10,
      marginTop: 7,
      flexShrink: 0,
      animation: 'dot-breathe 2s ease-in-out infinite',
    }} />
  );
}

function VersionBlock({ entry, index }) {
  const ref = useRef(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new IntersectionObserver(
      ([e]) => {
        if (e.isIntersecting) el.classList.add('visible');
      },
      { threshold: 0.1 }
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const sections = entry.sections || {};

  return (
    <div
      ref={ref}
      className="fade-up glass-card"
      style={{
        padding: '32px 32px 28px',
        marginBottom: 28,
        transitionDelay: `${index * 0.08}s`,
      }}
    >
      {/* Version header */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: 14,
        marginBottom: 8,
        flexWrap: 'wrap',
      }}>
        <span style={{
          fontFamily: 'var(--font-stat)',
          fontWeight: 700,
          fontSize: '2rem',
          background: 'linear-gradient(135deg, var(--accent-cyan), var(--accent-purple))',
          WebkitBackgroundClip: 'text',
          WebkitTextFillColor: 'transparent',
          backgroundClip: 'text',
          lineHeight: 1.2,
        }}>
          {entry.version}
        </span>

        {entry.latest && (
          <span style={{
            fontFamily: 'var(--font-stat)',
            fontSize: '0.55rem',
            letterSpacing: '0.18em',
            color: 'var(--accent-cyan)',
            background: 'rgba(0,245,255,0.1)',
            border: '1px solid var(--accent-cyan)',
            borderRadius: 20,
            padding: '3px 10px',
            boxShadow: '0 0 12px rgba(0,245,255,0.25), 0 0 24px rgba(0,245,255,0.1)',
            animation: 'pulse-glow 1.5s ease-in-out infinite',
          }}>
            LATEST
          </span>
        )}
      </div>

      {/* Date */}
      <div style={{
        color: 'var(--text-secondary)',
        fontSize: '0.85rem',
        marginBottom: 20,
      }}>
        {entry.date}
      </div>

      {/* Sections */}
      {Object.entries(sections).map(([sectionName, items]) => {
        const cfg = sectionColors[sectionName] || sectionColors.Added;
        return (
          <div key={sectionName} style={{ marginBottom: 18 }}>
            {/* Section label */}
            <div style={{
              display: 'inline-block',
              fontFamily: 'var(--font-stat)',
              fontSize: '0.6rem',
              letterSpacing: '0.15em',
              color: cfg.color,
              background: cfg.bg,
              border: `1px solid ${cfg.color}`,
              borderRadius: 6,
              padding: '2px 10px',
              marginBottom: 10,
            }}>
              {sectionName.toUpperCase()}
            </div>

            {/* Items */}
            <ul style={{
              listStyle: 'none',
              padding: 0,
              margin: 0,
              display: 'flex',
              flexDirection: 'column',
              gap: 6,
            }}>
              {items.map((item, i) => (
                <li key={i} style={{
                  display: 'flex',
                  alignItems: 'flex-start',
                  fontSize: '0.85rem',
                  color: 'var(--text-secondary)',
                  lineHeight: 1.6,
                }}>
                  <AnimatedDot color={cfg.dotColor} />
                  <span>{item}</span>
                </li>
              ))}
            </ul>
          </div>
        );
      })}
    </div>
  );
}

export default function Changelog() {
  return (
    <div style={{
      minHeight: '100vh',
      background: 'var(--bg-primary)',
      paddingTop: 100,
      paddingBottom: 80,
    }}>
      {/* Keyframes */}
      <style>{`
        @keyframes dot-breathe {
          0%, 100% { opacity: 0.6; transform: scale(1); }
          50% { opacity: 1; transform: scale(1.4); }
        }
      `}</style>

      {/* Header */}
      <div style={{
        maxWidth: 1280,
        margin: '0 auto',
        padding: '0 24px',
        textAlign: 'center',
        marginBottom: 64,
      }}>
        <h1 style={{
          fontFamily: 'var(--font-hero)',
          fontWeight: 700,
          fontSize: 'clamp(2rem, 5vw, 3rem)',
          background: 'linear-gradient(135deg, var(--accent-cyan), var(--accent-purple))',
          WebkitBackgroundClip: 'text',
          WebkitTextFillColor: 'transparent',
          backgroundClip: 'text',
          margin: '0 0 12px',
        }}>
          Changelog
        </h1>
        <p style={{
          color: 'var(--text-secondary)',
          fontSize: '1.05rem',
          maxWidth: 600,
          margin: '0 auto',
        }}>
          Every release, every change — fully documented.
        </p>
      </div>

      {/* Version blocks */}
      <div style={{
        maxWidth: 780,
        margin: '0 auto',
        padding: '0 24px',
      }}>
        {CHANGELOG_DATA.map((entry, index) => (
          <VersionBlock key={entry.version} entry={entry} index={index} />
        ))}
      </div>
    </div>
  );
}

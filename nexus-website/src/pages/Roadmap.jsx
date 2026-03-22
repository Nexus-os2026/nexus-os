import React, { useEffect, useRef } from 'react';
import { ROADMAP_DATA } from '../data/constants';

const statusConfig = {
  completed: {
    label: 'SHIPPED',
    bg: 'rgba(0,255,136,0.12)',
    border: 'var(--accent-green)',
    color: 'var(--accent-green)',
    nodeColor: 'var(--accent-green)',
  },
  current: {
    label: 'IN PROGRESS',
    bg: 'rgba(0,245,255,0.12)',
    border: 'var(--accent-cyan)',
    color: 'var(--accent-cyan)',
    nodeColor: 'var(--accent-cyan)',
    pulse: true,
  },
  in_progress: {
    label: 'IN PROGRESS',
    bg: 'rgba(0,245,255,0.08)',
    border: 'var(--accent-cyan)',
    color: 'var(--accent-cyan)',
    nodeColor: 'var(--accent-cyan)',
  },
  planned: {
    label: 'PLANNED',
    bg: 'transparent',
    border: 'var(--text-dim)',
    color: 'var(--text-dim)',
    nodeColor: 'var(--text-dim)',
    dashed: true,
  },
};

function CheckIcon({ completed }) {
  return (
    <span style={{
      display: 'inline-flex',
      alignItems: 'center',
      justifyContent: 'center',
      width: 16,
      height: 16,
      borderRadius: '50%',
      background: completed ? 'rgba(0,255,136,0.15)' : 'rgba(74,85,104,0.2)',
      border: `1px solid ${completed ? 'var(--accent-green)' : 'var(--text-dim)'}`,
      marginRight: 8,
      flexShrink: 0,
      transition: 'all 0.3s ease',
    }}>
      {completed && (
        <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
          <path
            d="M2 5L4.5 7.5L8 3"
            stroke="var(--accent-green)"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
            style={{
              strokeDasharray: 12,
              strokeDashoffset: 0,
              animation: 'check-draw 0.4s ease-out forwards',
            }}
          />
        </svg>
      )}
    </span>
  );
}

function MilestoneNode({ milestone, index, isLast }) {
  const ref = useRef(null);
  const cfg = statusConfig[milestone.status] || statusConfig.planned;

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          el.classList.add('visible');
        }
      },
      { threshold: 0.15 }
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const isCompleted = milestone.status === 'completed';

  return (
    <div
      ref={ref}
      className="fade-up"
      style={{
        position: 'relative',
        paddingLeft: 48,
        paddingBottom: isLast ? 0 : 48,
        transitionDelay: `${index * 0.08}s`,
      }}
    >
      {/* Node dot */}
      <div style={{
        position: 'absolute',
        left: 0,
        top: 4,
        width: 20,
        height: 20,
        borderRadius: '50%',
        background: cfg.pulse
          ? cfg.nodeColor
          : milestone.status === 'planned'
            ? 'transparent'
            : cfg.nodeColor,
        border: milestone.status === 'planned'
          ? `2px dashed ${cfg.nodeColor}`
          : `2px solid ${cfg.nodeColor}`,
        boxShadow: cfg.pulse
          ? `0 0 16px ${cfg.nodeColor}, 0 0 32px rgba(0,245,255,0.3)`
          : isCompleted
            ? `0 0 8px rgba(0,255,136,0.3)`
            : 'none',
        animation: cfg.pulse ? 'node-pulse 2s ease-in-out infinite' : 'none',
        zIndex: 2,
      }} />

      {/* Content card */}
      <div className="glass-card" style={{
        padding: '24px 28px',
      }}>
        {/* Version badge */}
        <div style={{
          display: 'inline-block',
          fontFamily: 'var(--font-stat)',
          fontSize: '0.7rem',
          letterSpacing: '0.12em',
          color: cfg.color,
          background: cfg.bg,
          border: `1px ${cfg.dashed ? 'dashed' : 'solid'} ${cfg.border}`,
          borderRadius: 6,
          padding: '3px 10px',
          marginBottom: 10,
        }}>
          {milestone.version}
        </div>

        {/* Title */}
        <h3 style={{
          fontFamily: 'var(--font-hero)',
          fontWeight: 600,
          fontSize: '1.15rem',
          color: 'var(--text-primary)',
          margin: '0 0 6px',
        }}>
          {milestone.title}
        </h3>

        {/* Date */}
        <div style={{
          color: 'var(--text-secondary)',
          fontSize: '0.8rem',
          marginBottom: 12,
        }}>
          {milestone.date}
        </div>

        {/* Status badge */}
        <div style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: 6,
          fontFamily: 'var(--font-stat)',
          fontSize: '0.6rem',
          letterSpacing: '0.15em',
          color: cfg.color,
          background: cfg.bg,
          border: `1px ${cfg.dashed ? 'dashed' : 'solid'} ${cfg.border}`,
          borderRadius: 20,
          padding: '4px 12px',
          marginBottom: 16,
          animation: cfg.pulse ? 'pulse-glow 1.5s ease-in-out infinite' : 'none',
        }}>
          {cfg.pulse && (
            <span style={{
              width: 6,
              height: 6,
              borderRadius: '50%',
              background: cfg.color,
              animation: 'node-pulse 1.2s ease-in-out infinite',
            }} />
          )}
          {cfg.label}
        </div>

        {/* Items list */}
        <ul style={{
          listStyle: 'none',
          padding: 0,
          margin: 0,
          display: 'flex',
          flexDirection: 'column',
          gap: 8,
        }}>
          {milestone.items.map((item, i) => (
            <li key={i} style={{
              display: 'flex',
              alignItems: 'flex-start',
              fontSize: '0.85rem',
              color: 'var(--text-secondary)',
              lineHeight: 1.5,
            }}>
              <CheckIcon completed={isCompleted} />
              <span>{item}</span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}

export default function Roadmap() {
  return (
    <div style={{
      minHeight: '100vh',
      background: 'var(--bg-primary)',
      paddingTop: 100,
      paddingBottom: 80,
    }}>
      {/* Keyframes */}
      <style>{`
        @keyframes node-pulse {
          0%, 100% { box-shadow: 0 0 8px var(--accent-cyan), 0 0 20px rgba(0,245,255,0.2); }
          50% { box-shadow: 0 0 16px var(--accent-cyan), 0 0 40px rgba(0,245,255,0.4); }
        }
        @keyframes check-draw {
          from { stroke-dashoffset: 12; }
          to { stroke-dashoffset: 0; }
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
          Development Roadmap
        </h1>
        <p style={{
          color: 'var(--text-secondary)',
          fontSize: '1.05rem',
          maxWidth: 600,
          margin: '0 auto',
        }}>
          From kernel to complete operating system — every milestone governed.
        </p>
      </div>

      {/* Timeline */}
      <div style={{
        maxWidth: 720,
        margin: '0 auto',
        padding: '0 24px',
        position: 'relative',
      }}>
        {/* Vertical line */}
        <div style={{
          position: 'absolute',
          left: 33,
          top: 0,
          bottom: 0,
          width: 2,
          background: 'linear-gradient(to bottom, var(--accent-green) 0%, var(--accent-green) 45%, var(--accent-cyan) 55%, var(--text-dim) 75%, rgba(74,85,104,0.2) 100%)',
          zIndex: 1,
        }} />

        {ROADMAP_DATA.map((milestone, index) => (
          <MilestoneNode
            key={milestone.version}
            milestone={milestone}
            index={index}
            isLast={index === ROADMAP_DATA.length - 1}
          />
        ))}
      </div>
    </div>
  );
}

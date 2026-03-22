import React from 'react';

const icons = {
  shield: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <defs>
        <clipPath id="shield-clip">
          <path d="M24 4L6 12v12c0 11 8 18 18 22 10-4 18-11 18-22V12L24 4z" />
        </clipPath>
      </defs>
      <path d="M24 4L6 12v12c0 11 8 18 18 22 10-4 18-11 18-22V12L24 4z"
        stroke="currentColor" strokeWidth="2" fill="none"
        style={{ animation: 'pulse-glow 3s ease-in-out infinite' }} />
      <rect x="0" y="0" width="48" height="4" fill="currentColor" opacity="0.15"
        clipPath="url(#shield-clip)"
        style={{ animation: 'scan-line 2.5s linear infinite' }} />
    </svg>
  ),

  dna: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <g style={{ animation: 'rotate-slow 8s linear infinite', transformOrigin: 'center' }}>
        <path d="M16 6c0 8 16 10 16 18s-16 10-16 18" stroke="currentColor" strokeWidth="2" fill="none" />
        <path d="M32 6c0 8-16 10-16 18s16 10 16 18" stroke="currentColor" strokeWidth="2" fill="none" />
        <line x1="16" y1="15" x2="32" y2="15" stroke="currentColor" strokeWidth="1.5" opacity="0.5" />
        <line x1="16" y1="24" x2="32" y2="24" stroke="currentColor" strokeWidth="1.5" opacity="0.5" />
        <line x1="16" y1="33" x2="32" y2="33" stroke="currentColor" strokeWidth="1.5" opacity="0.5" />
      </g>
    </svg>
  ),

  lock: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <rect x="10" y="22" width="28" height="20" rx="4" stroke="currentColor" strokeWidth="2" fill="none"
        style={{ animation: 'pulse-glow 3s ease-in-out infinite' }} />
      <path d="M16 22V16a8 8 0 0116 0v6" stroke="currentColor" strokeWidth="2" fill="none" />
      <circle cx="24" cy="32" r="3" fill="currentColor"
        style={{ animation: 'shimmer 2s ease-in-out infinite' }} />
    </svg>
  ),

  brain: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <path d="M24 44V24" stroke="currentColor" strokeWidth="2" />
      <path d="M24 24c-4-2-10-2-12 2s0 10 4 12" stroke="currentColor" strokeWidth="2" fill="none"
        style={{ animation: 'pulse-glow 2.5s ease-in-out infinite' }} />
      <path d="M24 24c4-2 10-2 12 2s0 10-4 12" stroke="currentColor" strokeWidth="2" fill="none"
        style={{ animation: 'pulse-glow 2.5s ease-in-out infinite 0.3s' }} />
      <path d="M12 18c-2-6 2-12 8-14" stroke="currentColor" strokeWidth="2" fill="none" />
      <path d="M36 18c2-6-2-12-8-14" stroke="currentColor" strokeWidth="2" fill="none" />
      <circle cx="18" cy="18" r="2" fill="currentColor" style={{ animation: 'neural-pulse 1.5s ease-in-out infinite' }} />
      <circle cx="30" cy="18" r="2" fill="currentColor" style={{ animation: 'neural-pulse 1.5s ease-in-out infinite 0.5s' }} />
      <circle cx="24" cy="12" r="2" fill="currentColor" style={{ animation: 'neural-pulse 1.5s ease-in-out infinite 1s' }} />
    </svg>
  ),

  code: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <g style={{ animation: 'expand-contract 3s ease-in-out infinite' }}>
        <path d="M16 14l-10 10 10 10" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" fill="none" />
        <path d="M32 14l10 10-10 10" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" fill="none" />
      </g>
      <line x1="28" y1="8" x2="20" y2="40" stroke="currentColor" strokeWidth="2" opacity="0.5" />
    </svg>
  ),

  building: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <g style={{ animation: 'breathe 4s ease-in-out infinite' }}>
        <rect x="8" y="28" width="12" height="16" rx="2" stroke="currentColor" strokeWidth="2" fill="none" />
        <rect x="28" y="20" width="12" height="24" rx="2" stroke="currentColor" strokeWidth="2" fill="none" />
        <rect x="18" y="8" width="12" height="36" rx="2" stroke="currentColor" strokeWidth="2" fill="none" />
        <line x1="22" y1="14" x2="26" y2="14" stroke="currentColor" strokeWidth="1.5" opacity="0.5" />
        <line x1="22" y1="20" x2="26" y2="20" stroke="currentColor" strokeWidth="1.5" opacity="0.5" />
        <line x1="22" y1="26" x2="26" y2="26" stroke="currentColor" strokeWidth="1.5" opacity="0.5" />
      </g>
    </svg>
  ),

  globe: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <circle cx="24" cy="24" r="18" stroke="currentColor" strokeWidth="2" fill="none" />
      <ellipse cx="24" cy="24" rx="10" ry="18" stroke="currentColor" strokeWidth="1.5" fill="none" />
      <line x1="6" y1="24" x2="42" y2="24" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
      <line x1="8" y1="16" x2="40" y2="16" stroke="currentColor" strokeWidth="1" opacity="0.3" />
      <line x1="8" y1="32" x2="40" y2="32" stroke="currentColor" strokeWidth="1" opacity="0.3" />
      <circle cx="24" cy="24" r="2" fill="currentColor"
        style={{ animation: 'orbit 6s linear infinite', transformOrigin: '24px 24px' }} />
    </svg>
  ),

  cpu: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <rect x="12" y="12" width="24" height="24" rx="4" stroke="currentColor" strokeWidth="2" fill="none" />
      <rect x="18" y="18" width="12" height="12" rx="2" stroke="currentColor" strokeWidth="1.5" fill="none"
        style={{ animation: 'pulse-glow 2s ease-in-out infinite' }} />
      {[8, 18, 28, 38].map((x) => (
        <React.Fragment key={`cpu-v-${x}`}>
          <line x1={x <= 18 ? 18 : 36} y1={x <= 18 ? x + 6 : x - 20} x2={x <= 18 ? 12 : 42} y2={x <= 18 ? x + 6 : x - 20}
            stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
        </React.Fragment>
      ))}
      <line x1="12" y1="20" x2="6" y2="20" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
      <line x1="12" y1="28" x2="6" y2="28" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
      <line x1="36" y1="20" x2="42" y2="20" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
      <line x1="36" y1="28" x2="42" y2="28" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
      <line x1="20" y1="12" x2="20" y2="6" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
      <line x1="28" y1="12" x2="28" y2="6" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
      <line x1="20" y1="36" x2="20" y2="42" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
      <line x1="28" y1="36" x2="28" y2="42" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
    </svg>
  ),

  eye: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <path d="M4 24s8-14 20-14 20 14 20 14-8 14-20 14S4 24 4 24z"
        stroke="currentColor" strokeWidth="2" fill="none" />
      <circle cx="24" cy="24" r="8" stroke="currentColor" strokeWidth="2" fill="none" />
      <circle cx="24" cy="24" r="3" fill="currentColor"
        style={{ animation: 'pulse-glow 2s ease-in-out infinite' }} />
    </svg>
  ),

  layers: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <g style={{ animation: 'breathe 3s ease-in-out infinite' }}>
        <path d="M24 6L4 18l20 12 20-12L24 6z" stroke="currentColor" strokeWidth="2" fill="none" opacity="0.4" />
        <path d="M4 24l20 12 20-12" stroke="currentColor" strokeWidth="2" fill="none" opacity="0.6" />
        <path d="M4 30l20 12 20-12" stroke="currentColor" strokeWidth="2" fill="none"
          style={{ animation: 'pulse-glow 2.5s ease-in-out infinite' }} />
      </g>
    </svg>
  ),

  zap: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <path d="M26 4L8 28h14L20 44 38 20H24L26 4z" stroke="currentColor" strokeWidth="2" fill="none"
        style={{ animation: 'pulse-glow 1.5s ease-in-out infinite' }} />
    </svg>
  ),

  calendar: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <rect x="6" y="10" width="36" height="32" rx="4" stroke="currentColor" strokeWidth="2" fill="none" />
      <line x1="6" y1="20" x2="42" y2="20" stroke="currentColor" strokeWidth="2" />
      <line x1="16" y1="6" x2="16" y2="14" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
      <line x1="32" y1="6" x2="32" y2="14" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
      <circle cx="16" cy="28" r="2" fill="currentColor" style={{ animation: 'neural-pulse 2s ease-in-out infinite' }} />
      <circle cx="24" cy="28" r="2" fill="currentColor" style={{ animation: 'neural-pulse 2s ease-in-out infinite 0.3s' }} />
      <circle cx="32" cy="28" r="2" fill="currentColor" style={{ animation: 'neural-pulse 2s ease-in-out infinite 0.6s' }} />
    </svg>
  ),

  network: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <circle cx="24" cy="12" r="4" stroke="currentColor" strokeWidth="2" fill="none" />
      <circle cx="12" cy="36" r="4" stroke="currentColor" strokeWidth="2" fill="none" />
      <circle cx="36" cy="36" r="4" stroke="currentColor" strokeWidth="2" fill="none" />
      <line x1="24" y1="16" x2="12" y2="32" stroke="currentColor" strokeWidth="1.5" opacity="0.5" />
      <line x1="24" y1="16" x2="36" y2="32" stroke="currentColor" strokeWidth="1.5" opacity="0.5" />
      <line x1="16" y1="36" x2="32" y2="36" stroke="currentColor" strokeWidth="1.5" opacity="0.5" />
      <circle cx="24" cy="12" r="2" fill="currentColor" style={{ animation: 'pulse-glow 2s ease-in-out infinite' }} />
    </svg>
  ),

  key: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <circle cx="16" cy="20" r="10" stroke="currentColor" strokeWidth="2" fill="none"
        style={{ animation: 'pulse-glow 3s ease-in-out infinite' }} />
      <line x1="24" y1="24" x2="42" y2="24" stroke="currentColor" strokeWidth="2" />
      <line x1="36" y1="24" x2="36" y2="30" stroke="currentColor" strokeWidth="2" />
      <line x1="42" y1="24" x2="42" y2="30" stroke="currentColor" strokeWidth="2" />
    </svg>
  ),

  database: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <ellipse cx="24" cy="12" rx="16" ry="6" stroke="currentColor" strokeWidth="2" fill="none" />
      <path d="M8 12v24c0 3.3 7.2 6 16 6s16-2.7 16-6V12" stroke="currentColor" strokeWidth="2" fill="none" />
      <ellipse cx="24" cy="24" rx="16" ry="6" stroke="currentColor" strokeWidth="1.5" fill="none" opacity="0.4" />
      <ellipse cx="24" cy="36" rx="16" ry="6" stroke="currentColor" strokeWidth="1.5" fill="none" opacity="0.4"
        style={{ animation: 'pulse-glow 2.5s ease-in-out infinite' }} />
    </svg>
  ),

  plug: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <path d="M18 6v10M30 6v10" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
      <rect x="12" y="16" width="24" height="10" rx="3" stroke="currentColor" strokeWidth="2" fill="none" />
      <path d="M20 26v6a4 4 0 008 0v-6" stroke="currentColor" strokeWidth="2" fill="none" />
      <line x1="24" y1="36" x2="24" y2="44" stroke="currentColor" strokeWidth="2"
        style={{ animation: 'pulse-glow 2s ease-in-out infinite' }} />
    </svg>
  ),

  check: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <circle cx="24" cy="24" r="18" stroke="currentColor" strokeWidth="2" fill="none" />
      <path d="M14 24l7 7 13-13" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" fill="none"
        style={{ animation: 'pulse-glow 2s ease-in-out infinite' }} />
    </svg>
  ),

  arrow_down: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" style={{ color, animation: 'float 2s ease-in-out infinite' }}>
      <path d="M12 4v16M5 13l7 7 7-7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  ),

  star: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" style={{ color }}>
      <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"
        stroke="currentColor" strokeWidth="1.5" fill="none"
        style={{ animation: 'pulse-glow 2.5s ease-in-out infinite' }} />
    </svg>
  ),

  git: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <circle cx="24" cy="8" r="4" stroke="currentColor" strokeWidth="2" fill="none" />
      <circle cx="24" cy="40" r="4" stroke="currentColor" strokeWidth="2" fill="none" />
      <circle cx="36" cy="24" r="4" stroke="currentColor" strokeWidth="2" fill="none" />
      <path d="M24 12v6c0 4 4 6 8 6h0" stroke="currentColor" strokeWidth="2" fill="none" />
      <path d="M24 36v-6c0-4 4-6 8-6" stroke="currentColor" strokeWidth="2" fill="none" opacity="0.5" />
      <circle cx="24" cy="8" r="2" fill="currentColor" style={{ animation: 'pulse-glow 2s ease-in-out infinite' }} />
    </svg>
  ),

  users: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <circle cx="18" cy="16" r="6" stroke="currentColor" strokeWidth="2" fill="none" />
      <path d="M6 40v-4a8 8 0 0116 0v4" stroke="currentColor" strokeWidth="2" fill="none" />
      <circle cx="34" cy="16" r="6" stroke="currentColor" strokeWidth="2" fill="none" opacity="0.5" />
      <path d="M28 40v-4a8 8 0 0116 0v4" stroke="currentColor" strokeWidth="2" fill="none" opacity="0.5" />
    </svg>
  ),

  book: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <path d="M8 6h14a2 2 0 012 2v32a2 2 0 01-2 2H8a2 2 0 01-2-2V8a2 2 0 012-2z" stroke="currentColor" strokeWidth="2" fill="none" />
      <path d="M26 6h14a2 2 0 012 2v32a2 2 0 01-2 2H26a2 2 0 01-2-2V8a2 2 0 012-2z" stroke="currentColor" strokeWidth="2" fill="none" />
      <line x1="24" y1="6" x2="24" y2="42" stroke="currentColor" strokeWidth="1" opacity="0.3" />
      <line x1="12" y1="16" x2="20" y2="16" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
      <line x1="12" y1="22" x2="18" y2="22" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
      <line x1="28" y1="16" x2="36" y2="16" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
      <line x1="28" y1="22" x2="34" y2="22" stroke="currentColor" strokeWidth="1.5" opacity="0.4" />
    </svg>
  ),

  mail: ({ size, color }) => (
    <svg width={size} height={size} viewBox="0 0 48 48" fill="none" style={{ color }}>
      <rect x="6" y="12" width="36" height="24" rx="4" stroke="currentColor" strokeWidth="2" fill="none" />
      <path d="M6 16l18 12 18-12" stroke="currentColor" strokeWidth="2" fill="none"
        style={{ animation: 'pulse-glow 3s ease-in-out infinite' }} />
    </svg>
  ),
};

export default function AnimatedIcon({ name, size = 48, color = 'var(--accent-cyan)' }) {
  const IconComponent = icons[name];
  if (!IconComponent) return null;
  return <IconComponent size={size} color={color} />;
}

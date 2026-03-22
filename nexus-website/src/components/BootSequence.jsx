import { useState, useEffect } from 'react';
import { useReducedMotion } from '../hooks/useReducedMotion';

const BOOT_LINES = [
  { text: 'NEXUS OS v9.3.0', delay: 0 },
  { text: 'INITIALIZING KERNEL............ OK', delay: 300 },
  { text: 'LOADING GOVERNANCE ENGINE...... OK', delay: 600 },
  { text: 'DARWIN CORE ONLINE............. OK', delay: 900 },
  { text: '53 AGENTS STANDING BY.......... OK', delay: 1200 },
  { text: '', delay: 1400 },
  { text: 'SYSTEM READY', delay: 1500 },
];

const TOTAL_DURATION = 2200;
const STORAGE_KEY = 'nexus-command-boot-v3';

export default function BootSequence({ onComplete }) {
  const reducedMotion = useReducedMotion();
  const [visibleLines, setVisibleLines] = useState(0);
  const [fading, setFading] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const shouldSkip = reducedMotion || sessionStorage.getItem(STORAGE_KEY);

  useEffect(() => {
    if (shouldSkip) {
      onComplete?.();
      return undefined;
    }

    const timers = BOOT_LINES.map((line, i) =>
      setTimeout(() => setVisibleLines(i + 1), line.delay)
    );

    const fadeTimer = setTimeout(() => setFading(true), TOTAL_DURATION);
    const doneTimer = setTimeout(() => {
      setDismissed(true);
      sessionStorage.setItem(STORAGE_KEY, '1');
      onComplete?.();
    }, TOTAL_DURATION + 600);

    return () => {
      timers.forEach(clearTimeout);
      clearTimeout(fadeTimer);
      clearTimeout(doneTimer);
    };
  }, [onComplete, shouldSkip]);

  if (dismissed || shouldSkip) return null;

  return (
    <div style={{
      position: 'fixed',
      inset: 0,
      zIndex: 10000,
      background: '#0a0e17',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      opacity: fading ? 0 : 1,
      transition: 'opacity 0.6s ease',
      overflow: 'hidden',
    }}>
      <div style={{
        position: 'absolute',
        inset: 0,
        background: 'radial-gradient(circle at 50% 50%, rgba(0, 212, 255, 0.12), transparent 50%)',
        opacity: 0.6,
      }} />
      <div style={{
        position: 'absolute',
        inset: '12% 8%',
        border: '1px solid rgba(0, 212, 255, 0.12)',
        clipPath: 'polygon(0 0, calc(100% - 24px) 0, 100% 24px, 100% 100%, 24px 100%, 0 calc(100% - 24px))',
        boxShadow: '0 0 50px rgba(0, 212, 255, 0.08), inset 0 0 40px rgba(0, 212, 255, 0.04)',
      }} />
      <div style={{
        position: 'relative',
        zIndex: 2,
        width: 'min(720px, calc(100vw - 48px))',
        padding: '36px 30px',
        border: '1px solid rgba(0, 212, 255, 0.16)',
        background: 'linear-gradient(135deg, rgba(13, 21, 32, 0.94), rgba(8, 13, 22, 0.88))',
        boxShadow: '0 0 30px rgba(0, 212, 255, 0.1), inset 0 1px 0 rgba(0, 212, 255, 0.08)',
        clipPath: 'polygon(0 0, calc(100% - 20px) 0, 100% 20px, 100% 100%, 20px 100%, 0 calc(100% - 20px))',
      }}>
        <div style={{
          display: 'flex',
          justifyContent: 'space-between',
          gap: 16,
          marginBottom: 24,
          color: '#5a7a8a',
          fontFamily: "'Orbitron', sans-serif",
          fontSize: '0.65rem',
          letterSpacing: '0.28em',
          textTransform: 'uppercase',
        }}>
          <span>Command Interface</span>
          <span>Governance Active</span>
        </div>
        <div style={{
          height: 1,
          background: 'linear-gradient(90deg, rgba(0, 212, 255, 0.7), rgba(0, 212, 255, 0.04), transparent)',
          marginBottom: 18,
        }} />
        <div style={{
        fontFamily: "'JetBrains Mono', monospace",
        fontSize: 'clamp(0.7rem, 1.5vw, 0.9rem)',
        color: '#00d4ff',
        lineHeight: 2,
          minHeight: 220,
      }}>
        {BOOT_LINES.slice(0, visibleLines).map((line, i) => (
          <div key={i} style={{
            opacity: line.text === 'SYSTEM READY' ? 1 : 0.8,
            color: line.text === 'SYSTEM READY' ? '#00ff88' : '#00d4ff',
            fontWeight: line.text === 'SYSTEM READY' || i === 0 ? 700 : 400,
            letterSpacing: line.text === 'SYSTEM READY' ? '0.3em' : '0.05em',
            fontSize: line.text === 'SYSTEM READY' ? '1.2em' : undefined,
            marginTop: line.text === 'SYSTEM READY' ? 8 : 0,
          }}>
            {line.text}
            {i === visibleLines - 1 && line.text !== '' && (
              <span style={{ animation: 'typewriter-blink 0.8s step-end infinite' }}>_</span>
            )}
          </div>
        ))}
      </div>
        <div style={{
          marginTop: 22,
          height: 6,
          border: '1px solid rgba(0, 212, 255, 0.12)',
          background: 'rgba(0, 212, 255, 0.04)',
          overflow: 'hidden',
        }}>
          <div style={{
            width: `${Math.min((visibleLines / BOOT_LINES.length) * 100, 100)}%`,
            height: '100%',
            background: 'linear-gradient(90deg, #00d4ff, #00ff88)',
            boxShadow: '0 0 14px rgba(0, 212, 255, 0.35)',
            transition: 'width 0.2s ease',
          }} />
        </div>
      </div>

      {/* Scan line effect */}
      <div style={{
        position: 'absolute',
        inset: 0,
        background: 'repeating-linear-gradient(0deg, transparent, transparent 2px, rgba(0, 212, 255, 0.03) 2px, rgba(0, 212, 255, 0.03) 4px)',
        pointerEvents: 'none',
      }} />
    </div>
  );
}

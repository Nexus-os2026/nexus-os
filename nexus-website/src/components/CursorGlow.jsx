import { useEffect, useRef } from 'react';
import { useReducedMotion } from '../hooks/useReducedMotion';

export default function CursorGlow() {
  const ref = useRef(null);
  const reducedMotion = useReducedMotion();

  useEffect(() => {
    if (reducedMotion) return undefined;

    const isMobile = window.matchMedia('(max-width: 768px)').matches;
    if (isMobile) return;

    const el = ref.current;
    if (!el) return;

    function onMove(e) {
      el.style.left = e.clientX + 'px';
      el.style.top = e.clientY + 'px';
      el.style.opacity = '1';
    }

    function onLeave() {
      el.style.opacity = '0';
    }

    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseleave', onLeave);
    return () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseleave', onLeave);
    };
  }, [reducedMotion]);

  return <div ref={ref} className="cursor-glow" style={{ opacity: 0 }} />;
}

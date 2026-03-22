import { useEffect, useRef } from 'react';

export function useScrollReveal(options = {}) {
  const threshold = options.threshold || 0.15;
  const ref = useRef(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          el.classList.add('visible');
          observer.unobserve(el);
        }
      },
      { threshold }
    );

    observer.observe(el);
    return () => observer.disconnect();
  }, [threshold]);

  return ref;
}

export function useScrollRevealChildren(options = {}) {
  const threshold = options.threshold || 0.15;
  const stagger = options.stagger || 100;
  const ref = useRef(null);

  useEffect(() => {
    const container = ref.current;
    if (!container) return;

    const children = container.querySelectorAll('.fade-up');
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            const delay = Array.from(children).indexOf(entry.target) * stagger;
            setTimeout(() => entry.target.classList.add('visible'), delay);
            observer.unobserve(entry.target);
          }
        });
      },
      { threshold }
    );

    children.forEach((child) => observer.observe(child));
    return () => observer.disconnect();
  }, [stagger, threshold]);

  return ref;
}

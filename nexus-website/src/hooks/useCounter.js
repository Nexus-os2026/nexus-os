import { useState, useEffect, useRef } from 'react';

export function useCounter(target, duration = 2000) {
  const [count, setCount] = useState(0);
  const [started, setStarted] = useState(false);
  const ref = useRef(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting && !started) {
          setStarted(true);
          observer.unobserve(el);
        }
      },
      { threshold: 0.5 }
    );

    observer.observe(el);
    return () => observer.disconnect();
  }, [started]);

  useEffect(() => {
    if (!started) return;

    const numericTarget = typeof target === 'string'
      ? parseFloat(target.replace(/[^0-9.]/g, '')) * (target.includes('K') ? 1000 : 1)
      : target;

    const startTime = performance.now();

    function update(currentTime) {
      const elapsed = currentTime - startTime;
      const progress = Math.min(elapsed / duration, 1);
      const eased = 1 - Math.pow(1 - progress, 4);
      setCount(Math.floor(eased * numericTarget));

      if (progress < 1) {
        requestAnimationFrame(update);
      } else {
        setCount(numericTarget);
      }
    }

    requestAnimationFrame(update);
  }, [started, target, duration]);

  const formatted = typeof target === 'string' && target.includes('K')
    ? `${Math.floor(count / 1000)}K`
    : count.toLocaleString();

  return { ref, value: formatted, rawCount: count };
}

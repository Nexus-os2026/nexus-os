import { useCallback, useEffect, useRef } from "react";

interface LivingBackgroundProps {
  status?: "healthy" | "busy" | "alert";
  agentCount?: number;
}

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  alpha: number;
  alphaDir: number;
  depth: number;
}

const STATUS_COLORS: Record<string, [number, number, number]> = {
  healthy: [0, 255, 157],
  busy: [245, 158, 11],
  alert: [239, 68, 68],
};

export default function LivingBackground({ status = "healthy", agentCount = 4 }: LivingBackgroundProps): JSX.Element {
  const shellRef = useRef<HTMLDivElement>(null);
  const auraRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animRef = useRef<number>(0);
  const particlesRef = useRef<Particle[]>([]);
  const pointerTargetRef = useRef({ x: 0, y: 0 });
  const pointerCurrentRef = useRef({ x: 0, y: 0 });
  const prefersReducedMotion = useRef(false);

  const getColor = useCallback(() => {
    return STATUS_COLORS[status] || STATUS_COLORS.healthy;
  }, [status]);

  useEffect(() => {
    prefersReducedMotion.current = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const targetCount = Math.min(84, Math.max(28, agentCount * 8));

    function resize() {
      if (!canvas) return;
      canvas.width = window.innerWidth;
      canvas.height = window.innerHeight;
    }
    resize();
    window.addEventListener("resize", resize);

    // Initialize particles
    while (particlesRef.current.length < targetCount) {
      particlesRef.current.push(createParticle(canvas.width, canvas.height));
    }
    while (particlesRef.current.length > targetCount) {
      particlesRef.current.pop();
    }

    function createParticle(w: number, h: number): Particle {
      return {
        x: Math.random() * w,
        y: Math.random() * h,
        vx: (Math.random() - 0.5) * 0.22,
        vy: (Math.random() - 0.5) * 0.22,
        radius: Math.random() * 1.9 + 0.6,
        alpha: Math.random() * 0.5 + 0.1,
        alphaDir: (Math.random() - 0.5) * 0.005,
        depth: Math.random() * 0.9 + 0.2,
      };
    }

    const onPointerMove = (event: MouseEvent) => {
      if (prefersReducedMotion.current) {
        return;
      }
      pointerTargetRef.current = {
        x: (event.clientX / window.innerWidth - 0.5) * 2,
        y: (event.clientY / window.innerHeight - 0.5) * 2,
      };
    };

    const onPointerLeave = () => {
      pointerTargetRef.current = { x: 0, y: 0 };
    };

    window.addEventListener("mousemove", onPointerMove);
    window.addEventListener("mouseleave", onPointerLeave);

    function animate() {
      if (!canvas || !ctx) return;
      const pointerCurrent = pointerCurrentRef.current;
      const pointerTarget = pointerTargetRef.current;
      pointerCurrent.x += (pointerTarget.x - pointerCurrent.x) * 0.04;
      pointerCurrent.y += (pointerTarget.y - pointerCurrent.y) * 0.04;

      if (auraRef.current && gridRef.current) {
        const shiftX = pointerCurrent.x * 22;
        const shiftY = pointerCurrent.y * 16;
        auraRef.current.style.transform = `translate3d(${shiftX}px, ${shiftY}px, 0) scale(1.04)`;
        gridRef.current.style.transform = `translate3d(${shiftX * -0.4}px, ${shiftY * -0.3}px, 0)`;
      }

      if (prefersReducedMotion.current) {
        // Draw static particles once
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        const [r, g, b] = getColor();
        for (const p of particlesRef.current) {
          ctx.beginPath();
          ctx.arc(p.x, p.y, p.radius, 0, Math.PI * 2);
          ctx.fillStyle = `rgba(${r}, ${g}, ${b}, 0.2)`;
          ctx.fill();
        }
        return;
      }

      ctx.clearRect(0, 0, canvas.width, canvas.height);
      const [r, g, b] = getColor();
      const connectionDistance = 132;

      for (const p of particlesRef.current) {
        p.x += p.vx;
        p.y += p.vy;
        p.alpha += p.alphaDir;

        if (p.alpha <= 0.05 || p.alpha >= 0.6) {
          p.alphaDir = -p.alphaDir;
        }

        // Wrap around
        if (p.x < 0) p.x = canvas.width;
        if (p.x > canvas.width) p.x = 0;
        if (p.y < 0) p.y = canvas.height;
        if (p.y > canvas.height) p.y = 0;

        // Draw particle
        const drawX = p.x + pointerCurrent.x * 18 * p.depth;
        const drawY = p.y + pointerCurrent.y * 14 * p.depth;
        ctx.beginPath();
        ctx.arc(drawX, drawY, p.radius, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(${r}, ${g}, ${b}, ${p.alpha})`;
        ctx.fill();
      }

      // Draw connections
      const particles = particlesRef.current;
      for (let i = 0; i < particles.length; i++) {
        for (let j = i + 1; j < particles.length; j++) {
          const leftX = particles[i].x + pointerCurrent.x * 18 * particles[i].depth;
          const leftY = particles[i].y + pointerCurrent.y * 14 * particles[i].depth;
          const rightX = particles[j].x + pointerCurrent.x * 18 * particles[j].depth;
          const rightY = particles[j].y + pointerCurrent.y * 14 * particles[j].depth;
          const dx = leftX - rightX;
          const dy = leftY - rightY;
          const dist = Math.sqrt(dx * dx + dy * dy);
          if (dist < connectionDistance) {
            const lineAlpha = (1 - dist / connectionDistance) * 0.14 * ((particles[i].depth + particles[j].depth) / 2);
            ctx.beginPath();
            ctx.moveTo(leftX, leftY);
            ctx.lineTo(rightX, rightY);
            ctx.strokeStyle = `rgba(${r}, ${g}, ${b}, ${lineAlpha})`;
            ctx.lineWidth = 0.5;
            ctx.stroke();
          }
        }
      }

      animRef.current = requestAnimationFrame(animate);
    }

    animRef.current = requestAnimationFrame(animate);

    return () => {
      cancelAnimationFrame(animRef.current);
      window.removeEventListener("resize", resize);
      window.removeEventListener("mousemove", onPointerMove);
      window.removeEventListener("mouseleave", onPointerLeave);
    };
  }, [status, agentCount, getColor]);

  const [r, g, b] = getColor();

  return (
    <div ref={shellRef} className="living-background" aria-hidden="true">
      <div className="living-background__aura-clip">
        <div
          ref={auraRef}
          className="living-background__aura"
          style={{
            background: `
              radial-gradient(circle at 18% 22%, rgba(${r}, ${g}, ${b}, 0.2), transparent 24%),
              radial-gradient(circle at 84% 16%, rgba(140, 123, 255, 0.18), transparent 22%),
              radial-gradient(circle at 50% 78%, rgba(108, 185, 255, 0.12), transparent 32%)
            `,
          }}
        />
      </div>
      <div ref={gridRef} className="living-background__grid" />
      <canvas ref={canvasRef} className="living-background__canvas" />
      <div className="living-background__vignette" />
    </div>
  );
}

import { useEffect, useRef } from "react";

export type VoiceOrbState = "idle" | "listening" | "processing" | "speaking";

interface VoiceOrbProps {
  state: VoiceOrbState;
  amplitude?: number;
  size?: number;
}

interface OrbParticle {
  angle: number;
  distance: number;
  speed: number;
  size: number;
}

function paletteFor(state: VoiceOrbState): { core: string; glow: string; shell: string } {
  if (state === "listening") {
    return { core: "#68f4ff", glow: "#00c8ff", shell: "#8df8ff" };
  }
  if (state === "processing") {
    return { core: "#7c9dff", glow: "#a06bff", shell: "#d1bcff" };
  }
  if (state === "speaking") {
    return { core: "#7ef7e8", glow: "#25d0ff", shell: "#b5fff7" };
  }
  return { core: "#7fd4ff", glow: "#2f88ff", shell: "#d6f5ff" };
}

export function VoiceOrb({ state, amplitude = 0.16, size = 168 }: VoiceOrbProps): JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      return;
    }

    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    canvas.width = Math.floor(size * dpr);
    canvas.height = Math.floor(size * dpr);
    canvas.style.width = `${size}px`;
    canvas.style.height = `${size}px`;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const particles = Array.from({ length: 24 }, (): OrbParticle => ({
      angle: Math.random() * Math.PI * 2,
      distance: 24 + Math.random() * 40,
      speed: 0.002 + Math.random() * 0.006,
      size: 1 + Math.random() * 2
    }));

    const center = size / 2;
    let raf = 0;

    const draw = (ts: number): void => {
      raf = window.requestAnimationFrame(draw);
      const t = ts * 0.001;
      const palette = paletteFor(state);
      const baseRadius = size * 0.24;

      const ripple = state === "listening" ? Math.sin(t * 6) * 4 : Math.sin(t * 1.8) * 1.2;
      const pulse = state === "speaking" ? Math.max(0, amplitude) * 14 : 0;
      const fragment = state === "processing" ? 1 : 0;

      const radius = baseRadius + ripple + pulse;

      ctx.clearRect(0, 0, size, size);

      const glow = ctx.createRadialGradient(center, center, 8, center, center, radius * 2.3);
      glow.addColorStop(0, `${palette.glow}55`);
      glow.addColorStop(1, "rgba(0, 0, 0, 0)");
      ctx.fillStyle = glow;
      ctx.beginPath();
      ctx.arc(center, center, radius * 2.3, 0, Math.PI * 2);
      ctx.fill();

      for (let ring = 0; ring < 3; ring += 1) {
        ctx.strokeStyle = `rgba(122, 238, 255, ${0.16 - ring * 0.04})`;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.arc(center, center, radius + ring * 8 + Math.sin(t * 2 + ring) * 1.8, 0, Math.PI * 2);
        ctx.stroke();
      }

      const sphere = ctx.createRadialGradient(
        center - radius * 0.34,
        center - radius * 0.36,
        radius * 0.1,
        center,
        center,
        radius
      );
      sphere.addColorStop(0, palette.shell);
      sphere.addColorStop(0.46, palette.core);
      sphere.addColorStop(1, "rgba(6, 12, 30, 0.95)");
      ctx.fillStyle = sphere;
      ctx.beginPath();
      ctx.arc(center, center, radius, 0, Math.PI * 2);
      ctx.fill();

      if (state === "speaking") {
        ctx.strokeStyle = "rgba(112, 249, 255, 0.32)";
        ctx.lineWidth = 1;
        for (let index = 0; index < 12; index += 1) {
          const a = (Math.PI * 2 * index) / 12 + t * 0.9;
          const len = radius + 18 + Math.sin(t * 6 + index) * 4;
          ctx.beginPath();
          ctx.moveTo(center + Math.cos(a) * radius, center + Math.sin(a) * radius);
          ctx.lineTo(center + Math.cos(a) * len, center + Math.sin(a) * len);
          ctx.stroke();
        }
      }

      for (const particle of particles) {
        particle.angle += particle.speed * (fragment > 0 ? 2.8 : 1);
        const radial = particle.distance + Math.sin(t * 2 + particle.angle * 2) * 3;
        const orbit = radial + fragment * 18;
        const px = center + Math.cos(particle.angle) * orbit;
        const py = center + Math.sin(particle.angle) * orbit;
        ctx.fillStyle = `rgba(151, 252, 255, ${0.48 + fragment * 0.3})`;
        ctx.beginPath();
        ctx.arc(px, py, particle.size + fragment * 0.8, 0, Math.PI * 2);
        ctx.fill();
      }
    };

    raf = window.requestAnimationFrame(draw);
    return () => {
      window.cancelAnimationFrame(raf);
    };
  }, [amplitude, size, state]);

  return <canvas ref={canvasRef} className="voice-orb" aria-hidden="true" />;
}

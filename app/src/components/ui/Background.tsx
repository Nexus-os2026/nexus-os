import { useMemo } from "react";

interface BackgroundProps {
  particleCount?: number;
}

interface Particle {
  id: number;
  left: number;
  top: number;
  opacity: number;
  duration: number;
  delay: number;
}

function pseudoRandom(seed: number): number {
  const x = Math.sin(seed * 12.9898) * 43758.5453;
  return x - Math.floor(x);
}

export function Background({ particleCount = 40 }: BackgroundProps): JSX.Element {
  const particles = useMemo<Particle[]>(
    () =>
      Array.from({ length: particleCount }, (_, index) => {
        const seed = index + 1;
        return {
          id: index,
          left: pseudoRandom(seed * 1.13) * 100,
          top: pseudoRandom(seed * 2.07) * 100,
          opacity: 0.18 + pseudoRandom(seed * 3.41) * 0.6,
          duration: 14 + pseudoRandom(seed * 4.27) * 18,
          delay: pseudoRandom(seed * 5.17) * -18
        };
      }),
    [particleCount]
  );

  return (
    <div className="cyber-background" aria-hidden="true">
      <div className="cyber-grid-plane" />
      <div className="cyber-particles">
        {particles.map((particle) => (
          <span
            key={particle.id}
            className="cyber-particle"
            style={{
              left: `${particle.left}%`,
              top: `${particle.top}%`,
              opacity: particle.opacity,
              animationDuration: `${particle.duration}s`,
              animationDelay: `${particle.delay}s`
            }}
          />
        ))}
      </div>
      <div className="cyber-scanlines" />
      <span className="cyber-corner top-left" />
      <span className="cyber-corner top-right" />
      <span className="cyber-corner bottom-left" />
      <span className="cyber-corner bottom-right" />
    </div>
  );
}

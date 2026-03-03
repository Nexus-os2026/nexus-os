import { useMemo } from "react";

interface RadialGaugeProps {
  value: number;
  max?: number;
  label?: string;
  size?: number;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

export function RadialGauge({ value, max = 100, label = "Fuel", size = 132 }: RadialGaugeProps): JSX.Element {
  const normalized = clamp(value / max, 0, 1);
  const radius = size * 0.36;
  const circumference = Math.PI * radius;
  const offset = circumference * (1 - normalized);

  const sparks = useMemo(
    () =>
      Array.from({ length: 16 }, (_, index) => {
        const angle = Math.PI * (index / 15);
        const r = radius + 8 + Math.sin(index) * 2;
        return {
          id: index,
          x: size / 2 - Math.cos(angle) * r,
          y: size / 2 + Math.sin(angle) * r,
          delay: index * 0.08
        };
      }),
    [radius, size]
  );

  return (
    <div className="viz-radial-gauge" style={{ width: size, height: size }}>
      <svg viewBox={`0 0 ${size} ${size}`} className="viz-radial-gauge__svg" role="img" aria-label={`${label} ${Math.round(normalized * 100)} percent`}>
        <defs>
          <linearGradient id="arc-reactor-gradient" x1="0%" y1="0%" x2="100%" y2="0%">
            <stop offset="0%" stopColor="#22d3ee" />
            <stop offset="60%" stopColor="#38bdf8" />
            <stop offset="100%" stopColor="#a78bfa" />
          </linearGradient>
        </defs>
        <path
          d={`M ${size / 2 - radius} ${size / 2} A ${radius} ${radius} 0 0 1 ${size / 2 + radius} ${size / 2}`}
          className="viz-radial-gauge__track"
        />
        <path
          d={`M ${size / 2 - radius} ${size / 2} A ${radius} ${radius} 0 0 1 ${size / 2 + radius} ${size / 2}`}
          className="viz-radial-gauge__fill"
          strokeDasharray={circumference}
          strokeDashoffset={offset}
        />
        <circle cx={size / 2} cy={size / 2} r={size * 0.18} className="viz-radial-gauge__core" />
      </svg>
      <div className="viz-radial-gauge__value">{Math.round(normalized * 100)}%</div>
      <div className="viz-radial-gauge__label">{label}</div>
      {sparks.map((spark) => (
        <span
          key={spark.id}
          className="viz-radial-gauge__spark"
          style={{
            left: spark.x,
            top: spark.y,
            animationDelay: `${spark.delay}s`
          }}
        />
      ))}
    </div>
  );
}

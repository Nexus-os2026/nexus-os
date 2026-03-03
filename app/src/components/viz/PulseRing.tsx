interface PulseRingProps {
  active?: boolean;
  rings?: number;
  size?: number;
}

export function PulseRing({ active = true, rings = 4, size = 56 }: PulseRingProps): JSX.Element {
  return (
    <div className={`viz-pulse-ring ${active ? "active" : ""}`} style={{ width: size, height: size }}>
      {Array.from({ length: rings }, (_, index) => (
        <span
          key={index}
          className="viz-pulse-ring__ring"
          style={{ animationDelay: `${index * 0.26}s` }}
        />
      ))}
      <span className="viz-pulse-ring__core" />
    </div>
  );
}

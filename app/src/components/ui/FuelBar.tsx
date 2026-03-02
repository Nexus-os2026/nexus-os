interface FuelBarProps {
  value: number;
  max?: number;
  label?: string;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function FuelBar({ value, max = 10_000, label = "Fuel" }: FuelBarProps): JSX.Element {
  const safeMax = max > 0 ? max : 1;
  const percentage = clamp(Math.round((value / safeMax) * 100), 0, 100);

  return (
    <div className="fuel-bar">
      <div className="fuel-bar__meta">
        <span>{label}</span>
        <span>{percentage}%</span>
      </div>
      <div className="fuel-bar__track">
        <div className="fuel-bar__fill" style={{ width: `${percentage}%` }} />
      </div>
    </div>
  );
}

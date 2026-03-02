import { GlassPanel } from "./GlassPanel";

type MetricTrend = "up" | "down" | "flat";

interface MetricCardProps {
  label: string;
  value: string | number;
  trend?: MetricTrend;
  delta?: string;
  className?: string;
}

function trendGlyph(trend: MetricTrend): string {
  if (trend === "up") {
    return "▲";
  }
  if (trend === "down") {
    return "▼";
  }
  return "■";
}

export function MetricCard({
  label,
  value,
  trend = "flat",
  delta = "stable",
  className
}: MetricCardProps): JSX.Element {
  return (
    <GlassPanel className={className}>
      <article className="metric-card">
        <span className="metric-card__label">{label}</span>
        <p className="metric-card__value">{value}</p>
        <span className={`metric-card__trend ${trend}`}>
          {trendGlyph(trend)} {delta}
        </span>
      </article>
    </GlassPanel>
  );
}

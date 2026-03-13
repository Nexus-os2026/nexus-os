interface HeatMapProps {
  values: number[];
  columns?: number;
  title?: string;
}

function tone(value: number): string {
  if (value > 0.86) {
    return "#7fffd4";
  }
  if (value > 0.62) {
    return "var(--nexus-accent)";
  }
  if (value > 0.35) {
    return "#38bdf8";
  }
  if (value > 0.18) {
    return "#1e3a8a";
  }
  return "#0b1220";
}

export function HeatMap({ values, columns = 12, title = "Activity Heatmap" }: HeatMapProps): JSX.Element {
  return (
    <section className="viz-heatmap" aria-label={title}>
      <p className="viz-heatmap__title">{title}</p>
      <div
        className="viz-heatmap__grid"
        style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}
      >
        {values.map((value, index) => (
          <span
            key={`${index}-${value.toFixed(3)}`}
            className="viz-heatmap__cell"
            style={{ backgroundColor: tone(value), opacity: 0.2 + value * 0.8 }}
            title={`Intensity ${Math.round(value * 100)}%`}
          />
        ))}
      </div>
    </section>
  );
}

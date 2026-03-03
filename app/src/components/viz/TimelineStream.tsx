interface TimelineItem {
  id: string;
  label: string;
  timestamp: number;
  level?: "info" | "warn" | "error" | "success";
}

interface TimelineStreamProps {
  items: TimelineItem[];
}

function levelClass(level: TimelineItem["level"]): string {
  if (level === "warn") {
    return "warn";
  }
  if (level === "error") {
    return "error";
  }
  if (level === "success") {
    return "success";
  }
  return "info";
}

export function TimelineStream({ items }: TimelineStreamProps): JSX.Element {
  const ordered = [...items].sort((a, b) => a.timestamp - b.timestamp);

  return (
    <section className="viz-timeline" aria-label="Audit timeline stream">
      <div className="viz-timeline__rail" />
      <div className="viz-timeline__scroller">
        {ordered.map((item) => (
          <article key={item.id} className="viz-timeline__item">
            <span className={`viz-timeline__node ${levelClass(item.level)}`} />
            <div className="viz-timeline__card">
              <p className="viz-timeline__time">
                {new Date(item.timestamp * 1000).toLocaleTimeString("en-GB", {
                  hour12: false,
                  hour: "2-digit",
                  minute: "2-digit",
                  second: "2-digit"
                })}
              </p>
              <p className="viz-timeline__label">{item.label}</p>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}

interface ActivityFeedProps {
  entries: string[];
}

export function ActivityFeed({ entries }: ActivityFeedProps): JSX.Element {
  const lines = entries.length > 0 ? entries : ["system > Waiting for live agent activity [i]"];
  const marquee = [...lines, ...lines];

  return (
    <footer className="activity-feed" aria-label="Real-time activity feed">
      <div className="activity-feed-track">
        {marquee.map((entry, index) => (
          <span key={`${index}-${entry}`} className="activity-feed-item">
            {entry}
          </span>
        ))}
      </div>
    </footer>
  );
}

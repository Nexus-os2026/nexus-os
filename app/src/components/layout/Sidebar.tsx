import { useMemo, useState } from "react";
import "./sidebar.css";

export interface SidebarItem {
  id: string;
  label: string;
  icon: string;
  shortcut: string;
}

interface SidebarProps {
  items: SidebarItem[];
  activeId: string;
  onSelect: (id: string) => void;
  version: string;
}

export function Sidebar({ items, activeId, onSelect, version }: SidebarProps): JSX.Element {
  const [expanded, setExpanded] = useState(true);

  const activeLabel = useMemo(
    () => items.find((item) => item.id === activeId)?.label ?? "Overview",
    [activeId, items]
  );

  return (
    <aside className={`nexus-sidebar-shell ${expanded ? "expanded" : "collapsed"}`}>
      <div className="nexus-sidebar-top">
        <button
          type="button"
          className="nexus-sidebar-toggle"
          onClick={() => setExpanded((prev) => !prev)}
          aria-label={expanded ? "Collapse sidebar" : "Expand sidebar"}
        >
          {expanded ? "◀" : "▶"}
        </button>
        {expanded ? <p className="nexus-sidebar-active">NEXUS // {activeLabel.toUpperCase()}</p> : null}
      </div>

      <nav className="nexus-sidebar-nav">
        {items.map((item) => (
          <button
            key={item.id}
            type="button"
            className={`nexus-sidebar-item ${activeId === item.id ? "active" : ""}`}
            onClick={() => onSelect(item.id)}
            title={`${item.label} (${item.shortcut})`}
          >
            <span className="nexus-sidebar-active-bar" />
            <span className="nexus-sidebar-icon-wrap">
              <span className="nexus-sidebar-icon">{item.icon}</span>
            </span>
            {expanded ? (
              <span className="nexus-sidebar-item-text">
                <span className="nexus-sidebar-label">{item.label}</span>
                <span className="nexus-sidebar-shortcut">{item.shortcut}</span>
              </span>
            ) : null}
          </button>
        ))}
      </nav>

      <div className="nexus-sidebar-bottom">
        <span className="nexus-avatar">◉</span>
        {expanded ? (
          <p className="nexus-version">
            <span className="nexus-health-dot" />
            NEXUS {version}
          </p>
        ) : null}
      </div>
    </aside>
  );
}

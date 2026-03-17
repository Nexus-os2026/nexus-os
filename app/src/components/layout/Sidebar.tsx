import { useMemo } from "react";
import "./sidebar.css";

export interface SidebarItem {
  id: string;
  label: string;
  icon: string;
  shortcut: string;
  badge?: number;
  section?: string;
}

interface SidebarProps {
  items: SidebarItem[];
  activeId: string;
  onSelect: (id: string) => void;
  version: string;
}

export function Sidebar({ items, activeId, onSelect, version }: SidebarProps): JSX.Element {
  const activeLabel = useMemo(
    () => items.find((item) => item.id === activeId)?.label ?? "Overview",
    [activeId, items]
  );

  const grouped = useMemo(() => {
    const sections: { section: string; items: SidebarItem[] }[] = [];
    let current: { section: string; items: SidebarItem[] } | null = null;
    for (const item of items) {
      const sec = item.section ?? "";
      if (!current || current.section !== sec) {
        current = { section: sec, items: [] };
        sections.push(current);
      }
      current.items.push(item);
    }
    return sections;
  }, [items]);

  return (
    <aside className="nexus-sidebar-shell expanded">
      <div className="nexus-sidebar-top">
        <p className="nexus-sidebar-active">NEXUS // {activeLabel.toUpperCase()}</p>
      </div>

      <nav className="nexus-sidebar-nav">
        {grouped.map((group) => (
          <div key={group.section || "_default"}>
            {group.section ? (
              <div className="nexus-sidebar-section">{group.section}</div>
            ) : null}
            {group.items.map((item) => (
              <button
                key={item.id}
                type="button"
                className={`nexus-sidebar-item ${activeId === item.id ? "active" : ""}`}
                onClick={() => onSelect(item.id)}
                title={item.shortcut ? `${item.label} (${item.shortcut})` : item.label}
              >
                <span className="nexus-sidebar-active-bar" />
                <span className="nexus-sidebar-icon-wrap">
                  <span className="nexus-sidebar-icon">{item.icon}</span>
                </span>
                {item.badge && item.badge > 0 ? (
                  <span
                    style={{
                      position: "absolute",
                      top: 4,
                      right: 8,
                      background: "#f87171",
                      color: "#fff",
                      borderRadius: 10,
                      padding: "0 5px",
                      fontSize: "0.65rem",
                      fontWeight: 700,
                      lineHeight: "16px",
                      minWidth: 16,
                      textAlign: "center",
                      zIndex: 2,
                    }}
                  >
                    {item.badge}
                  </span>
                ) : null}
                <span className="nexus-sidebar-item-text">
                  <span className="nexus-sidebar-label">{item.label}</span>
                  {item.shortcut ? <span className="nexus-sidebar-shortcut">{item.shortcut}</span> : null}
                </span>
              </button>
            ))}
          </div>
        ))}
      </nav>

      <div className="nexus-sidebar-bottom">
        <span className="nexus-avatar">◉</span>
        <p className="nexus-version">
          <span className="nexus-health-dot" />
          NEXUS {version}
        </p>
      </div>
    </aside>
  );
}

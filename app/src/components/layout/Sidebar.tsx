import { useEffect, useMemo, useState } from "react";
import {
  MessageSquare, Users, Terminal, Shield, Clock, History,
  LayoutDashboard, Dna, Brain, Moon, GitBranch, GitMerge,
  ShieldCheck, Fingerprint, Lock, Monitor,
  Network, Landmark, Code2, Code,
  Workflow, Upload, Search,
  Award, Link, Layers, Key, CheckCircle, Scale,
  Palette, Mail, Play, Store, Bot, Mic, Rocket, BookOpen,
  FileCode, TerminalSquare, FolderOpen, Database, Globe, Globe2,
  MessageCircle, FileText, Cpu, StickyNote, Kanban, Activity,
  Settings, ChevronDown, ChevronRight,
  ShieldAlert, UserCog, Boxes, ScrollText, ClipboardCheck, HeartPulse,
  PlugZap, Timer, Gauge, LogIn, Building2, BarChart3, Receipt, Server, Zap,
  Target, FileSearch, GitCompare, GitCompareArrows, FlaskConical, Map, Route,
  Coins, Eye, Wrench, Factory, Sparkles, Hammer,
  type LucideIcon
} from "lucide-react";
import "./sidebar.css";

const ICON_MAP: Record<string, LucideIcon> = {
  MessageSquare, Users, Terminal, Shield, Clock, History,
  LayoutDashboard, Dna, Brain, Moon, GitBranch, GitMerge,
  ShieldCheck, Fingerprint, Lock, Monitor,
  Network, Landmark, Code2, Code,
  Workflow, Upload, Search,
  Award, Link, Layers, Key, CheckCircle, Scale,
  Palette, Mail, Play, Store, Bot, Mic, Rocket, BookOpen,
  FileCode, TerminalSquare, FolderOpen, Database, Globe, Globe2,
  MessageCircle, FileText, Cpu, StickyNote, Kanban, Activity,
  Settings,
  ShieldAlert, UserCog, Boxes, ScrollText, ClipboardCheck, HeartPulse,
  PlugZap, Timer, Gauge, LogIn, Building2, BarChart3, Receipt, Server, Zap,
  Target, FileSearch, GitCompare, GitCompareArrows, FlaskConical, Map, Route,
  Coins, Eye, Wrench, Factory, Sparkles, Hammer,
};

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
  fitnessScore?: number;
  connected?: boolean;
}

export function Sidebar({ items, activeId, onSelect, version, fitnessScore = 50, connected = false }: SidebarProps): JSX.Element {
  // All sections collapsed by default; persist expanded state to localStorage
  const [expandedSections, setExpandedSections] = useState<Set<string>>(() => {
    try {
      const saved = localStorage.getItem("nexus-sidebar-expanded");
      if (saved) return new Set(JSON.parse(saved) as string[]);
    } catch { /* ignore */ }
    return new Set<string>();
  });

  useEffect(() => {
    try {
      localStorage.setItem("nexus-sidebar-expanded", JSON.stringify([...expandedSections]));
    } catch { /* ignore */ }
  }, [expandedSections]);

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

  function toggleSection(section: string): void {
    setExpandedSections((prev) => {
      const next = new Set(prev);
      if (next.has(section)) {
        next.delete(section);
      } else {
        next.add(section);
      }
      return next;
    });
  }

  return (
    <aside className="nexus-sidebar-shell expanded">
      <div className="nexus-sidebar-top">
        <div className="nexus-sidebar-logo">
          <div className="nexus-sidebar-logo-mark" aria-hidden="true">
            <svg width="22" height="22" viewBox="0 0 24 24" fill="none">
              <path d="M12 2L2 7l10 5 10-5-10-5z" fill="rgba(74,247,211,0.16)" stroke="var(--nexus-accent)" strokeWidth="1.5" strokeLinejoin="round" />
              <path d="M2 17l10 5 10-5" stroke="var(--nexus-accent)" strokeWidth="1.5" strokeLinejoin="round" opacity="0.45" />
              <path d="M2 12l10 5 10-5" stroke="var(--nexus-accent)" strokeWidth="1.5" strokeLinejoin="round" opacity="0.72" />
            </svg>
          </div>
          <div className="nexus-sidebar-brand-block">
            <span className="nexus-sidebar-kicker">AI Operating System</span>
            <span className="nexus-sidebar-brand">NEXUS // CONTROL</span>
          </div>
        </div>
        <div className="nexus-sidebar-status-pill">
          <span className="nexus-sidebar-status-dot" style={connected ? undefined : { background: "#eab308", boxShadow: "0 0 8px rgba(234,179,8,0.5)" }} />
          {connected ? "Neural mesh online" : "Simulation mode"}
        </div>
      </div>

      <nav className="nexus-sidebar-nav">
        {grouped.map((group) => {
          const isCollapsed = group.section ? !expandedSections.has(group.section) : false;
          return (
            <div key={group.section || "_default"} className="nexus-sidebar-group">
              {group.section ? (
                <button type="button"
                  className="nexus-sidebar-section"
                  onClick={() => toggleSection(group.section)}
                >
                  <span>{group.section}</span>
                  {isCollapsed
                    ? <ChevronRight size={10} aria-hidden="true" />
                    : <ChevronDown size={10} aria-hidden="true" />
                  }
                </button>
              ) : null}
              <div
                className="nexus-sidebar-section-items"
                style={{
                  maxHeight: isCollapsed ? 0 : "2000px",
                  overflow: "hidden",
                  transition: "max-height 0.3s ease",
                }}
              >
                {group.items.map((item) => {
                  const IconComponent = ICON_MAP[item.icon];
                  return (
                    <button type="button"
                      key={item.id}
                      className={`nexus-sidebar-item ${activeId === item.id ? "active" : ""}`}
                      onClick={() => onSelect(item.id)}
                      title={item.shortcut ? `${item.label} (${item.shortcut})` : item.label}
                    >
                      <span className="nexus-sidebar-active-bar" />
                      <span className="nexus-sidebar-icon-wrap">
                        {IconComponent
                          ? <IconComponent size={15} aria-hidden="true" />
                          : <span className="nexus-sidebar-icon" style={{ overflow: "hidden", maxWidth: 26, display: "inline-block" }}>{item.icon.slice(0, 2)}</span>
                        }
                      </span>
                      {item.badge && item.badge > 0 ? (
                        <span className="nexus-sidebar-badge">{item.badge}</span>
                      ) : null}
                      <span className="nexus-sidebar-item-text">
                        <span className="nexus-sidebar-label">{item.label}</span>
                        {item.shortcut ? <span className="nexus-sidebar-shortcut">{item.shortcut}</span> : null}
                      </span>
                    </button>
                  );
                })}
              </div>
            </div>
          );
        })}
      </nav>

      <div className="nexus-sidebar-bottom">
        <div className="nexus-sidebar-core-ring">
          <svg width="36" height="36" viewBox="0 0 32 32" aria-label="OS fitness">
            <circle cx="16" cy="16" r="13" fill="none" stroke="rgba(118,190,255,0.12)" strokeWidth="2" />
            <circle
              cx="16"
              cy="16"
              r="13"
              fill="none"
              stroke={fitnessScore >= 80 ? "var(--nexus-accent)" : fitnessScore >= 50 ? "#eab308" : "#ef4444"}
              strokeWidth="2"
              strokeDasharray={`${(fitnessScore / 100) * 81.7} 81.7`}
              strokeLinecap="round"
              transform="rotate(-90 16 16)"
              style={{ filter: `drop-shadow(0 0 4px ${fitnessScore >= 80 ? "rgba(74,247,211,0.55)" : fitnessScore >= 50 ? "rgba(234,179,8,0.55)" : "rgba(239,68,68,0.55)"})`, transition: "stroke-dasharray 0.6s ease, stroke 0.4s ease" }}
            />
            <text x="16" y="17.5" textAnchor="middle" fill={fitnessScore >= 80 ? "var(--nexus-accent)" : fitnessScore >= 50 ? "#eab308" : "#ef4444"} fontSize="8" fontFamily="var(--font-mono)" fontWeight="600">
              {fitnessScore}
            </text>
          </svg>
        </div>
        <div className="nexus-version-block">
          <span className="nexus-version-label">Core Fitness {fitnessScore}%</span>
          <span className="nexus-version-status">
            <span className="nexus-health-dot" />
            {version} operational
          </span>
        </div>
      </div>
    </aside>
  );
}

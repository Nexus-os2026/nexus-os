import { Link } from 'react-router-dom';
import { COMMIT_COUNT, GITLAB_PROJECT, STATS, VERSION } from '../data/constants';

const pageLinks = [
  { to: '/', label: 'Home' },
  { to: '/features', label: 'Features' },
  { to: '/architecture', label: 'Architecture' },
  { to: '/agents', label: 'Agents' },
  { to: '/comparison', label: 'Compare' },
  { to: '/roadmap', label: 'Roadmap' },
  { to: '/docs', label: 'Docs' },
  { to: '/enterprise', label: 'Enterprise' },
  { to: '/changelog', label: 'Changelog' },
  { to: '/community', label: 'Community' },
];

export default function Footer() {
  return (
    <footer className="footer-shell">
      <div className="footer-grid">
        <div className="holo-panel">
          <div className="panel-kicker">Command Footer</div>
          <div className="panel-title" style={{ fontSize: '1.35rem', marginBottom: 10 }}>NEXUS OS</div>
          <p className="panel-copy">
            GOVERNED AI AGENT OPERATING SYSTEM. LOCAL-FIRST. AIR-GAPPABLE. TAMPER-EVIDENT.
          </p>
          <div className="footer-tags">
            <span className="status-pill">{VERSION}</span>
            <span className="status-pill">MIT LICENSE</span>
          </div>
        </div>

        <div className="holo-panel">
          <div className="panel-kicker">Navigation</div>
          <div className="footer-links-grid">
            {pageLinks.map((link) => (
              <Link key={link.to} to={link.to} className="footer-link">
                {link.label}
              </Link>
            ))}
          </div>
        </div>

        <div className="holo-panel">
          <div className="panel-kicker">Live Telemetry</div>
          <div className="metric-grid compact">
            <div className="metric-card compact">
              <span className="stat-label">Agents</span>
              <span className="stat-value">{STATS.agents}</span>
            </div>
            <div className="metric-card compact">
              <span className="stat-label">Tests</span>
              <span className="stat-value">{STATS.tests}</span>
            </div>
            <div className="metric-card compact">
              <span className="stat-label">Commits</span>
              <span className="stat-value">{COMMIT_COUNT}</span>
            </div>
            <div className="metric-card compact">
              <span className="stat-label">Crash Sites</span>
              <span className="stat-value">0</span>
            </div>
          </div>
        </div>

        <div className="holo-panel">
          <div className="panel-kicker">External Links</div>
          <div className="footer-links-stack">
            <a href={GITLAB_PROJECT.web_url} target="_blank" rel="noopener noreferrer" className="footer-link">Repository</a>
            <a href={`${GITLAB_PROJECT.web_url}/-/releases`} target="_blank" rel="noopener noreferrer" className="footer-link">Releases</a>
            <a href={`${GITLAB_PROJECT.web_url}/-/issues`} target="_blank" rel="noopener noreferrer" className="footer-link">Issue Tracker</a>
            <a href="/docs" className="footer-link">Technical Database</a>
          </div>
        </div>
      </div>

      <div className="footer-bottom">
        <span>DON'T TRUST. VERIFY.</span>
        <span>NEXUS-OS.DEV</span>
      </div>
    </footer>
  );
}

import { useState, useEffect } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { GITLAB_PROJECT, VERSION } from '../data/constants';

const navLinks = [
  { to: '/', label: 'Home' },
  { to: '/features', label: 'Features' },
  { to: '/architecture', label: 'Architecture' },
  { to: '/agents', label: 'Agents' },
  { to: '/comparison', label: 'Compare' },
  { to: '/docs', label: 'Docs' },
  { to: '/roadmap', label: 'Roadmap' },
];

export default function Navbar() {
  const [scrolled, setScrolled] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const location = useLocation();

  useEffect(() => {
    const handleScroll = () => setScrolled(window.scrollY > 50);
    window.addEventListener('scroll', handleScroll, { passive: true });
    return () => window.removeEventListener('scroll', handleScroll);
  }, []);

  return (
    <>
      <nav className={`nav-shell ${scrolled ? 'is-scrolled' : ''}`}>
        <div className="nav-inner">
          <div className="nav-brand-block">
            <Link to="/" className="nav-brand">
              <span className="nav-wordmark">NEXUS OS</span>
              <span className="nav-version">{VERSION}</span>
            </Link>
            <div className="nav-dossier desktop-nav">
              <span>CLASSIFIED</span>
              <span>53 AGENTS</span>
              <span>GOVERNANCE ACTIVE</span>
            </div>
          </div>

          <div className="nav-links desktop-nav">
            {navLinks.map((link) => {
              const isActive = location.pathname === link.to;
              return (
                <Link key={link.to} to={link.to} className={`nav-link ${isActive ? 'is-active' : ''}`}>
                  {link.label}
                </Link>
              );
            })}
          </div>

          <div className="nav-actions">
            <a
              href={`${GITLAB_PROJECT.web_url}/-/releases`}
              target="_blank"
              rel="noopener noreferrer"
              className="nexus-button desktop-nav"
            >
              Download
            </a>
            <button
              onClick={() => setMobileOpen((open) => !open)}
              className="mobile-toggle"
              aria-label="Toggle menu"
            >
              <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
                {mobileOpen ? (
                  <path d="M6 6l12 12M6 18L18 6" strokeLinecap="round" />
                ) : (
                  <>
                    <line x1="3" y1="6" x2="21" y2="6" />
                    <line x1="3" y1="12" x2="21" y2="12" />
                    <line x1="3" y1="18" x2="21" y2="18" />
                  </>
                )}
              </svg>
            </button>
          </div>
        </div>
      </nav>

      {mobileOpen && (
        <div className="mobile-menu">
          <div className="mobile-menu-panel holo-panel">
            <div className="mobile-menu-header">
              <span>TACTICAL MENU</span>
              <span>{VERSION}</span>
            </div>
            <div className="mobile-menu-links">
              {navLinks.map((link, index) => (
                <Link
                  key={link.to}
                  to={link.to}
                  className={`mobile-nav-link ${location.pathname === link.to ? 'is-active' : ''}`}
                  onClick={() => setMobileOpen(false)}
                  style={{ animationDelay: `${index * 0.04}s` }}
                >
                  {link.label}
                </Link>
              ))}
            </div>
            <a
              href={`${GITLAB_PROJECT.web_url}/-/releases`}
              target="_blank"
              rel="noopener noreferrer"
              className="nexus-button nexus-button-primary"
              onClick={() => setMobileOpen(false)}
            >
              Download {VERSION}
            </a>
          </div>
        </div>
      )}
    </>
  );
}

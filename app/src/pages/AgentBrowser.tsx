import { useCallback, useEffect, useRef, useState } from "react";
import { BrowserToolbar } from "../components/browser/BrowserToolbar";
import { ResearchMode } from "../components/browser/ResearchMode";
import { BuildMode } from "../components/browser/BuildMode";
import { LearnMode } from "../components/browser/LearnMode";
import { hasDesktopRuntime, navigateTo } from "../api/backend";
import type { ActivityMessage, BrowserMode } from "../types";
import "./agent-browser.css";

function makeId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.floor(Math.random() * 100_000)}`;
}

interface HistoryEntry {
  url: string;
  timestamp: number;
  agent: string;
}

interface GovernanceStats {
  domainsBlocked: number;
  piiRedactions: number;
  fuelConsumed: number;
  auditEvents: number;
}

export function AgentBrowser(): JSX.Element {
  const [url, setUrl] = useState("");
  const [mode, setMode] = useState<BrowserMode>("research");
  const [loading, setLoading] = useState(false);
  const [history, setHistory] = useState<string[]>([]);
  const [historyIdx, setHistoryIdx] = useState(-1);
  const [activities, setActivities] = useState<ActivityMessage[]>([]);
  const [blocked, setBlocked] = useState<string | null>(null);
  const [iframeSrc, setIframeSrc] = useState<string | null>(null);

  // History dropdown
  const [historyLog, setHistoryLog] = useState<HistoryEntry[]>([]);
  const [showHistory, setShowHistory] = useState(false);

  // Governance sidebar
  const [showGovernance, setShowGovernance] = useState(false);
  const [govStats, setGovStats] = useState<GovernanceStats>({
    domainsBlocked: 0,
    piiRedactions: 0,
    fuelConsumed: 0,
    auditEvents: 0,
  });

  const urlInputRef = useRef<HTMLInputElement>(null);

  const addActivity = useCallback(
    (type: ActivityMessage["message_type"], content: string, agentName = "Browser") => {
      setActivities((prev) => [
        ...prev,
        {
          id: makeId(),
          timestamp: Date.now(),
          agent_id: "browser-agent",
          agent_name: agentName,
          message_type: type,
          content,
        },
      ]);
      // Track governance stats
      setGovStats((prev) => ({
        ...prev,
        auditEvents: prev.auditEvents + 1,
        domainsBlocked: prev.domainsBlocked + (type === "blocked" ? 1 : 0),
        fuelConsumed: prev.fuelConsumed + 25,
      }));
    },
    []
  );

  const handleNavigate = useCallback(
    async (target: string) => {
      setLoading(true);
      setBlocked(null);

      addActivity("navigating", `Requesting: ${target}`);

      if (hasDesktopRuntime()) {
        try {
          const result = await navigateTo(target);
          if (!result.allowed) {
            setBlocked(result.deny_reason ?? "URL blocked by egress policy");
            addActivity("blocked", `Denied: ${result.deny_reason ?? "egress policy"}`);
            setLoading(false);
            return;
          }
          addActivity("info", `Loaded: ${result.title || result.url}`);
        } catch (err) {
          addActivity("blocked", `Error: ${String(err)}`);
          setBlocked(String(err));
          setLoading(false);
          return;
        }
      } else {
        // Mock mode — simulate governance check
        addActivity("deciding", "Checking egress governance policy...", "Firewall");
        await new Promise((r) => setTimeout(r, 300));
        addActivity("info", `Page loaded: ${target}`);
      }

      setUrl(target);
      setIframeSrc(target);
      setHistory((prev) => {
        const next = [...prev.slice(0, historyIdx + 1), target];
        setHistoryIdx(next.length - 1);
        return next;
      });
      setHistoryLog((prev) => [
        { url: target, timestamp: Date.now(), agent: "Browser" },
        ...prev,
      ]);
      setLoading(false);
    },
    [addActivity, historyIdx]
  );

  const handleBack = useCallback(() => {
    if (historyIdx <= 0) return;
    const prev = history[historyIdx - 1];
    setHistoryIdx(historyIdx - 1);
    setUrl(prev);
    setIframeSrc(prev);
    setBlocked(null);
    addActivity("navigating", `Back to: ${prev}`);
  }, [history, historyIdx, addActivity]);

  const handleForward = useCallback(() => {
    if (historyIdx >= history.length - 1) return;
    const next = history[historyIdx + 1];
    setHistoryIdx(historyIdx + 1);
    setUrl(next);
    setIframeSrc(next);
    setBlocked(null);
    addActivity("navigating", `Forward to: ${next}`);
  }, [history, historyIdx, addActivity]);

  const handleRefresh = useCallback(() => {
    if (iframeSrc) {
      addActivity("navigating", `Refreshing: ${iframeSrc}`);
      const src = iframeSrc;
      setIframeSrc(null);
      requestAnimationFrame(() => setIframeSrc(src));
    }
  }, [iframeSrc, addActivity]);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Ctrl+L — focus URL bar
      if (e.ctrlKey && e.key === "l") {
        e.preventDefault();
        urlInputRef.current?.focus();
        urlInputRef.current?.select();
      }
      // Ctrl+1/2/3 — switch modes
      if (e.ctrlKey && e.key === "1") {
        e.preventDefault();
        setMode("research");
      }
      if (e.ctrlKey && e.key === "2") {
        e.preventDefault();
        setMode("build");
      }
      if (e.ctrlKey && e.key === "3") {
        e.preventDefault();
        setMode("learn");
      }
      // Ctrl+R — refresh
      if (e.ctrlKey && e.key === "r") {
        e.preventDefault();
        handleRefresh();
      }
      // Ctrl+H — toggle history
      if (e.ctrlKey && e.key === "h") {
        e.preventDefault();
        setShowHistory((p) => !p);
      }
      // Ctrl+G — toggle governance
      if (e.ctrlKey && e.key === "g") {
        e.preventDefault();
        setShowGovernance((p) => !p);
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [handleRefresh]);

  function formatHistoryTime(ts: number): string {
    return new Date(ts).toLocaleTimeString(undefined, {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }

  return (
    <div className="agent-browser-root">
      <BrowserToolbar
        url={url}
        mode={mode}
        loading={loading}
        canGoBack={historyIdx > 0}
        canGoForward={historyIdx < history.length - 1}
        onNavigate={(u) => {
          void handleNavigate(u);
        }}
        onBack={handleBack}
        onForward={handleForward}
        onRefresh={handleRefresh}
        onModeChange={setMode}
        urlInputRef={urlInputRef}
        onToggleHistory={() => setShowHistory((p) => !p)}
        onToggleGovernance={() => setShowGovernance((p) => !p)}
        showGovernance={showGovernance}
      />

      {blocked && (
        <div className="browser-blocked-banner">
          {blocked}
        </div>
      )}

      {/* History dropdown */}
      {showHistory && (
        <div className="browser-history-dropdown">
          <div className="browser-history-header">
            <span className="browser-history-title">Browsing History</span>
            <button
              className="browser-history-close"
              onClick={() => setShowHistory(false)}
            >
              x
            </button>
          </div>
          {historyLog.length === 0 ? (
            <div className="browser-history-empty">No pages visited yet</div>
          ) : (
            <div className="browser-history-list">
              {historyLog.map((entry, i) => (
                <button
                  key={i}
                  className="browser-history-item"
                  onClick={() => {
                    setShowHistory(false);
                    void handleNavigate(entry.url);
                  }}
                >
                  <span className="browser-history-item-url">
                    {entry.url.replace(/^https?:\/\//, "").slice(0, 60)}
                  </span>
                  <span className="browser-history-item-meta">
                    <span className="browser-history-item-agent">{entry.agent}</span>
                    <span className="browser-history-item-time">
                      {formatHistoryTime(entry.timestamp)}
                    </span>
                  </span>
                </button>
              ))}
            </div>
          )}
        </div>
      )}

      <div className="browser-main-area">
        {/* Governance sidebar */}
        {showGovernance && (
          <div className="governance-sidebar">
            <div className="governance-sidebar-header">
              <span className="governance-sidebar-title">Governance</span>
            </div>
            <div className="governance-stat-grid">
              <div className="governance-stat">
                <span className="governance-stat-value governance-stat--blocked">
                  {govStats.domainsBlocked}
                </span>
                <span className="governance-stat-label">Domains Blocked</span>
              </div>
              <div className="governance-stat">
                <span className="governance-stat-value governance-stat--pii">
                  {govStats.piiRedactions}
                </span>
                <span className="governance-stat-label">PII Redactions</span>
              </div>
              <div className="governance-stat">
                <span className="governance-stat-value governance-stat--fuel">
                  {govStats.fuelConsumed}
                </span>
                <span className="governance-stat-label">Fuel Consumed</span>
              </div>
              <div className="governance-stat">
                <span className="governance-stat-value governance-stat--audit">
                  {govStats.auditEvents}
                </span>
                <span className="governance-stat-label">Audit Events</span>
              </div>
            </div>
            <div className="governance-info">
              <div className="governance-info-item">
                <span className="governance-info-dot governance-info-dot--green" />
                Egress policy active
              </div>
              <div className="governance-info-item">
                <span className="governance-info-dot governance-info-dot--green" />
                PII firewall enabled
              </div>
              <div className="governance-info-item">
                <span className="governance-info-dot governance-info-dot--green" />
                Audit chain verified
              </div>
              <div className="governance-info-item">
                <span className="governance-info-dot governance-info-dot--green" />
                Fuel metering on
              </div>
            </div>
            <div className="governance-shortcuts">
              <div className="governance-shortcut">
                <kbd>Ctrl+L</kbd> Focus URL
              </div>
              <div className="governance-shortcut">
                <kbd>Ctrl+1/2/3</kbd> Switch mode
              </div>
              <div className="governance-shortcut">
                <kbd>Ctrl+R</kbd> Refresh
              </div>
              <div className="governance-shortcut">
                <kbd>Ctrl+H</kbd> History
              </div>
              <div className="governance-shortcut">
                <kbd>Ctrl+G</kbd> Governance
              </div>
            </div>
          </div>
        )}

        {/* Mode panels with transitions */}
        <div className="browser-mode-content">
          <div
            className={`browser-mode-panel ${mode === "research" ? "browser-mode-panel--active" : ""}`}
          >
            {mode === "research" && (
              <ResearchMode
                activities={activities}
                onActivity={addActivity}
                iframeSrc={iframeSrc}
                onIframeSrc={setIframeSrc}
              />
            )}
          </div>
          <div
            className={`browser-mode-panel ${mode === "build" ? "browser-mode-panel--active" : ""}`}
          >
            {mode === "build" && <BuildMode onActivity={addActivity} />}
          </div>
          <div
            className={`browser-mode-panel ${mode === "learn" ? "browser-mode-panel--active" : ""}`}
          >
            {mode === "learn" && <LearnMode onActivity={addActivity} />}
          </div>
        </div>
      </div>
    </div>
  );
}

export default AgentBrowser;

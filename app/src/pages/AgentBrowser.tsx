import { useCallback, useRef, useState } from "react";
import { BrowserToolbar } from "../components/browser/BrowserToolbar";
import { ActivityStream } from "../components/browser/ActivityStream";
import { ResearchMode } from "../components/browser/ResearchMode";
import { hasDesktopRuntime, navigateTo } from "../api/backend";
import type { ActivityMessage, BrowserMode } from "../types";
import "./agent-browser.css";

function makeId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.floor(Math.random() * 100_000)}`;
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

  const iframeRef = useRef<HTMLIFrameElement>(null);
  const dividerRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [activityWidth, setActivityWidth] = useState(40); // percent

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
      // Force iframe reload by toggling src
      const src = iframeSrc;
      setIframeSrc(null);
      requestAnimationFrame(() => setIframeSrc(src));
    }
  }, [iframeSrc, addActivity]);

  // Resizable divider logic
  const handleDividerMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const container = containerRef.current;
      if (!container) return;

      const startX = e.clientX;
      const startWidth = activityWidth;
      const containerWidth = container.getBoundingClientRect().width;

      const onMove = (me: MouseEvent) => {
        const delta = me.clientX - startX;
        const pct = startWidth + (delta / containerWidth) * 100;
        setActivityWidth(Math.max(15, Math.min(65, pct)));
      };

      const onUp = () => {
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
      };

      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
    },
    [activityWidth]
  );

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
      />

      {blocked && (
        <div className="browser-blocked-banner">
          ⛔ {blocked}
        </div>
      )}

      {mode === "research" ? (
        <ResearchMode
          activities={activities}
          onActivity={addActivity}
          iframeSrc={iframeSrc}
          onIframeSrc={setIframeSrc}
        />
      ) : (
        <div className="browser-split-container" ref={containerRef}>
          <div
            className="browser-activity-panel"
            style={{ width: `${activityWidth}%` }}
          >
            <ActivityStream messages={activities} />
          </div>

          <div
            className="browser-resize-handle"
            ref={dividerRef}
            onMouseDown={handleDividerMouseDown}
          />

          <div className="browser-view-panel">
            <div className="browser-iframe-container">
              {iframeSrc && !blocked ? (
                <iframe
                  ref={iframeRef}
                  className="browser-iframe"
                  src={iframeSrc}
                  title="Agent Browser"
                  sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
                  onLoad={() => setLoading(false)}
                />
              ) : (
                <div className="browser-placeholder">
                  <span className="browser-placeholder-icon">⌁</span>
                  <span className="browser-placeholder-text">Agent Browser</span>
                  <span className="browser-placeholder-hint">
                    Enter a URL above to begin browsing
                  </span>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default AgentBrowser;

import { useCallback, useState, type KeyboardEvent } from "react";
import type { BrowserMode } from "../../types";

interface BrowserToolbarProps {
  url: string;
  mode: BrowserMode;
  loading: boolean;
  canGoBack: boolean;
  canGoForward: boolean;
  onNavigate: (url: string) => void;
  onBack: () => void;
  onForward: () => void;
  onRefresh: () => void;
  onModeChange: (mode: BrowserMode) => void;
}

const MODE_LABELS: Record<BrowserMode, string> = {
  research: "Research",
  build: "Build",
  learn: "Learn",
};

export function BrowserToolbar({
  url,
  mode,
  loading,
  canGoBack,
  canGoForward,
  onNavigate,
  onBack,
  onForward,
  onRefresh,
  onModeChange,
}: BrowserToolbarProps): JSX.Element {
  const [draft, setDraft] = useState(url);

  // Sync draft when external url changes
  const [prevUrl, setPrevUrl] = useState(url);
  if (url !== prevUrl) {
    setDraft(url);
    setPrevUrl(url);
  }

  const handleSubmit = useCallback(() => {
    let target = draft.trim();
    if (!target) return;
    if (!/^https?:\/\//i.test(target)) {
      target = `https://${target}`;
    }
    onNavigate(target);
  }, [draft, onNavigate]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") {
        e.preventDefault();
        handleSubmit();
      }
    },
    [handleSubmit]
  );

  return (
    <div className="browser-toolbar">
      {/* Mode tabs */}
      <div className="browser-mode-tabs">
        {(Object.keys(MODE_LABELS) as BrowserMode[]).map((m) => (
          <button
            key={m}
            type="button"
            className={`browser-mode-tab ${mode === m ? "active" : ""}`}
            onClick={() => onModeChange(m)}
          >
            {MODE_LABELS[m]}
          </button>
        ))}
      </div>

      {/* Navigation controls */}
      <div className="browser-nav-row">
        <button
          type="button"
          className="browser-nav-btn"
          disabled={!canGoBack}
          onClick={onBack}
          title="Back"
        >
          ◁
        </button>
        <button
          type="button"
          className="browser-nav-btn"
          disabled={!canGoForward}
          onClick={onForward}
          title="Forward"
        >
          ▷
        </button>
        <button
          type="button"
          className="browser-nav-btn"
          onClick={onRefresh}
          title="Refresh"
        >
          {loading ? "◌" : "↻"}
        </button>

        <div className="browser-url-bar">
          {loading && <span className="browser-loading-indicator" />}
          <input
            type="text"
            className="browser-url-input"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Enter URL..."
            spellCheck={false}
          />
          <button
            type="button"
            className="browser-go-btn"
            onClick={handleSubmit}
            title="Navigate"
          >
            →
          </button>
        </div>
      </div>
    </div>
  );
}

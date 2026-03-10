import { useCallback, useState, type KeyboardEvent, type Ref } from "react";
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
  urlInputRef?: Ref<HTMLInputElement>;
  onToggleHistory?: () => void;
  onToggleGovernance?: () => void;
  showGovernance?: boolean;
}

const MODE_LABELS: Record<BrowserMode, { label: string; shortcut: string }> = {
  research: { label: "Research", shortcut: "1" },
  build: { label: "Build", shortcut: "2" },
  learn: { label: "Learn", shortcut: "3" },
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
  urlInputRef,
  onToggleHistory,
  onToggleGovernance,
  showGovernance,
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
            title={`Ctrl+${MODE_LABELS[m].shortcut}`}
          >
            {MODE_LABELS[m].label}
          </button>
        ))}

        <div className="browser-toolbar-spacer" />

        {/* History toggle */}
        {onToggleHistory && (
          <button
            type="button"
            className="browser-toolbar-icon-btn"
            onClick={onToggleHistory}
            title="History (Ctrl+H)"
          >
{"\u23F2"}
          </button>
        )}

        {/* Governance toggle */}
        {onToggleGovernance && (
          <button
            type="button"
            className={`browser-toolbar-icon-btn ${showGovernance ? "browser-toolbar-icon-btn--active" : ""}`}
            onClick={onToggleGovernance}
            title="Governance (Ctrl+G)"
          >
            {"\u2694"}
          </button>
        )}
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
          {"\u25C1"}
        </button>
        <button
          type="button"
          className="browser-nav-btn"
          disabled={!canGoForward}
          onClick={onForward}
          title="Forward"
        >
          {"\u25B7"}
        </button>
        <button
          type="button"
          className="browser-nav-btn"
          onClick={onRefresh}
          title="Refresh (Ctrl+R)"
        >
          {loading ? "\u25CC" : "\u21BB"}
        </button>

        <div className="browser-url-bar">
          {loading && <span className="browser-loading-indicator" />}
          <input
            ref={urlInputRef}
            type="text"
            className="browser-url-input"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Enter URL... (Ctrl+L to focus)"
            spellCheck={false}
          />
          <button
            type="button"
            className="browser-go-btn"
            onClick={handleSubmit}
            title="Navigate"
          >
            {"\u2192"}
          </button>
        </div>
      </div>
    </div>
  );
}

import { useCallback, useEffect, useRef, useState } from "react";
import {
  hasDesktopRuntime,
  startLearning,
  learningAgentAction,
} from "../../api/backend";
import { KnowledgeCard } from "./KnowledgeCard";
import type {
  ActivityMessage,
  KnowledgeEntry,
  LearningSessionState,
  LearningSource,
  LearningSuggestion,
} from "../../types";

function makeId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.floor(Math.random() * 100_000)}`;
}

function delay(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

const DEFAULT_SOURCES: LearningSource[] = [
  { url: "https://docs.rust-lang.org/stable/reference/", label: "Rust Reference", category: "documentation" },
  { url: "https://github.com/nickel-org/nickel.rs/releases", label: "Nickel.rs Releases", category: "github" },
  { url: "https://blog.rust-lang.org/", label: "Rust Blog", category: "blog" },
  { url: "https://doc.rust-lang.org/cargo/reference/", label: "Cargo Reference", category: "documentation" },
  { url: "https://github.com/nickel-org/nickel.rs/blob/master/CHANGELOG.md", label: "Nickel Changelog", category: "changelog" },
  { url: "https://web.dev/blog/", label: "web.dev Blog", category: "blog" },
  { url: "https://react.dev/reference/react", label: "React Docs", category: "documentation" },
  { url: "https://tauri.app/blog", label: "Tauri Blog", category: "blog" },
];

const CATEGORY_ICONS: Record<string, string> = {
  documentation: "📖",
  github: "🔗",
  blog: "📝",
  changelog: "📋",
};

const MOCK_KNOWLEDGE: KnowledgeEntry[] = [
  {
    id: "mock-1",
    title: "Rust 1.78 Stable: Diagnostic Attributes",
    source_url: "https://blog.rust-lang.org/",
    key_points: [
      "#[diagnostic::on_unimplemented] now stable for better error messages",
      "Pattern types RFC accepted for future editions",
      "std::io improvements for Windows named pipes",
    ],
    timestamp: Date.now() - 120_000,
    relevance_score: 0.92,
    category: "blog",
    is_new: true,
    change_summary: "New stable Rust release with diagnostic improvements",
  },
  {
    id: "mock-2",
    title: "React 19 Server Components",
    source_url: "https://react.dev/reference/react",
    key_points: [
      "Server Components now stable — zero client JS by default",
      "New use() hook for reading promises and context in render",
      "Actions API for form mutations with optimistic updates",
    ],
    timestamp: Date.now() - 300_000,
    relevance_score: 0.85,
    category: "documentation",
    is_new: true,
    change_summary: "Major React paradigm shift — server-first rendering",
  },
  {
    id: "mock-3",
    title: "Tauri 2.0 Mobile Support",
    source_url: "https://tauri.app/blog",
    key_points: [
      "iOS and Android builds now supported via tauri-mobile",
      "Unified plugin system across desktop and mobile",
      "New permission model for mobile-specific capabilities",
    ],
    timestamp: Date.now() - 600_000,
    relevance_score: 0.78,
    category: "blog",
    is_new: false,
    change_summary: null,
  },
];

const MOCK_SUGGESTIONS: LearningSuggestion[] = [
  {
    id: "sug-1",
    title: "Migrate to diagnostic attributes",
    description: "Nexus OS error types could use #[diagnostic::on_unimplemented] for better developer experience",
    source_url: "https://blog.rust-lang.org/",
    relevance: "high",
    timestamp: Date.now() - 60_000,
  },
  {
    id: "sug-2",
    title: "Evaluate Tauri 2.0 mobile plugins",
    description: "Mobile deployment could leverage the new Tauri mobile plugin system for Android/iOS agent control",
    source_url: "https://tauri.app/blog",
    relevance: "medium",
    timestamp: Date.now() - 180_000,
  },
];

interface LearnModeProps {
  onActivity: (
    type: ActivityMessage["message_type"],
    content: string,
    agentName?: string,
  ) => void;
}

type LearnPanel = "browser" | "feed" | "knowledge";

export function LearnMode({ onActivity }: LearnModeProps): JSX.Element {
  const [session, setSession] = useState<LearningSessionState | null>(null);
  const [sources, setSources] = useState<LearningSource[]>(DEFAULT_SOURCES);
  const [running, setRunning] = useState(false);
  const [currentUrl, setCurrentUrl] = useState<string | null>(null);
  const [feedMessages, setFeedMessages] = useState<Array<{ id: string; time: number; type: string; text: string }>>([]);
  const [knowledge, setKnowledge] = useState<KnowledgeEntry[]>([]);
  const [suggestions, setSuggestions] = useState<LearningSuggestion[]>([]);
  const [activePanel, setActivePanel] = useState<LearnPanel>("browser");
  const [fuelUsed, setFuelUsed] = useState(0);
  const [pagesVisited, setPagesVisited] = useState(0);

  const feedRef = useRef<HTMLDivElement>(null);
  const runningRef = useRef(false);

  const addFeed = useCallback((type: string, text: string) => {
    setFeedMessages((prev) => [
      ...prev,
      { id: makeId(), time: Date.now(), type, text },
    ]);
  }, []);

  // Auto-scroll feed
  useEffect(() => {
    if (feedRef.current) {
      feedRef.current.scrollTop = feedRef.current.scrollHeight;
    }
  }, [feedMessages]);

  const runMockLearning = useCallback(async () => {
    runningRef.current = true;
    setRunning(true);
    setFeedMessages([]);
    setKnowledge([]);
    setSuggestions([]);
    setFuelUsed(0);
    setPagesVisited(0);

    const mockSessionId = makeId();
    addFeed("info", "Learning session started — scanning sources...");
    onActivity("info", "Learning session started", "LearnAgent");

    for (let i = 0; i < sources.length && runningRef.current; i++) {
      const src = sources[i];

      // Step 1: Navigate to source
      addFeed("navigating", `Browsing: ${src.label}`);
      onActivity("navigating", `Browsing: ${src.url}`, "LearnAgent");
      setCurrentUrl(src.url);
      setPagesVisited((p) => p + 1);
      setFuelUsed((f) => f + 25);
      await delay(800 + Math.random() * 600);

      if (!runningRef.current) break;

      // Step 2: Extract key information
      addFeed("extracting", `Extracting key information from ${src.label}...`);
      onActivity("extracting", `Reading ${src.label}`, "LearnAgent");
      setFuelUsed((f) => f + 50);
      await delay(1000 + Math.random() * 800);

      if (!runningRef.current) break;

      // Step 3: Compare with existing knowledge
      addFeed("deciding", `Comparing with existing knowledge base...`);
      await delay(500 + Math.random() * 400);

      if (!runningRef.current) break;

      // Step 4: Add knowledge entry if we have mock data for this index
      if (i < MOCK_KNOWLEDGE.length) {
        const entry = { ...MOCK_KNOWLEDGE[i], id: makeId(), timestamp: Date.now() };
        setKnowledge((prev) => [...prev, entry]);
        addFeed("info", `Knowledge updated: ${entry.title}`);
        onActivity("info", `Learned: ${entry.title}`, "LearnAgent");

        if (entry.change_summary) {
          addFeed("merging", `Change detected: ${entry.change_summary}`);
        }

        entry.key_points.forEach((pt) => {
          addFeed("extracting", `  • ${pt}`);
        });
      } else {
        // Generate a generic entry for remaining sources
        const entry: KnowledgeEntry = {
          id: makeId(),
          title: `${src.label} — Latest Updates`,
          source_url: src.url,
          key_points: [
            `Reviewed ${src.label} for recent changes`,
            "No significant updates since last check",
          ],
          timestamp: Date.now(),
          relevance_score: 0.3 + Math.random() * 0.3,
          category: src.category,
          is_new: false,
          change_summary: null,
        };
        setKnowledge((prev) => [...prev, entry]);
        addFeed("info", `Checked: ${src.label} — no major changes`);
      }

      setFuelUsed((f) => f + 50);
      await delay(400);
    }

    if (runningRef.current) {
      // Add suggestions at the end
      setSuggestions(MOCK_SUGGESTIONS.map((s) => ({ ...s, id: makeId(), timestamp: Date.now() })));
      addFeed("info", `Learning complete — ${sources.length} sources scanned, ${MOCK_SUGGESTIONS.length} improvement suggestions`);
      onActivity("info", "Learning session complete", "LearnAgent");
    }

    setCurrentUrl(null);
    setRunning(false);
    runningRef.current = false;
  }, [sources, addFeed, onActivity]);

  const runDesktopLearning = useCallback(async () => {
    runningRef.current = true;
    setRunning(true);
    setFeedMessages([]);
    setKnowledge([]);
    setSuggestions([]);
    setFuelUsed(0);
    setPagesVisited(0);

    try {
      const sess = await startLearning(sources);
      setSession(sess);
      addFeed("info", `Learning session ${sess.session_id.slice(0, 8)} started`);
      onActivity("info", "Learning session started", "LearnAgent");

      for (let i = 0; i < sources.length && runningRef.current; i++) {
        const src = sources[i];

        // Browse
        setCurrentUrl(src.url);
        addFeed("navigating", `Browsing: ${src.label}`);
        const browseResult = await learningAgentAction(sess.session_id, "browse", src.url);
        setSession(browseResult);
        setFuelUsed(browseResult.fuel_used);
        setPagesVisited(browseResult.pages_visited);

        if (!runningRef.current) break;

        // Extract
        addFeed("extracting", `Extracting from ${src.label}...`);
        const extractResult = await learningAgentAction(sess.session_id, "extract", src.url);
        setSession(extractResult);
        setFuelUsed(extractResult.fuel_used);
        setKnowledge(extractResult.knowledge_base);

        if (!runningRef.current) break;

        // Compare
        addFeed("deciding", `Comparing with knowledge base...`);
        const compareResult = await learningAgentAction(sess.session_id, "compare", src.url);
        setSession(compareResult);
        setKnowledge(compareResult.knowledge_base);
        setSuggestions(compareResult.suggestions);

        const latest = compareResult.knowledge_base[compareResult.knowledge_base.length - 1];
        if (latest) {
          addFeed("info", `Learned: ${latest.title}`);
          latest.key_points.forEach((pt) => addFeed("extracting", `  • ${pt}`));
        }
      }

      addFeed("info", "Learning session complete");
      onActivity("info", "Learning session complete", "LearnAgent");
    } catch (err) {
      addFeed("info", `Error: ${String(err)}`);
      onActivity("blocked", `Learning error: ${String(err)}`, "LearnAgent");
    }

    setCurrentUrl(null);
    setRunning(false);
    runningRef.current = false;
  }, [sources, addFeed, onActivity]);

  const handleStart = useCallback(() => {
    if (hasDesktopRuntime()) {
      void runDesktopLearning();
    } else {
      void runMockLearning();
    }
  }, [runDesktopLearning, runMockLearning]);

  const handleStop = useCallback(() => {
    runningRef.current = false;
    setRunning(false);
    addFeed("info", "Learning session stopped by user");
    onActivity("info", "Learning stopped", "LearnAgent");
  }, [addFeed, onActivity]);

  const handleRemoveSource = useCallback((idx: number) => {
    setSources((prev) => prev.filter((_, i) => i !== idx));
  }, []);

  const handleAddSource = useCallback(() => {
    const url = prompt("Enter documentation URL:");
    if (!url) return;
    const label = prompt("Label for this source:") || url;
    setSources((prev) => [...prev, { url, label, category: "documentation" }]);
  }, []);

  const feedTypeColor: Record<string, string> = {
    navigating: "var(--nexus-accent)",
    extracting: "var(--nexus-accent)",
    deciding: "#a78bfa",
    merging: "#f472b6",
    info: "#94a3b8",
  };

  return (
    <div className="learn-mode-root">
      {/* Top controls */}
      <div className="learn-controls">
        <div className="learn-controls-left">
          <button
            className={`learn-btn ${running ? "learn-btn--stop" : "learn-btn--start"}`}
            onClick={running ? handleStop : handleStart}
          >
            {running ? "⏹ Stop" : "▶ Start Learning"}
          </button>
          <span className="learn-status-text">
            {running ? "Agent is learning..." : `${sources.length} sources configured`}
          </span>
        </div>
        <div className="learn-controls-right">
          <span className="learn-stat">⚡ {fuelUsed} fuel</span>
          <span className="learn-stat">📄 {pagesVisited} pages</span>
          <span className="learn-stat">🧠 {knowledge.length} entries</span>
        </div>
      </div>

      {/* Panel tabs */}
      <div className="learn-panel-tabs">
        {(["browser", "feed", "knowledge"] as LearnPanel[]).map((p) => (
          <button
            key={p}
            className={`learn-panel-tab${activePanel === p ? " learn-panel-tab--active" : ""}`}
            onClick={() => setActivePanel(p)}
          >
            {p === "browser" ? "🌐 Browser" : p === "feed" ? "📡 Feed" : "🧠 Knowledge"}
            {p === "knowledge" && knowledge.length > 0 && (
              <span className="learn-panel-tab-count">{knowledge.length}</span>
            )}
            {p === "feed" && feedMessages.length > 0 && (
              <span className="learn-panel-tab-count">{feedMessages.length}</span>
            )}
          </button>
        ))}
      </div>

      {/* Panel content */}
      <div className="learn-panels">
        {activePanel === "browser" && (
          <div className="learn-browser-panel">
            <div className="learn-sources-list">
              <div className="learn-sources-header">
                <span className="learn-sources-title">Learning Sources</span>
                <button className="learn-sources-add" onClick={handleAddSource}>
                  + Add
                </button>
              </div>
              {sources.map((src, i) => (
                <div
                  key={i}
                  className={`learn-source-item${currentUrl === src.url ? " learn-source-item--active" : ""}`}
                >
                  <span className="learn-source-icon">
                    {CATEGORY_ICONS[src.category] || "📄"}
                  </span>
                  <div className="learn-source-info">
                    <span className="learn-source-label">{src.label}</span>
                    <span className="learn-source-url">{src.url}</span>
                  </div>
                  {currentUrl === src.url && (
                    <span className="learn-source-reading">Reading...</span>
                  )}
                  {!running && (
                    <button
                      className="learn-source-remove"
                      onClick={() => handleRemoveSource(i)}
                      title="Remove source"
                    >
                      ×
                    </button>
                  )}
                </div>
              ))}
            </div>

            <div className="learn-browser-view">
              {currentUrl ? (
                <div className="learn-browser-active">
                  <div className="learn-browser-urlbar">
                    <span className="learn-browser-urlbar-icon">🔒</span>
                    <span className="learn-browser-urlbar-url">{currentUrl}</span>
                  </div>
                  <div className="learn-browser-content">
                    <div className="learn-browser-scanning">
                      <div className="learn-scanning-pulse" />
                      <span>Agent is reading this page...</span>
                    </div>
                  </div>
                </div>
              ) : (
                <div className="learn-browser-idle">
                  <span className="learn-browser-idle-icon">⌁</span>
                  <span className="learn-browser-idle-text">
                    {running ? "Switching sources..." : "Start a learning session to begin"}
                  </span>
                </div>
              )}
            </div>
          </div>
        )}

        {activePanel === "feed" && (
          <div className="learn-feed-panel" ref={feedRef}>
            {feedMessages.length === 0 ? (
              <div className="learn-feed-empty">
                Start a learning session to see real-time takeaways
              </div>
            ) : (
              feedMessages.map((msg) => (
                <div key={msg.id} className="learn-feed-item">
                  <span
                    className="learn-feed-dot"
                    style={{ background: feedTypeColor[msg.type] || "#64748b" }}
                  />
                  <span className="learn-feed-text">{msg.text}</span>
                </div>
              ))
            )}
          </div>
        )}

        {activePanel === "knowledge" && (
          <div className="learn-knowledge-panel">
            {suggestions.length > 0 && (
              <div className="learn-suggestions">
                <div className="learn-suggestions-title">Improvement Suggestions</div>
                {suggestions.map((sug) => (
                  <div key={sug.id} className={`learn-suggestion learn-suggestion--${sug.relevance}`}>
                    <div className="learn-suggestion-header">
                      <span className="learn-suggestion-title">{sug.title}</span>
                      <span className={`learn-suggestion-badge learn-suggestion-badge--${sug.relevance}`}>
                        {sug.relevance}
                      </span>
                    </div>
                    <div className="learn-suggestion-desc">{sug.description}</div>
                  </div>
                ))}
              </div>
            )}

            {knowledge.length === 0 ? (
              <div className="learn-knowledge-empty">
                No knowledge entries yet. Start a learning session.
              </div>
            ) : (
              <div className="learn-knowledge-grid">
                {knowledge.map((entry) => (
                  <KnowledgeCard key={entry.id} entry={entry} />
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

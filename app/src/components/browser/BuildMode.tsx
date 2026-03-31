import { useCallback, useEffect, useRef, useState } from "react";
import {
  hasDesktopRuntime,
  startBuild,
  buildAppendCode,
  buildAddMessage,
  completeBuild,
  getBuildCode,
} from "../../api/backend";
import type {
  ActivityMessage,
  BuildAgentMessage,
  BuildSessionState,
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

const ROLE_COLORS: Record<string, string> = {
  supervisor: "var(--nexus-accent)",
  coder: "var(--nexus-accent)",
  designer: "#f472b6",
};

interface BuildModeProps {
  onActivity: (
    type: ActivityMessage["message_type"],
    content: string,
    agentName?: string,
  ) => void;
}

/** Simple HTML/CSS/JS syntax highlighting using regex token replacement. */
function highlightCode(code: string): string {
  let html = code
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

  // CSS properties
  html = html.replace(
    /([\w-]+)\s*:/g,
    '<span class="build-hl-prop">$1</span>:',
  );
  // HTML tags
  html = html.replace(
    /&lt;(\/?[\w-]+)/g,
    '&lt;<span class="build-hl-tag">$1</span>',
  );
  // HTML attributes
  html = html.replace(
    /\s([\w-]+)=/g,
    ' <span class="build-hl-attr">$1</span>=',
  );
  // Strings
  html = html.replace(
    /(&quot;|")(.*?)(\1)/g,
    '<span class="build-hl-str">"$2"</span>',
  );
  // Comments
  html = html.replace(
    /(&lt;!--.*?--&gt;)/g,
    '<span class="build-hl-comment">$1</span>',
  );
  // CSS selectors (lines starting with . or # or tag name followed by {)
  html = html.replace(
    /^([.#]?[\w-]+(?:\s*,\s*[.#]?[\w-]+)*)\s*\{/gm,
    '<span class="build-hl-sel">$1</span> {',
  );

  return html;
}

// Mock code generation removed — builds require a real LLM provider.

/** Conversation script for the build session. */
function generateConversation(
  description: string,
): { agent: string; role: BuildAgentMessage["role"]; content: string; phase: number }[] {
  return [
    {
      agent: "Supervisor",
      role: "supervisor",
      content: `Create a landing page with hero section: ${description}`,
      phase: 0,
    },
    {
      agent: "Coder",
      role: "coder",
      content: "Understood. Building HTML structure with semantic elements...",
      phase: 1,
    },
    {
      agent: "Designer",
      role: "designer",
      content: "Suggest using gradient background for hero. Dark theme with cyan accents.",
      phase: 2,
    },
    {
      agent: "Coder",
      role: "coder",
      content: "Applied gradient: #0f0c29 to #302b63. Adding responsive grid for features.",
      phase: 3,
    },
    {
      agent: "Designer",
      role: "designer",
      content: "Card hover effects look good. Increase border-radius to 12px for softer feel.",
      phase: 4,
    },
    {
      agent: "Coder",
      role: "coder",
      content: "Finalizing layout. Adding footer and button hover animations.",
      phase: 5,
    },
    {
      agent: "Supervisor",
      role: "supervisor",
      content: "Build complete. All components rendered successfully.",
      phase: 6,
    },
  ];
}

export function BuildMode({ onActivity }: BuildModeProps): JSX.Element {
  const [description, setDescription] = useState("");
  const [session, setSession] = useState<BuildSessionState | null>(null);
  const [running, setRunning] = useState(false);
  const [code, setCode] = useState("");
  const [previewHtml, setPreviewHtml] = useState("");
  const [messages, setMessages] = useState<BuildAgentMessage[]>([]);
  const [cursorLine, setCursorLine] = useState(0);
  const codeRef = useRef<HTMLPreElement>(null);
  const chatEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll code to bottom
  useEffect(() => {
    if (codeRef.current) {
      codeRef.current.scrollTop = codeRef.current.scrollHeight;
    }
  }, [code]);

  // Auto-scroll chat
  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length]);

  // Update preview with debounce
  useEffect(() => {
    if (!code) return;
    const timer = setTimeout(() => {
      const lower = code.toLowerCase();
      if (lower.includes("<html") || lower.includes("<!doctype")) {
        setPreviewHtml(code);
      } else {
        setPreviewHtml(
          `<!DOCTYPE html><html><head><meta charset="utf-8"><style>body{margin:0;font-family:system-ui,sans-serif}</style></head><body>${code}</body></html>`,
        );
      }
    }, 500);
    return () => clearTimeout(timer);
  }, [code]);

  const addMessage = useCallback(
    (agentName: string, role: BuildAgentMessage["role"], content: string) => {
      const msg: BuildAgentMessage = {
        id: makeId(),
        timestamp: Date.now() / 1000,
        agent_name: agentName,
        role,
        content,
      };
      setMessages((prev) => [...prev, msg]);
    },
    [],
  );

  const handleStart = useCallback(async () => {
    if (!description.trim() || running) return;
    setRunning(true);
    setCode("");
    setPreviewHtml("");
    setMessages([]);

    onActivity("info", `Build started: "${description}"`, "Supervisor");

    let sess: BuildSessionState | null = null;

    if (hasDesktopRuntime()) {
      try {
        sess = await startBuild(description);
        setSession(sess);
      } catch (err) {
        console.error("Build session init failed:", err);
      }
    }

    // If no desktop session could be established, show error
    if (!sess) {
      addMessage("System", "supervisor", "Build requires an LLM provider. Configure one in Settings.");
      onActivity("blocked", "Build requires LLM provider. Configure in Settings.", "System");
      setRunning(false);
      return;
    }

    // UI conversation animation while real backend build runs via sessionId.
    // These messages are UX chrome — real code comes from the backend session.
    const conversation = generateConversation(description);
    const sessionId = sess.session_id;

    // Phase 0: Supervisor assigns task
    addMessage(conversation[0].agent, conversation[0].role, conversation[0].content);
    onActivity("info", conversation[0].content, "Supervisor");
    await delay(800);

    // Phase 1: Coder starts
    addMessage(conversation[1].agent, conversation[1].role, conversation[1].content);
    onActivity("coding", "Building HTML structure...", "Coder");
    await delay(600);

    // Stream conversation messages while the backend builds
    let nextConvoIdx = 2;
    for (let i = nextConvoIdx; i < conversation.length; i++) {
      const convo = conversation[i];
      addMessage(convo.agent, convo.role, convo.content);
      const actType = convo.role === "designer" ? "designing" as const : "coding" as const;
      onActivity(actType, convo.content, convo.agent);
      if (sessionId) {
        try {
          await buildAddMessage(sessionId, convo.agent, convo.role, convo.content);
        } catch (err) {
          console.error("Build message send failed:", err);
        }
      }
      await delay(600);
    }

    // Fetch the built code from the session
    if (sessionId) {
      try {
        const built = await getBuildCode(sessionId);
        if (typeof built === "string") {
          setCode(built);
          setCursorLine(built.split("\n").length);
        }
      } catch (err) {
        console.error("Failed to fetch build code:", err);
      }
    }

    // Complete the build session
    if (sessionId) {
      try {
        const completed = await completeBuild(sessionId);
        setSession(completed);
      } catch (err) {
        console.error("Build completion failed:", err);
      }
    }

    setSession((prev) => (prev ? { ...prev, status: "complete" } : prev));
    onActivity("info", "Build complete — preview ready", "Supervisor");
    setRunning(false);
  }, [description, running, onActivity, addMessage]);

  return (
    <div className="build-mode">
      {/* Build input / status bar */}
      {!session || session.status === "complete" ? (
        <div className="build-input-bar">
          <input
            type="text"
            className="build-description-input"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleStart();
            }}
            placeholder="Describe what to build... (e.g. landing page with hero section and feature cards)"
            disabled={running}
          />
          <button
            type="button"
            className="build-start-btn"
            onClick={() => void handleStart()}
            disabled={running || !description.trim()}
          >
            {running ? "Building..." : session ? "Rebuild" : "Build"}
          </button>
        </div>
      ) : (
        <div className="build-status-bar">
          <span className="build-status-icon">
            {session.status === "coding" ? "◌" : "◈"}
          </span>
          <span className="build-status-text">
            {session.status === "planning"
              ? "Planning build..."
              : `Building: ${session.description}`}
          </span>
          <span className="build-status-stats">
            Line {cursorLine} | Fuel: {session.fuel_used}
          </span>
        </div>
      )}

      {/* Three-panel layout */}
      <div className="build-panels">
        {/* Left: Code editor */}
        <div className="build-code-panel">
          <div className="build-panel-header">
            <span className="build-panel-title">Code</span>
            {code && (
              <span className="build-panel-meta">
                {code.split("\n").length} lines
              </span>
            )}
          </div>
          <div className="build-code-scroll">
            <pre
              ref={codeRef}
              className="build-code-content"
              dangerouslySetInnerHTML={{
                __html: code
                  ? highlightCode(code) +
                    (running
                      ? '<span class="build-cursor">|</span>'
                      : "")
                  : '<span class="build-code-placeholder">Code will appear here as agents write...</span>',
              }}
            />
          </div>
        </div>

        {/* Right: Preview */}
        <div className="build-preview-panel">
          <div className="build-panel-header">
            <span className="build-panel-title">Preview</span>
            {previewHtml && (
              <span className="build-panel-meta build-panel-live">LIVE</span>
            )}
          </div>
          <div className="build-preview-frame-container">
            {previewHtml ? (
              <iframe
                className="build-preview-iframe"
                srcDoc={previewHtml}
                title="Build Preview"
                sandbox="allow-scripts"
              />
            ) : (
              <div className="build-preview-placeholder">
                <span className="build-preview-placeholder-icon">◇</span>
                <span>Preview will render here</span>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Bottom: Agent conversation */}
      <div className="build-chat-panel">
        <div className="build-panel-header">
          <span className="build-panel-title">Agent Conversation</span>
          <span className="build-panel-meta">{messages.length}</span>
        </div>
        <div className="build-chat-messages">
          {messages.length === 0 && (
            <div className="build-chat-empty">
              Agents will discuss the build here...
            </div>
          )}
          {messages.map((msg) => (
            <div key={msg.id} className="build-chat-msg">
              <span
                className="build-chat-agent"
                style={{ color: ROLE_COLORS[msg.role] ?? "#94a3b8" }}
              >
                {msg.agent_name}:
              </span>
              <span className="build-chat-text">{msg.content}</span>
            </div>
          ))}
          <div ref={chatEndRef} />
        </div>
      </div>
    </div>
  );
}

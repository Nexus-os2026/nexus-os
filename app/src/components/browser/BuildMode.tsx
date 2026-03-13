import { useCallback, useEffect, useRef, useState } from "react";
import {
  hasDesktopRuntime,
  startBuild,
  buildAppendCode,
  buildAddMessage,
  completeBuild,
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

/** Generate mock code for a build description, split into typeable chunks. */
function generateMockCode(description: string): string[] {
  const desc = description.toLowerCase();
  const chunks: string[] = [];

  // HTML structure
  chunks.push('<!DOCTYPE html>\n<html lang="en">\n<head>\n');
  chunks.push('  <meta charset="utf-8">\n');
  chunks.push(`  <title>${description.slice(0, 50)}</title>\n`);
  chunks.push("  <style>\n");

  // CSS reset + base
  chunks.push("    * { margin: 0; padding: 0; box-sizing: border-box; }\n");
  chunks.push(
    "    body {\n      font-family: system-ui, -apple-system, sans-serif;\n      line-height: 1.6;\n      color: #1a1a2e;\n    }\n",
  );

  // Hero section
  if (desc.includes("hero") || desc.includes("landing")) {
    chunks.push(
      "    .hero {\n      min-height: 80vh;\n      display: flex;\n      flex-direction: column;\n      align-items: center;\n      justify-content: center;\n",
    );
    chunks.push(
      "      background: linear-gradient(135deg, #0f0c29, #302b63, #24243e);\n      color: white;\n      text-align: center;\n      padding: 2rem;\n    }\n",
    );
    chunks.push(
      "    .hero h1 {\n      font-size: 3.5rem;\n      font-weight: 800;\n      margin-bottom: 1rem;\n",
    );
    chunks.push(
      "      background: linear-gradient(to right, var(--nexus-accent), #3b82f6);\n      -webkit-background-clip: text;\n      -webkit-text-fill-color: transparent;\n    }\n",
    );
    chunks.push(
      "    .hero p {\n      font-size: 1.25rem;\n      opacity: 0.85;\n      max-width: 600px;\n      margin-bottom: 2rem;\n    }\n",
    );
  }

  // Button
  chunks.push(
    "    .btn {\n      padding: 0.75rem 2rem;\n      border: none;\n      border-radius: 8px;\n      font-size: 1rem;\n      font-weight: 600;\n      cursor: pointer;\n",
  );
  chunks.push(
    "      background: var(--nexus-accent);\n      color: #0f0c29;\n      transition: transform 0.2s, box-shadow 0.2s;\n    }\n",
  );
  chunks.push(
    "    .btn:hover {\n      transform: translateY(-2px);\n      box-shadow: 0 8px 25px rgba(0, 255, 157, 0.3);\n    }\n",
  );

  // Features section
  if (desc.includes("feature") || desc.includes("card") || desc.includes("landing")) {
    chunks.push(
      "    .features {\n      display: grid;\n      grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));\n      gap: 2rem;\n      padding: 4rem 2rem;\n      max-width: 1200px;\n      margin: 0 auto;\n    }\n",
    );
    chunks.push(
      "    .card {\n      background: #f8fafc;\n      border-radius: 12px;\n      padding: 2rem;\n      box-shadow: 0 4px 20px rgba(0,0,0,0.08);\n      transition: transform 0.2s;\n    }\n",
    );
    chunks.push(
      "    .card:hover { transform: translateY(-4px); }\n",
    );
    chunks.push(
      "    .card h3 { font-size: 1.25rem; margin-bottom: 0.5rem; color: #302b63; }\n",
    );
    chunks.push(
      "    .card p { color: #64748b; }\n",
    );
  }

  // Footer
  chunks.push(
    "    .footer {\n      text-align: center;\n      padding: 2rem;\n      background: #0f0c29;\n      color: rgba(255,255,255,0.6);\n      font-size: 0.875rem;\n    }\n",
  );

  chunks.push("  </style>\n</head>\n<body>\n");

  // HTML body
  if (desc.includes("hero") || desc.includes("landing")) {
    chunks.push('  <section class="hero">\n');
    chunks.push("    <h1>Welcome to the Future</h1>\n");
    chunks.push(
      "    <p>Build something extraordinary with intelligent agents that understand your vision.</p>\n",
    );
    chunks.push('    <button class="btn">Get Started</button>\n');
    chunks.push("  </section>\n\n");
  }

  if (desc.includes("feature") || desc.includes("card") || desc.includes("landing")) {
    chunks.push('  <section class="features">\n');
    for (const [title, text] of [
      ["Fast", "Lightning-fast performance powered by modern architecture."],
      ["Secure", "Enterprise-grade security with zero-trust governance."],
      ["Intelligent", "AI-powered agents that learn and adapt to your needs."],
    ]) {
      chunks.push(`    <div class="card">\n      <h3>${title}</h3>\n      <p>${text}</p>\n    </div>\n`);
    }
    chunks.push("  </section>\n\n");
  }

  chunks.push(
    '  <footer class="footer">\n    <p>Built by Nexus OS Agents</p>\n  </footer>\n',
  );
  chunks.push("\n</body>\n</html>");

  return chunks;
}

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
      } catch {
        // fall through to mock
      }
    }

    // Mock session if no Tauri
    if (!sess) {
      sess = {
        session_id: makeId(),
        description,
        status: "planning",
        code: "",
        preview_html: "",
        messages: [],
        fuel_used: 0,
        llm_calls: 0,
      };
      setSession(sess);
    }

    const conversation = generateConversation(description);
    const codeChunks = generateMockCode(description);
    const sessionId = sess.session_id;

    // Phase 0: Supervisor assigns task
    addMessage(conversation[0].agent, conversation[0].role, conversation[0].content);
    onActivity("info", conversation[0].content, "Supervisor");
    await delay(800);

    // Phase 1: Coder starts
    addMessage(conversation[1].agent, conversation[1].role, conversation[1].content);
    onActivity("coding", "Building HTML structure...", "Coder");
    await delay(600);

    // Type code chunks with interleaved conversation
    let codeAccum = "";
    let nextConvoIdx = 2;
    const chunksPerConvo = Math.ceil(codeChunks.length / (conversation.length - 2));

    for (let i = 0; i < codeChunks.length; i++) {
      const chunk = codeChunks[i];

      // Type character by character for first few chunks, then line-by-line
      if (i < 3) {
        // Character-by-character typing
        for (let c = 0; c < chunk.length; c++) {
          codeAccum += chunk[c];
          if (c % 3 === 0) {
            setCode(codeAccum);
            setCursorLine(codeAccum.split("\n").length);
            await delay(15);
          }
        }
        setCode(codeAccum);
      } else {
        // Line-by-line for speed
        const lines = chunk.split("\n");
        for (const line of lines) {
          codeAccum += line + "\n";
          setCode(codeAccum);
          setCursorLine(codeAccum.split("\n").length);
          await delay(40 + Math.random() * 30);
        }
      }

      // Backend sync
      if (hasDesktopRuntime() && sessionId) {
        try {
          await buildAppendCode(sessionId, chunk, "Coder");
        } catch {
          // continue mock
        }
      }

      // Interleave conversation at intervals
      if (
        nextConvoIdx < conversation.length - 1 &&
        (i + 1) % chunksPerConvo === 0
      ) {
        const convo = conversation[nextConvoIdx];
        addMessage(convo.agent, convo.role, convo.content);
        const actType = convo.role === "designer" ? "designing" as const : "coding" as const;
        onActivity(actType, convo.content, convo.agent);

        if (hasDesktopRuntime() && sessionId) {
          try {
            await buildAddMessage(sessionId, convo.agent, convo.role, convo.content);
          } catch {
            // continue
          }
        }

        nextConvoIdx++;
        await delay(500);
      }
    }

    // Remaining conversation messages
    while (nextConvoIdx < conversation.length) {
      const convo = conversation[nextConvoIdx];
      addMessage(convo.agent, convo.role, convo.content);
      onActivity("info", convo.content, convo.agent);
      nextConvoIdx++;
      await delay(400);
    }

    // Complete
    if (hasDesktopRuntime() && sessionId) {
      try {
        const completed = await completeBuild(sessionId);
        setSession(completed);
      } catch {
        // mock complete
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

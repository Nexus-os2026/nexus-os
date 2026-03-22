import { useState } from 'react';

function escapeHtml(value) {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;');
}

const HIGHLIGHTERS = {
  rust: [
    { regex: /(\/\/.*$)/gm, className: 'comment' },
    { regex: /("(?:[^"\\]|\\.)*")/g, className: 'string' },
    { regex: /\b(use|pub|fn|let|mut|async|await|struct|enum|impl|match|if|else|for|while|loop|return|const|trait|where|crate|mod|Self|self|super)\b/g, className: 'keyword' },
    { regex: /\b(Result|Option|String|Vec|Duration|usize|u8|u16|u32|u64|i32|i64|f32|f64|bool)\b/g, className: 'type' },
    { regex: /\b([A-Za-z_]\w*)\s*(?=\()/g, className: 'function' },
  ],
  bash: [
    { regex: /(#.*$)/gm, className: 'comment' },
    { regex: /("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')/g, className: 'string' },
    { regex: /\b(npm|npx|cargo|cd|curl|git|pnpm|yarn|node)\b/g, className: 'keyword' },
    { regex: /(--?[A-Za-z-]+)/g, className: 'type' },
  ],
  javascript: [
    { regex: /(\/\/.*$)/gm, className: 'comment' },
    { regex: /("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`)/g, className: 'string' },
    { regex: /\b(import|from|export|const|let|var|return|if|else|for|while|function|await|async|new)\b/g, className: 'keyword' },
    { regex: /\b([A-Za-z_]\w*)\s*(?=\()/g, className: 'function' },
  ],
};

function highlightCode(code, language) {
  const escaped = escapeHtml(code.trim());
  const rules = HIGHLIGHTERS[language] || HIGHLIGHTERS.rust;

  return rules.reduce(
    (result, rule) => result.replace(rule.regex, `<span class="token ${rule.className}">$1</span>`),
    escaped,
  );
}

export default function CodeBlock({ code, language = 'rust', label }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback
    }
  };

  return (
    <div className="terminal-block">
      <div className="terminal-header">
        <div className="terminal-controls">
          <span />
          <span />
          <span />
        </div>
        <div className="terminal-label">{label || `${language.toUpperCase()} // GOVERNED TRACE`}</div>
        <button className="terminal-copy" onClick={handleCopy}>
          {copied ? 'COPIED' : 'COPY'}
        </button>
      </div>
      <pre className="terminal-body">
        <code dangerouslySetInnerHTML={{ __html: highlightCode(code, language) }} />
      </pre>
      <div className="terminal-footer">
        <span className="terminal-cursor" />
        <span>EXECUTION FRAME VERIFIED</span>
      </div>
    </div>
  );
}

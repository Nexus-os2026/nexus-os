/**
 * P0-002C5C: HTML built from text the app does not control.
 *
 * Model replies, note text and error messages reach the webview as plain
 * strings. The webview CSP is `null`, so any script that runs here can call
 * every registered IPC command: a string must never become markup, an event
 * handler or a `javascript:` link. These helpers escape the text first and
 * only then add the few elements the renderers need. Links and images keep
 * only http(s) URLs.
 */

/** Escape text for use in HTML content and in double- or single-quoted attributes. */
export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/** Whether escaped URL text is an http(s) URL. */
function isHttpUrl(escapedUrl: string): boolean {
  return /^https?:\/\/[^\s]+$/i.test(escapedUrl);
}

/** Chat text outside code fences: **bold**, `inline code` and line breaks. */
function renderChatText(text: string): string {
  return escapeHtml(text)
    .replace(/\*\*(.*?)\*\*/g, "<strong>$1</strong>")
    .replace(/(?<!`)`(?!`)([^`\n]+)`(?!`)/g, '<code class="ch-inline-code">$1</code>')
    .replace(/\n/g, "<br/>");
}

/**
 * A chat reply as HTML. Fenced code blocks keep their language label and a
 * Run button carrying the raw code (URI-encoded); everything else is text.
 */
export function renderChatContent(content: string): string {
  if (!content) return "";
  const parts: string[] = [];
  let last = 0;
  for (const match of content.matchAll(/```(\w+)?\n([\s\S]*?)```/g)) {
    const at = match.index ?? 0;
    parts.push(renderChatText(content.slice(last, at)));
    const lang = escapeHtml(match[1] || "text");
    const code = match[2];
    parts.push(
      `<div class="ch-code-block"><div class="ch-code-header"><span>${lang}</span>` +
        `<button type="button" class="ch-code-run" data-code="${encodeURIComponent(code.trim())}">▶ Run</button></div>` +
        `<pre class="ch-code-pre"><code>${escapeHtml(code)}</code></pre></div>`,
    );
    last = at + match[0].length;
  }
  parts.push(renderChatText(content.slice(last)));
  return parts.join("");
}

/**
 * Note markdown as HTML: headings, emphasis, code, quotes, rules, check and
 * plain lists, tables, and http(s) links and images. The note text is escaped
 * before any of it is added.
 */
export function renderNoteMarkdown(md: string): string {
  let html = escapeHtml(md)
    .replace(/```(\w*)\n([\s\S]*?)```/g, '<pre class="na-code-block"><code>$2</code></pre>')
    .replace(/`([^`]+)`/g, '<code class="na-inline-code">$1</code>')
    .replace(/^#### (.+)$/gm, "<h4>$1</h4>")
    .replace(/^### (.+)$/gm, "<h3>$1</h3>")
    .replace(/^## (.+)$/gm, "<h2>$1</h2>")
    .replace(/^# (.+)$/gm, "<h1>$1</h1>")
    .replace(/\*\*\*(.+?)\*\*\*/g, "<strong><em>$1</em></strong>")
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/\*(.+?)\*/g, "<em>$1</em>")
    .replace(/~~(.+?)~~/g, "<del>$1</del>")
    .replace(/^&gt; (.+)$/gm, "<blockquote>$1</blockquote>")
    .replace(/^---$/gm, "<hr />")
    .replace(/^- \[x\] (.+)$/gm, '<div class="na-checkbox checked">☑ $1</div>')
    .replace(/^- \[ \] (.+)$/gm, '<div class="na-checkbox">☐ $1</div>')
    .replace(/^- (.+)$/gm, "<li>$1</li>")
    .replace(/^\d+\. (.+)$/gm, "<li>$1</li>")
    .replace(/^\|(.+)\|$/gm, (match) => {
      const cells = match.split("|").filter((c) => c.trim());
      if (cells.every((c) => /^[\s-:]+$/.test(c))) return "";
      return `<tr>${cells.map((c) => `<td>${c.trim()}</td>`).join("")}</tr>`;
    })
    .replace(/!\[([^\]]*)\]\(([^)\s]+)\)/g, (match, alt: string, url: string) =>
      isHttpUrl(url) ? `<img alt="${alt}" src="${url}" class="na-img" />` : match,
    )
    .replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (match, text: string, url: string) =>
      isHttpUrl(url) ? `<a href="${url}" class="na-link">${text}</a>` : match,
    )
    .replace(/\n\n/g, "</p><p>")
    .replace(/\n/g, "<br />");

  html = html.replace(/((?:<li>.*?<\/li>\s*)+)/g, "<ul>$1</ul>");
  html = html.replace(/((?:<tr>.*?<\/tr>\s*)+)/g, '<table class="na-table">$1</table>');

  return `<p>${html}</p>`;
}

import { describe, expect, it } from "vitest";
import { escapeHtml, renderChatContent, renderNoteMarkdown, safeHttpUrl } from "../safeHtml";

/** Parse rendered HTML the way the webview would and report what it built. */
function parse(html: string) {
  const root = document.createElement("div");
  root.innerHTML = html;
  const elements = Array.from(root.querySelectorAll("*"));
  const handlers = elements.flatMap((el) =>
    Array.from(el.attributes)
      .filter((attr) => attr.name.toLowerCase().startsWith("on"))
      .map((attr) => `${el.tagName}.${attr.name}`),
  );
  const urls = elements.flatMap((el) =>
    ["href", "src"].map((name) => el.getAttribute(name)).filter((v): v is string => v !== null),
  );
  const tags = new Set(elements.map((el) => el.tagName.toLowerCase()));
  return { root, handlers, urls, tags };
}

const HOSTILE = [
  '<img src=x onerror="window.__TAURI_INTERNALS__.invoke(\'api_client_request\')">',
  "<script>alert(1)</script>",
  '<svg onload="alert(1)"></svg>',
  '<iframe src="javascript:alert(1)"></iframe>',
  '<a href="javascript:alert(1)">x</a>',
  '"><img src=x onerror=alert(1)>',
];

describe("P0-002C5C: text never becomes markup", () => {
  it("escapes every markup character", () => {
    expect(escapeHtml(`<a href="x" title='y'>&</a>`)).toBe(
      "&lt;a href=&quot;x&quot; title=&#39;y&#39;&gt;&amp;&lt;/a&gt;",
    );
  });

  it("renders a model reply as text, with only its own formatting", () => {
    for (const hostile of HOSTILE) {
      const reply = `Here is **bold** and \`code\`:\n${hostile}\n\`\`\`js\n${hostile}\n\`\`\`\nafter ${hostile}`;
      const { root, handlers, urls, tags } = parse(renderChatContent(reply));
      expect(handlers).toEqual([]);
      expect(urls).toEqual([]);
      for (const tag of ["img", "script", "svg", "iframe", "a"]) {
        expect(tags.has(tag), `${tag} from ${hostile}`).toBe(false);
      }
      // The hostile text is shown, not interpreted.
      expect(root.textContent).toContain(hostile);
      expect(root.querySelector("strong")?.textContent).toBe("bold");
      expect(root.querySelector("code.ch-inline-code")?.textContent).toBe("code");
      // The Run button carries the raw code, which stays text in the page.
      const run = root.querySelector("button.ch-code-run");
      expect(decodeURIComponent(run?.getAttribute("data-code") ?? "")).toBe(hostile);
      expect(root.querySelector("pre.ch-code-pre code")?.textContent).toBe(`${hostile}\n`);
    }
  });

  it("keeps a code block's language label as text", () => {
    const { root } = parse(renderChatContent("```python\nprint('hi')\n```"));
    expect(root.querySelector(".ch-code-header span")?.textContent).toBe("python");
  });

  it("renders note markdown as text with safe links only", () => {
    for (const hostile of HOSTILE) {
      const note = `# Title\n> quote\n- item ${hostile}\n${hostile}`;
      const { root, handlers, urls, tags } = parse(renderNoteMarkdown(note));
      expect(handlers).toEqual([]);
      expect(urls).toEqual([]);
      for (const tag of ["img", "script", "svg", "iframe", "a"]) {
        expect(tags.has(tag), `${tag} from ${hostile}`).toBe(false);
      }
      expect(root.querySelector("h1")?.textContent).toBe("Title");
      expect(root.querySelector("blockquote")?.textContent).toBe("quote");
      expect(root.textContent).toContain(hostile);
    }
    for (const hostile of [
      "[click](javascript:alert(1))",
      "[click](data:text/html,<script>alert(1)</script>)",
      "![pic](x\" onerror=\"alert(1))",
      "![pic](javascript:alert(1))",
    ]) {
      const { handlers, urls } = parse(renderNoteMarkdown(hostile));
      expect(handlers, hostile).toEqual([]);
      expect(urls, hostile).toEqual([]);
    }
    const { urls } = parse(
      renderNoteMarkdown("[docs](https://example.com/a?x=1&y=2) ![pic](https://example.com/p.png)"),
    );
    expect(urls).toEqual(["https://example.com/a?x=1&y=2", "https://example.com/p.png"]);
  });

  it("admits only http(s) URLs as link and window targets", () => {
    expect(safeHttpUrl("https://example.com/a?b=1")).toBe("https://example.com/a?b=1");
    expect(safeHttpUrl("http://localhost:3000/")).toBe("http://localhost:3000/");
    for (const hostile of [
      "javascript:alert(1)",
      " javascript:alert(1)",
      "JAVASCRIPT:alert(1)",
      "data:text/html,<script>alert(1)</script>",
      "file:///etc/passwd",
      "vbscript:msgbox(1)",
      "not a url",
      "",
      null,
      undefined,
    ]) {
      expect(safeHttpUrl(hostile), String(hostile)).toBeUndefined();
    }
  });
});

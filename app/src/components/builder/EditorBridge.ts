/**
 * EditorBridge — postMessage communication between parent editor and preview iframe.
 *
 * The bridge script is injected INTO the iframe as a <script>. It:
 * - Listens for hover/click on elements with data-nexus-section/slot attributes
 * - Sends element info (section, slot, computed styles) to parent via postMessage
 * - Receives token update messages from parent and applies via style.setProperty
 * - Creates/manages a hover highlight overlay div
 *
 * The parent uses sendToIframe() to send messages to the iframe.
 * All visual edits are TOKEN OPERATIONS — zero inline style= attributes.
 */

// ─── Message Types ──────────────────────────────────────────────────────────

/** Messages from iframe �� parent */
export type IframeMessage =
  | {
      type: "element-hover";
      sectionId: string | null;
      slotName: string | null;
      elementTag: string;
      rect: { top: number; left: number; width: number; height: number };
      computedStyles: Record<string, string>;
    }
  | {
      type: "element-select";
      sectionId: string | null;
      slotName: string | null;
      elementTag: string;
      rect: { top: number; left: number; width: number; height: number };
      computedStyles: Record<string, string>;
      resolvedTokens: Record<string, string>;
      currentText: string;
    }
  | {
      type: "text-edit";
      sectionId: string;
      slotName: string;
      newText: string;
    }
  | { type: "edit-mode-ready" };

/** Messages from parent → iframe */
export type EditorMessage =
  | { type: "highlight"; sectionId: string | null; rect: { top: number; left: number; width: number; height: number } }
  | { type: "clear-highlight" }
  | { type: "update-token"; tokenName: string; value: string }
  | { type: "update-section-token"; sectionId: string; tokenName: string; value: string }
  | { type: "enable-edit-mode" }
  | { type: "disable-edit-mode" };

// ─── Computed Style Keys ────────────────────────────────────────────────────

/** CSS properties to capture from computed styles */
const STYLE_KEYS = [
  "color",
  "backgroundColor",
  "borderColor",
  "borderRadius",
  "fontSize",
  "fontFamily",
  "fontWeight",
  "padding",
  "paddingTop",
  "paddingBottom",
  "paddingLeft",
  "paddingRight",
  "margin",
  "marginTop",
  "marginBottom",
] as const;

// ─── Bridge Script (injected into iframe) ───────────────────────────────────

/**
 * Returns a self-contained JavaScript string to inject into the preview iframe.
 * This script handles all edit-mode interactions inside the iframe.
 */
export function getEditorBridgeScript(): string {
  return `
(function() {
  'use strict';
  var editMode = false;
  var highlightEl = null;
  var selectedEl = null;

  // Style keys to capture
  var KEYS = ${JSON.stringify(STYLE_KEYS)};

  function getComputedProps(el) {
    var cs = window.getComputedStyle(el);
    var result = {};
    for (var i = 0; i < KEYS.length; i++) {
      result[KEYS[i]] = cs[KEYS[i]] || '';
    }
    return result;
  }

  // Resolve CSS custom properties from :root to hex values
  function rgbToHex(rgb) {
    var m = rgb.match(/rgba?\\((\\d+),\\s*(\\d+),\\s*(\\d+)/);
    if (!m) return rgb.indexOf('#') === 0 ? rgb : '';
    var r = parseInt(m[1],10), g = parseInt(m[2],10), b = parseInt(m[3],10);
    return '#' + ((1<<24)+(r<<16)+(g<<8)+b).toString(16).slice(1);
  }

  function resolveTokenColors() {
    var tokens = {};
    var root = document.documentElement;
    var cs = window.getComputedStyle(root);

    // Discover ALL CSS custom properties defined on :root.
    // LLM-generated HTML uses variable naming conventions (--accent, --bg, etc.)
    // that differ across builds, so we cannot hardcode expected names.
    var discovered = [];
    try {
      for (var s = 0; s < document.styleSheets.length; s++) {
        try {
          var rules = document.styleSheets[s].cssRules || document.styleSheets[s].rules;
          if (!rules) continue;
          for (var r = 0; r < rules.length; r++) {
            var rule = rules[r];
            if (rule.selectorText === ':root' && rule.style) {
              for (var p = 0; p < rule.style.length; p++) {
                var prop = rule.style[p];
                if (prop.indexOf('--') === 0) {
                  discovered.push(prop.substring(2)); // strip leading --
                }
              }
            }
          }
        } catch (e) { /* cross-origin sheet, skip */ }
      }
    } catch (e) { /* styleSheets access failed */ }

    // Fallback: also check well-known structured token names that may exist
    var fallbackNames = [
      'color-primary','color-secondary','color-accent',
      'color-bg','color-bg-secondary','color-text',
      'color-text-secondary','color-border',
      'btn-bg','btn-text','btn-border','btn-hover-bg',
      'hero-bg','hero-text','hero-accent',
      'nav-bg','nav-text','nav-border',
      'footer-bg','footer-text'
    ];
    for (var f = 0; f < fallbackNames.length; f++) {
      if (discovered.indexOf(fallbackNames[f]) === -1) {
        discovered.push(fallbackNames[f]);
      }
    }

    // Resolve each discovered variable to a hex color
    for (var i = 0; i < discovered.length; i++) {
      var name = discovered[i];
      var val = cs.getPropertyValue('--' + name).trim();
      if (val) {
        // Try to resolve as a color
        var tmp = document.createElement('div');
        tmp.style.color = val;
        tmp.style.display = 'none';
        document.body.appendChild(tmp);
        var resolved = window.getComputedStyle(tmp).color;
        document.body.removeChild(tmp);
        var hex = rgbToHex(resolved);
        // Only include if it resolved to a valid color (not lengths, fonts, etc.)
        if (hex && hex.match(/^#[0-9a-f]{6}$/i)) {
          tokens[name] = hex;
        }
      }
    }
    return tokens;
  }

  function getSection(el) {
    var cur = el;
    while (cur && cur !== document.body) {
      if (cur.getAttribute && cur.getAttribute('data-nexus-section')) {
        return cur.getAttribute('data-nexus-section');
      }
      cur = cur.parentElement;
    }
    return null;
  }

  function getSlot(el) {
    var cur = el;
    while (cur && cur !== document.body) {
      if (cur.getAttribute && cur.getAttribute('data-nexus-slot')) {
        return cur.getAttribute('data-nexus-slot');
      }
      cur = cur.parentElement;
    }
    return null;
  }

  function getRect(el) {
    var r = el.getBoundingClientRect();
    return { top: r.top, left: r.left, width: r.width, height: r.height };
  }

  function getTextContent(el) {
    // Get direct text, not children's text
    var text = '';
    for (var i = 0; i < el.childNodes.length; i++) {
      if (el.childNodes[i].nodeType === 3) text += el.childNodes[i].textContent;
    }
    return text.trim() || el.textContent.trim().substring(0, 200);
  }

  // Create highlight overlay
  function ensureHighlight() {
    if (!highlightEl) {
      highlightEl = document.createElement('div');
      highlightEl.id = '__nexus-highlight';
      highlightEl.style.cssText = 'position:fixed;pointer-events:none;border:2px solid #00d4aa;border-radius:4px;background:rgba(0,212,170,0.06);z-index:999999;display:none;transition:all 80ms ease;box-shadow:0 0 0 1px rgba(0,212,170,0.15);';
      document.body.appendChild(highlightEl);
    }
    return highlightEl;
  }

  function showHighlight(rect) {
    var h = ensureHighlight();
    h.style.top = rect.top + 'px';
    h.style.left = rect.left + 'px';
    h.style.width = rect.width + 'px';
    h.style.height = rect.height + 'px';
    h.style.display = 'block';
  }

  function hideHighlight() {
    if (highlightEl) highlightEl.style.display = 'none';
  }

  function sendToParent(msg) {
    window.parent.postMessage(msg, '*');
  }

  // Create a second highlight for selected element (persistent)
  var selectHighlightEl = null;
  function ensureSelectHighlight() {
    if (!selectHighlightEl) {
      selectHighlightEl = document.createElement('div');
      selectHighlightEl.id = '__nexus-select-highlight';
      selectHighlightEl.style.cssText = 'position:fixed;pointer-events:none;border:2px solid #00d4aa;border-radius:4px;background:rgba(0,212,170,0.10);z-index:999998;display:none;transition:all 80ms ease;box-shadow:0 0 0 2px rgba(0,212,170,0.25);';
      document.body.appendChild(selectHighlightEl);
    }
    return selectHighlightEl;
  }

  function showSelectHighlight(rect) {
    var h = ensureSelectHighlight();
    h.style.top = rect.top + 'px';
    h.style.left = rect.left + 'px';
    h.style.width = rect.width + 'px';
    h.style.height = rect.height + 'px';
    h.style.display = 'block';
  }

  function hideSelectHighlight() {
    if (selectHighlightEl) selectHighlightEl.style.display = 'none';
  }

  // Hover handler — show hover highlight, but don't affect selection
  function onMouseOver(e) {
    if (!editMode) return;
    var el = e.target;
    if (el.id === '__nexus-highlight' || el.id === '__nexus-select-highlight') return;
    // Don't show hover highlight on already-selected element
    if (el === selectedEl) return;
    var rect = getRect(el);
    showHighlight(rect);
  }

  function onMouseOut(e) {
    if (!editMode) return;
    // Hide hover highlight when leaving any element
    hideHighlight();
  }

  // Click handler — select element and lock highlight
  function onClick(e) {
    if (!editMode) return;
    e.preventDefault();
    e.stopPropagation();
    var el = e.target;
    if (el.id === '__nexus-highlight' || el.id === '__nexus-select-highlight') return;
    selectedEl = el;
    hideHighlight(); // hide hover highlight
    var rect = getRect(el);
    showSelectHighlight(rect); // show persistent selection highlight
    sendToParent({
      type: 'element-select',
      sectionId: getSection(el),
      slotName: getSlot(el),
      elementTag: el.tagName.toLowerCase(),
      rect: rect,
      computedStyles: getComputedProps(el),
      resolvedTokens: resolveTokenColors(),
      currentText: getTextContent(el)
    });
  }

  // Receive messages from parent
  window.addEventListener('message', function(e) {
    var d = e.data;
    if (!d || !d.type) return;

    switch (d.type) {
      case 'enable-edit-mode':
        editMode = true;
        document.body.style.cursor = 'crosshair';
        document.addEventListener('mouseover', onMouseOver, true);
        document.addEventListener('mouseout', onMouseOut, true);
        document.addEventListener('click', onClick, true);
        sendToParent({ type: 'edit-mode-ready' });
        break;

      case 'disable-edit-mode':
        editMode = false;
        document.body.style.cursor = '';
        document.removeEventListener('mouseover', onMouseOver, true);
        document.removeEventListener('mouseout', onMouseOut, true);
        document.removeEventListener('click', onClick, true);
        hideHighlight();
        hideSelectHighlight();
        selectedEl = null;
        break;

      case 'clear-highlight':
        hideHighlight();
        hideSelectHighlight();
        selectedEl = null;
        break;

      case 'update-token':
        // Layer 1: global token on :root
        document.documentElement.style.setProperty('--' + d.tokenName, d.value);
        break;

      case 'update-section-token':
        // Layer 3: scoped override on section element
        var section = document.querySelector('[data-nexus-section="' + d.sectionId + '"]');
        if (section) {
          section.style.setProperty('--' + d.tokenName, d.value);
        }
        break;

      case 'highlight':
        if (d.rect) showHighlight(d.rect);
        break;
    }
  });
})();
`;
}

// ─── Parent-side Utilities ──────────────────────────────────────────────────

/** Send a message from the parent editor to the preview iframe. */
export function sendToIframe(iframe: HTMLIFrameElement, message: EditorMessage): void {
  if (iframe.contentWindow) {
    iframe.contentWindow.postMessage(message, "*");
  }
}

/** Check if a message is a valid IframeMessage from the bridge. */
export function isIframeMessage(data: unknown): data is IframeMessage {
  if (!data || typeof data !== "object") return false;
  const msg = data as Record<string, unknown>;
  return (
    msg.type === "element-hover" ||
    msg.type === "element-select" ||
    msg.type === "text-edit" ||
    msg.type === "edit-mode-ready"
  );
}

/**
 * Inject the editor bridge script into HTML before the closing </body> tag.
 * Returns the modified HTML string.
 */
export function injectBridgeScript(html: string): string {
  const script = `<script>${getEditorBridgeScript()}<\/script>`;
  const bodyClose = html.lastIndexOf("</body>");
  if (bodyClose !== -1) {
    return html.slice(0, bodyClose) + script + html.slice(bodyClose);
  }
  // Fallback: append to end
  return html + script;
}

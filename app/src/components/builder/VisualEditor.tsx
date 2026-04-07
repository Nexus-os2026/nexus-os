/**
 * VisualEditor — wraps the preview iframe with edit mode overlay and property panel.
 *
 * Dual preview modes:
 * 1. HTML mode — iframe uses srcdoc with assembled HTML (existing)
 * 2. React mode — iframe uses src pointing to Vite dev server URL
 *
 * Both modes share the same PropertyPanel, EditorBridge, and postMessage protocol.
 * Communication with the iframe is via postMessage (< 1ms latency).
 * Visual updates happen BEFORE the Tauri persist round-trip completes.
 */

import { useRef, useCallback, useEffect, useState } from "react";
import { injectBridgeScript, sendToIframe, isIframeMessage, type IframeMessage } from "./EditorBridge";
import PropertyPanel, { type SelectedElement } from "./PropertyPanel";

/* === Design tokens === */
const C = {
  bg: "#0a0e14",
  surface: "#111820",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  mono: "'JetBrains Mono','Fira Code','Cascadia Code',monospace",
  sans: "system-ui,-apple-system,sans-serif",
};

// ─── Types ──────────────────────────────────────────────────────────────────

interface VisualEditorProps {
  /** Output mode determines which preview path to use. */
  outputMode?: "html" | "react";
  /** Assembled HTML string for HTML mode (srcdoc). */
  html?: string;
  /** Vite dev server URL for React mode (iframe src). */
  devServerUrl?: string;
  /** Whether dev server is currently starting. */
  devServerLoading?: boolean;
  editMode: boolean;
  viewport: "desktop" | "tablet" | "mobile" | "wide";
  onTokenChange: (layer: 1 | 3, sectionId: string | null, tokenName: string, value: string) => void;
  onTextChange: (sectionId: string, slotName: string, newText: string) => void;
  iframeRef?: React.RefObject<HTMLIFrameElement | null>;
}

const VP_WIDTHS: Record<string, string> = {
  mobile: "375px",
  tablet: "768px",
  desktop: "100%",
  wide: "1440px",
};

// ─── Component ──────────────────────────────────────────────────────────────

export default function VisualEditor({
  outputMode = "html",
  html,
  devServerUrl,
  devServerLoading,
  editMode,
  viewport,
  onTokenChange,
  onTextChange,
  iframeRef: externalRef,
}: VisualEditorProps) {
  const internalRef = useRef<HTMLIFrameElement>(null);
  const iframeRef = externalRef || internalRef;
  const [selected, setSelected] = useState<SelectedElement | null>(null);
  const [editModeReady, setEditModeReady] = useState(false);

  // For HTML mode: inject bridge script into HTML when editing
  const isHtmlMode = outputMode === "html";
  const isReactMode = outputMode === "react";
  const iframeSrcDoc = isHtmlMode ? (editMode && html ? injectBridgeScript(html) : html) : undefined;
  // For React mode: bridge script is already in the Vite-served index.html
  const iframeSrc = isReactMode ? devServerUrl : undefined;
  const vpMax = VP_WIDTHS[viewport] || "100%";

  // Handle messages from iframe — accept both srcdoc (null origin) and localhost origins
  const handleMessage = useCallback(
    (event: MessageEvent) => {
      // Origin check: accept null (srcdoc) or localhost in allowed port range
      const origin = event.origin;
      const isValidOrigin =
        origin === "null" ||
        origin === "" ||
        /^http:\/\/127\.0\.0\.1:1517[3-9]$/.test(origin) ||
        /^http:\/\/127\.0\.0\.1:1518[0-3]$/.test(origin) ||
        /^http:\/\/localhost:1517[3-9]$/.test(origin) ||
        /^http:\/\/localhost:1518[0-3]$/.test(origin);
      if (!isValidOrigin && origin !== window.location.origin) return;

      if (!isIframeMessage(event.data)) return;
      const msg = event.data as IframeMessage;

      switch (msg.type) {
        case "edit-mode-ready":
          setEditModeReady(true);
          break;
        case "element-hover":
          break;
        case "element-select":
          setSelected({
            sectionId: msg.sectionId,
            slotName: msg.slotName,
            elementTag: msg.elementTag,
            computedStyles: msg.computedStyles,
            resolvedTokens: msg.resolvedTokens ?? {},
            currentText: msg.currentText,
          });
          break;
        case "text-edit":
          onTextChange(msg.sectionId, msg.slotName, msg.newText);
          break;
      }
    },
    [onTextChange],
  );

  // Listen for postMessage events
  useEffect(() => {
    window.addEventListener("message", handleMessage);
    return () => window.removeEventListener("message", handleMessage);
  }, [handleMessage]);

  // Enable/disable edit mode in iframe when prop changes
  useEffect(() => {
    const iframe = iframeRef.current;
    if (!iframe) return;

    const onLoad = () => {
      if (editMode) {
        sendToIframe(iframe, { type: "enable-edit-mode" });
      } else {
        sendToIframe(iframe, { type: "disable-edit-mode" });
        setSelected(null);
        setEditModeReady(false);
      }
    };

    iframe.addEventListener("load", onLoad);
    if (iframe.contentWindow) {
      onLoad();
    }

    return () => iframe.removeEventListener("load", onLoad);
  }, [editMode, iframeRef, iframeSrcDoc, iframeSrc]);

  // Forward token changes to iframe for instant preview, then to parent for persist
  const handleTokenChange = useCallback(
    (layer: 1 | 3, sectionId: string | null, tokenName: string, value: string) => {
      const iframe = iframeRef.current;
      if (iframe) {
        if (layer === 1) {
          sendToIframe(iframe, { type: "update-token", tokenName, value });
        } else if (sectionId) {
          sendToIframe(iframe, { type: "update-section-token", sectionId, tokenName, value });
        }
      }
      onTokenChange(layer, sectionId, tokenName, value);
    },
    [iframeRef, onTokenChange],
  );

  const handleDeselect = useCallback(() => {
    setSelected(null);
    const iframe = iframeRef.current;
    if (iframe) {
      sendToIframe(iframe, { type: "clear-highlight" });
    }
  }, [iframeRef]);

  // Determine if we have content to show
  const hasContent = isHtmlMode ? !!html : !!devServerUrl;

  return (
    <div style={{ flex: 1, display: "flex", overflow: "hidden", height: "100%" }}>
      {/* Preview Area */}
      <div style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" }}>
        {/* Edit mode indicator */}
        {editMode && (
          <div style={{
            height: 28, minHeight: 28, display: "flex", alignItems: "center", padding: "0 12px",
            background: C.accentDim, borderBottom: `1px solid rgba(0,212,170,0.2)`,
            fontSize: 10, color: C.accent, gap: 8,
          }}>
            <span style={{ width: 6, height: 6, borderRadius: "50%", background: editModeReady ? C.accent : C.dim }} />
            <span>Edit Mode {editModeReady ? "Active" : "Loading..."}</span>
            <span style={{ color: C.dim }}>Click any element to edit</span>
            {isReactMode && <span style={{ marginLeft: "auto", color: C.dim }}>Vite HMR</span>}
          </div>
        )}

        {/* Iframe container */}
        <div style={{ flex: 1, display: "flex", justifyContent: "center", alignItems: "stretch", overflow: "hidden", padding: 10 }}>
          {hasContent ? (
            <div style={{
              width: "100%", maxWidth: vpMax, height: "100%", margin: "0 auto",
              border: `1px solid ${editMode ? "rgba(0,212,170,0.3)" : C.border}`,
              borderRadius: 6, overflow: "hidden", background: "#fff",
              transition: "max-width 0.3s ease, border-color 0.2s ease",
            }}>
              {isHtmlMode ? (
                <iframe
                  ref={iframeRef as React.RefObject<HTMLIFrameElement>}
                  srcDoc={iframeSrcDoc}
                  sandbox="allow-scripts"
                  style={{ width: "100%", height: "100%", border: "none" }}
                  title="Preview"
                />
              ) : (
                <iframe
                  ref={iframeRef as React.RefObject<HTMLIFrameElement>}
                  src={iframeSrc}
                  sandbox="allow-scripts allow-same-origin"
                  style={{ width: "100%", height: "100%", border: "none" }}
                  title="Preview"
                />
              )}
            </div>
          ) : devServerLoading ? (
            <div style={{ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", width: "100%", gap: 12 }}>
              <div style={{ width: 44, height: 44, borderRadius: "50%", border: `3px solid ${C.border}`, borderTopColor: C.accent, animation: "nbspin 0.8s linear infinite" }} />
              <div style={{ fontSize: 12, color: C.muted }}>Starting preview server...</div>
              <div style={{ fontSize: 10, color: C.dim }}>Installing dependencies and starting Vite</div>
            </div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", width: "100%", gap: 12 }}>
              <div style={{ fontSize: 12, color: C.dim }}>No preview available</div>
            </div>
          )}
        </div>
      </div>

      {/* Property Panel (visible in edit mode when element selected) */}
      {editMode && (
        <PropertyPanel
          selected={selected}
          onTokenChange={handleTokenChange}
          onTextChange={onTextChange}
          onDeselect={handleDeselect}
        />
      )}
    </div>
  );
}

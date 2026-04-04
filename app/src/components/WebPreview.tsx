import { useCallback, useEffect, useRef, useState } from "react";
import { builderReadPreview } from "../api/backend";

// ── Types ──

type Viewport = "desktop" | "tablet" | "mobile";

interface ViewportConfig {
  width: number;
  height: number;
  label: string;
  icon: string;
}

const VIEWPORTS: Record<Viewport, ViewportConfig> = {
  desktop: { width: 1280, height: 800, label: "Desktop", icon: "\u{1F5A5}" },
  tablet: { width: 768, height: 1024, label: "Tablet", icon: "\u{1F4F1}" },
  mobile: { width: 375, height: 812, label: "Mobile", icon: "\u{1F4F2}" },
};

// ── Colors ──

const BG = "#0d1117";
const BG_SURFACE = "#161b22";
const TEXT = "#e6edf3";
const TEXT_SEC = "#8b949e";
const ACCENT = "#58a6ff";
const BORDER = "#30363d";

// ── Props ──

interface WebPreviewProps {
  /** Project directory path (contains current/index.html). */
  projectDir: string;
  /** Increment to force reload (after build, iteration, rollback). */
  reloadKey: number;
}

export function WebPreview({ projectDir, reloadKey }: WebPreviewProps) {
  const [viewport, setViewport] = useState<Viewport>("desktop");
  const [htmlContent, setHtmlContent] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [scale, setScale] = useState(1);
  const wrapperRef = useRef<HTMLDivElement>(null);

  // Load HTML content
  useEffect(() => {
    setLoading(true);
    setError(null);
    builderReadPreview(projectDir)
      .then((html) => {
        setHtmlContent(html);
        setLoading(false);
      })
      .catch((err) => {
        setError(typeof err === "string" ? err : String(err));
        setLoading(false);
      });
  }, [projectDir, reloadKey]);

  // Calculate scale to fit viewport in container
  const updateScale = useCallback(() => {
    if (!wrapperRef.current) return;
    const containerWidth = wrapperRef.current.clientWidth - 32; // padding
    const vp = VIEWPORTS[viewport];
    const newScale = Math.min(1, containerWidth / vp.width);
    setScale(newScale);
  }, [viewport]);

  useEffect(() => {
    updateScale();
    const observer = new ResizeObserver(updateScale);
    if (wrapperRef.current) observer.observe(wrapperRef.current);
    return () => observer.disconnect();
  }, [updateScale]);

  const vp = VIEWPORTS[viewport];
  const scaledHeight = vp.height * scale;

  return (
    <div
      style={{
        background: BG,
        border: "1px solid " + BORDER,
        borderRadius: 8,
        overflow: "hidden",
        display: "flex",
        flexDirection: "column" as const,
        fontFamily: "system-ui, -apple-system, sans-serif",
      }}
    >
      {/* Toolbar */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "8px 12px",
          borderBottom: "1px solid " + BORDER,
          background: BG_SURFACE,
          flexShrink: 0,
        }}
      >
        <div style={{ display: "flex", gap: 4 }}>
          {(Object.keys(VIEWPORTS) as Viewport[]).map((vk) => (
            <button
              key={vk}
              onClick={() => setViewport(vk)}
              title={VIEWPORTS[vk].label + " (" + VIEWPORTS[vk].width + "\u00D7" + VIEWPORTS[vk].height + ")"}
              style={{
                padding: "4px 10px",
                borderRadius: 4,
                border:
                  viewport === vk
                    ? "1px solid " + ACCENT
                    : "1px solid " + BORDER,
                background:
                  viewport === vk ? "rgba(88,166,255,0.15)" : "transparent",
                color: viewport === vk ? ACCENT : TEXT_SEC,
                fontSize: 12,
                fontWeight: viewport === vk ? 600 : 400,
                cursor: "pointer",
                display: "flex",
                alignItems: "center",
                gap: 4,
              }}
            >
              <span style={{ fontSize: 14 }}>{VIEWPORTS[vk].icon}</span>
              <span>{VIEWPORTS[vk].label}</span>
            </button>
          ))}
        </div>

        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <span
            style={{ fontSize: 11, color: TEXT_SEC, fontFamily: "monospace" }}
          >
            {vp.width}&times;{vp.height}
            {scale < 1 && (
              <span style={{ color: "#484f58" }}>
                {" "}
                ({Math.round(scale * 100)}%)
              </span>
            )}
          </span>
          <button
            onClick={() => {
              // Force reload by incrementing a local counter
              setHtmlContent(null);
              setLoading(true);
              builderReadPreview(projectDir)
                .then((html) => {
                  setHtmlContent(html);
                  setLoading(false);
                })
                .catch((err) => {
                  setError(String(err));
                  setLoading(false);
                });
            }}
            title="Refresh preview"
            style={{
              padding: "3px 8px",
              borderRadius: 4,
              border: "1px solid " + BORDER,
              background: "transparent",
              color: TEXT_SEC,
              fontSize: 14,
              cursor: "pointer",
            }}
          >
            {"\u21BB"}
          </button>
        </div>
      </div>

      {/* Preview viewport */}
      <div
        ref={wrapperRef}
        style={{
          display: "flex",
          justifyContent: "center",
          alignItems: "flex-start",
          overflow: "hidden",
          background: "#1a1a2e",
          padding: 16,
          minHeight: 400,
          height: scaledHeight + 32,
          transition: "height 0.3s ease",
        }}
      >
        {loading && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              height: "100%",
              color: TEXT_SEC,
              fontSize: 13,
            }}
          >
            Loading preview...
          </div>
        )}

        {error && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              height: "100%",
              color: "#f85149",
              fontSize: 13,
              textAlign: "center" as const,
              padding: 20,
            }}
          >
            {error}
          </div>
        )}

        {htmlContent && !error && !loading && (
          <div
            style={{
              width: vp.width,
              height: vp.height,
              transformOrigin: "top center",
              transform: `scale(${scale})`,
              transition: "transform 0.3s ease, width 0.3s ease, height 0.3s ease",
              border: "1px solid " + BORDER,
              borderRadius: 4,
              overflow: "hidden",
              background: "#ffffff",
              flexShrink: 0,
            }}
          >
            <iframe
              srcDoc={htmlContent}
              sandbox="allow-scripts"
              title="Website Preview"
              style={{
                width: "100%",
                height: "100%",
                border: "none",
                display: "block",
              }}
            />
          </div>
        )}
      </div>
    </div>
  );
}

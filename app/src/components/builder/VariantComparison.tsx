/**
 * VariantComparison — side-by-side comparison of 3 generated variants.
 *
 * Shows 3 iframe previews in a CSS Grid. User picks the winner.
 * - Desktop: 3 columns side by side
 * - Tablet: 2 columns + 1 below
 * - Mobile: tab navigation between variants
 *
 * No external libraries — pure CSS Grid + iframes via srcdoc.
 */

import { useState, useCallback } from "react";
import type { VariantPayload } from "../../api/backend";

const C = {
  bg: "#0a0e14",
  surface: "#111820",
  border: "#1a2332",
  borderActive: "#00d4aa",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  accentGlow: "rgba(0,212,170,0.25)",
  sans: "system-ui,-apple-system,sans-serif",
};

interface VariantComparisonProps {
  variants: VariantPayload[];
  onSelect: (variantId: string, html: string) => void;
  onBack: () => void;
  onRegenerate: () => void;
  loading?: boolean;
}

export default function VariantComparison({
  variants,
  onSelect,
  onBack,
  onRegenerate,
  loading,
}: VariantComparisonProps) {
  const [mobileTab, setMobileTab] = useState(0);
  const [hoveredId, setHoveredId] = useState<string | null>(null);

  const handleSelect = useCallback(
    (v: VariantPayload) => {
      onSelect(v.id, v.assembled_html);
    },
    [onSelect],
  );

  return (
    <div
      style={{
        position: "absolute",
        inset: 0,
        background: C.bg,
        display: "flex",
        flexDirection: "column",
        zIndex: 50,
      }}
    >
      {/* Header bar */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "8px 16px",
          borderBottom: `1px solid ${C.border}`,
          minHeight: 40,
        }}
      >
        <button
          onClick={onBack}
          style={{
            background: "transparent",
            border: "none",
            color: C.muted,
            cursor: "pointer",
            fontSize: 12,
            fontFamily: C.sans,
            padding: "4px 8px",
          }}
        >
          Back to Editor
        </button>

        <span
          style={{
            color: C.text,
            fontSize: 12,
            fontWeight: 600,
            fontFamily: C.sans,
          }}
        >
          {loading ? "Generating variants..." : `${variants.length} Variants — $0.00`}
        </span>

        <button
          onClick={onRegenerate}
          disabled={loading}
          style={{
            background: loading ? "transparent" : C.accentDim,
            border: loading ? `1px solid ${C.dim}` : `1px solid ${C.accentGlow}`,
            color: loading ? C.dim : C.accent,
            cursor: loading ? "default" : "pointer",
            fontSize: 11,
            fontFamily: C.sans,
            padding: "4px 10px",
            borderRadius: 4,
          }}
        >
          Generate 3 More
        </button>
      </div>

      {/* Mobile tab bar (< 768px handled by media query, but we always render tabs) */}
      <div
        className="variant-tabs"
        style={{
          display: "none", // shown via CSS @media
          padding: "4px 16px",
          gap: 4,
          borderBottom: `1px solid ${C.border}`,
        }}
      >
        {variants.map((v, i) => (
          <button
            key={v.id}
            onClick={() => setMobileTab(i)}
            style={{
              flex: 1,
              background: mobileTab === i ? C.accentDim : "transparent",
              border:
                mobileTab === i
                  ? `1px solid ${C.accentGlow}`
                  : `1px solid ${C.border}`,
              color: mobileTab === i ? C.accent : C.muted,
              cursor: "pointer",
              fontSize: 10,
              fontFamily: C.sans,
              padding: "4px 8px",
              borderRadius: 4,
              fontWeight: mobileTab === i ? 600 : 400,
            }}
          >
            {v.label}
          </button>
        ))}
      </div>

      {/* Variant grid */}
      {loading ? (
        <div
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          <div
            style={{
              color: C.muted,
              fontSize: 13,
              fontFamily: C.sans,
              textAlign: "center",
            }}
          >
            <div
              style={{
                width: 24,
                height: 24,
                border: `2px solid ${C.dim}`,
                borderTopColor: C.accent,
                borderRadius: "50%",
                margin: "0 auto 12px",
                animation: "spin 0.8s linear infinite",
              }}
            />
            Generating 3 visual variants...
            <div style={{ fontSize: 10, color: C.dim, marginTop: 4 }}>
              Token swap only — instant, $0.00
            </div>
          </div>
        </div>
      ) : (
        <div
          className="variant-grid"
          style={{
            flex: 1,
            display: "grid",
            gridTemplateColumns: "repeat(3, 1fr)",
            gap: 8,
            padding: 8,
            overflow: "hidden",
          }}
        >
          {variants.map((v, i) => (
            <div
              key={v.id}
              className="variant-card"
              data-mobile-visible={mobileTab === i ? "true" : "false"}
              style={{
                display: "flex",
                flexDirection: "column",
                border: `1px solid ${
                  hoveredId === v.id ? C.borderActive : C.border
                }`,
                borderRadius: 6,
                overflow: "hidden",
                background: C.surface,
                transition: "border-color 0.15s ease",
              }}
              onMouseEnter={() => setHoveredId(v.id)}
              onMouseLeave={() => setHoveredId(null)}
            >
              {/* Label */}
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  padding: "6px 10px",
                  borderBottom: `1px solid ${C.border}`,
                }}
              >
                <span
                  style={{
                    color: C.text,
                    fontSize: 11,
                    fontWeight: 600,
                    fontFamily: C.sans,
                  }}
                >
                  {v.label}
                </span>
                <span
                  style={{
                    color: C.accent,
                    fontSize: 9,
                    fontFamily: C.sans,
                    background: C.accentDim,
                    padding: "1px 6px",
                    borderRadius: 3,
                  }}
                >
                  Free
                </span>
              </div>

              {/* iframe preview */}
              <div style={{ flex: 1, position: "relative", minHeight: 0 }}>
                <iframe
                  srcDoc={v.assembled_html}
                  style={{
                    width: "100%",
                    height: "100%",
                    border: "none",
                    background: "#fff",
                  }}
                  sandbox="allow-same-origin"
                  title={`Variant preview: ${v.label}`}
                />
              </div>

              {/* Select button */}
              <button
                onClick={() => handleSelect(v)}
                style={{
                  background: C.accentDim,
                  border: "none",
                  borderTop: `1px solid ${C.border}`,
                  color: C.accent,
                  cursor: "pointer",
                  fontSize: 11,
                  fontWeight: 600,
                  fontFamily: C.sans,
                  padding: "8px 0",
                  transition: "background 0.15s ease",
                }}
                onMouseEnter={(e) =>
                  (e.currentTarget.style.background = C.accentGlow)
                }
                onMouseLeave={(e) =>
                  (e.currentTarget.style.background = C.accentDim)
                }
              >
                Select
              </button>
            </div>
          ))}
        </div>
      )}

      {/* Responsive CSS (injected via style tag) */}
      <style>{`
        @keyframes spin {
          to { transform: rotate(360deg); }
        }
        @media (max-width: 1023px) {
          .variant-grid {
            grid-template-columns: repeat(2, 1fr) !important;
          }
        }
        @media (max-width: 767px) {
          .variant-tabs {
            display: flex !important;
          }
          .variant-grid {
            grid-template-columns: 1fr !important;
          }
          .variant-card[data-mobile-visible="false"] {
            display: none !important;
          }
        }
      `}</style>
    </div>
  );
}

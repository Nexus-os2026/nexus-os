/**
 * AuditTrailViewer — governance timeline for Enterprise Trust Pack.
 *
 * Chronological timeline of all governance events: builds, quality checks,
 * edits, deploys, exports. Filterable, searchable, exportable.
 */

import { useState, useCallback, useEffect } from "react";
import {
  builderGetAuditTrail,
  builderExportAuditTrail,
  builderGenerateTrustPack,
  type AuditEvent,
  type TrustPackResult,
} from "../../api/backend";

const C = {
  bg: "#0a0e14",
  surface: "#111820",
  surfaceAlt: "#0d1219",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  dim: "#3e4c5e",
  accent: "#6366f1",
  accentDim: "rgba(99,102,241,0.10)",
  green: "#3fb950",
  blue: "#3b82f6",
  orange: "#f0c040",
  red: "#f85149",
  purple: "#8b5cf6",
  sans: "system-ui,-apple-system,sans-serif",
  mono: "'JetBrains Mono','Fira Code',monospace",
};

const EVENT_COLORS: Record<string, string> = {
  BuildStarted: C.blue,
  BuildCompleted: C.green,
  QualityCheck: C.green,
  AutoFix: C.orange,
  VisualEdit: C.blue,
  TextEdit: C.blue,
  ThemeChange: C.orange,
  VariantGenerated: C.purple,
  VariantSelected: C.purple,
  BackendGenerated: C.accent,
  Deployed: C.purple,
  Rollback: C.red,
  ImageGenerated: C.accent,
  DesignImported: C.accent,
  Exported: C.green,
  Archived: C.dim,
};

interface AuditTrailViewerProps {
  projectId: string;
  onClose: () => void;
}

export default function AuditTrailViewer({
  projectId,
  onClose,
}: AuditTrailViewerProps) {
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState("");
  const [filterType, setFilterType] = useState<string | null>(null);
  const [trustPackResult, setTrustPackResult] =
    useState<TrustPackResult | null>(null);
  const [generating, setGenerating] = useState(false);

  const loadEvents = useCallback(async () => {
    setLoading(true);
    try {
      const data = await builderGetAuditTrail(
        projectId,
        filterType ?? undefined,
        search || undefined,
      );
      setEvents(data);
    } catch {
      /* ignore */
    }
    setLoading(false);
  }, [projectId, filterType, search]);

  useEffect(() => {
    loadEvents();
  }, [loadEvents]);

  const handleExport = useCallback(
    async (format: "csv" | "json") => {
      try {
        const data = await builderExportAuditTrail(projectId, format);
        const blob = new Blob([data], { type: "text/plain" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = `audit_trail.${format}`;
        a.click();
        URL.revokeObjectURL(url);
      } catch {
        /* ignore */
      }
    },
    [projectId],
  );

  const handleGenerateTrustPack = useCallback(async () => {
    setGenerating(true);
    try {
      const result = await builderGenerateTrustPack(projectId);
      setTrustPackResult(result);
    } catch {
      /* ignore */
    }
    setGenerating(false);
  }, [projectId]);

  const eventTypes = [
    "BuildStarted",
    "BuildCompleted",
    "QualityCheck",
    "AutoFix",
    "VisualEdit",
    "Deployed",
  ];

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.7)",
        zIndex: 9000,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        fontFamily: C.sans,
      }}
      onClick={onClose}
    >
      <div
        style={{
          background: C.bg,
          border: `1px solid ${C.border}`,
          borderRadius: 12,
          width: 680,
          maxHeight: "80vh",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div
          style={{
            padding: "16px 20px",
            borderBottom: `1px solid ${C.border}`,
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
          }}
        >
          <div>
            <div
              style={{ fontSize: 16, fontWeight: 600, color: C.text }}
            >
              Audit Trail
            </div>
            <div style={{ fontSize: 12, color: C.muted, marginTop: 2 }}>
              {events.length} governance events
            </div>
          </div>
          <button
            onClick={onClose}
            style={{
              background: "none",
              border: "none",
              color: C.muted,
              cursor: "pointer",
              fontSize: 18,
            }}
          >
            x
          </button>
        </div>

        {/* Controls */}
        <div
          style={{
            padding: "10px 20px",
            borderBottom: `1px solid ${C.border}`,
            display: "flex",
            gap: 8,
            alignItems: "center",
            flexWrap: "wrap",
          }}
        >
          <input
            type="text"
            placeholder="Search events..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            style={{
              background: C.surface,
              border: `1px solid ${C.border}`,
              borderRadius: 6,
              padding: "6px 10px",
              color: C.text,
              fontSize: 13,
              flex: 1,
              minWidth: 140,
              outline: "none",
            }}
          />
          <select
            value={filterType ?? ""}
            onChange={(e) =>
              setFilterType(e.target.value || null)
            }
            style={{
              background: C.surface,
              border: `1px solid ${C.border}`,
              borderRadius: 6,
              padding: "6px 10px",
              color: C.text,
              fontSize: 13,
            }}
          >
            <option value="">All Events</option>
            {eventTypes.map((t) => (
              <option key={t} value={t}>
                {t.replace(/([A-Z])/g, " $1").trim()}
              </option>
            ))}
          </select>
        </div>

        {/* Timeline */}
        <div
          style={{
            flex: 1,
            overflow: "auto",
            padding: "12px 20px",
          }}
        >
          {loading ? (
            <div style={{ color: C.muted, textAlign: "center", padding: 20 }}>
              Loading...
            </div>
          ) : events.length === 0 ? (
            <div style={{ color: C.muted, textAlign: "center", padding: 20 }}>
              No events found
            </div>
          ) : (
            [...events].reverse().map((event) => {
              const color = EVENT_COLORS[event.event_type] ?? C.dim;
              return (
                <div
                  key={event.id}
                  style={{
                    display: "flex",
                    gap: 12,
                    padding: "8px 0",
                    borderBottom: `1px solid ${C.border}`,
                  }}
                >
                  <div
                    style={{
                      width: 8,
                      height: 8,
                      borderRadius: "50%",
                      background: color,
                      marginTop: 6,
                      flexShrink: 0,
                    }}
                  />
                  <div style={{ flex: 1 }}>
                    <div
                      style={{
                        fontSize: 13,
                        color: C.text,
                        lineHeight: 1.4,
                      }}
                    >
                      {event.description}
                    </div>
                    <div
                      style={{
                        fontSize: 11,
                        color: C.muted,
                        fontFamily: C.mono,
                        marginTop: 2,
                      }}
                    >
                      {event.timestamp}
                    </div>
                  </div>
                  <div
                    style={{
                      fontSize: 11,
                      color,
                      fontWeight: 500,
                      whiteSpace: "nowrap",
                    }}
                  >
                    {event.event_type
                      .replace(/([A-Z])/g, " $1")
                      .trim()}
                  </div>
                </div>
              );
            })
          )}
        </div>

        {/* Trust Pack Result */}
        {trustPackResult && (
          <div
            style={{
              padding: "10px 20px",
              borderTop: `1px solid ${C.border}`,
              background: C.surfaceAlt,
            }}
          >
            <div style={{ fontSize: 12, color: C.green, fontWeight: 600 }}>
              Trust Pack Generated: {trustPackResult.total_files} files
              {trustPackResult.signed ? " (Ed25519 signed)" : ""}
            </div>
            <div style={{ fontSize: 11, color: C.muted, marginTop: 2 }}>
              {trustPackResult.files_generated
                .map((f) => f.filename)
                .join(", ")}
            </div>
          </div>
        )}

        {/* Footer */}
        <div
          style={{
            padding: "12px 20px",
            borderTop: `1px solid ${C.border}`,
            display: "flex",
            gap: 8,
            justifyContent: "flex-end",
          }}
        >
          <button
            onClick={() => handleExport("csv")}
            style={{
              background: C.surface,
              border: `1px solid ${C.border}`,
              borderRadius: 6,
              padding: "6px 12px",
              color: C.text,
              fontSize: 12,
              cursor: "pointer",
            }}
          >
            Export CSV
          </button>
          <button
            onClick={() => handleExport("json")}
            style={{
              background: C.surface,
              border: `1px solid ${C.border}`,
              borderRadius: 6,
              padding: "6px 12px",
              color: C.text,
              fontSize: 12,
              cursor: "pointer",
            }}
          >
            Export JSON
          </button>
          <button
            onClick={handleGenerateTrustPack}
            disabled={generating}
            style={{
              background: C.accent,
              border: "none",
              borderRadius: 6,
              padding: "6px 14px",
              color: "#fff",
              fontSize: 12,
              fontWeight: 600,
              cursor: generating ? "wait" : "pointer",
              opacity: generating ? 0.6 : 1,
            }}
          >
            {generating ? "Generating..." : "Generate Trust Pack"}
          </button>
        </div>
      </div>
    </div>
  );
}

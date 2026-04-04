import { useCallback, useEffect, useState } from "react";
import {
  builderIterate,
  builderListCheckpoints,
  builderRollback,
} from "../api/backend";

// ── Types ──

interface Checkpoint {
  id: string;
  timestamp: string;
  description: string;
  cost: number;
  parent_id: string | null;
  lines: number;
  chars: number;
}

// ── Colors ──

const BG = "#0d1117";
const BG_SURFACE = "#161b22";
const TEXT = "#e6edf3";
const TEXT_SECONDARY = "#8b949e";
const ACCENT = "#58a6ff";
const SUCCESS = "#3fb950";
const BORDER = "#30363d";

// ── Props ──

interface IterationPanelProps {
  /** The project directory (output_dir from the build). */
  projectDir: string;
  /** Called when the preview should refresh (after iteration or rollback). */
  onPreviewRefresh?: () => void;
  /** Model to use for iterations (defaults to sonnet). */
  model?: string;
}

export function IterationPanel({
  projectDir,
  onPreviewRefresh,
  model,
}: IterationPanelProps) {
  const [changeRequest, setChangeRequest] = useState("");
  const [checkpoints, setCheckpoints] = useState<Checkpoint[]>([]);
  const [currentId, setCurrentId] = useState<string | null>(null);
  const [iterating, setIterating] = useState(false);
  const [rolling, setRolling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastTier, setLastTier] = useState<{
    tier: string;
    cost: number;
    elapsed: number;
    detail: any;
  } | null>(null);

  const loadCheckpoints = useCallback(async () => {
    try {
      const cps = await builderListCheckpoints(projectDir);
      if (Array.isArray(cps)) {
        setCheckpoints(cps);
        if (cps.length > 0) {
          setCurrentId(cps[cps.length - 1].id);
        }
      }
    } catch {
      // checkpoints not available yet
    }
  }, [projectDir]);

  useEffect(() => {
    loadCheckpoints();
  }, [loadCheckpoints]);

  const handleIterate = useCallback(async () => {
    if (!changeRequest.trim() || iterating) return;
    setIterating(true);
    setError(null);
    try {
      const result = await builderIterate(projectDir, changeRequest, model);
      if (result && result.tier) {
        setLastTier({
          tier: result.tier,
          cost: result.cost ?? 0,
          elapsed: result.elapsed_seconds ?? 0,
          detail: result.tier_detail ?? null,
        });
      }
      setChangeRequest("");
      await loadCheckpoints();
      onPreviewRefresh?.();
    } catch (e: any) {
      setError(typeof e === "string" ? e : e?.message || "Iteration failed");
    } finally {
      setIterating(false);
    }
  }, [changeRequest, projectDir, model, iterating, loadCheckpoints, onPreviewRefresh]);

  const handleRollback = useCallback(
    async (cpId: string) => {
      if (rolling) return;
      setRolling(true);
      setError(null);
      try {
        await builderRollback(projectDir, cpId);
        setCurrentId(cpId);
        await loadCheckpoints();
        onPreviewRefresh?.();
      } catch (e: any) {
        setError(typeof e === "string" ? e : e?.message || "Rollback failed");
      } finally {
        setRolling(false);
      }
    },
    [projectDir, rolling, loadCheckpoints, onPreviewRefresh]
  );

  const containerStyle: React.CSSProperties = {
    background: BG,
    border: "1px solid " + BORDER,
    borderRadius: 8,
    padding: 16,
    fontFamily: "system-ui, -apple-system, sans-serif",
  };

  return (
    <div style={containerStyle}>
      {/* Iteration input */}
      <div style={{ marginBottom: 14 }}>
        <div
          style={{
            fontSize: 13,
            fontWeight: 700,
            color: TEXT,
            marginBottom: 8,
          }}
        >
          Iterate on Build
        </div>
        <textarea
          value={changeRequest}
          onChange={(e) => setChangeRequest(e.target.value)}
          placeholder="What would you like to change? e.g., 'Change the color scheme to ocean blue and teal'"
          rows={2}
          disabled={iterating}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              handleIterate();
            }
          }}
          style={{
            width: "100%",
            padding: 10,
            borderRadius: 6,
            border: "1px solid " + BORDER,
            background: BG_SURFACE,
            color: TEXT,
            fontSize: 13,
            fontFamily: "inherit",
            resize: "vertical",
            boxSizing: "border-box" as const,
            outline: "none",
            opacity: iterating ? 0.5 : 1,
          }}
        />
        <div
          style={{ display: "flex", gap: 8, marginTop: 8, alignItems: "center" }}
        >
          <button
            onClick={handleIterate}
            disabled={iterating || !changeRequest.trim()}
            style={{
              padding: "6px 16px",
              borderRadius: 6,
              border: "none",
              background:
                iterating || !changeRequest.trim() ? "#21262d" : ACCENT,
              color:
                iterating || !changeRequest.trim() ? TEXT_SECONDARY : "#fff",
              fontSize: 12,
              fontWeight: 600,
              cursor:
                iterating || !changeRequest.trim() ? "not-allowed" : "pointer",
            }}
          >
            {iterating ? "Generating..." : "Apply Change"}
          </button>
          {iterating && (
            <span style={{ fontSize: 11, color: TEXT_SECONDARY }}>
              Streaming in progress... watch the progress bar above
            </span>
          )}
        </div>
      </div>

      {error && (
        <div
          style={{
            fontSize: 12,
            color: "#f85149",
            background: "#1f0d0d",
            border: "1px solid #3a1a1a",
            borderRadius: 6,
            padding: 8,
            marginBottom: 10,
          }}
        >
          {error}
        </div>
      )}

      {/* Last iteration tier badge */}
      {lastTier && (
        <div
          style={{
            fontSize: 11,
            padding: "6px 10px",
            borderRadius: 6,
            marginBottom: 10,
            background:
              lastTier.tier === "css_variable"
                ? "#0d2818"
                : lastTier.tier === "section_edit"
                  ? "#0d1f2d"
                  : "#1f1a0d",
            border:
              "1px solid " +
              (lastTier.tier === "css_variable"
                ? "#1a4d2e"
                : lastTier.tier === "section_edit"
                  ? "#1a3a5f"
                  : "#4d3a1a"),
            color: TEXT,
            display: "flex",
            alignItems: "center",
            gap: 8,
          }}
        >
          <span
            style={{
              fontWeight: 700,
              color:
                lastTier.tier === "css_variable"
                  ? SUCCESS
                  : lastTier.tier === "section_edit"
                    ? ACCENT
                    : "#f0ad4e",
              fontSize: 10,
              textTransform: "uppercase" as const,
              letterSpacing: 0.5,
            }}
          >
            {lastTier.tier === "css_variable"
              ? "CSS edit"
              : lastTier.tier === "section_edit"
                ? "Section edit"
                : "Full regen"}
          </span>
          <span style={{ color: TEXT_SECONDARY }}>
            {lastTier.elapsed < 1
              ? "instant"
              : `${lastTier.elapsed.toFixed(1)}s`}
          </span>
          <span
            style={{
              fontFamily: "monospace",
              color:
                lastTier.cost === 0 ? SUCCESS : TEXT_SECONDARY,
            }}
          >
            ${lastTier.cost.toFixed(4)}
          </span>
          {lastTier.tier === "css_variable" &&
            lastTier.detail?.css_changes && (
              <span style={{ color: TEXT_SECONDARY, fontSize: 10 }}>
                {(lastTier.detail.css_changes as any[])
                  .map(
                    (c: any) =>
                      `${c.variable}: ${c.old_value ?? "?"} → ${c.new_value}`
                  )
                  .join(", ")}
              </span>
            )}
          {lastTier.tier === "section_edit" &&
            lastTier.detail?.section && (
              <span style={{ color: TEXT_SECONDARY, fontSize: 10 }}>
                {lastTier.detail.action}: {lastTier.detail.section}
              </span>
            )}
        </div>
      )}

      {/* Checkpoint timeline */}
      {checkpoints.length > 0 && (
        <div>
          <div
            style={{
              fontSize: 11,
              color: TEXT_SECONDARY,
              textTransform: "uppercase" as const,
              letterSpacing: 1,
              marginBottom: 8,
            }}
          >
            Checkpoints ({checkpoints.length})
          </div>
          <div
            style={{
              maxHeight: 240,
              overflowY: "auto" as const,
              paddingRight: 4,
            }}
          >
            {checkpoints.map((cp, idx) => {
              const isCurrent =
                currentId === cp.id ||
                (!currentId && idx === checkpoints.length - 1);
              const isInitial = cp.id === "cp_001";

              return (
                <div
                  key={cp.id}
                  style={{
                    display: "flex",
                    alignItems: "flex-start",
                    gap: 10,
                    marginBottom: 2,
                  }}
                >
                  {/* Timeline line + dot */}
                  <div
                    style={{
                      display: "flex",
                      flexDirection: "column" as const,
                      alignItems: "center",
                      minWidth: 16,
                      paddingTop: 4,
                    }}
                  >
                    <div
                      style={{
                        width: 8,
                        height: 8,
                        borderRadius: "50%",
                        background: isCurrent ? ACCENT : SUCCESS,
                        border: isCurrent
                          ? "2px solid " + ACCENT
                          : "2px solid " + BORDER,
                        flexShrink: 0,
                      }}
                    />
                    {idx < checkpoints.length - 1 && (
                      <div
                        style={{
                          width: 1,
                          height: 28,
                          background: BORDER,
                        }}
                      />
                    )}
                  </div>

                  {/* Checkpoint info */}
                  <div
                    style={{
                      flex: 1,
                      padding: "4px 8px",
                      borderRadius: 4,
                      background: isCurrent ? "#0d1f2d" : "transparent",
                      border: isCurrent
                        ? "1px solid " + ACCENT
                        : "1px solid transparent",
                      cursor: isCurrent || rolling ? "default" : "pointer",
                    }}
                    onClick={() => {
                      if (!isCurrent && !rolling) handleRollback(cp.id);
                    }}
                    title={
                      isCurrent
                        ? "Current version"
                        : `Click to rollback to ${cp.id}`
                    }
                  >
                    <div
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                      }}
                    >
                      <span
                        style={{
                          fontSize: 12,
                          fontWeight: 600,
                          color: isCurrent ? ACCENT : TEXT,
                        }}
                      >
                        {cp.id}
                        {isInitial && (
                          <span
                            style={{
                              fontSize: 9,
                              color: SUCCESS,
                              marginLeft: 6,
                              fontWeight: 400,
                            }}
                          >
                            INITIAL
                          </span>
                        )}
                        {isCurrent && (
                          <span
                            style={{
                              fontSize: 9,
                              color: ACCENT,
                              marginLeft: 6,
                              fontWeight: 400,
                            }}
                          >
                            CURRENT
                          </span>
                        )}
                      </span>
                      {cp.cost > 0 && (
                        <span
                          style={{
                            fontSize: 10,
                            color: TEXT_SECONDARY,
                            fontFamily: "monospace",
                          }}
                        >
                          ${cp.cost.toFixed(4)}
                        </span>
                      )}
                    </div>
                    <div
                      style={{
                        fontSize: 11,
                        color: TEXT_SECONDARY,
                        marginTop: 2,
                      }}
                    >
                      {cp.description}
                      {cp.lines > 0 && (
                        <span style={{ marginLeft: 8, color: "#484f58" }}>
                          {cp.lines} lines
                        </span>
                      )}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}

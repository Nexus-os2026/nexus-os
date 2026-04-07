/**
 * CostIndicator — persistent cost display for the builder.
 *
 * Shows:
 * - Current operation cost (e.g., "$0.00" during local ops)
 * - Running total for the project
 * - Per-operation breakdown
 *
 * Always visible. Small, unobtrusive. Positioned in the toolbar.
 */

const C = {
  dim: "#3e4c5e",
  accent: "#00d4aa",
  muted: "#94a3b8",
  mono: "'JetBrains Mono','Fira Code','Cascadia Code',monospace",
};

interface CostIndicatorProps {
  /** Current operation cost (live, during build). */
  currentCost?: number;
  /** Total project cost accumulated. */
  totalCost: number;
  /** Number of builds. */
  buildCount: number;
  /** Number of edits. */
  editCount: number;
  /** Whether a build is in progress. */
  building?: boolean;
}

export default function CostIndicator({
  currentCost,
  totalCost,
  buildCount,
  editCount,
  building,
}: CostIndicatorProps) {
  const isFree = totalCost === 0 && (currentCost === undefined || currentCost === 0);
  const costColor = isFree ? C.accent : C.muted;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 6,
        fontSize: 10,
        fontFamily: C.mono,
        color: C.dim,
      }}
    >
      {building && currentCost !== undefined && (
        <span style={{ color: costColor }}>
          {currentCost === 0 ? "$0 (free)" : `$${currentCost.toFixed(4)}`}
        </span>
      )}
      {!building && (buildCount > 0 || editCount > 0) && (
        <span style={{ color: costColor }} title={`${buildCount} build${buildCount !== 1 ? "s" : ""}, ${editCount} edit${editCount !== 1 ? "s" : ""}`}>
          Total: ${totalCost.toFixed(4)}
        </span>
      )}
    </div>
  );
}

/**
 * Track C #2: cross-page swarm status indicator.
 *
 * Floating fixed-position badge surfacing active-run state and
 * pending-plan state to all 88 pages. Suppressed on /agents (where
 * richer chrome already exists). Click → navigate to /agents.
 *
 * Behaviour:
 *   - currentPage === "agents"            → null
 *   - terminal-state hold (5s)            → "COMPLETED ✓" / "CANCELLED"
 *   - activeRun present                   → "RUNNING · {done}/{total}"
 *   - selectIsPlanPending                 → "AWAITING APPROVAL"
 *   - else                                → null
 *
 * Terminal state is captured by direct swarmBus subscription —
 * `swarm_completed` / `swarm_cancelled` clear `activeRun` immediately
 * in the store dispatcher (`store.ts:284–290`), so this component
 * holds its own 5s fade-out window in local state to give the user a
 * closing acknowledgement.
 */

import { useEffect, useState } from "react";
import { useUiAudio } from "../../audio/soundEngine";
import {
  selectActiveRun,
  selectIsPlanPending,
  selectRunProgress,
} from "../../lib/swarm/selectors";
import { useSwarmStore } from "../../lib/swarm/store";
import { swarmBus } from "../../lib/swarm/swarm_bus";

type TerminalKind = "completed" | "cancelled";

const TERMINAL_HOLD_MS = 5_000;

const COLORS: Record<
  "running" | "pending" | TerminalKind,
  { fg: string; border: string; bg: string }
> = {
  pending: {
    fg: "var(--nexus-purple, #8c7bff)",
    border: "rgba(140, 123, 255, 0.55)",
    bg: "rgba(140, 123, 255, 0.12)",
  },
  running: {
    fg: "var(--nexus-accent, #4af7d3)",
    border: "rgba(74, 247, 211, 0.55)",
    bg: "rgba(74, 247, 211, 0.10)",
  },
  completed: {
    fg: "#2ad39d",
    border: "rgba(42, 211, 157, 0.55)",
    bg: "rgba(42, 211, 157, 0.12)",
  },
  cancelled: {
    fg: "#94a3b8",
    border: "rgba(100, 116, 139, 0.55)",
    bg: "rgba(100, 116, 139, 0.12)",
  },
};

function prefersReducedMotion(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export interface SwarmStatusBadgeProps {
  readonly onNavigate: (page: string) => void;
  readonly currentPage: string;
}

export default function SwarmStatusBadge({
  onNavigate,
  currentPage,
}: SwarmStatusBadgeProps): JSX.Element | null {
  const activeRun = useSwarmStore(selectActiveRun);
  const planPending = useSwarmStore(selectIsPlanPending);
  const progress = useSwarmStore(selectRunProgress);
  const { play } = useUiAudio();

  const [terminal, setTerminal] = useState<TerminalKind | null>(null);

  useEffect(() => {
    // Subscribe to terminal events directly. The store dispatcher
    // clears `activeRun` synchronously on these, so we cannot derive
    // the terminal hold from store state alone — we have to catch the
    // event itself.
    let timer: ReturnType<typeof setTimeout> | null = null;
    const unsubscribe = swarmBus.subscribe((ev) => {
      if (ev.event === "swarm_completed" || ev.event === "swarm_cancelled") {
        const kind: TerminalKind =
          ev.event === "swarm_completed" ? "completed" : "cancelled";
        setTerminal(kind);
        if (timer !== null) clearTimeout(timer);
        timer = setTimeout(() => {
          setTerminal(null);
          timer = null;
        }, TERMINAL_HOLD_MS);
      }
    });
    return () => {
      unsubscribe();
      if (timer !== null) clearTimeout(timer);
    };
  }, []);

  // Render gates.
  if (currentPage === "agents") return null;

  let label: string;
  let kind: "running" | "pending" | TerminalKind;
  if (terminal !== null) {
    kind = terminal;
    label = terminal === "completed" ? "COMPLETED ✓" : "CANCELLED";
  } else if (activeRun !== null) {
    kind = "running";
    label = `RUNNING · ${progress.done}/${progress.total}`;
  } else if (planPending) {
    kind = "pending";
    label = "AWAITING APPROVAL";
  } else {
    return null;
  }

  const palette = COLORS[kind];
  const reduceMotion = prefersReducedMotion();

  const ariaLabel = (() => {
    switch (kind) {
      case "running":
        return `Swarm running, ${progress.done} of ${progress.total} nodes complete. Click to view.`;
      case "pending":
        return "Swarm plan awaiting approval. Click to view.";
      case "completed":
        return "Swarm run completed. Click to view.";
      case "cancelled":
        return "Swarm run cancelled. Click to view.";
    }
  })();

  const handleClick = (): void => {
    play("click");
    onNavigate("agents");
  };

  return (
    <button
      type="button"
      data-testid="swarm-status-badge"
      data-state={kind}
      aria-label={ariaLabel}
      onClick={handleClick}
      className="nexus-topbar-chip"
      style={{
        position: "fixed",
        top: 16,
        right: 16,
        zIndex: 40,
        cursor: "pointer",
        color: palette.fg,
        background: palette.bg,
        border: `1px solid ${palette.border}`,
        animation:
          kind === "running" && !reduceMotion
            ? "swarm-node-pulse 1.5s ease-out infinite"
            : undefined,
      }}
    >
      <span
        className="nexus-topbar-chip__signal"
        style={{ background: palette.fg }}
      />
      <span data-testid="swarm-status-badge-label">{label}</span>
    </button>
  );
}

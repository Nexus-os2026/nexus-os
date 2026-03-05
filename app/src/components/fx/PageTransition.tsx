import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";

interface PageTransitionProps {
  pageKey: string;
  children: ReactNode;
}

type TransitionPhase = "idle" | "out" | "in";

export function PageTransition({ pageKey, children }: PageTransitionProps): JSX.Element {
  const [phase, setPhase] = useState<TransitionPhase>("idle");
  const prevKeyRef = useRef(pageKey);
  const timerRef = useRef<number>(0);

  useEffect(() => {
    if (prevKeyRef.current === pageKey) {
      return;
    }
    prevKeyRef.current = pageKey;

    // Clear any pending timers from previous transitions
    window.clearTimeout(timerRef.current);

    setPhase("out");
    timerRef.current = window.setTimeout(() => {
      setPhase("in");
      timerRef.current = window.setTimeout(() => {
        setPhase("idle");
      }, 230);
    }, 170);

    return () => {
      window.clearTimeout(timerRef.current);
    };
  }, [pageKey]);

  return (
    <div className={`page-transition page-transition-${phase}`}>
      <div className="page-transition__layer">{children}</div>
      <div className="page-transition__cyber-wipe" aria-hidden="true" />
      <div className="page-transition__data-dissolve" aria-hidden="true" />
    </div>
  );
}

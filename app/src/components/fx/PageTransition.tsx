import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";

interface PageTransitionProps {
  pageKey: string;
  children: ReactNode;
}

type TransitionPhase = "idle" | "out" | "in";

export function PageTransition({ pageKey, children }: PageTransitionProps): JSX.Element {
  const [renderedChildren, setRenderedChildren] = useState(children);
  const [phase, setPhase] = useState<TransitionPhase>("idle");
  const prevKeyRef = useRef(pageKey);

  useEffect(() => {
    if (prevKeyRef.current === pageKey) {
      return;
    }
    prevKeyRef.current = pageKey;

    setPhase("out");
    const swapTimer = window.setTimeout(() => {
      setRenderedChildren(children);
      setPhase("in");
    }, 170);

    const settleTimer = window.setTimeout(() => {
      setPhase("idle");
    }, 400);

    return () => {
      window.clearTimeout(swapTimer);
      window.clearTimeout(settleTimer);
    };
  }, [children, pageKey]);

  useEffect(() => {
    if (phase === "idle") {
      setRenderedChildren(children);
    }
  }, [children, phase]);

  return (
    <div className={`page-transition page-transition-${phase}`}>
      <div className="page-transition__layer">{renderedChildren}</div>
      <div className="page-transition__cyber-wipe" aria-hidden="true" />
      <div className="page-transition__data-dissolve" aria-hidden="true" />
    </div>
  );
}

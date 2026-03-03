import { useEffect, useRef } from "react";
import type { ReactNode } from "react";

interface HoloPanelProps {
  children: ReactNode;
  className?: string;
  depth?: "background" | "mid" | "foreground";
  title?: string;
}

function depthFactor(depth: HoloPanelProps["depth"]): number {
  if (depth === "background") {
    return 0.4;
  }
  if (depth === "foreground") {
    return 1;
  }
  return 0.7;
}

export function HoloPanel({
  children,
  className = "",
  depth = "mid",
  title
}: HoloPanelProps): JSX.Element {
  const ref = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const element = ref.current;
    if (!element) {
      return;
    }

    const factor = depthFactor(depth);

    const onMove = (event: MouseEvent): void => {
      const rect = element.getBoundingClientRect();
      const x = (event.clientX - rect.left) / rect.width - 0.5;
      const y = (event.clientY - rect.top) / rect.height - 0.5;
      element.style.setProperty("--holo-shift-x", `${x * 12 * factor}px`);
      element.style.setProperty("--holo-shift-y", `${y * 10 * factor}px`);
      element.style.setProperty("--holo-tilt", `${x * 3.5 * factor}deg`);
    };

    const onLeave = (): void => {
      element.style.setProperty("--holo-shift-x", "0px");
      element.style.setProperty("--holo-shift-y", "0px");
      element.style.setProperty("--holo-tilt", "0deg");
    };

    element.addEventListener("mousemove", onMove);
    element.addEventListener("mouseleave", onLeave);

    return () => {
      element.removeEventListener("mousemove", onMove);
      element.removeEventListener("mouseleave", onLeave);
    };
  }, [depth]);

  return (
    <section
      ref={ref}
      className={`holo-panel holo-panel-${depth} ${className}`.trim()}
      data-depth={depth}
    >
      <div className="holo-panel__refraction" />
      <div className="holo-panel__grain" />
      {title ? (
        <header className="holo-panel__header">
          <span className="holo-panel__title">{title}</span>
        </header>
      ) : null}
      <div className="holo-panel__content">{children}</div>
    </section>
  );
}

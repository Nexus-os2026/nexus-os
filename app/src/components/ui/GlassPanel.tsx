import type { PropsWithChildren, ReactNode } from "react";

interface GlassPanelProps extends PropsWithChildren {
  title?: string;
  subtitle?: string;
  headerRight?: ReactNode;
  className?: string;
  contentClassName?: string;
}

function joinClasses(...classes: Array<string | undefined>): string {
  return classes.filter((value) => value && value.length > 0).join(" ");
}

export function GlassPanel({
  title,
  subtitle,
  headerRight,
  className,
  contentClassName,
  children
}: GlassPanelProps): JSX.Element {
  const rootClass = joinClasses("glass-panel fade-slide-up", className);
  const bodyClass = joinClasses("glass-panel__content", contentClassName);

  return (
    <section className={rootClass}>
      {title || subtitle || headerRight ? (
        <header className="glass-panel__header">
          <div className="glass-panel__title-wrap">
            <span className="glass-panel__hex" />
            <div>
              {title ? <h3 className="glass-panel__title">{title}</h3> : null}
              {subtitle ? <p className="glass-panel__subtitle">{subtitle}</p> : null}
            </div>
          </div>
          {headerRight}
        </header>
      ) : null}
      <div className={bodyClass}>{children}</div>
    </section>
  );
}

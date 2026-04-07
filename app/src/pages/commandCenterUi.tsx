import type { CSSProperties, ReactNode } from "react";

export const commandPageStyle: CSSProperties = {
  padding: 24,
  color: "#f8fafc",
  maxWidth: 1480,
  margin: "0 auto",
};

export const commandHeaderMetaStyle: CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  gap: 14,
  fontSize: "0.78rem",
  color: "#94a3b8",
  letterSpacing: "0.08em",
  textTransform: "uppercase",
};

export const commandSurfaceStyle: CSSProperties = {
  background: "linear-gradient(180deg, rgba(8, 15, 26, 0.92), rgba(6, 11, 19, 0.86))",
  border: "1px solid rgba(0, 255, 255, 0.12)",
  borderRadius: 18,
  padding: 18,
  boxShadow: "0 22px 60px -36px rgba(0, 255, 204, 0.35), inset 0 1px 0 rgba(255, 255, 255, 0.03)",
  backdropFilter: "blur(12px)",
};

export const commandInsetStyle: CSSProperties = {
  background: "rgba(8, 15, 26, 0.68)",
  border: "1px solid rgba(148, 163, 184, 0.16)",
  borderRadius: 14,
  padding: 14,
};

export const commandScrollStyle: CSSProperties = {
  overflowY: "auto",
  scrollbarWidth: "thin",
};

export const commandLabelStyle: CSSProperties = {
  fontSize: "0.72rem",
  color: "#7dd3fc",
  letterSpacing: "0.14em",
  textTransform: "uppercase",
  fontFamily: "monospace",
};

export const commandMutedStyle: CSSProperties = {
  color: "#94a3b8",
  fontSize: "0.82rem",
  lineHeight: 1.6,
};

export const commandMonoValueStyle: CSSProperties = {
  fontFamily: "monospace",
  color: "#f8fafc",
};

export function alpha(hex: string, opacity: number): string {
  const normalized = hex.replace("#", "");
  if (normalized.length !== 6) return hex;
  const r = Number.parseInt(normalized.slice(0, 2), 16);
  const g = Number.parseInt(normalized.slice(2, 4), 16);
  const b = Number.parseInt(normalized.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${opacity})`;
}

export function buttonStyle(accent = "#00ffcc", destructive = false): CSSProperties {
  const color = destructive ? "#f87171" : accent;
  return {
    borderRadius: 10,
    border: `1px solid ${alpha(color, 0.65)}`,
    background: alpha(color, destructive ? 0.1 : 0.08),
    color,
    fontFamily: "monospace",
    fontSize: "0.78rem",
    fontWeight: 700,
    letterSpacing: "0.08em",
    textTransform: "uppercase",
    padding: "10px 14px",
    cursor: "pointer",
    transition: "all 0.18s ease",
  };
}

export const inputStyle: CSSProperties = {
  width: "100%",
  background: "rgba(4, 10, 18, 0.92)",
  color: "#f8fafc",
  border: "1px solid rgba(0, 255, 255, 0.16)",
  borderRadius: 10,
  padding: "11px 12px",
  fontFamily: "monospace",
  fontSize: "0.82rem",
  outline: "none",
  boxSizing: "border-box",
};

export const textareaStyle: CSSProperties = {
  ...inputStyle,
  resize: "vertical",
  minHeight: 96,
};

export function formatTimestamp(timestamp?: number | null, style: "full" | "short" = "full"): string {
  if (!timestamp) return "Never";
  const millis = timestamp > 1_000_000_000_000 ? timestamp : timestamp * 1000;
  const date = new Date(millis);
  if (Number.isNaN(date.getTime())) return "Unknown";
  if (style === "short") {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return date.toLocaleString([], {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatRelative(timestamp?: number | null): string {
  if (!timestamp) return "never";
  const millis = timestamp > 1_000_000_000_000 ? timestamp : timestamp * 1000;
  const delta = Date.now() - millis;
  const minute = 60_000;
  const hour = 60 * minute;
  const day = 24 * hour;
  if (delta < minute) return "just now";
  if (delta < hour) return `${Math.max(1, Math.round(delta / minute))}m ago`;
  if (delta < day) return `${Math.max(1, Math.round(delta / hour))}h ago`;
  return `${Math.max(1, Math.round(delta / day))}d ago`;
}

export function toTitleCase(value: string): string {
  return value
    .replace(/[_-]+/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase());
}

export function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 28);
}

export function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, value));
}

export function normalizeArray<T>(value: unknown): T[] {
  return Array.isArray(value) ? (value as T[]) : [];
}

export function EntityGroup({
  title,
  items,
}: {
  title: string;
  items: string[];
}): JSX.Element {
  return (
    <div style={{ marginBottom: 14 }}>
      <div style={{ ...commandLabelStyle, marginBottom: 8 }}>{title}</div>
      {items.length > 0 ? (
        <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
          {items.map((item) => (
            <span
              key={`${title}-${item}`}
              style={{
                padding: "6px 10px",
                borderRadius: 999,
                border: "1px solid rgba(125, 211, 252, 0.18)",
                background: "rgba(8, 15, 26, 0.86)",
                color: "#e2e8f0",
                fontSize: "0.76rem",
              }}
            >
              {item}
            </span>
          ))}
        </div>
      ) : (
        <EmptyState text={`No ${title.toLowerCase()} extracted`} compact />
      )}
    </div>
  );
}

export function ActionButton({
  children,
  accent,
  destructive,
  disabled,
  onClick,
}: {
  children: ReactNode;
  accent?: string;
  destructive?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}): JSX.Element {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      style={{
        ...buttonStyle(accent, destructive),
        opacity: disabled ? 0.48 : 1,
        cursor: disabled ? "not-allowed" : "pointer",
      }}
    >
      {children}
    </button>
  );
}

export function Panel({
  title,
  accent = "#00ffcc",
  children,
  action,
  style,
}: {
  title: string;
  accent?: string;
  children: ReactNode;
  action?: ReactNode;
  style?: CSSProperties;
}): JSX.Element {
  return (
    <section style={{ ...commandSurfaceStyle, ...style }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12, marginBottom: 14 }}>
        <h2
          style={{
            margin: 0,
            fontSize: "0.95rem",
            fontFamily: "monospace",
            color: accent,
            letterSpacing: "0.16em",
            textTransform: "uppercase",
          }}
        >
          {title}
        </h2>
        {action}
      </div>
      {children}
    </section>
  );
}

export function StatusDot({ color }: { color: string }): JSX.Element {
  return (
    <span
      style={{
        width: 10,
        height: 10,
        borderRadius: "50%",
        background: color,
        boxShadow: `0 0 16px ${alpha(color, 0.8)}`,
        display: "inline-block",
        flexShrink: 0,
      }}
    />
  );
}

export function MetricBar({
  value,
  color = "#00ffcc",
  height = 8,
}: {
  value: number;
  color?: string;
  height?: number;
}): JSX.Element {
  return (
    <div
      style={{
        width: "100%",
        height,
        borderRadius: 999,
        overflow: "hidden",
        background: "rgba(15, 23, 42, 0.95)",
        border: "1px solid rgba(148, 163, 184, 0.12)",
      }}
    >
      <div
        style={{
          width: `${clampPercent(value)}%`,
          height: "100%",
          borderRadius: 999,
          background: `linear-gradient(90deg, ${alpha(color, 0.2)}, ${color})`,
        }}
      />
    </div>
  );
}

export function EmptyStateIcon(): JSX.Element {
  return (
    <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="#475569" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="10" />
      <path d="M8 12h8" />
    </svg>
  );
}

export function EmptyState({
  text,
  compact,
  icon,
  cta,
  onAction,
}: {
  text: string;
  compact?: boolean;
  icon?: ReactNode;
  cta?: string;
  onAction?: () => void;
}): JSX.Element {
  if (compact) {
    return (
      <div style={{ ...commandMutedStyle, padding: 0, textAlign: "left" }}>
        {text}
      </div>
    );
  }
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        padding: "48px 24px",
        textAlign: "center",
        minHeight: 200,
      }}
    >
      <div
        style={{
          width: 64,
          height: 64,
          borderRadius: 16,
          background: "rgba(6, 182, 212, 0.06)",
          border: "1px solid rgba(6, 182, 212, 0.12)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontSize: 28,
          marginBottom: 20,
          color: "#64748b",
        }}
      >
        {icon || <EmptyStateIcon />}
      </div>
      <div
        style={{
          fontSize: 15,
          fontWeight: 500,
          color: "#e2e8f0",
          marginBottom: 8,
          letterSpacing: "0.01em",
        }}
      >
        {text}
      </div>
      {cta && (
        <button
          onClick={onAction}
          style={{
            marginTop: 16,
            borderRadius: 8,
            border: "1px solid rgba(6, 182, 212, 0.5)",
            background: "rgba(6, 182, 212, 0.1)",
            color: "#06b6d4",
            fontFamily: "monospace",
            fontSize: "0.8rem",
            fontWeight: 600,
            letterSpacing: "0.06em",
            textTransform: "uppercase",
            padding: "10px 20px",
            cursor: "pointer",
            transition: "all 0.18s ease",
          }}
        >
          {cta}
        </button>
      )}
    </div>
  );
}

export function DataRow({
  label,
  value,
  valueColor,
}: {
  label: string;
  value: ReactNode;
  valueColor?: string;
}): JSX.Element {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", gap: 12, padding: "5px 0", fontSize: "0.82rem" }}>
      <span style={{ color: "#94a3b8" }}>{label}</span>
      <span style={{ ...commandMonoValueStyle, color: valueColor ?? "#f8fafc", textAlign: "right" }}>{value}</span>
    </div>
  );
}

export function CommandModal({
  open,
  title,
  accent = "#00ffcc",
  children,
  footer,
}: {
  open: boolean;
  title: string;
  accent?: string;
  children: ReactNode;
  footer?: ReactNode;
}): JSX.Element | null {
  if (!open) return null;
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(2, 6, 23, 0.76)",
        backdropFilter: "blur(8px)",
        zIndex: 60,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 24,
      }}
    >
      <div
        style={{
          width: "min(920px, 100%)",
          maxHeight: "82vh",
          overflow: "auto",
          ...commandSurfaceStyle,
          borderColor: alpha(accent, 0.4),
        }}
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12, marginBottom: 14 }}>
          <h3
            style={{
              margin: 0,
              color: accent,
              fontFamily: "monospace",
              letterSpacing: "0.16em",
              textTransform: "uppercase",
            }}
          >
            {title}
          </h3>
        </div>
        {children}
        {footer ? <div style={{ marginTop: 18 }}>{footer}</div> : null}
      </div>
    </div>
  );
}

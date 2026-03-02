import { useEffect, useState } from "react";
import type { ButtonHTMLAttributes, CSSProperties, MouseEvent } from "react";

type CyberButtonVariant = "primary" | "danger" | "ghost";

export interface CyberButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "onClick"> {
  variant?: CyberButtonVariant;
  loading?: boolean;
  onClick?: (event: MouseEvent<HTMLButtonElement>) => void;
}

interface RippleState {
  id: number;
  x: number;
  y: number;
}

function joinClasses(...classes: Array<string | undefined>): string {
  return classes.filter((value) => value && value.length > 0).join(" ");
}

export function CyberButton({
  variant = "primary",
  loading = false,
  className,
  children,
  disabled,
  onClick,
  ...rest
}: CyberButtonProps): JSX.Element {
  const [ripple, setRipple] = useState<RippleState | null>(null);

  useEffect(() => {
    if (!ripple) {
      return;
    }
    const timer = window.setTimeout(() => setRipple(null), 650);
    return () => {
      window.clearTimeout(timer);
    };
  }, [ripple]);

  const handleClick = (event: MouseEvent<HTMLButtonElement>): void => {
    const rect = event.currentTarget.getBoundingClientRect();
    setRipple({
      id: Date.now(),
      x: event.clientX - rect.left,
      y: event.clientY - rect.top
    });
    onClick?.(event);
  };

  const variantClass =
    variant === "danger" ? "cyber-btn--danger" : variant === "ghost" ? "cyber-btn--ghost" : undefined;

  return (
    <button
      type="button"
      {...rest}
      disabled={disabled || loading}
      className={joinClasses("cyber-btn", variantClass, className)}
      onClick={handleClick}
    >
      <span className="cyber-btn__label">{children}</span>
      {loading ? <span className="cyber-btn__scanner" /> : null}
      {ripple ? (
        <span
          key={ripple.id}
          className="cyber-btn__ripple"
          style={{ left: ripple.x, top: ripple.y } as CSSProperties}
        />
      ) : null}
    </button>
  );
}

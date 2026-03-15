import { useEffect } from "react";

interface SplashScreenProps {
  ready: boolean;
  visible: boolean;
  onDismiss: () => void;
}

export function SplashScreen({ ready, visible, onDismiss }: SplashScreenProps): JSX.Element | null {
  useEffect(() => {
    if (!visible) {
      return;
    }
    const timer = window.setTimeout(() => {
      onDismiss();
    }, 3000);
    return () => {
      window.clearTimeout(timer);
    };
  }, [onDismiss, visible]);

  useEffect(() => {
    if (ready && visible) {
      onDismiss();
    }
  }, [onDismiss, ready, visible]);

  if (!visible) {
    return null;
  }

  return (
    <div className="splash-screen" role="status" aria-live="polite" aria-label="NexusOS is loading">
      <div className="splash-grid" />
      <div className="splash-center">
        <svg className="splash-logo" viewBox="0 0 240 240" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
          <polygon className="splash-hex" points="120,18 207,68 207,172 120,222 33,172 33,68" />
          <path className="splash-n" d="M82 164V72h30l48 63V72h30v92h-30l-48-62v62z" />
          <g className="splash-traces">
            <line x1="120" y1="18" x2="120" y2="2" />
            <line x1="207" y1="68" x2="230" y2="56" />
            <line x1="207" y1="172" x2="230" y2="184" />
            <line x1="33" y1="68" x2="10" y2="56" />
            <line x1="33" y1="172" x2="10" y2="184" />
            <line x1="120" y1="222" x2="120" y2="238" />
          </g>
          <g className="splash-pads">
            <circle cx="120" cy="2" r="3.8" />
            <circle cx="230" cy="56" r="3.8" />
            <circle cx="230" cy="184" r="3.8" />
            <circle cx="10" cy="56" r="3.8" />
            <circle cx="10" cy="184" r="3.8" />
            <circle cx="120" cy="238" r="3.8" />
          </g>
        </svg>

        <h1 className="splash-title">NexusOS</h1>
        <p className="splash-tagline">Don&apos;t trust. Verify.</p>
        <p className="splash-version">v8.0.0</p>

        <div className="splash-loader" aria-hidden="true">
          <div className="splash-loader-scan" />
          <div className="splash-loader-circuits" />
        </div>
      </div>
    </div>
  );
}

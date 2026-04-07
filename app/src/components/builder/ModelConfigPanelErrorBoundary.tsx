import { Component, type ErrorInfo, type ReactNode } from "react";

const C = {
  surface: "#111820",
  border: "#1a2332",
  text: "#e2e8f0",
  muted: "#94a3b8",
  err: "#f85149",
  accent: "#00d4aa",
  accentDim: "rgba(0,212,170,0.10)",
  mono: "'JetBrains Mono','Fira Code','Cascadia Code',monospace",
};

interface Props {
  children: ReactNode;
  onClose: () => void;
}

interface State {
  hasError: boolean;
  error: string | null;
}

export default class ModelConfigPanelErrorBoundary extends Component<Props, State> {
  public state: State = { hasError: false, error: null };

  public static getDerivedStateFromError(error: unknown): State {
    return {
      hasError: true,
      error: error instanceof Error ? error.message : String(error),
    };
  }

  public componentDidCatch(error: unknown, info: ErrorInfo): void {
    const msg = error instanceof Error ? error.message : String(error);
    const stack = error instanceof Error ? (error.stack ?? "") : "";
    console.error("[ModelConfigPanel] crashed:", msg, info.componentStack);
    try {
      const invoke = (window as any).__TAURI_INTERNALS__?.invoke;
      if (invoke) {
        invoke("log_frontend_error", {
          message: `[ModelConfigPanel] ${msg}`,
          stack,
          componentStack: info.componentStack ?? "",
        }).catch(() => {});
      }
    } catch { /* */ }
  }

  public render(): ReactNode {
    if (!this.state.hasError) {
      return this.props.children;
    }

    return (
      <div
        style={{
          width: 340,
          background: C.surface,
          border: `1px solid ${C.border}`,
          borderRadius: 8,
          boxShadow: "0 8px 32px rgba(0,0,0,0.5), 0 2px 8px rgba(0,0,0,0.3)",
          padding: 16,
          display: "flex",
          flexDirection: "column",
          gap: 10,
        }}
      >
        <div style={{ fontSize: 13, fontWeight: 700, color: C.text, fontFamily: C.mono }}>
          MODEL CONFIGURATION
        </div>
        <div style={{ fontSize: 12, color: C.err, fontFamily: C.mono }}>
          {"\u26A0\uFE0F"} Model panel failed to load.
        </div>
        {this.state.error && (
          <div style={{ fontSize: 11, color: C.muted, fontFamily: C.mono, wordBreak: "break-word" }}>
            {this.state.error}
          </div>
        )}
        <div style={{ display: "flex", gap: 8 }}>
          <button
            onClick={() => this.setState({ hasError: false, error: null })}
            style={{
              background: C.accentDim,
              color: C.accent,
              border: `1px solid ${C.border}`,
              borderRadius: 4,
              padding: "6px 12px",
              fontSize: 11,
              fontFamily: C.mono,
              fontWeight: 600,
              cursor: "pointer",
            }}
          >
            Retry
          </button>
          <button
            onClick={this.props.onClose}
            style={{
              background: "transparent",
              color: C.muted,
              border: `1px solid ${C.border}`,
              borderRadius: 4,
              padding: "6px 12px",
              fontSize: 11,
              fontFamily: C.mono,
              cursor: "pointer",
            }}
          >
            Close
          </button>
        </div>
      </div>
    );
  }
}

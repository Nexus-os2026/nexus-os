import { Component, type ErrorInfo, type ReactNode } from "react";

type PageErrorBoundaryProps = {
  children: ReactNode;
  pageLabel: string;
  onOpenSafePage: () => void;
};

type PageErrorBoundaryState = {
  hasError: boolean;
  message: string | null;
};

export default class PageErrorBoundary extends Component<
  PageErrorBoundaryProps,
  PageErrorBoundaryState
> {
  public state: PageErrorBoundaryState = {
    hasError: false,
    message: null,
  };

  public static getDerivedStateFromError(error: unknown): PageErrorBoundaryState {
    return {
      hasError: true,
      message: error instanceof Error ? error.message : "Unknown page error",
    };
  }

  public componentDidCatch(error: unknown, info: ErrorInfo): void {
    const msg = error instanceof Error ? error.message : String(error);
    const stack = error instanceof Error ? (error.stack ?? "") : "";
    const componentStack = info.componentStack ?? "";
    console.error(`[PageErrorBoundary] ${this.props.pageLabel}:`, msg, stack);
    // Log to backend (fire-and-forget)
    try {
      const invoke = (window as any).__TAURI_INTERNALS__?.invoke;
      if (invoke) {
        invoke("log_frontend_error", { message: `[${this.props.pageLabel}] ${msg}`, stack, componentStack }).catch(() => {});
      }
    } catch { /* */ }
  }

  public render(): ReactNode {
    if (!this.state.hasError) {
      return this.props.children;
    }

    return (
      <section style={{ maxWidth: 896, margin: "0 auto", display: "flex", flexDirection: "column", gap: 16, padding: "32px 16px" }}>
        <div style={{ borderRadius: 24, border: "1px solid rgba(251,113,133,0.35)", background: "rgba(244,63,94,0.10)", padding: 24 }}>
          <p style={{ fontSize: 12, textTransform: "uppercase", letterSpacing: "0.24em", color: "rgba(255,200,210,0.75)" }}>Page Recovery</p>
          <h2 style={{ marginTop: 8, fontSize: 24, color: "#fff1f2" }}>{this.props.pageLabel} failed to render</h2>
          <p style={{ marginTop: 12, fontSize: 14, color: "rgba(255,225,230,0.75)" }}>
            The page hit a runtime error before it could finish loading. The app shell is still healthy.
          </p>
          {this.state.message ? (
            <pre style={{ marginTop: 16, overflowX: "auto", borderRadius: 16, border: "1px solid rgba(253,164,175,0.2)", background: "rgba(2,6,23,0.55)", padding: 16, fontSize: 12, color: "rgba(255,225,230,0.8)", whiteSpace: "pre-wrap", wordBreak: "break-word" }}>
              {this.state.message}
            </pre>
          ) : null}
          <div style={{ marginTop: 20, display: "flex", flexWrap: "wrap", gap: 12 }}>
            <button type="button"
              onClick={this.props.onOpenSafePage}
              style={{ borderRadius: 9999, border: "1px solid rgba(34,211,238,0.3)", background: "rgba(6,182,212,0.10)", padding: "8px 16px", fontSize: 14, color: "#cffafe", cursor: "pointer" }}
            >
              Open Chat
            </button>
            <button type="button"
              onClick={() => this.setState({ hasError: false, message: null })}
              style={{ borderRadius: 9999, border: "1px solid rgba(148,163,184,0.3)", background: "rgba(100,116,139,0.10)", padding: "8px 16px", fontSize: 14, color: "#e2e8f0", cursor: "pointer" }}
            >
              Retry
            </button>
          </div>
        </div>
      </section>
    );
  }
}

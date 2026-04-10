import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  message: string | null;
}

export default class RootErrorBoundary extends Component<Props, State> {
  public state: State = { hasError: false, message: null };

  public static getDerivedStateFromError(error: unknown): State {
    return {
      hasError: true,
      message: error instanceof Error ? error.message : "Unknown error",
    };
  }

  public componentDidCatch(error: unknown, info: ErrorInfo): void {
    console.error("[Nexus OS] Root crash caught:", error, info);
    // Log to backend (fire-and-forget)
    try {
      const msg = error instanceof Error ? error.message : String(error);
      const stack = error instanceof Error ? (error.stack ?? "") : "";
      const componentStack = info.componentStack ?? "";
      const invoke = (window as any).__TAURI_INTERNALS__?.invoke;
      if (invoke) {
        invoke("log_frontend_error", { message: `[ROOT] ${msg}`, stack, componentStack }).catch(() => {});
      }
    } catch { /* */ }
  }

  private handleReload = () => {
    window.location.reload();
  };

  private handleClearAndReload = () => {
    try {
      localStorage.removeItem("nexus-chat-conversations");
    } catch { /* ignore */ }
    window.location.reload();
  };

  public render(): ReactNode {
    if (!this.state.hasError) {
      return this.props.children;
    }

    return (
      <div
        style={{
          minHeight: "100vh",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          background: "#0a0e1a",
          color: "#e2e8f0",
          fontFamily: "'Exo 2', system-ui, sans-serif",
          padding: 32,
        }}
      >
        <div
          style={{
            maxWidth: 520,
            width: "100%",
            background: "rgba(30,41,59,0.6)",
            border: "1px solid rgba(248,113,113,0.3)",
            borderRadius: 16,
            padding: 32,
          }}
        >
          <div style={{ fontSize: 14, textTransform: "uppercase", letterSpacing: "0.15em", color: "rgba(248,113,113,0.75)", marginBottom: 8 }}>
            System Recovery
          </div>
          <h1 style={{ margin: "0 0 12px", fontSize: 22, color: "#f1f5f9" }}>
            Nexus OS encountered an error
          </h1>
          <p style={{ color: "#94a3b8", fontSize: 14, lineHeight: 1.6, margin: "0 0 16px" }}>
            The app hit a runtime error. Your data is safe — the sidebar, agents, and backend are unaffected.
          </p>
          {this.state.message && (
            <pre
              style={{
                background: "rgba(0,0,0,0.3)",
                border: "1px solid rgba(248,113,113,0.2)",
                borderRadius: 8,
                padding: 12,
                fontSize: 12,
                color: "rgba(248,113,113,0.85)",
                overflow: "auto",
                maxHeight: 120,
                margin: "0 0 20px",
                whiteSpace: "pre-wrap",
                wordBreak: "break-word",
              }}
            >
              {this.state.message}
            </pre>
          )}
          <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
            <button type="button"
              onClick={this.handleReload}
              style={{
                padding: "10px 20px",
                background: "rgba(74,247,211,0.15)",
                border: "1px solid rgba(74,247,211,0.3)",
                borderRadius: 8,
                color: "#4af7d3",
                cursor: "pointer",
                fontWeight: 600,
                fontSize: 14,
              }}
            >
              Reload App
            </button>
            <button type="button"
              onClick={this.handleClearAndReload}
              style={{
                padding: "10px 20px",
                background: "rgba(100,116,139,0.15)",
                border: "1px solid rgba(100,116,139,0.3)",
                borderRadius: 8,
                color: "#94a3b8",
                cursor: "pointer",
                fontWeight: 600,
                fontSize: 14,
              }}
            >
              Clear Chat Cache &amp; Reload
            </button>
          </div>
        </div>
      </div>
    );
  }
}

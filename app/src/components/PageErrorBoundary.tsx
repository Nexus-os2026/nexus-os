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

  public componentDidCatch(_error: unknown, _info: ErrorInfo): void {}

  public render(): ReactNode {
    if (!this.state.hasError) {
      return this.props.children;
    }

    return (
      <section className="mx-auto flex max-w-4xl flex-col gap-4 px-4 py-8 sm:px-6">
        <div className="rounded-3xl border border-rose-400/35 bg-rose-500/10 p-6">
          <p className="text-xs uppercase tracking-[0.24em] text-rose-200/75">Page Recovery</p>
          <h2 className="mt-2 text-2xl text-rose-50">{this.props.pageLabel} failed to render</h2>
          <p className="mt-3 text-sm text-rose-100/75">
            The page hit a runtime error before it could finish loading. The app shell is still healthy.
          </p>
          {this.state.message ? (
            <pre className="mt-4 overflow-x-auto rounded-2xl border border-rose-300/20 bg-slate-950/55 p-4 text-xs text-rose-100/80">
              {this.state.message}
            </pre>
          ) : null}
          <div className="mt-5 flex flex-wrap gap-3">
            <button
              type="button"
              onClick={this.props.onOpenSafePage}
              className="rounded-full border border-cyan-400/30 bg-cyan-500/10 px-4 py-2 text-sm text-cyan-100"
            >
              Open Chat
            </button>
          </div>
        </div>
      </section>
    );
  }
}

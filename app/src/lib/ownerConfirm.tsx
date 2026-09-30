// In-page confirmation and notice dialogs.
//
// The desktop registers the official Tauri dialog plugin so the BACKEND can
// show native dialogs (Phase One project selection and owner approval). That
// plugin also replaces the webview's `window.alert` and `window.confirm` with
// asynchronous IPC calls the webview is not permitted to make, so a
// synchronous `if (window.confirm(...))` would no longer wait for the user.
// Frontend code therefore asks through these in-page dialogs instead; they
// grant nothing and are never used for Nexus authority decisions.

import { useEffect, useRef } from "react";
import { createRoot } from "react-dom/client";

interface DialogProps {
  message: string;
  confirmLabel: string;
  cancelLabel: string | null;
  onClose: (confirmed: boolean) => void;
}

function Dialog({ message, confirmLabel, cancelLabel, onClose }: DialogProps) {
  const confirmRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    confirmRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={cancelLabel ? "Confirm" : "Notice"}
      className="fixed inset-0 z-[9999] flex items-center justify-center bg-black/60"
    >
      <div className="max-w-lg w-[90vw] rounded-lg border border-slate-600 bg-slate-900 p-5 text-slate-100 shadow-xl">
        <p className="whitespace-pre-wrap break-words text-sm">{message}</p>
        <div className="mt-4 flex justify-end gap-2">
          {cancelLabel ? (
            <button
              type="button"
              className="rounded border border-slate-500 px-3 py-1 text-sm hover:bg-slate-800"
              onClick={() => onClose(false)}
            >
              {cancelLabel}
            </button>
          ) : null}
          <button
            type="button"
            ref={confirmRef}
            className="rounded bg-cyan-700 px-3 py-1 text-sm font-medium hover:bg-cyan-600"
            onClick={() => onClose(true)}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

function show(message: string, confirmLabel: string, cancelLabel: string | null): Promise<boolean> {
  return new Promise((resolve) => {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const root = createRoot(host);
    let settled = false;
    const onClose = (confirmed: boolean) => {
      if (settled) return;
      settled = true;
      root.unmount();
      host.remove();
      resolve(confirmed);
    };
    root.render(
      <Dialog
        message={message}
        confirmLabel={confirmLabel}
        cancelLabel={cancelLabel}
        onClose={onClose}
      />
    );
  });
}

/** Ask the user to confirm an action in the page. Resolves `true` only if confirmed. */
export function confirmAction(message: string, confirmLabel = "Confirm"): Promise<boolean> {
  return show(message, confirmLabel, "Cancel");
}

/** Show a notice in the page. */
export function notify(message: string): Promise<void> {
  return show(message, "OK", null).then(() => undefined);
}

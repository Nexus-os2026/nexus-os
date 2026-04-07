import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import RootErrorBoundary from "./components/RootErrorBoundary";
import "./styles/theme.css";
import "./styles/nexus-design-system.css";
import "./styles/animations.css";
import "./styles/fx.css";
import "./index.css";

// Last-resort crash handler — if React completely fails, show recovery UI
window.addEventListener("error", (e) => {
  console.error("[Nexus OS] Uncaught error:", e.error);
  try {
    const invoke = (window as any).__TAURI_INTERNALS__?.invoke;
    if (invoke) {
      invoke("log_frontend_error", {
        message: `[GLOBAL] ${e.error?.message ?? e.message ?? "Unknown"}`,
        stack: e.error?.stack ?? "",
        componentStack: "",
      }).catch(() => {});
    }
  } catch { /* */ }
  const root = document.getElementById("root");
  if (root && root.innerHTML.trim() === "") {
    root.innerHTML = `<div style="min-height:100vh;display:flex;align-items:center;justify-content:center;background:#0a0e1a;color:#e2e8f0;font-family:system-ui;padding:32px">
      <div style="max-width:480px;background:rgba(30,41,59,0.6);border:1px solid rgba(248,113,113,0.3);border-radius:16px;padding:32px">
        <div style="color:rgba(248,113,113,0.75);font-size:12px;text-transform:uppercase;letter-spacing:0.15em;margin-bottom:8px">System Recovery</div>
        <h1 style="margin:0 0 12px;font-size:20px">Nexus OS encountered an error</h1>
        <p style="color:#94a3b8;font-size:14px;margin:0 0 16px">The app crashed but your data is safe.</p>
        <pre style="background:rgba(0,0,0,0.3);border:1px solid rgba(248,113,113,0.2);border-radius:8px;padding:12px;font-size:11px;color:rgba(248,113,113,0.85);overflow:auto;max-height:100px;margin:0 0 16px">${e.message || "Unknown error"}</pre>
        <div style="display:flex;gap:10px">
          <button onclick="location.reload()" style="padding:10px 20px;background:rgba(74,247,211,0.15);border:1px solid rgba(74,247,211,0.3);border-radius:8px;color:#4af7d3;cursor:pointer;font-weight:600">Reload</button>
          <button onclick="try{localStorage.removeItem('nexus-chat-conversations')}catch(e){};location.reload()" style="padding:10px 20px;background:rgba(100,116,139,0.15);border:1px solid rgba(100,116,139,0.3);border-radius:8px;color:#94a3b8;cursor:pointer;font-weight:600">Clear Cache & Reload</button>
        </div>
      </div>
    </div>`;
  }
});

window.addEventListener("unhandledrejection", (e) => {
  console.error("[Nexus OS] Unhandled promise rejection:", e.reason);
  e.preventDefault(); // Prevent silent swallowing in Tauri WebView
  try {
    const invoke = (window as any).__TAURI_INTERNALS__?.invoke;
    if (invoke) {
      const reason = e.reason instanceof Error ? e.reason : { message: String(e.reason), stack: "" };
      invoke("log_frontend_error", {
        message: `[PROMISE] ${reason.message ?? String(e.reason)}`,
        stack: reason.stack ?? "",
        componentStack: "",
      }).catch(() => {});
    }
  } catch { /* */ }
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RootErrorBoundary>
      <App />
    </RootErrorBoundary>
  </React.StrictMode>
);

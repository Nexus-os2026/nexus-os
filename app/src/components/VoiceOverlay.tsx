export interface VoiceOverlayState {
  visible: boolean;
  listening: boolean;
  transcription: string;
  responseText: string;
}

interface VoiceOverlayProps {
  state: VoiceOverlayState;
  onDismiss: () => void;
}

export function VoiceOverlay({ state, onDismiss }: VoiceOverlayProps): JSX.Element | null {
  if (!state.visible) {
    return null;
  }

  return (
    <aside className="fixed bottom-5 right-5 z-50 w-full max-w-sm rounded-2xl border border-cyan-300/35 bg-slate-950/90 p-4 shadow-xl backdrop-blur-md">
      <div className="mb-2 flex items-center justify-between">
        <h3 className="nexus-display text-lg text-cyan-100">Jarvis Mode</h3>
        <button onClick={onDismiss} className="nexus-btn nexus-btn-secondary">Close</button>
      </div>

      <div className="mb-2 flex items-center gap-2 text-xs text-cyan-100/75">
        <span
          className={`h-2 w-2 rounded-full ${state.listening ? "animate-pulse bg-cyan-300" : "bg-slate-500"}`}
        />
        {state.listening ? "Listening" : "Idle"}
      </div>

      <div className="space-y-2 text-sm">
        <p className="rounded border border-slate-700/70 bg-slate-900/95 p-2 text-slate-100">
          <span className="font-semibold">Transcription:</span> {state.transcription || "..."}
        </p>
        <p className="rounded border border-cyan-300/20 bg-slate-950 p-2 text-slate-100">
          <span className="font-semibold">Response:</span> {state.responseText || "..."}
        </p>
      </div>
    </aside>
  );
}

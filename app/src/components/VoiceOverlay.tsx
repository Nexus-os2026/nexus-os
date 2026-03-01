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
    <aside className="fixed bottom-5 right-5 z-50 w-full max-w-sm rounded-2xl border border-slate-300 bg-white/85 p-4 shadow-xl backdrop-blur-md">
      <div className="mb-2 flex items-center justify-between">
        <h3 className="font-display text-lg text-ink">Jarvis Mode</h3>
        <button onClick={onDismiss} className="rounded bg-slate-200 px-2 py-1 text-xs">Close</button>
      </div>

      <div className="mb-2 flex items-center gap-2 text-xs text-slate-700">
        <span
          className={`h-2 w-2 rounded-full ${state.listening ? "bg-emerald-500 animate-pulse" : "bg-slate-400"}`}
        />
        {state.listening ? "Listening" : "Idle"}
      </div>

      <div className="space-y-2 text-sm">
        <p className="rounded bg-mist p-2 text-ink">
          <span className="font-semibold">Transcription:</span> {state.transcription || "..."}
        </p>
        <p className="rounded bg-slate-100 p-2 text-ink">
          <span className="font-semibold">Response:</span> {state.responseText || "..."}
        </p>
      </div>
    </aside>
  );
}

import { VoiceOrb, type VoiceOrbState } from "./fx/VoiceOrb";

export interface VoiceOverlayState {
  visible: boolean;
  listening: boolean;
  transcription: string;
  responseText: string;
  phase?: VoiceOrbState;
  amplitude?: number;
}

interface VoiceOverlayProps {
  state: VoiceOverlayState;
  onDismiss: () => void;
}

export function VoiceOverlay({ state, onDismiss }: VoiceOverlayProps): JSX.Element | null {
  if (!state.visible) {
    return null;
  }

  const phase: VoiceOrbState = state.phase ?? (state.listening ? "listening" : "idle");

  return (
    <aside className="fixed bottom-5 right-5 z-50 w-full max-w-sm rounded-2xl border border-cyan-300/35 bg-slate-950/88 p-4 shadow-xl backdrop-blur-md holo-tooltip">
      <div className="mb-2 flex items-center justify-between">
        <h3 className="nexus-display text-lg text-cyan-100">Jarvis Mode</h3>
        <button onClick={onDismiss} className="nexus-btn nexus-btn-secondary">Close</button>
      </div>

      <div className="mb-3 flex items-center gap-4 text-xs text-cyan-100/75">
        <VoiceOrb state={phase} amplitude={state.amplitude ?? 0.24} size={112} />
        <div className="grid gap-1">
          <span className="text-cyan-100/80 uppercase tracking-[0.12em]">Voice Core</span>
          <span className="text-cyan-100/65">{phase.toUpperCase()}</span>
        </div>
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

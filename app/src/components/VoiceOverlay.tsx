import { useEffect, useRef } from "react";
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
  onTranscript?: (text: string) => void;
}

/* eslint-disable @typescript-eslint/no-explicit-any */
function getSpeechRecognition(): (new () => any) | null {
  const w = window as unknown as Record<string, any>;
  return (w.SpeechRecognition ?? w.webkitSpeechRecognition ?? null) as
    | (new () => any)
    | null;
}
/* eslint-enable @typescript-eslint/no-explicit-any */

export function VoiceOverlay({ state, onDismiss, onTranscript }: VoiceOverlayProps): JSX.Element | null {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const recognitionRef = useRef<any>(null);
  const unsupportedRef = useRef(false);

  useEffect(() => {
    if (!state.visible || !state.listening) {
      if (recognitionRef.current) {
        recognitionRef.current.abort();
        recognitionRef.current = null;
      }
      return;
    }

    const SpeechRec = getSpeechRecognition();
    if (!SpeechRec) {
      unsupportedRef.current = true;
      onTranscript?.("Speech recognition not supported in this browser.");
      return;
    }

    const recognition = new SpeechRec();
    recognition.continuous = true;
    recognition.interimResults = true;
    recognition.lang = "en-US";
    recognitionRef.current = recognition;

    recognition.onresult = (event: { results: { length: number; [i: number]: { 0: { transcript: string } } } }) => {
      let transcript = "";
      for (let i = 0; i < event.results.length; i++) {
        transcript += event.results[i][0].transcript;
      }
      if (transcript.trim().length > 0) {
        onTranscript?.(transcript.trim());
      }
    };

    recognition.onerror = (event: { error: string }) => {
      if (event.error !== "aborted" && event.error !== "no-speech") {
        onTranscript?.(`Speech error: ${event.error}`);
      }
    };

    recognition.onend = () => {
      // Restart if still listening
      if (state.visible && state.listening && recognitionRef.current === recognition) {
        try {
          recognition.start();
        } catch {
          // already started or page unloaded
        }
      }
    };

    try {
      recognition.start();
    } catch {
      // already running
    }

    return () => {
      recognition.abort();
      recognitionRef.current = null;
    };
  }, [state.visible, state.listening, onTranscript]);

  if (!state.visible) {
    return null;
  }

  const phase: VoiceOrbState = state.phase ?? (state.listening ? "listening" : "idle");

  return (
    <aside className="fixed bottom-5 right-5 z-50 w-full max-w-sm rounded-2xl border border-cyan-300/35 bg-slate-950/88 p-4 shadow-xl backdrop-blur-md holo-tooltip">
      <div className="mb-2 flex items-center justify-between">
        <h3 className="nexus-display text-lg text-cyan-100">Jarvis Mode</h3>
        <button type="button" onClick={onDismiss} className="nexus-btn nexus-btn-secondary">Close</button>
      </div>

      <div className="mb-3 flex items-center gap-4 text-xs text-cyan-100/75">
        <VoiceOrb state={phase} amplitude={state.amplitude ?? 0.24} size={112} />
        <div className="grid gap-1">
          <span className="text-cyan-100/80 uppercase tracking-[0.12em]">Voice Core</span>
          <span className="text-cyan-100/65">{phase.toUpperCase()}</span>
        </div>
      </div>

      {unsupportedRef.current && (
        <p className="mb-2 rounded border border-amber-400/30 bg-amber-950/40 p-2 text-xs text-amber-200">
          Speech recognition is not supported in this browser. Use Chrome or Edge for voice input.
        </p>
      )}

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

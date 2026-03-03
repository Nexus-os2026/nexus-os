import { useCallback, useMemo, useRef, useState } from "react";

export type UiSound = "notification" | "transition" | "click" | "success" | "error";

const ENABLED_KEY = "nexus-ui-sound-enabled";
const VOLUME_KEY = "nexus-ui-sound-volume";

interface ToneEvent {
  frequency: number;
  duration: number;
  type: OscillatorType;
  gain: number;
}

function tonePattern(kind: UiSound): ToneEvent[] {
  if (kind === "notification") {
    return [{ frequency: 880, duration: 0.06, type: "sine", gain: 0.15 }];
  }
  if (kind === "transition") {
    return [
      { frequency: 320, duration: 0.06, type: "triangle", gain: 0.08 },
      { frequency: 240, duration: 0.07, type: "triangle", gain: 0.08 }
    ];
  }
  if (kind === "click") {
    return [{ frequency: 520, duration: 0.04, type: "square", gain: 0.07 }];
  }
  if (kind === "success") {
    return [
      { frequency: 440, duration: 0.05, type: "sine", gain: 0.12 },
      { frequency: 660, duration: 0.08, type: "sine", gain: 0.12 }
    ];
  }
  return [
    { frequency: 190, duration: 0.08, type: "sawtooth", gain: 0.09 },
    { frequency: 160, duration: 0.1, type: "sawtooth", gain: 0.07 }
  ];
}

function readBoolean(key: string, fallback: boolean): boolean {
  if (typeof window === "undefined") {
    return fallback;
  }
  const raw = window.localStorage.getItem(key);
  if (raw === null) {
    return fallback;
  }
  return raw === "true";
}

function readNumber(key: string, fallback: number): number {
  if (typeof window === "undefined") {
    return fallback;
  }
  const raw = window.localStorage.getItem(key);
  if (raw === null) {
    return fallback;
  }
  const value = Number(raw);
  if (Number.isNaN(value)) {
    return fallback;
  }
  return value;
}

export function useUiAudio(): {
  enabled: boolean;
  volume: number;
  setEnabled: (value: boolean) => void;
  setVolume: (value: number) => void;
  play: (sound: UiSound) => void;
} {
  const [enabled, setEnabledState] = useState<boolean>(() => readBoolean(ENABLED_KEY, false));
  const [volume, setVolumeState] = useState<number>(() => readNumber(VOLUME_KEY, 0.32));
  const contextRef = useRef<AudioContext | null>(null);

  const safeVolume = useMemo(() => Math.max(0, Math.min(1, volume)), [volume]);

  const setEnabled = useCallback((value: boolean) => {
    setEnabledState(value);
    window.localStorage.setItem(ENABLED_KEY, String(value));
  }, []);

  const setVolume = useCallback((value: number) => {
    const next = Math.max(0, Math.min(1, value));
    setVolumeState(next);
    window.localStorage.setItem(VOLUME_KEY, String(next));
  }, []);

  const play = useCallback(
    (sound: UiSound): void => {
      if (!enabled || typeof window === "undefined") {
        return;
      }
      if (!contextRef.current) {
        const AudioCtx = window.AudioContext || (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
        if (!AudioCtx) {
          return;
        }
        contextRef.current = new AudioCtx();
      }

      const ctx = contextRef.current;
      if (!ctx) {
        return;
      }

      if (ctx.state === "suspended") {
        void ctx.resume();
      }

      const pattern = tonePattern(sound);
      let cursor = ctx.currentTime;
      for (const step of pattern) {
        const oscillator = ctx.createOscillator();
        const gain = ctx.createGain();
        oscillator.type = step.type;
        oscillator.frequency.setValueAtTime(step.frequency, cursor);

        gain.gain.setValueAtTime(0.0001, cursor);
        gain.gain.exponentialRampToValueAtTime(step.gain * safeVolume, cursor + 0.01);
        gain.gain.exponentialRampToValueAtTime(0.0001, cursor + step.duration);

        oscillator.connect(gain);
        gain.connect(ctx.destination);
        oscillator.start(cursor);
        oscillator.stop(cursor + step.duration);

        cursor += step.duration * 0.92;
      }
    },
    [enabled, safeVolume]
  );

  return { enabled, volume: safeVolume, setEnabled, setVolume, play };
}

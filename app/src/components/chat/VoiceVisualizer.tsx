import { useMemo } from "react";

export type VoiceVisualizerState = "idle" | "listening" | "processing" | "speaking";

interface VoiceVisualizerProps {
  state: VoiceVisualizerState;
  level: number;
}

interface ArcBar {
  id: number;
  scale: number;
  opacity: number;
  rotation: number;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function VoiceVisualizer({ state, level }: VoiceVisualizerProps): JSX.Element {
  const safeLevel = clamp(level, 0.05, 1);
  const bars = useMemo<ArcBar[]>(
    () =>
      Array.from({ length: 18 }, (_, index) => {
        const phase = Math.abs(Math.sin((index + 1) * safeLevel * 2.4));
        const base = state === "idle" ? 0.22 : state === "processing" ? 0.42 : 0.34;
        return {
          id: index,
          scale: base + phase * (state === "speaking" ? 1.25 : state === "listening" ? 0.95 : 0.72),
          opacity: 0.3 + phase * 0.62,
          rotation: index * 20
        };
      }),
    [safeLevel, state]
  );

  return (
    <aside className={`voice-visualizer voice-visualizer--${state}`} aria-label={`Voice state: ${state}`}>
      <div className="voice-visualizer-core">
        {bars.map((bar) => (
          <span
            key={bar.id}
            className="voice-arc-bar"
            style={{
              transform: `rotate(${bar.rotation}deg) translateY(-42px) scaleY(${bar.scale})`,
              opacity: bar.opacity
            }}
          />
        ))}
        <span className="voice-visualizer-center" />
      </div>
      <p className="voice-visualizer-label">JARVIS VOICE // {state.toUpperCase()}</p>
    </aside>
  );
}

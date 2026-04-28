/**
 * Push-to-talk recorder.
 *
 * Two capture paths, picked at `stopAndTranscribe` time based on
 * `hasDesktopRuntime()`:
 *
 *   - Desktop: AudioContext + ScriptProcessor capture → WAV bytes →
 *     `transcribePushToTalk` Tauri command (Track C #3 commit 2).
 *     Local STT only; no cloud fallback.
 *   - Mock / browser dev: existing browser SpeechRecognition path.
 *     Preserved so non-desktop developers still get a working mic.
 */

import { hasDesktopRuntime, transcribePushToTalk } from "../api/backend";
import { encodeFloat32ChunksToWav } from "./wav_encoder";

export interface PushToTalkResult {
  transcript: string;
  source: "local-stt" | "web-speech" | "mock-whisper";
}

interface SpeechRecognitionResultItem {
  transcript: string;
}

interface SpeechRecognitionEventLike extends Event {
  results: ArrayLike<ArrayLike<SpeechRecognitionResultItem>>;
}

interface SpeechRecognitionLike {
  lang: string;
  interimResults: boolean;
  continuous: boolean;
  maxAlternatives: number;
  onresult: ((event: SpeechRecognitionEventLike) => void) | null;
  onerror: ((event: Event) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
}

interface WindowWithSpeechRecognition extends Window {
  SpeechRecognition?: new () => SpeechRecognitionLike;
  webkitSpeechRecognition?: new () => SpeechRecognitionLike;
}

const TARGET_SAMPLE_RATE = 16_000;
const PROCESSOR_BUFFER_SIZE = 4096;

interface DesktopCaptureSession {
  audioContext: AudioContext;
  mediaStream: MediaStream;
  sourceNode: MediaStreamAudioSourceNode;
  processorNode: ScriptProcessorNode;
  chunks: Float32Array[];
  sourceSampleRate: number;
}

export class PushToTalk {
  private isRecording = false;
  private readonly recognition: SpeechRecognitionLike | null;
  private pendingTranscript: Promise<string> | null = null;
  private settleTranscript: ((text: string) => void) | null = null;
  private heardTranscript = "";
  private desktop: DesktopCaptureSession | null = null;
  /**
   * Track C #3 commit 2: the in-flight `startDesktopCapture` promise.
   * `stopAndTranscribe` awaits it so any setup-time rejection
   * (NotAllowedError, missing AudioContext, etc.) surfaces to the
   * single consumer-facing await rather than leaking as an
   * unhandled rejection.
   */
  private desktopStart: Promise<void> | null = null;

  public constructor() {
    this.recognition = this.createRecognition();
  }

  public startRecording(): void {
    this.isRecording = true;
    if (hasDesktopRuntime()) {
      // The setup promise is awaited inside stopAndTranscribe so
      // permission/AudioContext errors land on the single
      // consumer-facing await chain. Attach a no-op `.catch` here
      // to keep Node's unhandled-rejection tracker quiet for the
      // window between start() and stop() — the real error is
      // re-thrown when stopAndTranscribe awaits this same promise.
      this.desktopStart = this.startDesktopCapture();
      this.desktopStart.catch(() => {
        // Intentional swallow; the rejection is observed via
        // stopAndTranscribe's await of the same promise reference.
      });
      return;
    }

    this.heardTranscript = "";
    if (!this.recognition) {
      return;
    }

    this.pendingTranscript = new Promise<string>((resolve) => {
      this.settleTranscript = resolve;
    });

    this.recognition.onresult = (event) => {
      const last = event.results[event.results.length - 1];
      if (last && last[0]) {
        this.heardTranscript = `${this.heardTranscript} ${last[0].transcript}`.trim();
      }
    };
    this.recognition.onerror = () => {
      this.finishRecognition("");
    };
    this.recognition.onend = () => {
      this.finishRecognition(this.heardTranscript);
    };
    this.recognition.start();
  }

  public async stopAndTranscribe(): Promise<PushToTalkResult> {
    if (!this.isRecording) {
      return { transcript: "", source: "mock-whisper" };
    }

    this.isRecording = false;

    if (hasDesktopRuntime()) {
      return this.stopDesktopCaptureAndTranscribe();
    }

    if (this.recognition && this.pendingTranscript) {
      this.recognition.stop();
      const transcript = await this.withTimeout(this.pendingTranscript, 1500);
      return { transcript, source: "web-speech" };
    }

    return { transcript: "", source: "mock-whisper" };
  }

  private async startDesktopCapture(): Promise<void> {
    if (!navigator.mediaDevices?.getUserMedia) {
      throw new Error("Microphone capture is not available in this environment.");
    }

    let mediaStream: MediaStream;
    try {
      mediaStream = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch (err) {
      const name = (err as { name?: string }).name ?? "";
      if (name === "NotAllowedError" || name === "SecurityError") {
        throw new Error("Microphone permission denied. Enable in system settings.");
      }
      if (name === "NotFoundError" || name === "OverconstrainedError") {
        throw new Error("No microphone detected.");
      }
      if (name === "NotReadableError" || name === "AbortError") {
        throw new Error("Microphone is in use by another application.");
      }
      throw new Error(
        `Microphone capture failed: ${err instanceof Error ? err.message : String(err)}`,
      );
    }

    const AudioCtor =
      window.AudioContext ??
      (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioCtor) {
      mediaStream.getTracks().forEach((track) => track.stop());
      throw new Error("AudioContext is not available in this browser.");
    }

    const audioContext = new AudioCtor();
    const sourceNode = audioContext.createMediaStreamSource(mediaStream);
    const processorNode = audioContext.createScriptProcessor(PROCESSOR_BUFFER_SIZE, 1, 1);
    const chunks: Float32Array[] = [];

    processorNode.onaudioprocess = (event: AudioProcessingEvent): void => {
      const channelData = event.inputBuffer.getChannelData(0);
      // Defensive copy — onaudioprocess buffers are reused by the engine.
      chunks.push(new Float32Array(channelData));
    };

    sourceNode.connect(processorNode);
    processorNode.connect(audioContext.destination);

    this.desktop = {
      audioContext,
      mediaStream,
      sourceNode,
      processorNode,
      chunks,
      sourceSampleRate: audioContext.sampleRate,
    };
  }

  private async stopDesktopCaptureAndTranscribe(): Promise<PushToTalkResult> {
    // Surface any setup-time rejection (permission denied, etc.)
    // before we try to read session fields.
    if (this.desktopStart) {
      const start = this.desktopStart;
      this.desktopStart = null;
      await start;
    }
    const session = this.desktop;
    this.desktop = null;
    if (!session) {
      return { transcript: "", source: "local-stt" };
    }

    try {
      session.processorNode.disconnect();
      session.sourceNode.disconnect();
    } catch {
      // disconnect throws if the node was already torn down; ignore.
    }
    session.mediaStream.getTracks().forEach((track) => track.stop());
    try {
      await session.audioContext.close();
    } catch {
      // Closing an already-closed context throws on some webviews.
    }

    const wavBytes = encodeFloat32ChunksToWav(
      session.chunks,
      session.sourceSampleRate,
      TARGET_SAMPLE_RATE,
    );

    const result = await transcribePushToTalk(wavBytes, TARGET_SAMPLE_RATE);
    return { transcript: result.text, source: "local-stt" };
  }

  /**
   * Test seam: report active recording state. Useful for assertions
   * in unit tests without exposing the private mutable field.
   */
  public recording(): boolean {
    return this.isRecording;
  }

  private createRecognition(): SpeechRecognitionLike | null {
    if (typeof window === "undefined") {
      return null;
    }

    const voiceWindow = window as WindowWithSpeechRecognition;
    const Recognition = voiceWindow.SpeechRecognition ?? voiceWindow.webkitSpeechRecognition;
    if (!Recognition) {
      return null;
    }

    const recognition = new Recognition();
    recognition.lang = "en-US";
    recognition.interimResults = false;
    recognition.continuous = false;
    recognition.maxAlternatives = 1;
    return recognition;
  }

  private finishRecognition(text: string): void {
    const settle = this.settleTranscript;
    this.pendingTranscript = null;
    this.settleTranscript = null;
    if (settle) {
      settle(text.trim());
    }
  }

  private async withTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
    let timer: ReturnType<typeof setTimeout> | null = null;
    try {
      return await Promise.race([
        promise,
        new Promise<T>((resolve) => {
          timer = setTimeout(() => resolve("" as T), timeoutMs);
        }),
      ]);
    } finally {
      if (timer) {
        clearTimeout(timer);
      }
    }
  }
}

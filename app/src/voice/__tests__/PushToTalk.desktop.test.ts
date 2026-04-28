import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mockCommands } from "../../test/setup";

// Test seam: setup must precede the PushToTalk + backend imports so
// our `__TAURI_INTERNALS__` mock and `mediaDevices` stub are in place
// when those modules consult them.

interface FakeAudioContext {
  sampleRate: number;
  destination: object;
  createMediaStreamSource: () => { connect: () => void; disconnect: () => void };
  createScriptProcessor: () => {
    connect: () => void;
    disconnect: () => void;
    onaudioprocess: ((e: { inputBuffer: { getChannelData: () => Float32Array } }) => void) | null;
  };
  close: () => Promise<void>;
}

let fireProcessor: ((samples: Float32Array) => void) | null = null;
let getUserMediaImpl: () => Promise<MediaStream> = () => {
  // Default: succeed with a stub MediaStream.
  return Promise.resolve({
    getTracks: () => [{ stop: () => {} }],
  } as unknown as MediaStream);
};

function installAudioStubs(): void {
  // Stub navigator.mediaDevices.getUserMedia
  Object.defineProperty(window.navigator, "mediaDevices", {
    configurable: true,
    writable: true,
    value: {
      getUserMedia: (...args: unknown[]) => getUserMediaImpl.apply(null, args as []),
    },
  });
  // Stub window.AudioContext
  const audioCtorImpl = function (this: FakeAudioContext): FakeAudioContext {
    let processorCb:
      | ((e: { inputBuffer: { getChannelData: () => Float32Array } }) => void)
      | null = null;
    const processor = {
      connect: () => {},
      disconnect: () => {},
      get onaudioprocess() {
        return processorCb;
      },
      set onaudioprocess(
        cb: ((e: { inputBuffer: { getChannelData: () => Float32Array } }) => void) | null,
      ) {
        processorCb = cb;
        // Wire the test-side trigger so a test can push samples.
        fireProcessor = (samples: Float32Array): void => {
          if (processorCb !== null) {
            processorCb({ inputBuffer: { getChannelData: () => samples } });
          }
        };
      },
    };
    this.sampleRate = 48_000;
    this.destination = {};
    this.createMediaStreamSource = () => ({
      connect: () => {},
      disconnect: () => {},
    });
    this.createScriptProcessor = () => processor;
    this.close = () => Promise.resolve();
    return this;
  } as unknown as { new (): FakeAudioContext };
  Object.defineProperty(window, "AudioContext", {
    configurable: true,
    writable: true,
    value: audioCtorImpl,
  });
}

beforeEach(() => {
  installAudioStubs();
  // Tauri's hasDesktopRuntime probes window.__TAURI_INTERNALS__ which
  // the global test setup already defines. Mock the transcribe response.
  mockCommands({
    transcribe_push_to_talk: {
      text: "hello world",
      language: "en",
      confidence: 0.91,
      latency_ms: 120,
      model: "tiny",
    },
  });
  fireProcessor = null;
  getUserMediaImpl = () =>
    Promise.resolve({
      getTracks: () => [{ stop: () => {} }],
    } as unknown as MediaStream);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("PushToTalk desktop capture", () => {
  it("startRecording engages the desktop path and isRecording becomes true", async () => {
    const { PushToTalk } = await import("../PushToTalk");
    const recorder = new PushToTalk();
    recorder.startRecording();
    expect(recorder.recording()).toBe(true);
    // Wait a tick for the async getUserMedia + AudioContext setup.
    await new Promise((r) => setTimeout(r, 0));
  });

  it("stopAndTranscribe returns local-stt result with the transcript", async () => {
    const { PushToTalk } = await import("../PushToTalk");
    const recorder = new PushToTalk();
    recorder.startRecording();
    await new Promise((r) => setTimeout(r, 0));
    // Push a buffer of silence so the recorder has something to encode.
    if (fireProcessor !== null) {
      fireProcessor(new Float32Array(4096));
    }
    const result = await recorder.stopAndTranscribe();
    expect(result.source).toBe("local-stt");
    expect(result.transcript).toBe("hello world");
  });

  it("getUserMedia NotAllowedError surfaces explicit permission message via stopAndTranscribe", async () => {
    getUserMediaImpl = () => {
      const err = new Error("denied");
      (err as { name: string }).name = "NotAllowedError";
      return Promise.reject(err);
    };
    const { PushToTalk } = await import("../PushToTalk");
    const recorder = new PushToTalk();
    recorder.startRecording();
    // Setup-time rejection bubbles to stopAndTranscribe's await chain.
    await expect(recorder.stopAndTranscribe()).rejects.toThrow(
      /Microphone permission denied/,
    );
  });

  it("getUserMedia NotFoundError maps to 'No microphone detected'", async () => {
    getUserMediaImpl = () => {
      const err = new Error("none");
      (err as { name: string }).name = "NotFoundError";
      return Promise.reject(err);
    };
    const { PushToTalk } = await import("../PushToTalk");
    const recorder = new PushToTalk();
    recorder.startRecording();
    await expect(recorder.stopAndTranscribe()).rejects.toThrow(
      /No microphone detected/,
    );
  });

  it("getUserMedia NotReadableError maps to 'in use by another application'", async () => {
    getUserMediaImpl = () => {
      const err = new Error("busy");
      (err as { name: string }).name = "NotReadableError";
      return Promise.reject(err);
    };
    const { PushToTalk } = await import("../PushToTalk");
    const recorder = new PushToTalk();
    recorder.startRecording();
    await expect(recorder.stopAndTranscribe()).rejects.toThrow(
      /in use by another application/,
    );
  });

  it("falls back to mock-whisper when not recording", async () => {
    const { PushToTalk } = await import("../PushToTalk");
    const recorder = new PushToTalk();
    const result = await recorder.stopAndTranscribe();
    expect(result.transcript).toBe("");
    expect(result.source).toBe("mock-whisper");
  });
});

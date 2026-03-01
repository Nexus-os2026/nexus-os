import { hasDesktopRuntime, transcribePushToTalk } from "../api/backend";

export interface PushToTalkResult {
  transcript: string;
  source: "tauri-stt" | "web-speech" | "mock-whisper";
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

export class PushToTalk {
  private isRecording = false;
  private readonly recognition: SpeechRecognitionLike | null;
  private pendingTranscript: Promise<string> | null = null;
  private settleTranscript: ((text: string) => void) | null = null;
  private heardTranscript = "";

  public constructor() {
    this.recognition = this.createRecognition();
  }

  public startRecording(): void {
    this.isRecording = true;
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
      return {
        transcript: "",
        source: "mock-whisper"
      };
    }

    this.isRecording = false;

    if (hasDesktopRuntime()) {
      const transcript = await transcribePushToTalk();
      return {
        transcript,
        source: "tauri-stt"
      };
    }

    if (this.recognition && this.pendingTranscript) {
      this.recognition.stop();
      const transcript = await this.withTimeout(this.pendingTranscript, 1500);
      return {
        transcript,
        source: "web-speech"
      };
    }

    await new Promise((resolve) => setTimeout(resolve, 250));

    return {
      transcript: "create an agent to post weekly Rust updates",
      source: "mock-whisper"
    };
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
        })
      ]);
    } finally {
      if (timer) {
        clearTimeout(timer);
      }
    }
  }
}

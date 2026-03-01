export interface PushToTalkResult {
  transcript: string;
  source: "mock-whisper";
}

export class PushToTalk {
  private isRecording = false;

  public startRecording(): void {
    this.isRecording = true;
  }

  public async stopAndTranscribe(): Promise<PushToTalkResult> {
    if (!this.isRecording) {
      return {
        transcript: "",
        source: "mock-whisper"
      };
    }

    this.isRecording = false;

    await new Promise((resolve) => setTimeout(resolve, 250));

    return {
      transcript: "create an agent to post weekly Rust updates",
      source: "mock-whisper"
    };
  }
}

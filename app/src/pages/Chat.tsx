import { useMemo, useState } from "react";
import { PushToTalk } from "../voice/PushToTalk";

interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
}

export function Chat(): JSX.Element {
  const [draft, setDraft] = useState("");
  const [isRecording, setIsRecording] = useState(false);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const voice = useMemo(() => new PushToTalk(), []);

  function sendMessage(content: string): void {
    const trimmed = content.trim();
    if (!trimmed) {
      return;
    }

    setMessages((prev) => [
      ...prev,
      { id: crypto.randomUUID(), role: "user", content: trimmed },
      {
        id: crypto.randomUUID(),
        role: "assistant",
        content: "Command queued. I will run it through governed policy checks."
      }
    ]);
    setDraft("");
  }

  async function togglePushToTalk(): Promise<void> {
    if (!isRecording) {
      voice.startRecording();
      setIsRecording(true);
      return;
    }

    const result = await voice.stopAndTranscribe();
    setIsRecording(false);
    if (result.transcript.trim()) {
      setDraft(result.transcript);
    }
  }

  return (
    <section className="soft-card rounded-2xl p-6 shadow-sm">
      <h2 className="font-display text-2xl text-ink">What would you like me to do?</h2>
      <p className="mt-1 text-sm text-slate-600">
        Chat-first control surface for NEXUS OS agents.
      </p>

      <div className="mt-6 space-y-3 rounded-xl border border-slate-200 bg-white/70 p-4">
        {messages.length === 0 ? (
          <p className="text-sm text-slate-500">No messages yet.</p>
        ) : (
          messages.map((message) => (
            <article
              key={message.id}
              className={`rounded-lg px-3 py-2 text-sm ${
                message.role === "user"
                  ? "ml-10 bg-ink text-white"
                  : "mr-10 bg-mist text-ink"
              }`}
            >
              {message.content}
            </article>
          ))
        )}
      </div>

      <form
        className="mt-4 flex flex-col gap-3 sm:flex-row"
        onSubmit={(event) => {
          event.preventDefault();
          sendMessage(draft);
        }}
      >
        <input
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          placeholder="Tell NEXUS OS what to do..."
          className="flex-1 rounded-xl border border-slate-300 bg-white px-4 py-3 text-sm outline-none ring-accent transition focus:ring-2"
        />
        <div className="flex gap-2">
          <button
            type="button"
            onClick={togglePushToTalk}
            className={`rounded-xl px-4 py-3 text-sm font-semibold text-white transition ${
              isRecording ? "bg-rose-600" : "bg-mint"
            }`}
            title="Push to talk"
          >
            {isRecording ? "Stop Mic" : "Mic"}
          </button>
          <button
            type="submit"
            className="rounded-xl bg-accent px-4 py-3 text-sm font-semibold text-white"
          >
            Send
          </button>
        </div>
      </form>
    </section>
  );
}

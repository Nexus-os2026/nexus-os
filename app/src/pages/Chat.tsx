import type { ChatMessage } from "../types";

interface ChatProps {
  messages: ChatMessage[];
  draft: string;
  isRecording: boolean;
  isSending: boolean;
  onDraftChange: (value: string) => void;
  onSend: () => void;
  onToggleMic: () => void;
}

function bubbleClass(role: ChatMessage["role"]): string {
  if (role === "user") {
    return "self-end max-w-[85%] rounded-2xl rounded-br-md bg-sky-600 px-4 py-3 text-sm text-white sm:max-w-[70%]";
  }
  return "self-start max-w-[85%] rounded-2xl rounded-bl-md bg-zinc-800 px-4 py-3 text-sm text-zinc-100 sm:max-w-[70%]";
}

export function Chat({
  messages,
  draft,
  isRecording,
  isSending,
  onDraftChange,
  onSend,
  onToggleMic
}: ChatProps): JSX.Element {
  return (
    <section className="flex h-[calc(100vh-10rem)] flex-col rounded-2xl border border-zinc-800 bg-zinc-900/80 shadow-[0_30px_80px_-45px_rgba(34,197,94,0.4)]">
      <header className="border-b border-zinc-800 px-5 py-4">
        <h2 className="font-display text-xl text-zinc-100">NEXUS Chat</h2>
        <p className="mt-1 text-xs text-zinc-400">
          Governed conversation loop with agent factory command routing.
        </p>
      </header>

      <div className="flex-1 space-y-4 overflow-y-auto px-5 py-5">
        {messages.length === 0 ? (
          <p className="rounded-xl border border-dashed border-zinc-700 bg-zinc-900/60 p-4 text-sm text-zinc-400">
            Start by describing a task. Example: create agent weekly-research with web.search and llm.query.
          </p>
        ) : (
          messages.map((message) => (
            <article key={message.id} className={`flex flex-col ${message.role === "user" ? "items-end" : "items-start"}`}>
              <div className={bubbleClass(message.role)}>
                <p className="whitespace-pre-wrap">{message.content || (message.streaming ? "…" : "")}</p>
              </div>
              <span className="mt-1 text-[11px] text-zinc-500">
                {new Date(message.timestamp).toLocaleTimeString()}
                {message.model ? ` · ${message.model}` : ""}
              </span>
            </article>
          ))
        )}
      </div>

      <form
        className="border-t border-zinc-800 p-4"
        onSubmit={(event) => {
          event.preventDefault();
          if (!isSending) {
            onSend();
          }
        }}
      >
        <div className="flex items-end gap-2">
          <button
            type="button"
            onClick={onToggleMic}
            className={`h-12 rounded-xl px-4 text-sm font-semibold transition ${
              isRecording ? "bg-rose-600 text-white" : "bg-zinc-800 text-zinc-200 hover:bg-zinc-700"
            }`}
            title="Push to talk"
          >
            {isRecording ? "Stop Mic" : "Mic"}
          </button>
          <textarea
            value={draft}
            onChange={(event) => onDraftChange(event.target.value)}
            placeholder="Ask NEXUS OS..."
            rows={2}
            className="min-h-12 flex-1 resize-none rounded-xl border border-zinc-700 bg-zinc-950 px-4 py-3 text-sm text-zinc-100 outline-none ring-emerald-500/60 transition focus:ring-2"
          />
          <button
            type="submit"
            disabled={isSending}
            className="h-12 rounded-xl bg-emerald-600 px-5 text-sm font-semibold text-white transition hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-70"
          >
            {isSending ? "Sending..." : "Send"}
          </button>
        </div>
      </form>
    </section>
  );
}

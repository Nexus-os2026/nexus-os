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
    return "self-end max-w-[88%] rounded-2xl rounded-br-md border border-cyan-200/70 bg-cyan-400/20 px-4 py-3 text-sm text-cyan-50 sm:max-w-[72%]";
  }
  return "self-start max-w-[88%] rounded-2xl rounded-bl-md border border-slate-700/80 bg-slate-900/90 px-4 py-3 text-sm text-slate-100 sm:max-w-[72%]";
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
    <section className="nexus-panel flex h-[calc(100vh-10rem)] flex-col overflow-hidden">
      <header className="border-b border-cyan-300/20 px-5 py-4">
        <h2 className="nexus-display text-2xl text-cyan-100">NexusOS Comms</h2>
        <p className="mt-1 text-xs text-cyan-100/65">
          Governed conversation loop with agent factory command routing.
        </p>
      </header>

      <div className="flex-1 space-y-4 overflow-y-auto px-5 py-5">
        {messages.length === 0 ? (
          <p className="rounded-xl border border-dashed border-cyan-300/35 bg-slate-900/75 p-4 text-sm text-cyan-100/70">
            Start by describing a task. Example: create agent weekly-research with web.search and llm.query.
          </p>
        ) : (
          messages.map((message) => (
            <article key={message.id} className={`flex flex-col ${message.role === "user" ? "items-end" : "items-start"}`}>
              <div className={bubbleClass(message.role)}>
                <p className="whitespace-pre-wrap">{message.content || (message.streaming ? "…" : "")}</p>
              </div>
              <span className="mt-1 text-[11px] text-cyan-100/50">
                {new Date(message.timestamp).toLocaleTimeString()}
                {message.model ? ` · ${message.model}` : ""}
              </span>
            </article>
          ))
        )}
      </div>

      <form
        className="border-t border-cyan-300/20 p-4"
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
              isRecording
                ? "border border-rose-300/60 bg-rose-500/80 text-white"
                : "border border-slate-700/80 bg-slate-900/90 text-cyan-100 hover:border-cyan-300/55"
            }`}
            title="Push to talk"
          >
            {isRecording ? "Stop Mic" : "Mic"}
          </button>
          <textarea
            value={draft}
            onChange={(event) => onDraftChange(event.target.value)}
            placeholder="Ask NexusOS..."
            rows={2}
            className="min-h-12 flex-1 resize-none rounded-xl border border-cyan-400/35 bg-slate-950/95 px-4 py-3 text-sm text-cyan-50 outline-none ring-cyan-400/35 transition focus:border-cyan-300/75 focus:ring-2"
          />
          <button
            type="submit"
            disabled={isSending}
            className="h-12 rounded-xl border border-cyan-300/70 bg-cyan-500/20 px-5 text-sm font-semibold text-cyan-50 transition hover:bg-cyan-400/25 disabled:cursor-not-allowed disabled:opacity-70"
          >
            {isSending ? "Sending..." : "Send"}
          </button>
        </div>
      </form>
    </section>
  );
}

interface SuggestionsProps {
  visible: boolean;
  onSelect: (value: string) => void;
}

const SUGGESTIONS = [
  "Create an agent",
  "Show status",
  "Voice mode",
  "Search the web"
];

export function Suggestions({ visible, onSelect }: SuggestionsProps): JSX.Element | null {
  if (!visible) {
    return null;
  }

  return (
    <div className="jarvis-suggestions">
      {SUGGESTIONS.map((item) => (
        <button
          key={item}
          type="button"
          className="jarvis-suggestion-chip"
          onClick={() => onSelect(item)}
        >
          {item}
        </button>
      ))}
    </div>
  );
}

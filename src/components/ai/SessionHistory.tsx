import { AiSession } from "../../lib/ai";

interface Props {
  sessions: AiSession[];
  activeId: string | null;
  onSelect: (id: string) => void;
  onNew: () => void;
  onDelete: (id: string) => void;
  disabled?: boolean;
}

export default function SessionHistory({ sessions, activeId, onSelect, onNew, onDelete, disabled }: Props) {
  return (
    <aside className="session-history">
      <button onClick={onNew} disabled={disabled}>
        New chat
      </button>
      <ul>
        {sessions.map((s) => (
          <li key={s.id} className={s.id === activeId ? "active" : ""}>
            <button
              className="session-title"
              disabled={disabled}
              onClick={() => onSelect(s.id)}
              title={`${s.provider} · ${s.model}`}
            >
              {s.title || "Untitled"}
            </button>
            <button
              className="icon"
              aria-label={`Delete ${s.title}`}
              disabled={disabled}
              onClick={() => onDelete(s.id)}
            >
              ×
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}

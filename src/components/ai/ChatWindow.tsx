import { useEffect, useRef } from "react";

export interface ChatItem {
  key: string;
  role: "user" | "assistant";
  content: string;
  streaming?: boolean;
}

interface Props {
  items: ChatItem[];
  error: string | null;
  notice: string | null;
}

export default function ChatWindow({ items, error, notice }: Props) {
  const bottom = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "end" });
  }, [items]);

  return (
    <div className="chat-window" aria-live="polite">
      {items.length === 0 && <p className="muted empty">Start a conversation below.</p>}
      {items.map((m) => (
        <div key={m.key} className={`bubble bubble-${m.role}`}>
          {m.content}
          {m.streaming && <span className="cursor" aria-hidden />}
        </div>
      ))}
      {notice && <p className="muted small">{notice}</p>}
      {error && <p className="error">{error}</p>}
      <div ref={bottom} />
    </div>
  );
}
